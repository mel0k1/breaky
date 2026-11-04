use crate::chacha;
use crate::gcm;
use crate::poly;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Suite {
    Aes128Gcm,
    ChaCha20Poly1305,
}

impl Suite {
    pub fn key_len(self) -> usize {
        match self {
            Suite::Aes128Gcm => 16,
            Suite::ChaCha20Poly1305 => 32,
        }
    }

    pub fn from_id(id: u16) -> Option<Suite> {
        match id {
            0x1301 => Some(Suite::Aes128Gcm),
            0x1303 => Some(Suite::ChaCha20Poly1305),
            _ => None,
        }
    }
}

fn mac_data(aad: &[u8], ct: &[u8]) -> Vec<u8> {
    let mut d = Vec::with_capacity(aad.len() + ct.len() + 32);
    d.extend_from_slice(aad);
    while d.len() % 16 != 0 {
        d.push(0);
    }
    d.extend_from_slice(ct);
    while d.len() % 16 != 0 {
        d.push(0);
    }
    d.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    d.extend_from_slice(&(ct.len() as u64).to_le_bytes());
    d
}

pub fn seal(
    suite: Suite,
    key: &[u8],
    nonce: &[u8; 12],
    aad: &[u8],
    pt: &[u8],
) -> (Vec<u8>, [u8; 16]) {
    match suite {
        Suite::Aes128Gcm => {
            let mut k = [0u8; 16];
            k.copy_from_slice(&key[..16]);
            gcm::Gcm::new(&k).seal(nonce, aad, pt)
        }
        Suite::ChaCha20Poly1305 => {
            let mut k = [0u8; 32];
            k.copy_from_slice(&key[..32]);
            let blk = chacha::block(&k, 0, nonce);
            let mut pk = [0u8; 32];
            pk.copy_from_slice(&blk[..32]);
            let mut ct = pt.to_vec();
            chacha::xor(&k, nonce, 1, &mut ct);
            let tag = poly::mac(&pk, &mac_data(aad, &ct));
            (ct, tag)
        }
    }
}

pub fn open(
    suite: Suite,
    key: &[u8],
    nonce: &[u8; 12],
    aad: &[u8],
    ct: &[u8],
    tag: &[u8; 16],
) -> Option<Vec<u8>> {
    match suite {
        Suite::Aes128Gcm => {
            let mut k = [0u8; 16];
            k.copy_from_slice(&key[..16]);
            gcm::Gcm::new(&k).open(nonce, aad, ct, tag)
        }
        Suite::ChaCha20Poly1305 => {
            let mut k = [0u8; 32];
            k.copy_from_slice(&key[..32]);
            let blk = chacha::block(&k, 0, nonce);
            let mut pk = [0u8; 32];
            pk.copy_from_slice(&blk[..32]);
            let expect = poly::mac(&pk, &mac_data(aad, ct));
            let mut diff = 0u8;
            for i in 0..16 {
                diff |= expect[i] ^ tag[i];
            }
            if diff != 0 {
                return None;
            }
            let mut pt = ct.to_vec();
            chacha::xor(&k, nonce, 1, &mut pt);
            Some(pt)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{open, seal, Suite};

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn suite_key_lens() {
        assert_eq!(Suite::Aes128Gcm.key_len(), 16);
        assert_eq!(Suite::ChaCha20Poly1305.key_len(), 32);
        assert_eq!(Suite::from_id(0x1301), Some(Suite::Aes128Gcm));
        assert_eq!(Suite::from_id(0x1303), Some(Suite::ChaCha20Poly1305));
        assert_eq!(Suite::from_id(0x1302), None);
    }

    #[test]
    fn rfc8439_2_8_2() {
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = 0x80 + i as u8;
        }
        let nonce = [
            0x07, 0x00, 0x00, 0x00, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
        ];
        let aad = unhex("50515253c0c1c2c3c4c5c6c7");
        let pt = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let (ct, tag) = seal(Suite::ChaCha20Poly1305, &key, &nonce, &aad, pt);
        assert_eq!(
            hex(&ct),
            "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d63dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b3692ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc3ff4def08e4b7a9de576d26586cec64b6116"
        );
        assert_eq!(hex(&tag), "1ae10b594f09e26a7e902ecbd0600691");
        assert_eq!(
            open(Suite::ChaCha20Poly1305, &key, &nonce, &aad, &ct, &tag).unwrap(),
            pt
        );
    }

    #[test]
    fn roundtrip_and_tamper_both_suites() {
        for suite in [Suite::Aes128Gcm, Suite::ChaCha20Poly1305] {
            let key = vec![9u8; suite.key_len()];
            let nonce = [3u8; 12];
            let aad = b"header";
            let pt = vec![0x55u8; 1000];
            let (ct, tag) = seal(suite, &key, &nonce, aad, &pt);
            assert_eq!(open(suite, &key, &nonce, aad, &ct, &tag).unwrap(), pt);
            let mut bad_tag = tag;
            bad_tag[0] ^= 1;
            assert!(open(suite, &key, &nonce, aad, &ct, &bad_tag).is_none());
            let mut bad_ct = ct.clone();
            bad_ct[500] ^= 1;
            assert!(open(suite, &key, &nonce, aad, &bad_ct, &tag).is_none());
            assert!(open(suite, &key, &nonce, b"other", &ct, &tag).is_none());
        }
    }
}
