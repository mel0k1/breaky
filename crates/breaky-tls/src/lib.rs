mod aead;
mod aes;
mod cache;
mod chacha;
pub mod chain;
mod der;
mod field;
mod gcm;
mod hash;
mod hmac;
mod p256;
mod p384;
pub mod pem;
mod poly;
pub mod roots;
mod rsa;
mod sha256;
mod sha512;
mod x25519;
pub mod x509;

pub use cache::ChainCache;

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream};

use hash::Hash;

const GROUP_X25519: u16 = 0x001d;
const SIG_ECDSA_P256_SHA256: u16 = 0x0403;
const SIG_RSA_PSS_SHA256: u16 = 0x0804;
const MAX_RECORD: usize = (1 << 14) + 256;

#[derive(Debug)]
pub enum TlsError {
    Io(io::Error),
    Protocol(&'static str),
    Alert(u8),
    Cert(&'static str),
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsError::Io(e) => write!(f, "io: {}", e),
            TlsError::Protocol(m) => write!(f, "protocol: {}", m),
            TlsError::Alert(d) => write!(f, "alert {}", d),
            TlsError::Cert(m) => write!(f, "certificate: {}", m),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<io::Error> for TlsError {
    fn from(e: io::Error) -> Self {
        TlsError::Io(e)
    }
}

impl From<TlsError> for io::Error {
    fn from(e: TlsError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, e.to_string())
    }
}

struct Keys {
    suite: aead::Suite,
    key: [u8; 32],
    key_len: usize,
    iv: [u8; 12],
    finished: Vec<u8>,
}

fn keys_for(hash: Hash, secret: &[u8], suite: aead::Suite) -> Keys {
    let key_len = suite.key_len();
    let mut key = [0u8; 32];
    key[..key_len].copy_from_slice(&hmac::expand_label(hash, secret, "key", &[], key_len));
    let mut iv = [0u8; 12];
    iv.copy_from_slice(&hmac::expand_label(hash, secret, "iv", &[], 12));
    let finished = hmac::expand_label(hash, secret, "finished", &[], hash.len());
    Keys {
        suite,
        key,
        key_len,
        iv,
        finished,
    }
}

fn random_bytes(buf: &mut [u8]) -> io::Result<()> {
    use std::fs::File;
    let mut f = File::open("/dev/urandom")?;
    f.read_exact(buf)
}

fn push_u16(v: &mut Vec<u8>, n: usize) {
    v.extend_from_slice(&(n as u16).to_be_bytes());
}

fn push_u24(v: &mut Vec<u8>, n: usize) {
    v.extend_from_slice(&(n as u32).to_be_bytes()[1..]);
}

fn be24(b: &[u8]) -> usize {
    ((b[0] as usize) << 16) | ((b[1] as usize) << 8) | b[2] as usize
}

fn handshake_msg(typ: u8, body: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(4 + body.len());
    m.push(typ);
    push_u24(&mut m, body.len());
    m.extend_from_slice(body);
    m
}

fn nonce_for(iv: &[u8; 12], seq: u64) -> [u8; 12] {
    let mut n = *iv;
    let s = seq.to_be_bytes();
    for i in 0..8 {
        n[4 + i] ^= s[i];
    }
    n
}

fn read_record(stream: &mut TcpStream) -> Result<([u8; 5], u8, Vec<u8>), TlsError> {
    let mut hdr = [0u8; 5];
    stream.read_exact(&mut hdr).map_err(TlsError::Io)?;
    let len = u16::from_be_bytes([hdr[3], hdr[4]]) as usize;
    if len > MAX_RECORD {
        return Err(TlsError::Protocol("record too large"));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).map_err(TlsError::Io)?;
    Ok((hdr, hdr[0], payload))
}

fn write_plain(
    stream: &mut TcpStream,
    version: u16,
    typ: u8,
    payload: &[u8],
) -> Result<(), TlsError> {
    let mut rec = Vec::with_capacity(5 + payload.len());
    rec.push(typ);
    rec.extend_from_slice(&version.to_be_bytes());
    push_u16(&mut rec, payload.len());
    rec.extend_from_slice(payload);
    stream.write_all(&rec).map_err(TlsError::Io)
}

fn write_encrypted(
    stream: &mut TcpStream,
    keys: &Keys,
    seq: u64,
    typ: u8,
    payload: &[u8],
) -> Result<(), TlsError> {
    let mut inner = payload.to_vec();
    inner.push(typ);
    let mut hdr = [0x17u8, 0x03, 0x03, 0, 0];
    let len = inner.len() + 16;
    hdr[3] = (len >> 8) as u8;
    hdr[4] = len as u8;
    let nonce = nonce_for(&keys.iv, seq);
    let (ct, tag) = aead::seal(
        keys.suite,
        &keys.key[..keys.key_len],
        &nonce,
        &hdr,
        &inner,
    );
    let mut rec = Vec::with_capacity(5 + ct.len() + 16);
    rec.extend_from_slice(&hdr);
    rec.extend_from_slice(&ct);
    rec.extend_from_slice(&tag);
    stream.write_all(&rec).map_err(TlsError::Io)
}

fn open_record(
    keys: &Keys,
    seq: u64,
    hdr: &[u8; 5],
    payload: &[u8],
) -> Result<(u8, Vec<u8>), TlsError> {
    if payload.len() < 17 {
        return Err(TlsError::Protocol("short record"));
    }
    let (ct, tag) = payload.split_at(payload.len() - 16);
    let mut tag_arr = [0u8; 16];
    tag_arr.copy_from_slice(tag);
    let nonce = nonce_for(&keys.iv, seq);
    let plain = aead::open(keys.suite, &keys.key[..keys.key_len], &nonce, hdr, ct, &tag_arr)
        .ok_or(TlsError::Protocol("decrypt failed"))?;
    let pos = plain
        .iter()
        .rposition(|b| *b != 0)
        .ok_or(TlsError::Protocol("empty inner"))?;
    let typ = plain[pos];
    match typ {
        21 | 22 | 23 => Ok((typ, plain[..pos].to_vec())),
        _ => Err(TlsError::Protocol("bad content type")),
    }
}

fn alert_err(payload: &[u8]) -> TlsError {
    if payload.is_empty() {
        TlsError::Protocol("empty alert")
    } else {
        TlsError::Alert(payload[payload.len() - 1])
    }
}

struct Hello {
    server_pub: [u8; 32],
    suite: aead::Suite,
}

fn parse_server_hello(b: &[u8]) -> Result<Hello, TlsError> {
    if b.len() < 39 {
        return Err(TlsError::Protocol("short server hello"));
    }
    let mut p = 34;
    let sid_len = b[p] as usize;
    p += 1;
    if b.len() < p + sid_len + 4 {
        return Err(TlsError::Protocol("short server hello"));
    }
    p += sid_len;
    let cipher = u16::from_be_bytes([b[p], b[p + 1]]);
    p += 2;
    let suite = aead::Suite::from_id(cipher).ok_or(TlsError::Protocol("cipher suite"))?;
    if b.len() < p + 2 {
        return Err(TlsError::Protocol("short server hello"));
    }
    let comp_len = b[p] as usize;
    p += 1;
    p += comp_len;
    if b.len() < p + 2 {
        return Err(TlsError::Protocol("short server hello"));
    }
    let ext_total = u16::from_be_bytes([b[p], b[p + 1]]) as usize;
    let end = p + 2 + ext_total;
    if b.len() < end {
        return Err(TlsError::Protocol("short server hello"));
    }
    let mut q = p + 2;
    let mut version_ok = false;
    let mut server_pub = None;
    while q + 4 <= end {
        let t = u16::from_be_bytes([b[q], b[q + 1]]);
        let l = u16::from_be_bytes([b[q + 2], b[q + 3]]) as usize;
        q += 4;
        if q + l > end {
            return Err(TlsError::Protocol("bad extension"));
        }
        let d = &b[q..q + l];
        q += l;
        if t == 43 && d == [0x03, 0x04] {
            version_ok = true;
        } else if t == 51 && d.len() == 36 {
            let group = u16::from_be_bytes([d[0], d[1]]);
            let klen = u16::from_be_bytes([d[2], d[3]]) as usize;
            if group == GROUP_X25519 && klen == 32 {
                let mut k = [0u8; 32];
                k.copy_from_slice(&d[4..36]);
                server_pub = Some(k);
            }
        }
    }
    if !version_ok {
        return Err(TlsError::Protocol("not tls 1.3"));
    }
    match server_pub {
        Some(s) => Ok(Hello {
            server_pub: s,
            suite,
        }),
        None => Err(TlsError::Protocol("no key share")),
    }
}

fn parse_chain(body: &[u8]) -> Result<Vec<x509::Cert>, TlsError> {
    if body.is_empty() {
        return Err(TlsError::Protocol("bad certificate"));
    }
    let ctx_len = body[0] as usize;
    if ctx_len != 0 || body.len() < 4 + ctx_len {
        return Err(TlsError::Protocol("bad certificate"));
    }
    let list_len = be24(&body[1 + ctx_len..4 + ctx_len]);
    if body.len() < 4 + ctx_len + list_len {
        return Err(TlsError::Protocol("bad certificate"));
    }
    let list = &body[4 + ctx_len..4 + ctx_len + list_len];
    let mut certs = Vec::new();
    let mut p = 0usize;
    while p + 3 <= list.len() {
        let cert_len = be24(&list[p..p + 3]);
        p += 3;
        if p + cert_len > list.len() {
            return Err(TlsError::Protocol("bad certificate"));
        }
        let der = &list[p..p + cert_len];
        p += cert_len;
        let cert = x509::parse(der).ok_or(TlsError::Cert("unparsable"))?;
        certs.push(cert);
        if p + 2 > list.len() {
            return Err(TlsError::Protocol("bad certificate"));
        }
        let ext_len = u16::from_be_bytes([list[p], list[p + 1]]) as usize;
        p += 2 + ext_len;
        if p > list.len() {
            return Err(TlsError::Protocol("bad certificate"));
        }
    }
    if certs.is_empty() {
        return Err(TlsError::Cert("empty chain"));
    }
    Ok(certs)
}

fn verify_certificate_verify(
    cert: &x509::Cert,
    body: &[u8],
    th: &[u8],
) -> Result<(), TlsError> {
    if body.len() < 4 {
        return Err(TlsError::Protocol("bad certificate verify"));
    }
    let alg = u16::from_be_bytes([body[0], body[1]]);
    let slen = u16::from_be_bytes([body[2], body[3]]) as usize;
    if body.len() != 4 + slen {
        return Err(TlsError::Protocol("bad certificate verify"));
    }
    let sig = &body[4..];
    let mut ctx = Vec::with_capacity(64 + 33 + 32);
    ctx.extend_from_slice(&[0x20u8; 64]);
    ctx.extend_from_slice(b"TLS 1.3, server CertificateVerify");
    ctx.push(0);
    ctx.extend_from_slice(th);
    let hash = sha256::hash(&ctx);
    let ok = match (&cert.key, alg) {
        (x509::PubKey::Rsa { n, e }, SIG_RSA_PSS_SHA256) => match rsa::RsaPub::new(n, e) {
            Some(k) => k.verify_pss_sha256(&hash, sig),
            None => false,
        },
        (x509::PubKey::EcP256 { x, y }, SIG_ECDSA_P256_SHA256) => {
            match chain::ecdsa_sig_parts(sig) {
                Ok((r, s)) => p256::verify_ecdsa_p256_sha256(&hash, x, y, r, s),
                Err(m) => return Err(TlsError::Cert(m)),
            }
        }
        _ => return Err(TlsError::Cert("unsupported signature algorithm")),
    };
    if !ok {
        return Err(TlsError::Cert("signature invalid"));
    }
    Ok(())
}

pub fn connect(stream: TcpStream, host: &str) -> Result<Client, TlsError> {
    let roots = roots::load().map_err(TlsError::Cert)?;
    connect_with_roots(stream, host, roots)
}

pub fn connect_with_roots(
    stream: TcpStream,
    host: &str,
    roots: &[x509::Cert],
) -> Result<Client, TlsError> {
    let mut stream = stream;
    let (read, write, _) = handshake(&mut stream, host, roots, None)?;
    Ok(Client {
        stream,
        pending: Vec::new(),
        read,
        write,
        read_seq: 0,
        write_seq: 0,
        got_data: false,
    })
}

pub fn connect_with_cache(
    stream: TcpStream,
    host: &str,
    cache: &ChainCache,
) -> Result<Client, TlsError> {
    let roots = roots::load().map_err(TlsError::Cert)?;
    let mut stream = stream;
    let (read, write, _) = handshake(&mut stream, host, roots, Some(cache))?;
    Ok(Client {
        stream,
        pending: Vec::new(),
        read,
        write,
        read_seq: 0,
        write_seq: 0,
        got_data: false,
    })
}

fn handshake(
    stream: &mut TcpStream,
    host: &str,
    roots: &[x509::Cert],
    cache: Option<&ChainCache>,
) -> Result<(Keys, Keys, aead::Suite), TlsError> {
    let mut rnd = [0u8; 96];
    random_bytes(&mut rnd).map_err(TlsError::Io)?;
    let mut client_random = [0u8; 32];
    let mut session_id = [0u8; 32];
    let mut client_priv = [0u8; 32];
    client_random.copy_from_slice(&rnd[0..32]);
    session_id.copy_from_slice(&rnd[32..64]);
    client_priv.copy_from_slice(&rnd[64..96]);
    let client_pub = x25519::base_mult(client_priv);

    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]);
    body.extend_from_slice(&client_random);
    body.push(32);
    body.extend_from_slice(&session_id);
    body.extend_from_slice(&[0x00, 0x06]);
    body.extend_from_slice(&0x1301u16.to_be_bytes());
    body.extend_from_slice(&0x1302u16.to_be_bytes());
    body.extend_from_slice(&0x1303u16.to_be_bytes());
    body.extend_from_slice(&[0x01, 0x00]);
    let mut ext = Vec::new();
    if host.parse::<IpAddr>().is_err() {
        let h = host.strip_suffix('.').unwrap_or(host);
        let mut sni = Vec::new();
        push_u16(&mut sni, h.len() + 3);
        sni.push(0x00);
        push_u16(&mut sni, h.len());
        sni.extend_from_slice(h.as_bytes());
        ext.extend_from_slice(&0x0000u16.to_be_bytes());
        push_u16(&mut ext, sni.len());
        ext.extend_from_slice(&sni);
    }
    ext.extend_from_slice(&0x000au16.to_be_bytes());
    push_u16(&mut ext, 4);
    ext.extend_from_slice(&[0x00, 0x02, 0x00, 0x1d]);
    ext.extend_from_slice(&0x000du16.to_be_bytes());
    push_u16(&mut ext, 6);
    ext.extend_from_slice(&[0x00, 0x04, 0x04, 0x03, 0x08, 0x04]);
    ext.extend_from_slice(&0x002bu16.to_be_bytes());
    push_u16(&mut ext, 3);
    ext.extend_from_slice(&[0x02, 0x03, 0x04]);
    ext.extend_from_slice(&0x0033u16.to_be_bytes());
    push_u16(&mut ext, 38);
    ext.extend_from_slice(&[0x00, 0x24, 0x00, 0x1d, 0x00, 0x20]);
    ext.extend_from_slice(&client_pub);
    push_u16(&mut body, ext.len());
    body.extend_from_slice(&ext);

    let ch = handshake_msg(1, &body);
    let mut transcript: Vec<u8> = Vec::new();
    transcript.extend_from_slice(&ch);
    write_plain(stream, 0x0301, 22, &ch)?;

    let (_, _, payload) = loop {
        let (hdr, typ, payload) = read_record(stream)?;
        match typ {
            20 => continue,
            21 => return Err(alert_err(&payload)),
            22 => break (hdr, typ, payload),
            _ => return Err(TlsError::Protocol("expected server hello")),
        }
    };
    if payload.len() < 4 || handshake_msg(payload[0], &payload[4..]) != payload {
        return Err(TlsError::Protocol("bad server hello framing"));
    }
    let hello = parse_server_hello(&payload[4..])?;
    transcript.extend_from_slice(&payload);
    if payload[0] == 2 && is_retry(&payload[4..]) {
        return Err(TlsError::Protocol("hello retry not supported"));
    }
    let shared = x25519::shared(client_priv, hello.server_pub);

    let hash = hello.suite.hash();
    let empty_hash = hash.one_shot(&[]);
    let zeros = vec![0u8; hash.len()];
    let early = hmac::extract(hash, &zeros, &zeros);
    let d1 = hmac::derive_secret(hash, &early, "derived", &empty_hash);
    let hs_secret = hmac::extract(hash, &d1, &shared);
    let th1 = hash.one_shot(&transcript);
    let c_hs = keys_for(
        hash,
        &hmac::derive_secret(hash, &hs_secret, "c hs traffic", &th1),
        hello.suite,
    );
    let s_hs = keys_for(
        hash,
        &hmac::derive_secret(hash, &hs_secret, "s hs traffic", &th1),
        hello.suite,
    );

    let mut buf: Vec<u8> = Vec::new();
    let mut got_ee = false;
    let mut got_cert = false;
    let mut got_cv = false;
    let mut certs: Vec<x509::Cert> = Vec::new();
    let mut read_seq: u64 = 0;
    loop {
        let (hdr, typ, payload) = read_record(stream)?;
        match typ {
            20 => continue,
            21 => return Err(alert_err(&payload)),
            23 => {
                let (inner, plain) = open_record(&s_hs, read_seq, &hdr, &payload)?;
                read_seq += 1;
                match inner {
                    22 => buf.extend_from_slice(&plain),
                    21 => return Err(alert_err(&plain)),
                    _ => return Err(TlsError::Protocol("unexpected content")),
                }
            }
            _ => return Err(TlsError::Protocol("unexpected record")),
        }
        let mut finished = false;
        while buf.len() >= 4 {
            let mlen = be24(&buf[1..4]);
            if buf.len() < 4 + mlen {
                break;
            }
            let msg: Vec<u8> = buf.drain(..4 + mlen).collect();
            match msg[0] {
                8 => {
                    if got_ee {
                        return Err(TlsError::Protocol("duplicate encrypted extensions"));
                    }
                    got_ee = true;
                }
                11 => {
                    if !got_ee || got_cert {
                        return Err(TlsError::Protocol("unexpected certificate"));
                    }
                    certs = parse_chain(&msg[4..])?;
                    let leaf = certs.first().ok_or(TlsError::Cert("empty chain"))?;
                    let now = x509::unix_now();
                    if cache.map_or(true, |c| !c.hit(host, &leaf.tbs, now)) {
                        chain::validate(&certs, roots, host, now).map_err(TlsError::Cert)?;
                        if let Some(c) = cache {
                            c.remember(host, &leaf.tbs, leaf.not_after);
                        }
                    }
                    got_cert = true;
                }
                15 => {
                    if !got_cert || got_cv {
                        return Err(TlsError::Protocol("unexpected certificate verify"));
                    }
                    let cert = certs.first().ok_or(TlsError::Cert("no certificate"))?;
                    let th = hash.one_shot(&transcript);
                    verify_certificate_verify(cert, &msg[4..], &th)?;
                    got_cv = true;
                }
                20 => {
                    if !got_ee || !got_cert || !got_cv {
                        return Err(TlsError::Protocol("unexpected finished"));
                    }
                    let th = hash.one_shot(&transcript);
                    let expect = hmac::hmac(hash, &s_hs.finished, &th);
                    if msg.len() != 4 + hash.len() || msg[4..] != expect[..] {
                        return Err(TlsError::Protocol("bad finished"));
                    }
                    finished = true;
                }
                _ => return Err(TlsError::Protocol("unexpected handshake message")),
            }
            transcript.extend_from_slice(&msg);
        }
        if finished {
            break;
        }
    }

    let master_salt = hmac::derive_secret(hash, &hs_secret, "derived", &empty_hash);
    let master = hmac::extract(hash, &master_salt, &zeros);
    let th2 = hash.one_shot(&transcript);
    let c_ap = keys_for(
        hash,
        &hmac::derive_secret(hash, &master, "c ap traffic", &th2),
        hello.suite,
    );
    let s_ap = keys_for(
        hash,
        &hmac::derive_secret(hash, &master, "s ap traffic", &th2),
        hello.suite,
    );

    let cf = hmac::hmac(hash, &c_hs.finished, &th2);
    let cf_msg = handshake_msg(20, &cf);
    write_plain(stream, 0x0303, 20, &[1])?;
    write_encrypted(stream, &c_hs, 0, 22, &cf_msg)?;
    Ok((s_ap, c_ap, hello.suite))
}

fn is_retry(body: &[u8]) -> bool {
    const HRR: [u8; 32] = [
        0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8,
        0x91, 0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8,
        0x33, 0x9c,
    ];
    body.len() >= 34 && body[2..34] == HRR
}

pub struct Client {
    stream: TcpStream,
    pending: Vec<u8>,
    read: Keys,
    write: Keys,
    read_seq: u64,
    write_seq: u64,
    got_data: bool,
}

impl Client {
    pub fn close(&mut self) -> Result<(), TlsError> {
        write_encrypted(&mut self.stream, &self.write, self.write_seq, 21, &[0x01, 0x00])?;
        self.write_seq += 1;
        self.stream.flush().map_err(TlsError::Io)
    }
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if !self.pending.is_empty() {
                let n = buf.len().min(self.pending.len());
                buf[..n].copy_from_slice(&self.pending[..n]);
                self.pending.drain(..n);
                self.got_data = true;
                return Ok(n);
            }
            let (hdr, typ, payload) = match read_record(&mut self.stream) {
                Ok(r) => r,
                Err(TlsError::Io(e))
                    if e.kind() == io::ErrorKind::UnexpectedEof && self.got_data =>
                {
                    return Ok(0);
                }
                Err(e) => return Err(e.into()),
            };
            match typ {
                20 => continue,
                21 => {
                    if payload.len() >= 2 && payload[1] == 0 {
                        return Ok(0);
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        format!("tls alert {:02x?}", payload),
                    ));
                }
                23 => {
                    let (inner, plain) = open_record(&self.read, self.read_seq, &hdr, &payload)?;
                    self.read_seq += 1;
                    match inner {
                        23 => {
                            self.got_data = true;
                            self.pending = plain;
                        }
                        21 => {
                            if plain.len() >= 2 && plain[1] == 0 {
                                return Ok(0);
                            }
                            return Err(io::Error::new(
                                io::ErrorKind::ConnectionAborted,
                                format!("tls alert {:02x?}", plain),
                            ));
                        }
                        22 => continue,
                        _ => {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "unexpected content",
                            ))
                        }
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected record",
                    ))
                }
            }
        }
    }
}

impl Write for Client {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for chunk in buf.chunks(1 << 14) {
            let mut inner = chunk.to_vec();
            inner.push(23);
            let mut hdr = [0x17u8, 0x03, 0x03, 0, 0];
            let len = inner.len() + 16;
            hdr[3] = (len >> 8) as u8;
            hdr[4] = len as u8;
            let nonce = nonce_for(&self.write.iv, self.write_seq);
            let (ct, tag) = aead::seal(
                self.write.suite,
                &self.write.key[..self.write.key_len],
                &nonce,
                &hdr,
                &inner,
            );
            self.write_seq += 1;
            let mut rec = Vec::with_capacity(5 + ct.len() + 16);
            rec.extend_from_slice(&hdr);
            rec.extend_from_slice(&ct);
            rec.extend_from_slice(&tag);
            self.stream.write_all(&rec)?;
        }
        self.stream.flush()?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{alert_err, handshake_msg, nonce_for, push_u24, TlsError};

    #[test]
    fn handshake_framing_roundtrip() {
        let bodies: [&[u8]; 4] = [&[], &[1], &[1, 2, 3], &[7u8; 70000]];
        for b in bodies {
            let m = handshake_msg(11, b);
            assert_eq!(m[0], 11);
            assert_eq!(&m[1..4], &push_u24_extract(&m));
            assert_eq!(m.len(), 4 + b.len());
        }
    }

    fn push_u24_extract(m: &[u8]) -> [u8; 3] {
        let mut v = Vec::new();
        push_u24(&mut v, m.len() - 4);
        [v[0], v[1], v[2]]
    }

    #[test]
    fn nonce_xor_seq() {
        let iv = [1u8; 12];
        let n0 = nonce_for(&iv, 0);
        assert_eq!(n0, iv);
        let n1 = nonce_for(&iv, 1);
        assert_eq!(n1[11], 0);
        assert_eq!(n1[4..], [1, 1, 1, 1, 1, 1, 1, 0][..]);
        let nbig = nonce_for(&iv, u64::MAX);
        assert_eq!(
            nbig[4..],
            [0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe][..]
        );
    }

    #[test]
    fn alert_mapping() {
        assert!(matches!(alert_err(&[]), TlsError::Protocol("empty alert")));
        assert!(matches!(alert_err(&[1, 2]), TlsError::Alert(2)));
    }
}
