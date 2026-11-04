use crate::field::{Mont, Uint};
use crate::sha256;

const LIMBS: usize = 64;

pub const PREFIX_SHA256: &[u8] = &[
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

pub const PREFIX_SHA384: &[u8] = &[
    0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02, 0x05,
    0x00, 0x04, 0x30,
];

pub const PREFIX_SHA512: &[u8] = &[
    0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03, 0x05,
    0x00, 0x04, 0x40,
];

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

    pub fn verify_pkcs1(&self, prefix: &[u8], digest: &[u8], sig: &[u8]) -> bool {
        let em = match self.em(sig) {
            Some(v) => v,
            None => return false,
        };
        let t = prefix.len() + digest.len();
        if em.len() < t + 11 {
            return false;
        }
        if em[0] != 0x00 || em[1] != 0x01 {
            return false;
        }
        let mut i = 2;
        while i < em.len() && em[i] == 0xff {
            i += 1;
        }
        if i - 2 < 8 || i >= em.len() || em[i] != 0x00 {
            return false;
        }
        let rest = &em[i + 1..];
        rest.len() == t
            && &rest[..prefix.len()] == prefix
            && &rest[prefix.len()..] == digest
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
    use super::{RsaPub, PREFIX_SHA256, PREFIX_SHA384, PREFIX_SHA512};
    use crate::sha256;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    const PSS_N: &str = "b7767e40c023253fb09a3f9c3801a54a6d40e597ae602752577e987a5f9b7846a6356ae35c9da0d938ce62616dff8e636395222bc25638959abfce00adef4249c054f9af83f86f3e4e18817b193edbed3f9fd0596fa94274c92eaca6d5ce27c0842318e9fe8ae3502dbfb4ba23564c421a336e4b1a369abc8fb6fa77a66f1eb7e6c812eff06cc9a64bc19bd7350e4a4617b132a2197ca2ebf8afdbdbd9796187ded118e99992e4bbcb57f1c0e9be6761584b3df2203f4ed57c3753c637c1de000cc7ba2060f0f87d82aa9a1f68cae7a456ce05daf00a3eba4b7fd193ab0646574eacc4f0c97d71fa48ac0c1bc352ad3bfe5a97ffe97af28760ce9e1fe00d19c7";
    const PSS_SIG: &str = "736e1c45167425d8f3147c856cfa4c22acc4085a6ea4783c1b748c3696a56b6c1accc66de035755c4028e64ae676f32770bdf5d685c0305b0b9f068c452eafec046eba2e2eb853ad5a66a34d030a427b7c631be0c4d0fc348878664dceb6556b8c5ec4eec82c05c493b74b463cb41d5c2524cba392f4d634a8c4e9c39011e3a836693609661d231bb197bd1c00803498314c94c49de23a84b3e59abdffc0b3f90ee3d623112842b88e4a61a978c3bbb1203935bfa2438d17fcd5ba36cad691ef000a33387a066dadbd6c5dd2fc0f95742d5a809fe465176a471ca60ce7553481cda6090104181c8748f03be5d2f68ab5dad060c986c5ea5659b0b00dcadcf454";
    const PKCS1_N: &str = "00bd245d788ce5241245e4a50e5db7fb1ed482f2a981df5f18fd7bbd1a7b0c9ad0535c2afde0c0148fff625cedd90b004eec0dfdabd9db98686e04d34128a1b179e923f6e57839e5f89cbafe32260f00b160f604b98bb8f930c4e39ef15164687725877d3113696f7d785ff44558a1ed71b4ca474ee3c9332c82de24809d878517accf4b451e58388d87f16bf14205910f47e7d3d672a07ac433f7964fff0d72bcc295519aaa77fa94122e7d6cedd1487cd43da1526b55a036fe5f417d79765c844e035a8c73c1e3fa5f2359e96910e7ce22195a915f73be42277a664ff2e061e010cac7959507a1fc05e399bb1aa83feaa21607e8cd311a940b958cc853fa548407d0e3af154fd6e35dbb319b05a2d5fb18eb7634fe7af73afc2c1736be49d1c487c9d8367ad29790f9ef67950d6c6ef9c0d7a0d729fe895b64702853623a6d2f9f7eb115f6820f4c2f763c14121b4b8b336c9ceb093851d421416a06fd83c43a0dd52f145f67211669218f094438153340cd5cc3283d0179b687e21f83592e55";
    const PKCS1_SIG: &str = "80ddff36f3e60efd3f6b03bef2b98eef0aa04025e3c20586dff32edd52c20c0c8528313a4c959cc1ab490c0687bf9a79b40a38355d05ec97810cc3c7c592084a045ee55e5d6242861d088f72da7754768d0638826e9f4b72e70345e5fbf0b1053f92cd430de8b49546e9f7976db1578fa96fa75227c3c261733187bba4cb0583fd1435d9f6d69cd5837810a7a57dafb2c3dc6cc3328653aca00a65e96aef7ce394301fe7cee7344ef33a4752f57bba2d1b4dbd95e2ec9034ffea1f230149145f4fbaecc757566b2f5d03ced143c0cc06a22d174207cb999d9519977f65838f6b34efe96f68084ffc100a32df8f9563302b9adbc076ce3ecdeae2670651dfff0b74c2fad0d9ba0a8189f92240f00d200a73dd357f664b6809cc6f97d24d57c52551de7db894ed6ba3f622b2bc37e8ac3940f86f9ff72708eaf5be468c7df897b997986af687ff82cddbd628b9d8ac055a4585cf5505eb94f6bef2cfc36680cbe2891c8dd0803ff6b0ac76a992ede7a6d45a7140ef73031079db431f52420581fd";
    const PKCS1_DIGEST: &str = "9ca9c4e976349cd04276e011585fff736b570f9c4cfa6d55b394272dc2259cf6";

    fn pss_key() -> RsaPub {
        RsaPub::new(&unhex(PSS_N), &[0x01, 0x00, 0x01]).unwrap()
    }

    fn pkcs1_key() -> RsaPub {
        RsaPub::new(&unhex(PKCS1_N), &[0x01, 0x00, 0x01]).unwrap()
    }

    #[test]
    fn pss_verify() {
        let k = pss_key();
        let mut h = [0u8; 32];
        h.copy_from_slice(&unhex(
            "c1f74872b6ea8ff19e9f0625dcb483cdc8111325d7e2b9dfaf4660aa51358ab2",
        ));
        assert!(k.verify_pss_sha256(&h, &unhex(PSS_SIG)));
    }

    #[test]
    fn pss_rejects_tampering() {
        let k = pss_key();
        let mut h = [0u8; 32];
        h.copy_from_slice(&unhex(
            "c1f74872b6ea8ff19e9f0625dcb483cdc8111325d7e2b9dfaf4660aa51358ab2",
        ));
        let mut sig = unhex(PSS_SIG);
        sig[100] ^= 1;
        assert!(!k.verify_pss_sha256(&h, &sig));
        h[0] ^= 1;
        assert!(!k.verify_pss_sha256(&h, &unhex(PSS_SIG)));
        assert!(!k.verify_pss_sha256(&h, &[]));
        let other = sha256::hash(b"breaky tls 1.3");
        let _ = other;
    }

    #[test]
    fn pkcs1_verify() {
        let k = pkcs1_key();
        let digest = unhex(PKCS1_DIGEST);
        assert!(k.verify_pkcs1(PREFIX_SHA256, &digest, &unhex(PKCS1_SIG)));
    }

    #[test]
    fn pkcs1_rejects_tampering() {
        let k = pkcs1_key();
        let digest = unhex(PKCS1_DIGEST);
        let mut sig = unhex(PKCS1_SIG);
        sig[17] ^= 1;
        assert!(!k.verify_pkcs1(PREFIX_SHA256, &digest, &sig));
        let mut d2 = digest.clone();
        d2[0] ^= 1;
        assert!(!k.verify_pkcs1(PREFIX_SHA256, &d2, &unhex(PKCS1_SIG)));
        assert!(!k.verify_pkcs1(PREFIX_SHA384, &digest, &unhex(PKCS1_SIG)));
        assert!(!k.verify_pkcs1(PREFIX_SHA512, &digest, &unhex(PKCS1_SIG)));
        assert!(!k.verify_pkcs1(PREFIX_SHA256, &digest, &[]));
        assert!(!k.verify_pkcs1(PREFIX_SHA256, &digest, &vec![1u8; 383]));
    }

    #[test]
    fn rejects_bogus_keys() {
        assert!(RsaPub::new(&[], &[1]).is_none());
        assert!(RsaPub::new(&[0u8; 600], &[1]).is_none());
        assert!(RsaPub::new(&[0, 0, 0], &[1]).is_none());
        assert!(RsaPub::new(&unhex(PSS_N), &[0x01, 0x00, 0x01]).is_some());
        assert!(RsaPub::new(&unhex(PKCS1_N), &[0x01, 0x00, 0x01]).is_some());
    }
}
