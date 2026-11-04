use crate::der::{self, Reader};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PubKey {
    Rsa { n: Vec<u8>, e: Vec<u8> },
    EcP256 { x: Vec<u8>, y: Vec<u8> },
    EcP384 { x: Vec<u8>, y: Vec<u8> },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SigAlg {
    RsaPkcs1Sha256,
    RsaPkcs1Sha384,
    RsaPkcs1Sha512,
    RsaPssSha256,
    EcdsaSha256,
    EcdsaSha384,
    EcdsaSha512,
    Other,
}

#[derive(Clone, Debug)]
pub struct Cert {
    pub key: PubKey,
    pub not_before: i64,
    pub not_after: i64,
    pub dns_names: Vec<String>,
    pub subject: Vec<u8>,
    pub issuer: Vec<u8>,
    pub tbs: Vec<u8>,
    pub sig_alg: SigAlg,
    pub sig: Vec<u8>,
    pub ca: bool,
    pub path_len: Option<u32>,
    pub key_cert_sign: bool,
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn tlv_both<'a>(r: &mut Reader<'a>) -> Option<(&'a [u8], &'a [u8])> {
    let start = r.rest();
    let (_, v) = r.tlv()?;
    Some((&start[..start.len() - r.rest().len()], v))
}

fn be_u32(b: &[u8]) -> u32 {
    let mut v = 0u32;
    for &x in b {
        v = (v << 8) | x as u32;
    }
    v
}

fn parse_sig_alg(content: &[u8]) -> Option<SigAlg> {
    let mut r = Reader::new(content);
    let oid = r.next(0x06)?;
    Some(if oid == der::OID_SHA256_RSA {
        SigAlg::RsaPkcs1Sha256
    } else if oid == der::OID_SHA384_RSA {
        SigAlg::RsaPkcs1Sha384
    } else if oid == der::OID_SHA512_RSA {
        SigAlg::RsaPkcs1Sha512
    } else if oid == der::OID_ECDSA_SHA256 {
        SigAlg::EcdsaSha256
    } else if oid == der::OID_ECDSA_SHA384 {
        SigAlg::EcdsaSha384
    } else if oid == der::OID_ECDSA_SHA512 {
        SigAlg::EcdsaSha512
    } else if oid == der::OID_RSA_PSS {
        let mut pr = match r.next(0x30) {
            Some(p) => Reader::new(p),
            None => return Some(SigAlg::Other),
        };
        if pr.next(0x06) == Some(der::OID_SHA2_256) {
            SigAlg::RsaPssSha256
        } else {
            SigAlg::Other
        }
    } else {
        SigAlg::Other
    })
}

pub fn parse(der: &[u8]) -> Option<Cert> {
    let mut outer = Reader::new(der);
    let cert = outer.next(0x30)?;
    let mut cr = Reader::new(cert);
    let (tbs_raw, tbs) = tlv_both(&mut cr)?;
    if tbs_raw[0] != 0x30 {
        return None;
    }
    let mut tr = Reader::new(tbs);
    if tr.rest().first() == Some(&0xa0) {
        tr.tlv()?;
    }
    tr.next(0x02)?;
    let (inner_alg_raw, _) = tlv_both(&mut tr)?;
    if inner_alg_raw[0] != 0x30 {
        return None;
    }
    let (issuer_raw, _) = tlv_both(&mut tr)?;
    if issuer_raw[0] != 0x30 {
        return None;
    }
    let validity = tr.next(0x30)?;
    let mut vr = Reader::new(validity);
    let not_before = parse_time(vr.tlv()?)?;
    let not_after = parse_time(vr.tlv()?)?;
    let (subject_raw, _) = tlv_both(&mut tr)?;
    if subject_raw[0] != 0x30 {
        return None;
    }
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
        if params == der::OID_P256 {
            if keybits.len() != 65 || keybits[0] != 0x04 {
                return None;
            }
            PubKey::EcP256 {
                x: keybits[1..33].to_vec(),
                y: keybits[33..65].to_vec(),
            }
        } else if params == der::OID_P384 {
            if keybits.len() != 97 || keybits[0] != 0x04 {
                return None;
            }
            PubKey::EcP384 {
                x: keybits[1..49].to_vec(),
                y: keybits[49..97].to_vec(),
            }
        } else {
            return None;
        }
    } else {
        return None;
    };
    while matches!(tr.rest().first(), Some(&0xa1) | Some(&0xa2)) {
        tr.tlv()?;
    }
    let mut dns_names = Vec::new();
    let mut ca = false;
    let mut path_len = None;
    let mut key_cert_sign = true;
    if tr.rest().first() == Some(&0xa3) {
        let exts = tr.tlv()?.1;
        let mut er = Reader::new(exts);
        let seq = er.next(0x30)?;
        let mut sr = Reader::new(seq);
        while let Some((_, ext)) = sr.tlv() {
            let mut xr = Reader::new(ext);
            let id = match xr.next(0x06) {
                Some(id) => id,
                None => continue,
            };
            if xr.rest().first() == Some(&0x01) {
                let _ = xr.tlv();
            }
            let val = match xr.next(0x04) {
                Some(v) => v,
                None => continue,
            };
            if id == der::OID_SAN {
                let mut vr = Reader::new(val);
                if let Some(names) = vr.next(0x30) {
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
            } else if id == der::OID_BASIC_CONSTRAINTS {
                let mut vr = Reader::new(val);
                if let Some(seq) = vr.next(0x30) {
                    let mut br = Reader::new(seq);
                    if let Some((tag, flag)) = br.tlv() {
                        if tag == 0x01 {
                            ca = flag.first().map(|b| *b != 0).unwrap_or(false);
                            if let Some((tag, n)) = br.tlv() {
                                if tag == 0x02 {
                                    let b = der::int_bytes(n);
                                    if !b.is_empty() && b.len() <= 4 {
                                        path_len = Some(be_u32(b));
                                    }
                                }
                            }
                        }
                    }
                }
            } else if id == der::OID_KEY_USAGE {
                let mut vr = Reader::new(val);
                if let Some(bits) = vr.next(0x03).and_then(unpack_bits) {
                    if !bits.is_empty() {
                        key_cert_sign = (bits[0] >> 2) & 1 == 1;
                    }
                }
            }
        }
    }
    let (outer_alg_raw, outer_alg) = tlv_both(&mut cr)?;
    if outer_alg_raw[0] != 0x30 || outer_alg_raw != inner_alg_raw {
        return None;
    }
    let sig_alg = parse_sig_alg(outer_alg)?;
    let sig_bits = cr.next(0x03)?;
    let sig = unpack_bits(sig_bits)?.to_vec();
    Some(Cert {
        key,
        not_before,
        not_after,
        dns_names,
        subject: subject_raw.to_vec(),
        issuer: issuer_raw.to_vec(),
        tbs: tbs_raw.to_vec(),
        sig_alg,
        sig,
        ca,
        path_len,
        key_cert_sign,
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

fn parse_time(tagged: (u8, &[u8])) -> Option<i64> {
    let (tag, b) = tagged;
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
    use super::{days_from_civil, host_matches, parse_sig_alg, SigAlg};
    use crate::der::{OID_ECDSA_SHA256, OID_RSA_PSS, OID_SHA2_256, OID_SHA256_RSA};

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

    #[test]
    fn sig_alg_oids() {
        let sha256_rsa = [0x06u8, 0x09]
            .iter()
            .copied()
            .chain(OID_SHA256_RSA.iter().copied())
            .chain([0x05, 0x00])
            .collect::<Vec<u8>>();
        assert_eq!(parse_sig_alg(&sha256_rsa), Some(SigAlg::RsaPkcs1Sha256));
        let ecdsa = [0x06u8, 0x08]
            .iter()
            .copied()
            .chain(OID_ECDSA_SHA256.iter().copied())
            .collect::<Vec<u8>>();
        assert_eq!(parse_sig_alg(&ecdsa), Some(SigAlg::EcdsaSha256));
        let pss_sha1 = [0x06u8, 0x09]
            .iter()
            .copied()
            .chain(OID_RSA_PSS.iter().copied())
            .collect::<Vec<u8>>();
        assert_eq!(parse_sig_alg(&pss_sha1), Some(SigAlg::Other));
        let pss_sha256 = [0x06u8, 0x09]
            .iter()
            .copied()
            .chain(OID_RSA_PSS.iter().copied())
            .chain([0x30u8, 0x0b, 0x06, 0x09])
            .chain(OID_SHA2_256.iter().copied())
            .collect::<Vec<u8>>();
        assert_eq!(parse_sig_alg(&pss_sha256), Some(SigAlg::RsaPssSha256));
        assert_eq!(parse_sig_alg(&[]), None);
    }
}
