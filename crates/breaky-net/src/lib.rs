use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::OnceLock;

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

pub fn get(url: &str) -> Result<Response, String> {
    let u = parse_url(url)?;
    get_url(&u, 0)
}

pub fn get_url(u: &Url, redirects: u8) -> Result<Response, String> {
    if redirects > 5 {
        return Err("too many redirects".to_string());
    }
    let mut stream = TcpStream::connect((u.host.as_str(), u.port))
        .map_err(|e| format!("connect {}: {}", u.host, e))?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: breaky/0.1\r\nAccept: text/html,text/css,*/*\r\nConnection: close\r\n\r\n",
        u.path, u.host
    );
    let mut raw = Vec::new();
    match u.scheme.as_str() {
        "https" => {
            let mut tls = tls::connect_with_cache(stream, &u.host, chains())
                .map_err(|e| format!("tls {}: {}", u.host, e))?;
            tls.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
            tls.read_to_end(&mut raw).map_err(|e| e.to_string())?;
            tls.close().map_err(|e| e.to_string())?;
        }
        _ => {
            stream
                .write_all(req.as_bytes())
                .map_err(|e| e.to_string())?;
            stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;
        }
    }
    let (status, headers, body) = parse_response(&raw)?;
    if matches!(status, 301 | 302 | 303 | 307 | 308) {
        let loc = headers.get("location").ok_or("redirect without location")?;
        let next = resolve(u, loc)?;
        return get_url(&next, redirects + 1);
    }
    let body = if headers
        .get("transfer-encoding")
        .map(|v| v.to_lowercase().contains("chunked"))
        .unwrap_or(false)
    {
        decode_chunked(&body)?
    } else if let Some(cl) = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
    {
        body.into_iter().take(cl).collect()
    } else {
        body
    };
    Ok(Response { status, body })
}

fn parse_response(raw: &[u8]) -> Result<(u16, HashMap<String, String>, Vec<u8>), String> {
    let split = find_sub(raw, b"\r\n\r\n").ok_or("bad response")?;
    let head = std::str::from_utf8(&raw[..split]).map_err(|_| "bad head")?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or("bad status line")?;
    let mut parts = status_line.split_whitespace();
    let proto = parts.next().ok_or("bad status line")?;
    if !proto.starts_with("HTTP/") {
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
    Ok((status, headers, raw[split + 4..].to_vec()))
}

fn decode_chunked(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let nl = find_sub(&raw[i..], b"\r\n").ok_or("bad chunk")? + i;
        let line = std::str::from_utf8(&raw[i..nl]).map_err(|_| "bad chunk size")?;
        let n = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "bad chunk size")?;
        i = nl + 2;
        if n == 0 {
            break;
        }
        if i + n > raw.len() {
            return Err("truncated chunk".to_string());
        }
        out.extend_from_slice(&raw[i..i + n]);
        i += n + 2;
    }
    Ok(out)
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

    fn base() -> Url {
        parse_url("http://example.com/docs/index.html").unwrap()
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
    fn response_and_chunks() {
        let raw =
            b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello";
        let (status, headers, body) = parse_response(raw).unwrap();
        assert_eq!(status, 404);
        assert_eq!(headers.get("content-type").unwrap(), "text/plain");
        assert_eq!(body, b"hello");
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=1\r\nde\r\n0\r\n\r\n";
        let (_, _, body) = parse_response(raw).unwrap();
        assert_eq!(decode_chunked(&body).unwrap(), b"abcde");
    }
}
