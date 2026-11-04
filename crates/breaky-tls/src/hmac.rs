pub use crate::hash::hmac;
use crate::hash::Hash;

pub fn extract(hash: Hash, salt: &[u8], ikm: &[u8]) -> Vec<u8> {
    hmac(hash, salt, ikm)
}

pub fn expand(hash: Hash, prk: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let hl = hash.len();
    let mut out = Vec::with_capacity(len + hl);
    let mut t: Vec<u8> = Vec::new();
    while out.len() < len {
        let counter = (out.len() / hl + 1) as u8;
        let mut msg = Vec::with_capacity(t.len() + info.len() + 1);
        msg.extend_from_slice(&t);
        msg.extend_from_slice(info);
        msg.push(counter);
        t = hmac(hash, prk, &msg);
        out.extend_from_slice(&t);
    }
    out.truncate(len);
    out
}

pub fn expand_label(hash: Hash, secret: &[u8], label: &str, context: &[u8], len: usize) -> Vec<u8> {
    let mut hkdf_label = Vec::with_capacity(2 + 1 + 6 + label.len() + 1 + context.len());
    hkdf_label.extend_from_slice(&(len as u16).to_be_bytes());
    hkdf_label.push((6 + label.len()) as u8);
    hkdf_label.extend_from_slice(b"tls13 ");
    hkdf_label.extend_from_slice(label.as_bytes());
    hkdf_label.push(context.len() as u8);
    hkdf_label.extend_from_slice(context);
    expand(hash, secret, &hkdf_label, len)
}

pub fn derive_secret(hash: Hash, secret: &[u8], label: &str, transcript_hash: &[u8]) -> Vec<u8> {
    expand_label(hash, secret, label, transcript_hash, hash.len())
}

#[cfg(test)]
mod tests {
    use super::{derive_secret, expand, expand_label, extract};
    use crate::hash::Hash;

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn rfc5869_case1() {
        let ikm = [0x0b; 22];
        let salt: Vec<u8> = (0x00..=0x0c).collect();
        let info: Vec<u8> = (0xf0..=0xf9).collect();
        let prk = extract(Hash::Sha256, &salt, &ikm);
        assert_eq!(
            hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
        assert_eq!(
            hex(&expand(Hash::Sha256, &prk, &info, 42)),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    #[test]
    fn rfc5869_case3() {
        let ikm = [0x0b; 22];
        let prk = extract(Hash::Sha256, &[], &ikm);
        assert_eq!(
            hex(&prk),
            "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04"
        );
        assert_eq!(
            hex(&expand(Hash::Sha256, &prk, &[], 42)),
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"
        );
    }

    #[test]
    fn hkdf_sha384() {
        let ikm = [0x0b; 22];
        let salt: Vec<u8> = (0x00..=0x0c).collect();
        let info: Vec<u8> = (0xf0..=0xf9).collect();
        let prk = extract(Hash::Sha384, &salt, &ikm);
        assert_eq!(
            hex(&prk),
            "704b39990779ce1dc548052c7dc39f303570dd13fb39f7acc564680bef80e8dec70ee9a7e1f3e293ef68eceb072a5ade"
        );
        assert_eq!(
            hex(&expand(Hash::Sha384, &prk, &info, 42)),
            "9b5097a86038b805309076a44b3a9f38063e25b516dcbf369f394cfab43685f748b6457763e4f0204fc5"
        );
    }

    #[test]
    fn expand_label_shape() {
        let secret = [7u8; 32];
        let a = expand_label(Hash::Sha256, &secret, "key", &[], 32);
        let b = expand(
            Hash::Sha256,
            &secret,
            &{
                let mut v = 32u16.to_be_bytes().to_vec();
                v.push(9);
                v.extend_from_slice(b"tls13 key");
                v.push(0);
                v
            },
            32,
        );
        assert_eq!(a, b);
        assert_eq!(derive_secret(Hash::Sha256, &secret, "iv", &[0u8; 32]).len(), 32);
        assert_eq!(derive_secret(Hash::Sha384, &secret, "iv", &[0u8; 48]).len(), 48);
    }
}
