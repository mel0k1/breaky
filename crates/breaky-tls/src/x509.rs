use crate::der::{self, Reader};
use std::time::{SystemTime, UNIX_EPOCH};

pub enum PubKey {
    Rsa { n: Vec<u8>, e: Vec<u8> },
    EcP256 { x: Vec<u8>, y: Vec<u8> },
}

pub struct Cert {
    pub key: PubKey,
    pub not_before: i64,
    pub not_after: i64,
    pub dns_names: Vec<String>,
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn parse(der: &[u8]) -> Option<Cert> {
    let mut outer = Reader::new(der);
    let cert = outer.next(0x30)?;
    let mut cr = Reader::new(cert);
    let tbs = cr.next(0x30)?;
    let mut tr = Reader::new(tbs);
    if tr.rest().first() == Some(&0xa0) {
        tr.tlv()?;
    }
    tr.next(0x02)?;
    tr.next(0x30)?;
    tr.next(0x30)?;
    let validity = tr.next(0x30)?;
    let mut vr = Reader::new(validity);
    let nb = vr.tlv()?;
    let na = vr.tlv()?;
    let not_before = parse_time(nb.0, nb.1)?;
    let not_after = parse_time(na.0, na.1)?;
    tr.next(0x30)?;
    let spki = tr.next(0x30)?;
    let mut sr = Reader::new(spki);
    let alg = sr.next(0x30)?;
    let bits = sr.next(0x03)?;
    let mut ar = Reader::new(alg);
    let oid = ar.next(0x06)?;
    let keybits = unpack_bits(bits)?;
    let key = if oid == der::OID_RSA {
        let mut kr = Reader::new(keybits);
        let rs = kr.next(0x30)?;
        let mut ir = Reader::new(rs);
        let n = ir.next(0x02)?;
        let e = ir.next(0x02)?;
        PubKey::Rsa {
            n: der::int_bytes(n).to_vec(),
            e: der::int_bytes(e).to_vec(),
        }
    } else if oid == der::OID_EC {
        let params = ar.next(0x06)?;
        if params != der::OID_P256 || keybits.len() != 65 || keybits[0] != 0x04 {
            return None;
        }
        PubKey::EcP256 {
            x: keybits[1..33].to_vec(),
            y: keybits[33..65].to_vec(),
        }
    } else {
        return None;
    };
    let mut dns_names = Vec::new();
    if tr.rest().first() == Some(&0xa3) {
        if let Some(exts) = tr.tlv() {
            let mut er = Reader::new(exts.1);
            if let Some(seq) = er.next(0x30) {
                let mut sr = Reader::new(seq);
                while let Some((_, ext)) = sr.tlv() {
                    let mut xr = Reader::new(ext);
                    let id = xr.next(0x06)?;
                    if id != der::OID_SAN {
                        continue;
                    }
                    let val = xr.next(0x04)?;
                    let mut vr = Reader::new(val);
                    let names = vr.next(0x30)?;
                    let mut nr = Reader::new(names);
                    while let Some((tag, name)) = nr.tlv() {
                        match tag {
                            0x82 => {
                                if let Ok(s) = std::str::from_utf8(name) {
                                    dns_names.push(s.to_string());
                                }
                            }
                            0x87 if name.len() == 4 => {
                                dns_names.push(format!(
                                    "{}.{}.{}.{}",
                                    name[0], name[1], name[2], name[3]
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    Some(Cert {
        key,
        not_before,
        not_after,
        dns_names,
    })
}

fn unpack_bits(b: &[u8]) -> Option<&[u8]> {
    if b.is_empty() {
        return None;
    }
    let unused = b[0] as usize;
    if unused > 7 {
        return None;
    }
    Some(&b[1..])
}

fn parse_time(tag: u8, b: &[u8]) -> Option<i64> {
    let (year, rest) = match tag {
        0x17 if b.len() == 13 => {
            let y = (b[0] - b'0') as i64 * 10 + (b[1] - b'0') as i64;
            (if y < 50 { 2000 + y } else { 1900 + y }, &b[2..])
        }
        0x18 if b.len() == 15 => {
            let y = (b[0] - b'0') as i64 * 1000
                + (b[1] - b'0') as i64 * 100
                + (b[2] - b'0') as i64 * 10
                + (b[3] - b'0') as i64;
            (y, &b[4..])
        }
        _ => return None,
    };
    let num = |s: &[u8]| -> Option<i64> {
        if s.len() < 2 {
            return None;
        }
        Some((s[0] - b'0') as i64 * 10 + (s[1] - b'0') as i64)
    };
    let month = num(&rest[0..2])?;
    let day = num(&rest[2..4])?;
    let hour = num(&rest[4..6])?;
    let min = num(&rest[6..8])?;
    let sec = num(&rest[8..10])?;
    if rest[10] != b'Z' {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month as u32, day as u32);
    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn host_matches(dns: &[String], host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host).to_lowercase();
    dns.iter().any(|d| {
        let d = d.trim_end_matches('.').to_lowercase();
        if d == host {
            return true;
        }
        if let Some(w) = d.strip_prefix("*.") {
            if let Some(label) = host.strip_suffix(w) {
                if let Some(base) = label.strip_suffix('.') {
                    return !base.is_empty() && !base.contains('.');
                }
            }
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::{days_from_civil, host_matches};

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hostname_exact_and_case() {
        let n = names(&["example.com", "www.Example.COM"]);
        assert!(host_matches(&n, "example.com"));
        assert!(host_matches(&n, "EXAMPLE.com"));
        assert!(host_matches(&n, "www.example.com"));
        assert!(host_matches(&n, "www.EXAMPLE.coM"));
        assert!(!host_matches(&n, "other.example.com"));
        assert!(!host_matches(&n, "example.org"));
        assert!(!host_matches(&n, ""));
    }

    #[test]
    fn hostname_wildcard() {
        let n = names(&["*.cdn.io"]);
        assert!(host_matches(&n, "a.cdn.io"));
        assert!(host_matches(
            &n,
            "deep-value.cdn.io".replace("deep-value", "x").as_str()
        ));
        assert!(!host_matches(&n, "a.b.cdn.io"));
        assert!(!host_matches(&n, "cdn.io"));
        assert!(!host_matches(&n, ".cdn.io"));
    }

    #[test]
    fn hostname_trailing_dot_and_ip() {
        assert!(host_matches(&names(&["example.com"]), "example.com."));
        assert!(host_matches(&names(&["127.0.0.1"]), "127.0.0.1"));
        assert!(!host_matches(&names(&["*.0.0.1"]), "1.2.3.4.0.0.1"));
    }

    #[test]
    fn civil_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
        assert_eq!(days_from_civil(2026, 1, 1), 20454);
    }
}
