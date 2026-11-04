use crate::sha256::Sha256;

pub fn mac(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&crate::sha256::hash(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(msg);
    let ih = inner.finish();
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&ih);
    outer.finish()
}

pub fn extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    mac(salt, ikm)
}

pub fn expand(prk: &[u8; 32], info: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 32);
    let mut t: Vec<u8> = Vec::new();
    while out.len() < len {
        let counter = (out.len() / 32 + 1) as u8;
        let mut msg = Vec::with_capacity(t.len() + info.len() + 1);
        msg.extend_from_slice(&t);
        msg.extend_from_slice(info);
        msg.push(counter);
        t = mac(prk, &msg).to_vec();
        out.extend_from_slice(&t);
    }
    out.truncate(len);
    out
}

pub fn expand_label(secret: &[u8; 32], label: &str, context: &[u8], len: usize) -> Vec<u8> {
    let mut hkdf_label = Vec::with_capacity(2 + 1 + 6 + label.len() + 1 + context.len());
    hkdf_label.extend_from_slice(&(len as u16).to_be_bytes());
    hkdf_label.push((6 + label.len()) as u8);
    hkdf_label.extend_from_slice(b"tls13 ");
    hkdf_label.extend_from_slice(label.as_bytes());
    hkdf_label.push(context.len() as u8);
    hkdf_label.extend_from_slice(context);
    expand(secret, &hkdf_label, len)
}

pub fn derive_secret(secret: &[u8; 32], label: &str, transcript_hash: &[u8; 32]) -> [u8; 32] {
    let v = expand_label(secret, label, transcript_hash, 32);
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    out
}

#[cfg(test)]
mod tests {
    use super::{derive_secret, expand, expand_label, extract, mac};

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn rfc4231_vectors() {
        let out = hex(&mac(&[0x0b; 20], b"Hi There"));
        assert_eq!(
            out,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        let out = hex(&mac(b"Jefe", b"what do ya want for nothing?"));
        assert_eq!(
            out,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        let out = hex(&mac(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        ));
        assert_eq!(
            out,
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn rfc5869_case1() {
        let ikm = [0x0b; 22];
        let salt: Vec<u8> = (0x00..=0x0c).collect();
        let info: Vec<u8> = (0xf0..=0xf9).collect();
        let prk = extract(&salt, &ikm);
        assert_eq!(
            hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
        assert_eq!(
            hex(&expand(&prk, &info, 42)),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    #[test]
    fn rfc5869_case3() {
        let ikm = [0x0b; 22];
        let prk = extract(&[], &ikm);
        assert_eq!(
            hex(&prk),
            "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04"
        );
        assert_eq!(
            hex(&expand(&prk, &[], 42)),
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"
        );
    }

    #[test]
    fn expand_label_shape() {
        let secret = [7u8; 32];
        let a = expand_label(&secret, "key", &[], 32);
        let b = expand(
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
        assert_eq!(derive_secret(&secret, "iv", &[0u8; 32]).len(), 32);
    }
}
