use crate::field::{Mont, Uint};
use crate::sha256;

const LIMBS: usize = 64;

pub struct RsaPub {
    n: Uint<LIMBS>,
    e: Uint<LIMBS>,
    bits: usize,
}

impl RsaPub {
    pub fn new(n_bytes: &[u8], e_bytes: &[u8]) -> Option<RsaPub> {
        if n_bytes.is_empty()
            || n_bytes.len() > LIMBS * 8
            || e_bytes.is_empty()
            || e_bytes.len() > 8
        {
            return None;
        }
        let n = Uint::<LIMBS>::from_be(n_bytes);
        if n.is_zero() {
            return None;
        }
        let mut e_limbs = [0u64; LIMBS];
        for (i, b) in e_bytes.iter().rev().enumerate() {
            e_limbs[0] |= (*b as u64) << (8 * i);
        }
        let e = Uint(e_limbs);
        let mut bits = 0;
        for i in (0..LIMBS).rev() {
            if n.0[i] != 0 {
                bits = i * 64 + (64 - n.0[i].leading_zeros() as usize);
                break;
            }
        }
        if bits == 0 {
            return None;
        }
        Some(RsaPub { n, e, bits })
    }

    fn em(&self, sig: &[u8]) -> Option<Vec<u8>> {
        if sig.is_empty() || sig.len() > LIMBS * 8 {
            return None;
        }
        let s = Uint::<LIMBS>::from_be(sig);
        if s.ge(&self.n) {
            return None;
        }
        let m = Mont::new(self.n);
        let r = m.pow(s, &self.e);
        let k = (self.bits + 7) / 8;
        let mut raw = vec![0u8; LIMBS * 8];
        r.to_be(&mut raw);
        Some(raw.split_off(raw.len() - k))
    }

    pub fn verify_pss_sha256(&self, msg_hash: &[u8; 32], sig: &[u8]) -> bool {
        let em = match self.em(sig) {
            Some(v) => v,
            None => return false,
        };
        let k = em.len();
        let hlen = 32;
        if k < hlen + 2 {
            return false;
        }
        if em[k - 1] != 0xbc {
            return false;
        }
        let dblen = k - hlen - 1;
        let mut db = vec![0u8; dblen];
        mgf1_sha256(&em[k - hlen - 1..k - 1], &mut db);
        for i in 0..dblen {
            db[i] ^= em[i];
        }
        let excess = 8 * k - (self.bits - 1);
        if excess == 0 || excess > 7 {
            return false;
        }
        if db[0] >> (8 - excess) != 0 {
            return false;
        }
        db[0] &= 0xff >> excess;
        let mut idx = 0;
        while idx < dblen && db[idx] == 0 {
            idx += 1;
        }
        if idx >= dblen || db[idx] != 1 {
            return false;
        }
        let salt = &db[idx + 1..];
        if salt.len() != hlen {
            return false;
        }
        let mut mprime = Vec::with_capacity(8 + hlen + hlen);
        mprime.extend_from_slice(&[0u8; 8]);
        mprime.extend_from_slice(msg_hash);
        mprime.extend_from_slice(salt);
        sha256::hash(&mprime) == em[k - hlen - 1..k - 1]
    }
}

pub fn mgf1_sha256(seed: &[u8], out: &mut [u8]) {
    let mut counter = 0u32;
    let mut done = 0;
    while done < out.len() {
        let mut h = sha256::Sha256::new();
        h.update(seed);
        h.update(&counter.to_be_bytes());
        let d = h.finish();
        let take = (out.len() - done).min(32);
        out[done..done + take].copy_from_slice(&d[..take]);
        done += take;
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::RsaPub;
    use crate::sha256;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    const N: &str = "b7767e40c023253fb09a3f9c3801a54a6d40e597ae602752577e987a5f9b7846a6356ae35c9da0d938ce62616dff8e636395222bc25638959abfce00adef4249c054f9af83f86f3e4e18817b193edbed3f9fd0596fa94274c92eaca6d5ce27c0842318e9fe8ae3502dbfb4ba23564c421a336e4b1a369abc8fb6fa77a66f1eb7e6c812eff06cc9a64bc19bd7350e4a4617b132a2197ca2ebf8afdbdbd9796187ded118e99992e4bbcb57f1c0e9be6761584b3df2203f4ed57c3753c637c1de000cc7ba2060f0f87d82aa9a1f68cae7a456ce05daf00a3eba4b7fd193ab0646574eacc4f0c97d71fa48ac0c1bc352ad3bfe5a97ffe97af28760ce9e1fe00d19c7";
    const SIG: &str = "736e1c45167425d8f3147c856cfa4c22acc4085a6ea4783c1b748c3696a56b6c1accc66de035755c4028e64ae676f32770bdf5d685c0305b0b9f068c452eafec046eba2e2eb853ad5a66a34d030a427b7c631be0c4d0fc348878664dceb6556b8c5ec4eec82c05c493b74b463cb41d5c2524cba392f4d634a8c4e9c39011e3a836693609661d231bb197bd1c00803498314c94c49de23a84b3e59abdffc0b3f90ee3d623112842b88e4a61a978c3bbb1203935bfa2438d17fcd5ba36cad691ef000a33387a066dadbd6c5dd2fc0f95742d5a809fe465176a471ca60ce7553481cda6090104181c8748f03be5d2f68ab5dad060c986c5ea5659b0b00dcadcf454";

    fn key() -> RsaPub {
        RsaPub::new(&unhex(N), &[0x01, 0x00, 0x01]).unwrap()
    }

    #[test]
    fn pss_verify() {
        let k = key();
        let mut h = [0u8; 32];
        h.copy_from_slice(&unhex(
            "c1f74872b6ea8ff19e9f0625dcb483cdc8111325d7e2b9dfaf4660aa51358ab2",
        ));
        assert!(k.verify_pss_sha256(&h, &unhex(SIG)));
    }

    #[test]
    fn pss_rejects_tampering() {
        let k = key();
        let mut h = [0u8; 32];
        h.copy_from_slice(&unhex(
            "c1f74872b6ea8ff19e9f0625dcb483cdc8111325d7e2b9dfaf4660aa51358ab2",
        ));
        let mut sig = unhex(SIG);
        sig[100] ^= 1;
        assert!(!k.verify_pss_sha256(&h, &sig));
        h[0] ^= 1;
        assert!(!k.verify_pss_sha256(&h, &unhex(SIG)));
        assert!(!k.verify_pss_sha256(&h, &[]));
        let other = sha256::hash(b"breaky tls 1.3");
        let _ = other;
    }

    #[test]
    fn rejects_bogus_keys() {
        assert!(RsaPub::new(&[], &[1]).is_none());
        assert!(RsaPub::new(&[0u8; 600], &[1]).is_none());
        assert!(RsaPub::new(&[0, 0, 0], &[1]).is_none());
        assert!(RsaPub::new(&unhex(N), &[0x01, 0x00, 0x01]).is_some());
    }
}
