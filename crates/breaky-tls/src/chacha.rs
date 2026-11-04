fn quarter(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] ^= s[a];
    s[d] = s[d].rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] ^= s[c];
    s[b] = s[b].rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] ^= s[a];
    s[d] = s[d].rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] ^= s[c];
    s[b] = s[b].rotate_left(7);
}

fn rounds(s: &mut [u32; 16]) {
    for _ in 0..10 {
        quarter(s, 0, 4, 8, 12);
        quarter(s, 1, 5, 9, 13);
        quarter(s, 2, 6, 10, 14);
        quarter(s, 3, 7, 11, 15);
        quarter(s, 0, 5, 10, 15);
        quarter(s, 1, 6, 11, 12);
        quarter(s, 2, 7, 8, 13);
        quarter(s, 3, 4, 9, 14);
    }
}

pub fn block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut st = [0u32; 16];
    st[0] = 0x61707865;
    st[1] = 0x3320646e;
    st[2] = 0x79622d32;
    st[3] = 0x6b206574;
    for i in 0..8 {
        let mut k = [0u8; 4];
        k.copy_from_slice(&key[i * 4..i * 4 + 4]);
        st[4 + i] = u32::from_le_bytes(k);
    }
    st[12] = counter;
    for i in 0..3 {
        let mut n = [0u8; 4];
        n.copy_from_slice(&nonce[i * 4..i * 4 + 4]);
        st[13 + i] = u32::from_le_bytes(n);
    }
    let mut w = st;
    rounds(&mut w);
    let mut out = [0u8; 64];
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&w[i].wrapping_add(st[i]).to_le_bytes());
    }
    out
}

pub fn xor(key: &[u8; 32], nonce: &[u8; 12], mut counter: u32, data: &mut [u8]) {
    for chunk in data.chunks_mut(64) {
        let ks = block(key, counter, nonce);
        counter = counter.wrapping_add(1);
        for (b, k) in chunk.iter_mut().zip(ks.iter()) {
            *b ^= k;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{block, xor};

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn rfc8439_2_3_2_block() {
        let key: Vec<u8> = (0x00..=0x1f).collect();
        let mut k = [0u8; 32];
        k.copy_from_slice(&key);
        let nonce = [
            0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x4a, 0x00, 0x00, 0x00, 0x00,
        ];
        let out = block(&k, 1, &nonce);
        assert_eq!(
            hex(&out),
            "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4ed2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e"
        );
    }

    #[test]
    fn rfc8439_2_4_2_xor() {
        let key: Vec<u8> = (0x00..=0x1f).collect();
        let mut k = [0u8; 32];
        k.copy_from_slice(&key);
        let nonce = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4a, 0x00, 0x00, 0x00, 0x00,
        ];
        let pt = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let mut data = pt.to_vec();
        xor(&k, &nonce, 1, &mut data);
        assert_eq!(
            hex(&data),
            "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0bf91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d807ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab77937365af90bbf74a35be6b40b8eedf2785e42874d"
        );
        xor(&k, &nonce, 1, &mut data);
        assert_eq!(data, pt);
    }
}
