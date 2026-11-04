use crate::der::Reader;
use crate::p256;
use crate::p384;
use crate::rsa::{RsaPub, PREFIX_SHA256, PREFIX_SHA384, PREFIX_SHA512};
use crate::sha256;
use crate::sha512;
use crate::x509::{Cert, PubKey, SigAlg};

const MAX_PATH: usize = 8;

pub fn validate(chain: &[Cert], roots: &[Cert], host: &str, now: i64) -> Result<(), &'static str> {
    let leaf = chain.first().ok_or("empty chain")?;
    check_validity(leaf, now)?;
    if leaf.dns_names.is_empty() || !crate::x509::host_matches(&leaf.dns_names, host) {
        return Err("hostname mismatch");
    }
    let mut used: Vec<usize> = Vec::new();
    let mut cur = leaf;
    loop {
        if let Some(root) = roots.iter().find(|r| r.subject == cur.issuer) {
            if root.path_len.map_or(false, |n| used.len() > n as usize) {
                return Err("path length exceeded");
            }
            verify_cert_sig(cur, &root.key)?;
            return Ok(());
        }
        let idx = chain
            .iter()
            .enumerate()
            .position(|(i, c)| i != 0 && !used.contains(&i) && c.subject == cur.issuer)
            .ok_or("no issuer for certificate")?;
        let issuer = &chain[idx];
        check_validity(issuer, now)?;
        if !issuer.ca || !issuer.key_cert_sign {
            return Err("issuer is not a ca");
        }
        if used.len() >= MAX_PATH {
            return Err("chain too long");
        }
        if issuer.path_len.map_or(false, |n| used.len() > n as usize) {
            return Err("path length exceeded");
        }
        verify_cert_sig(cur, &issuer.key)?;
        used.push(idx);
        cur = issuer;
    }
}

fn check_validity(cert: &Cert, now: i64) -> Result<(), &'static str> {
    if now < cert.not_before {
        return Err("not yet valid");
    }
    if now > cert.not_after {
        return Err("expired");
    }
    Ok(())
}

fn with_rsa(n: &[u8], e: &[u8], f: impl FnOnce(&RsaPub) -> bool) -> bool {
    RsaPub::new(n, e).as_ref().map_or(false, f)
}

fn h32(v: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&v[..32]);
    out
}

pub(crate) fn ecdsa_sig_parts(sig: &[u8]) -> Result<(&[u8], &[u8]), &'static str> {
    let mut r = Reader::new(sig);
    let seq = r.next(0x30).ok_or("bad ecdsa signature")?;
    let mut sr = Reader::new(seq);
    let rb = sr.next(0x02).ok_or("bad ecdsa signature")?;
    let sb = sr.next(0x02).ok_or("bad ecdsa signature")?;
    Ok((crate::der::int_bytes(rb), crate::der::int_bytes(sb)))
}

fn verify_cert_sig(cert: &Cert, key: &PubKey) -> Result<(), &'static str> {
    let ok = match (key, cert.sig_alg) {
        (PubKey::Rsa { n, e }, SigAlg::RsaPkcs1Sha256) => with_rsa(n, e, |k| {
            k.verify_pkcs1(PREFIX_SHA256, &sha256::hash(&cert.tbs), &cert.sig)
        }),
        (PubKey::Rsa { n, e }, SigAlg::RsaPkcs1Sha384) => with_rsa(n, e, |k| {
            k.verify_pkcs1(PREFIX_SHA384, &sha512::sha384(&cert.tbs), &cert.sig)
        }),
        (PubKey::Rsa { n, e }, SigAlg::RsaPkcs1Sha512) => with_rsa(n, e, |k| {
            k.verify_pkcs1(PREFIX_SHA512, &sha512::sha512(&cert.tbs), &cert.sig)
        }),
        (PubKey::Rsa { n, e }, SigAlg::RsaPssSha256) => with_rsa(n, e, |k| {
            k.verify_pss_sha256(&h32(&sha256::hash(&cert.tbs)), &cert.sig)
        }),
        (PubKey::EcP256 { x, y }, SigAlg::EcdsaSha256) => {
            let (r, s) = ecdsa_sig_parts(&cert.sig)?;
            p256::verify_ecdsa_p256_sha256(&sha256::hash(&cert.tbs), x, y, r, s)
        }
        (PubKey::EcP256 { x, y }, SigAlg::EcdsaSha384) => {
            let (r, s) = ecdsa_sig_parts(&cert.sig)?;
            p256::verify_ecdsa_p256_sha256(&h32(&sha512::sha384(&cert.tbs)), x, y, r, s)
        }
        (PubKey::EcP256 { x, y }, SigAlg::EcdsaSha512) => {
            let (r, s) = ecdsa_sig_parts(&cert.sig)?;
            p256::verify_ecdsa_p256_sha256(&h32(&sha512::sha512(&cert.tbs)), x, y, r, s)
        }
        (PubKey::EcP384 { x, y }, SigAlg::EcdsaSha256) => {
            let (r, s) = ecdsa_sig_parts(&cert.sig)?;
            p384::verify_ecdsa(&sha256::hash(&cert.tbs), x, y, r, s)
        }
        (PubKey::EcP384 { x, y }, SigAlg::EcdsaSha384) => {
            let (r, s) = ecdsa_sig_parts(&cert.sig)?;
            p384::verify_ecdsa(&sha512::sha384(&cert.tbs), x, y, r, s)
        }
        (PubKey::EcP384 { x, y }, SigAlg::EcdsaSha512) => {
            let (r, s) = ecdsa_sig_parts(&cert.sig)?;
            p384::verify_ecdsa(&sha512::sha512(&cert.tbs), x, y, r, s)
        }
        _ => return Err("unsupported signature algorithm"),
    };
    if ok {
        Ok(())
    } else {
        eprintln!("DBG cert sig fail: alg={:?} key={:?}", cert.sig_alg, std::mem::discriminant(key));
        Err("signature invalid")
    }
}

#[cfg(test)]
mod tests {
    use super::{ecdsa_sig_parts, h32, validate};

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn empty_chain() {
        assert_eq!(validate(&[], &[], "a", 0), Err("empty chain"));
    }

    #[test]
    fn ecdsa_der_parts() {
        let sig = [0x30u8, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02];
        let (r, s) = ecdsa_sig_parts(&sig).unwrap();
        assert_eq!(r, &[0x01]);
        assert_eq!(s, &[0x02]);
        assert!(ecdsa_sig_parts(&[0x02, 0x01, 0x01]).is_err());
    }

    #[test]
    fn h32_truncates() {
        let v = unhex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff");
        assert_eq!(h32(&v), unhex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")[..]);
        let longer = vec![7u8; 48];
        assert_eq!(h32(&longer), [7u8; 32]);
    }
}
