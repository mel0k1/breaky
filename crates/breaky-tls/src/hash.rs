use crate::sha256::Sha256;
use crate::sha512::Sha512;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hash {
    Sha256,
    Sha384,
}

impl Hash {
    pub fn len(self) -> usize {
        match self {
            Hash::Sha256 => 32,
            Hash::Sha384 => 48,
        }
    }

    pub fn one_shot(self, data: &[u8]) -> Vec<u8> {
        let mut d = self.stream();
        d.update(data);
        d.finish()
    }

    pub fn stream(self) -> Digester {
        match self {
            Hash::Sha256 => Digester::S256(Sha256::new()),
            Hash::Sha384 => Digester::S384(Sha512::new384()),
        }
    }
}

#[derive(Clone)]
pub enum Digester {
    S256(Sha256),
    S384(Sha512),
}

impl Digester {
    pub fn update(&mut self, data: &[u8]) {
        match self {
            Digester::S256(h) => h.update(data),
            Digester::S384(h) => h.update(data),
        }
    }

    pub fn finish(self) -> Vec<u8> {
        match self {
            Digester::S256(h) => h.finish().to_vec(),
            Digester::S384(h) => h.finish384().to_vec(),
        }
    }
}

pub fn hmac(hash: Hash, key: &[u8], msg: &[u8]) -> Vec<u8> {
    let bs = match hash {
        Hash::Sha256 => 64,
        Hash::Sha384 => 128,
    };
    let mut k = vec![0u8; bs];
    if key.len() > bs {
        k[..hash.len()].copy_from_slice(&hash.one_shot(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = vec![0x36u8; bs];
    let mut opad = vec![0x5cu8; bs];
    for i in 0..bs {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = hash.stream();
    inner.update(&ipad);
    inner.update(msg);
    let ih = inner.finish();
    let mut outer = hash.stream();
    outer.update(&opad);
    outer.update(&ih);
    outer.finish()
}

#[cfg(test)]
mod tests {
    use super::{hmac, Hash};

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn rfc4231_sha384() {
        let out = hex(&hmac(Hash::Sha384, &[0x0b; 20], b"Hi There"));
        assert_eq!(
            out,
            "afd03944d84895626b0825f4ab46907f15f9dadbe4101ec682aa034c7cebc59cfaea9ea9076ede7f4af152e8b2fa9cb6"
        );
        let out = hex(&hmac(Hash::Sha384, b"Jefe", b"what do ya want for nothing?"));
        assert_eq!(
            out,
            "af45d2e376484031617f78d2b58a6b1b9c7ef464f5a01b47e42ec3736322445e8e2240ca5e69e2c78b3239ecfab21649"
        );
        let out = hex(&hmac(
            Hash::Sha384,
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        ));
        assert_eq!(
            out,
            "4ece084485813e9088d2c63a041bc5b44f9ef1012a2b588f3cd11f05033ac4c60c2ef6ab4030fe8296248df163f44952"
        );
    }

    #[test]
    fn long_key_hashed() {
        let out = hmac(Hash::Sha256, &[0xaa; 200], b"x");
        assert_eq!(out.len(), 32);
        let out = hmac(Hash::Sha384, &[0xaa; 200], b"x");
        assert_eq!(out.len(), 48);
    }
}
