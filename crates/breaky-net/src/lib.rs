use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::OnceLock;
use std::time::Duration;

use tls::ChainCache;

fn chains() -> &'static ChainCache {
    static CHAINS: OnceLock<ChainCache> = OnceLock::new();
    CHAINS.get_or_init(ChainCache::new)
}

pub struct Url {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.port == 80 {
            write!(f, "{}://{}{}", self.scheme, self.host, self.path)
        } else {
            write!(
                f,
                "{}://{}:{}{}",
                self.scheme, self.host, self.port, self.path
            )
        }
    }
}

pub fn parse_url(s: &str) -> Result<Url, String> {
    let sep = s.find("://").ok_or_else(|| format!("no scheme: {}", s))?;
    let scheme = s[..sep].to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(format!("unsupported scheme: {}", scheme));
    }
    let rest = &s[sep + 3..];
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let netloc = &rest[..end];
    let tail = &rest[end..];
    let default_port = if scheme == "https" { 443 } else { 80 };
    let (host, port) = match netloc.rfind(':') {
        Some(p) => (
            netloc[..p].to_string(),
            netloc[p + 1..]
                .parse::<u16>()
                .map_err(|_| format!("bad port: {}", s))?,
        ),
        None => (netloc.to_string(), default_port),
    };
    if host.is_empty() {
        return Err(format!("no host: {}", s));
    }
    let hash = tail.find('#').unwrap_or(tail.len());
    let mut path = tail[..hash].to_string();
    if path.is_empty() {
        path.push('/');
    }
    Ok(Url {
        scheme,
        host,
        port,
        path,
    })
}

pub fn resolve(base: &Url, href: &str) -> Result<Url, String> {
    let h = match href.find('#') {
        Some(p) => &href[..p],
        None => href,
    };
    if h.contains("://") {
        return parse_url(h);
    }
    if h.starts_with("//") {
        return parse_url(&format!("{}:{}", base.scheme, h));
    }
    let path = if h.starts_with('/') {
        h.to_string()
    } else if h.is_empty() {
        base.path.clone()
    } else {
        let dir = match base.path.rfind('/') {
            Some(p) => &base.path[..p + 1],
            None => "/",
        };
        format!("{}{}", dir, h)
    };
    Ok(Url {
        scheme: base.scheme.clone(),
        host: base.host.clone(),
        port: base.port,
        path: normalize(&path),
    })
}

fn normalize(p: &str) -> String {
    let trailing = p.ends_with('/');
    let mut segs: Vec<&str> = Vec::new();
    for s in p.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            x => segs.push(x),
        }
    }
    let mut r = String::from("/");
    r.push_str(&segs.join("/"));
    if trailing && !r.ends_with('/') {
        r.push('/');
    }
    r
}

pub struct Client {
    conns: HashMap<String, Pooled>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            conns: HashMap::new(),
        }
    }

    pub fn get(&mut self, url: &str) -> Result<Response, String> {
        let u = parse_url(url)?;
        self.fetch(&u, 0)
    }

    pub fn get_url(&mut self, u: &Url) -> Result<Response, String> {
        self.fetch(u, 0)
    }

    fn fetch(&mut self, u: &Url, redirects: u8) -> Result<Response, String> {
        if redirects > 5 {
            return Err("too many redirects".to_string());
        }
        let key = format!("{}:{}:{}", u.scheme, u.host, u.port);
        let mut pooled = self.conns.remove(&key);
        for _ in 0..2 {
            let warm = pooled.is_some();
            let mut p = match pooled.take() {
                Some(p) => p,
                None => Pooled::new(open_conn(u)?),
            };
            match attempt(&mut p, u) {
                Ok((resp, reusable)) => {
                    if reusable {
                        self.conns.insert(key.clone(), p);
                    } else {
                        p.conn.close();
                    }
                    if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
                        let loc = resp
                            .headers
                            .get("location")
                            .cloned()
                            .ok_or("redirect without location")?;
                        let next = resolve(u, &loc)?;
                        return self.fetch(&next, redirects + 1);
                    }
                    return Ok(resp);
                }
                Err((msg, head)) => {
                    if head || !warm {
                        return Err(msg);
                    }
                }
            }
        }
        Err("request failed".to_string())
    }
}

pub fn get(url: &str) -> Result<Response, String> {
    Client::new().get(url)
}

fn open_conn(u: &Url) -> Result<Conn, String> {
    let stream = TcpStream::connect((u.host.as_str(), u.port))
        .map_err(|e| format!("connect {}: {}", u.host, e))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    match u.scheme.as_str() {
        "https" => Ok(Conn::Tls(
            tls::connect_with_cache(stream, &u.host, chains())
                .map_err(|e| format!("tls {}: {}", u.host, e))?,
        )),
        _ => Ok(Conn::Plain(stream)),
    }
}

enum Conn {
    Tls(tls::Client),
    Plain(TcpStream),
}

impl Conn {
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            Conn::Tls(c) => c.write_all(buf),
            Conn::Plain(s) => s.write_all(buf),
        }
    }

    fn read_some(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Tls(c) => c.read(buf),
            Conn::Plain(s) => s.read(buf),
        }
    }

    fn close(&mut self) {
        if let Conn::Tls(c) = self {
            let _ = c.close();
        }
    }
}

struct Pooled {
    conn: Conn,
    buf: Vec<u8>,
    pos: usize,
}

impl Pooled {
    fn new(conn: Conn) -> Pooled {
        Pooled {
            conn,
            buf: Vec::new(),
            pos: 0,
        }
    }

    fn more(&mut self) -> Result<bool, String> {
        self.buf.drain(..self.pos);
        self.pos = 0;
        let mut tmp = [0u8; 8192];
        match self.conn.read_some(&mut tmp) {
            Ok(0) => Ok(false),
            Ok(n) => {
                self.buf.extend_from_slice(&tmp[..n]);
                Ok(true)
            }
            Err(e) => Err(format!("read: {}", e)),
        }
    }

    fn byte(&mut self) -> Result<u8, String> {
        if self.pos >= self.buf.len() && !self.more()? {
            return Err("connection closed".to_string());
        }
        let b = self.buf[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, mut n: usize, out: &mut Vec<u8>) -> Result<(), String> {
        while n > 0 {
            if self.pos >= self.buf.len() && !self.more()? {
                return Err("connection closed".to_string());
            }
            let k = n.min(self.buf.len() - self.pos);
            out.extend_from_slice(&self.buf[self.pos..self.pos + k]);
            self.pos += k;
            n -= k;
        }
        Ok(())
    }

    fn take_rest(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        loop {
            if !self.more()? {
                return Ok(());
            }
            out.extend_from_slice(&self.buf[self.pos..]);
            self.pos = self.buf.len();
        }
    }

    fn line(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        loop {
            let b = self.byte()?;
            if b == b'\r' {
                if self.byte()? != b'\n' {
                    return Err("bad line ending".to_string());
                }
                return Ok(());
            }
            out.push(b);
            if out.len() > 1 << 12 {
                return Err("line too long".to_string());
            }
        }
    }

    fn until_head(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        let start = out.len();
        loop {
            if self.pos >= self.buf.len() && !self.more()? {
                return Err("connection closed".to_string());
            }
            let n = self.buf.len() - self.pos;
            out.extend_from_slice(&self.buf[self.pos..]);
            self.pos = self.buf.len();
            let from = start.max(out.len().saturating_sub(n + 3));
            if let Some(i) = find_sub(&out[from..], b"\r\n\r\n") {
                let hit = from + i;
                let extra = out.len() - (hit + 4);
                self.pos -= extra;
                out.truncate(hit);
                return Ok(());
            }
            if out.len() - start > (1 << 16) {
                return Err("head too large".to_string());
            }
        }
    }

    fn chunked(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        loop {
            let mut line = Vec::new();
            self.line(&mut line)?;
            let s = std::str::from_utf8(&line).map_err(|_| "bad chunk size".to_string())?;
            let n = usize::from_str_radix(s.split(';').next().unwrap_or("").trim(), 16)
                .map_err(|_| "bad chunk size".to_string())?;
            if n == 0 {
                break;
            }
            self.take(n, out)?;
            let mut sep = Vec::new();
            self.take(2, &mut sep)?;
            if sep != b"\r\n" {
                return Err("bad chunk framing".to_string());
            }
        }
        loop {
            let mut t = Vec::new();
            self.line(&mut t)?;
            if t.is_empty() {
                return Ok(());
            }
        }
    }
}

fn attempt(p: &mut Pooled, u: &Url) -> Result<(Response, bool), (String, bool)> {
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: breaky/0.1\r\nAccept: text/html,text/css,*/*\r\n\r\n",
        u.path, u.host
    );
    p.conn
        .write_all(req.as_bytes())
        .map_err(|e| (e.to_string(), false))?;
    let mut head = Vec::new();
    p.until_head(&mut head).map_err(|e| (e, false))?;
    let (proto, status, headers) = parse_head(&head).map_err(|e| (e, true))?;
    let mut reusable = reusable_head(&proto, &headers);
    let mut body = Vec::new();
    let no_body = status == 204 || status == 304 || (100..200).contains(&status);
    if !no_body {
        if headers
            .get("transfer-encoding")
            .map(|v| v.to_lowercase().contains("chunked"))
            .unwrap_or(false)
        {
            p.chunked(&mut body).map_err(|e| (e, true))?;
        } else if let Some(cl) = headers
            .get("content-length")
            .and_then(|v| v.parse::<usize>().ok())
        {
            p.take(cl, &mut body).map_err(|e| (e, true))?;
        } else {
            p.take_rest(&mut body).map_err(|e| (e, true))?;
            reusable = false;
        }
    }
    Ok((
        Response {
            status,
            headers,
            body,
        },
        reusable,
    ))
}

fn reusable_head(proto: &str, headers: &HashMap<String, String>) -> bool {
    let conn = headers
        .get("connection")
        .map(|v| v.to_lowercase())
        .unwrap_or_default();
    let tokens = || conn.split(',').map(|t| t.trim());
    match proto {
        "http/1.1" => !tokens().any(|t| t == "close"),
        "http/1.0" => tokens().any(|t| t == "keep-alive"),
        _ => false,
    }
}

fn parse_head(head: &[u8]) -> Result<(String, u16, HashMap<String, String>), String> {
    let text = std::str::from_utf8(head).map_err(|_| "bad head".to_string())?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next().ok_or("bad status line")?;
    let mut parts = status_line.split_whitespace();
    let proto = parts.next().ok_or("bad status line")?.to_lowercase();
    if !proto.starts_with("http/") {
        return Err("bad protocol".to_string());
    }
    let status = parts
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("bad status code")?;
    let mut headers = HashMap::new();
    for line in lines {
        if let Some(c) = line.find(':') {
            headers.insert(
                line[..c].trim().to_lowercase(),
                line[c + 1..].trim().to_string(),
            );
        }
    }
    Ok((proto, status, headers))
}

fn find_sub(h: &[u8], n: &[u8]) -> Option<usize> {
    if n.len() > h.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| &h[i..i + n.len()] == n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn base() -> Url {
        parse_url("http://example.com/docs/index.html").unwrap()
    }

    fn serve(scripts: Vec<Vec<u8>>) -> (String, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let n = Arc::new(AtomicUsize::new(0));
        let c = n.clone();
        let h = std::thread::spawn(move || {
            for script in scripts {
                let (mut s, _) = l.accept().unwrap();
                c.fetch_add(1, Ordering::SeqCst);
                let mut b = [0u8; 4096];
                let _ = Read::read(&mut s, &mut b);
                s.write_all(&script).unwrap();
            }
        });
        (format!("http://127.0.0.1:{}", port), n, h)
    }

    fn resp(body: &str, extra: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{}\r\n{}",
            body.len(),
            extra,
            body
        )
        .into_bytes()
    }

    #[test]
    fn parse_parts() {
        let u = parse_url("http://localhost:8080/a/b?q=1#frag").unwrap();
        assert_eq!(u.scheme, "http");
        assert_eq!(u.host, "localhost");
        assert_eq!(u.port, 8080);
        assert_eq!(u.path, "/a/b?q=1");
        let u = parse_url("http://example.com").unwrap();
        assert_eq!(u.port, 80);
        assert_eq!(u.path, "/");
    }

    #[test]
    fn reject_non_web_schemes() {
        assert!(parse_url("ftp://example.com").is_err());
        assert!(parse_url("gopher://example.com").is_err());
        assert!(parse_url("example.com").is_err());
    }

    #[test]
    fn parse_https_parts() {
        let u = parse_url("https://example.com").unwrap();
        assert_eq!(u.scheme, "https");
        assert_eq!(u.port, 443);
        assert_eq!(u.path, "/");
        let u = parse_url("HTTPS://example.io:8443/x").unwrap();
        assert_eq!(u.scheme, "https");
        assert_eq!(u.port, 8443);
        assert_eq!(u.path, "/x");
    }

    #[test]
    #[ignore]
    fn fetch_https_example() {
        let resp = get("https://example.com").unwrap();
        assert_eq!(resp.status, 200);
        let text = String::from_utf8_lossy(&resp.body).to_lowercase();
        assert!(text.contains("example domain"));
    }

    #[test]
    fn chain_cache_shared_per_process() {
        let a = chains();
        let b = chains();
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn resolve_kinds() {
        let b = base();
        assert_eq!(
            resolve(&b, "http://other.io/x.css").unwrap().to_string(),
            "http://other.io/x.css"
        );
        assert_eq!(
            resolve(&b, "//cdn.io/x.css").unwrap().to_string(),
            "http://cdn.io/x.css"
        );
        assert_eq!(
            resolve(&b, "/style/main.css").unwrap().to_string(),
            "http://example.com/style/main.css"
        );
        assert_eq!(
            resolve(&b, "main.css").unwrap().to_string(),
            "http://example.com/docs/main.css"
        );
        assert_eq!(
            resolve(&b, "../img/a.png").unwrap().to_string(),
            "http://example.com/img/a.png"
        );
        assert_eq!(
            resolve(&b, "./c/../d.css").unwrap().to_string(),
            "http://example.com/docs/d.css"
        );
        assert_eq!(
            resolve(&b, "/a.css#frag").unwrap().to_string(),
            "http://example.com/a.css"
        );
    }

    #[test]
    fn reusable_matrix() {
        let (p, _, hd) = parse_head(b"HTTP/1.1 200 OK\r\n").unwrap();
        assert!(reusable_head(&p, &hd));
        let (p, _, hd) = parse_head(b"HTTP/1.1 200 OK\r\nConnection: close\r\n").unwrap();
        assert!(!reusable_head(&p, &hd));
        let (p, _, hd) =
            parse_head(b"HTTP/1.1 200 OK\r\nConnection: Keep-Alive, Upgrade\r\n").unwrap();
        assert!(reusable_head(&p, &hd));
        let (p, _, hd) = parse_head(b"HTTP/1.0 200 OK\r\nConnection: keep-alive\r\n").unwrap();
        assert!(reusable_head(&p, &hd));
        let (p, _, hd) = parse_head(b"HTTP/1.0 200 OK\r\n").unwrap();
        assert!(!reusable_head(&p, &hd));
        let (p, _, hd) = parse_head(b"HTTP/0.9 200 OK\r\n").unwrap();
        assert!(!reusable_head(&p, &hd));
    }

    #[test]
    fn head_parsing() {
        let (proto, status, headers) =
            parse_head(b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n").unwrap();
        assert_eq!(proto, "http/1.1");
        assert_eq!(status, 404);
        assert_eq!(headers.get("content-type").unwrap(), "text/plain");
        assert!(parse_head(b"nope\r\n").is_err());
        assert!(parse_head(b"HTTP/1.1 abc OK\r\n").is_err());
    }

    #[test]
    fn keep_alive_reuses_connection() {
        let blob = [resp("alpha", ""), resp("beta", "")].concat();
        let (base, n, h) = serve(vec![blob]);
        let mut c = Client::new();
        let a = c.get(&base).unwrap();
        let b = c.get(&base).unwrap();
        assert_eq!(a.status, 200);
        assert_eq!(a.body, b"alpha");
        assert_eq!(b.body, b"beta");
        assert_eq!(n.load(Ordering::SeqCst), 1);
        h.join().unwrap();
    }

    #[test]
    fn head_split_across_reads() {
        let pad = "x".repeat(9000);
        let big = format!("X-Pad: {}\r\n", pad);
        let blob = [resp("one", &big), resp("two", "")].concat();
        let (base, n, h) = serve(vec![blob]);
        let mut c = Client::new();
        assert_eq!(c.get(&base).unwrap().body, b"one");
        assert_eq!(c.get(&base).unwrap().body, b"two");
        assert_eq!(n.load(Ordering::SeqCst), 1);
        h.join().unwrap();
    }

    #[test]
    fn connection_close_forces_new_connection() {
        let (base, n, h) = serve(vec![
            resp("first", "Connection: close\r\n"),
            resp("second", ""),
        ]);
        let mut c = Client::new();
        assert_eq!(c.get(&base).unwrap().body, b"first");
        assert_eq!(c.get(&base).unwrap().body, b"second");
        assert_eq!(n.load(Ordering::SeqCst), 2);
        h.join().unwrap();
    }

    #[test]
    fn chunked_over_keep_alive() {
        let c1 = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=1\r\nde\r\n0\r\n\r\n";
        let c2 = resp("tail", "");
        let blob = [&c1[..], &c2].concat();
        let (base, n, h) = serve(vec![blob]);
        let mut c = Client::new();
        assert_eq!(c.get(&base).unwrap().body, b"abcde");
        assert_eq!(c.get(&base).unwrap().body, b"tail");
        assert_eq!(n.load(Ordering::SeqCst), 1);
        h.join().unwrap();
    }

    #[test]
    fn stale_pooled_connection_reconnects() {
        let (base, n, h) = serve(vec![resp("gone", ""), resp("fresh", "")]);
        let mut c = Client::new();
        assert_eq!(c.get(&base).unwrap().body, b"gone");
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(c.get(&base).unwrap().body, b"fresh");
        assert_eq!(n.load(Ordering::SeqCst), 2);
        h.join().unwrap();
    }

    #[test]
    fn no_body_status_stays_reusable() {
        let c1 = b"HTTP/1.1 204 No Content\r\n\r\n";
        let c2 = resp("after", "");
        let blob = [&c1[..], &c2].concat();
        let (base, n, h) = serve(vec![blob]);
        let mut c = Client::new();
        let r = c.get(&base).unwrap();
        assert_eq!(r.status, 204);
        assert!(r.body.is_empty());
        assert_eq!(c.get(&base).unwrap().body, b"after");
        assert_eq!(n.load(Ordering::SeqCst), 1);
        h.join().unwrap();
    }
}
