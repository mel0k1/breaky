use crate::aes::Block;

fn gmul(x: &[u8; 16], y: &[u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = *y;
    for i in 0..128 {
        if (x[i / 8] >> (7 - (i % 8))) & 1 == 1 {
            for j in 0..16 {
                z[j] ^= v[j];
            }
        }
        let lsb = v[15] & 1;
        for j in (1..16).rev() {
            v[j] = (v[j] >> 1) | (v[j - 1] << 7);
        }
        v[0] >>= 1;
        if lsb == 1 {
            v[0] ^= 0xe1;
        }
    }
    z
}

fn ghash(h: &[u8; 16], data: &[u8]) -> [u8; 16] {
    let mut y = [0u8; 16];
    for chunk in data.chunks(16) {
        for (j, b) in chunk.iter().enumerate() {
            y[j] ^= b;
        }
        y = gmul(&y, h);
    }
    y
}

fn inc32(block: &mut [u8; 16]) {
    let mut c = u32::from_be_bytes([block[12], block[13], block[14], block[15]]);
    c = c.wrapping_add(1);
    block[12..16].copy_from_slice(&c.to_be_bytes());
}

pub struct Gcm<C> {
    aes: C,
    h: [u8; 16],
}

impl<C: Block> Gcm<C> {
    pub fn with_cipher(aes: C) -> Gcm<C> {
        let mut h = [0u8; 16];
        aes.encrypt_block(&mut h);
        Gcm { aes, h }
    }

    fn j0(nonce: &[u8; 12]) -> [u8; 16] {
        let mut j0 = [0u8; 16];
        j0[..12].copy_from_slice(nonce);
        j0[15] = 1;
        j0
    }

    fn ctr_xor(&self, j0: &[u8; 16], data: &mut [u8]) {
        let mut ctr = *j0;
        for chunk in data.chunks_mut(16) {
            inc32(&mut ctr);
            let mut ks = ctr;
            self.aes.encrypt_block(&mut ks);
            for (b, k) in chunk.iter_mut().zip(ks.iter()) {
                *b ^= *k;
            }
        }
    }

    fn tag_for(&self, j0: &[u8; 16], aad: &[u8], ct: &[u8]) -> [u8; 16] {
        let mut d = Vec::with_capacity(aad.len() + ct.len() + 32);
        d.extend_from_slice(aad);
        while d.len() % 16 != 0 {
            d.push(0);
        }
        d.extend_from_slice(ct);
        while d.len() % 16 != 0 {
            d.push(0);
        }
        d.extend_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
        d.extend_from_slice(&((ct.len() as u64) * 8).to_be_bytes());
        let s = ghash(&self.h, &d);
        let mut tag = *j0;
        self.aes.encrypt_block(&mut tag);
        for i in 0..16 {
            tag[i] ^= s[i];
        }
        tag
    }

    pub fn seal(&self, nonce: &[u8; 12], aad: &[u8], pt: &[u8]) -> (Vec<u8>, [u8; 16]) {
        let j0 = Self::j0(nonce);
        let mut ct = pt.to_vec();
        self.ctr_xor(&j0, &mut ct);
        let tag = self.tag_for(&j0, aad, &ct);
        (ct, tag)
    }

    pub fn open(&self, nonce: &[u8; 12], aad: &[u8], ct: &[u8], tag: &[u8; 16]) -> Option<Vec<u8>> {
        let j0 = Self::j0(nonce);
        let expect = self.tag_for(&j0, aad, ct);
        let mut diff = 0u8;
        for i in 0..16 {
            diff |= expect[i] ^ tag[i];
        }
        if diff != 0 {
            return None;
        }
        let mut pt = ct.to_vec();
        self.ctr_xor(&j0, &mut pt);
        Some(pt)
    }
}

#[cfg(test)]
mod tests {
    use super::Gcm;
    use crate::aes::Aes128;

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    fn key() -> [u8; 16] {
        let k = unhex("feffe9928665731c6d6a8f9467308308");
        let mut out = [0u8; 16];
        out.copy_from_slice(&k);
        out
    }

    fn nonce() -> [u8; 12] {
        let n = unhex("cafebabefacedbaddecaf888");
        let mut out = [0u8; 12];
        out.copy_from_slice(&n);
        out
    }

    #[test]
    fn nist_tc1() {
        let g = Gcm::with_cipher(Aes128::new(&[0u8; 16]));
        let (ct, tag) = g.seal(&[0u8; 12], &[], &[0u8; 16]);
        assert_eq!(hex(&ct), "0388dace60b6a392f328c2b971b2fe78");
        assert_eq!(hex(&tag), "ab6e47d42cec13bdf53a67b21257bddf");
        assert_eq!(g.open(&[0u8; 12], &[], &ct, &tag).unwrap(), vec![0u8; 16]);
    }

    #[test]
    fn nist_tc3() {
        let g = Gcm::with_cipher(Aes128::new(&key()));
        let pt = unhex("d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72");
        let (ct, tag) = g.seal(&nonce(), &[], &pt);
        assert_eq!(
            hex(&ct),
            "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e"
        );
        assert_eq!(hex(&tag), "67383e3899332afdd4d83a204575a052");
        assert_eq!(g.open(&nonce(), &[], &ct, &tag).unwrap(), pt);
    }

    #[test]
    fn nist_tc4_with_aad() {
        let g = Gcm::with_cipher(Aes128::new(&key()));
        let aad = unhex("feedfacedeadbeeffeedfacedeadbeefabaddad2");
        let pt = unhex("d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255");
        let (ct, tag) = g.seal(&nonce(), &aad, &pt);
        assert_eq!(
            hex(&ct),
            "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091473f5985"
        );
        assert_eq!(hex(&tag), "da80ce830cfda02da2a218a1744f4c76");
        assert_eq!(g.open(&nonce(), &aad, &ct, &tag).unwrap(), pt);
    }

    #[test]
    fn odd_lengths_and_tamper() {
        let g = Gcm::with_cipher(Aes128::new(&key()));
        for n in [1usize, 5, 15, 17, 31, 33, 100] {
            let pt: Vec<u8> = (0..n as u8).collect();
            let (ct, tag) = g.seal(&nonce(), b"hdr", &pt);
            assert_eq!(g.open(&nonce(), b"hdr", &ct, &tag).unwrap(), pt, "n={n}");
        }
        let (ct, tag) = g.seal(&nonce(), b"hdr", b"payload");
        let mut bad = tag;
        bad[7] ^= 1;
        assert!(g.open(&nonce(), b"hdr", &ct, &bad).is_none());
        assert!(g.open(&nonce(), b"hdr2", &ct, &tag).is_none());
        let mut bad_ct = ct.clone();
        bad_ct[0] ^= 1;
        assert!(g.open(&nonce(), b"hdr", &bad_ct, &tag).is_none());
        let mut bad_nonce = nonce();
        bad_nonce[11] ^= 1;
        assert!(g.open(&bad_nonce, b"hdr", &ct, &tag).is_none());
    }
}
