fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

pub fn mac(key: &[u8; 32], msg: &[u8]) -> [u8; 16] {
    let r = [
        le32(&key[0..4]) & 0x3ffffff,
        (le32(&key[3..7]) >> 2) & 0x3ffff03,
        (le32(&key[6..10]) >> 4) & 0x3ffc0ff,
        (le32(&key[9..13]) >> 6) & 0x3f03fff,
        (le32(&key[12..16]) >> 8) & 0x00fffff,
    ];
    let s = [r[1] * 5, r[2] * 5, r[3] * 5, r[4] * 5];
    let pad = [
        le32(&key[16..20]),
        le32(&key[20..24]),
        le32(&key[24..28]),
        le32(&key[28..32]),
    ];
    let mut h = [0u32; 5];

    let mut i = 0;
    while i < msg.len() {
        let n = (msg.len() - i).min(16);
        let hibit = if n == 16 { 1u32 << 24 } else { 0 };
        let mut blk = [0u8; 17];
        blk[..n].copy_from_slice(&msg[i..i + n]);
        blk[n] = 1;
        h[0] += le32(&blk[0..4]) & 0x3ffffff;
        h[1] += (le32(&blk[3..7]) >> 2) & 0x3ffffff;
        h[2] += (le32(&blk[6..10]) >> 4) & 0x3ffffff;
        h[3] += (le32(&blk[9..13]) >> 6) & 0x3ffffff;
        h[4] += (le32(&blk[12..16]) >> 8) | hibit;

        let d0 = h[0] as u64 * r[0] as u64
            + h[1] as u64 * s[3] as u64
            + h[2] as u64 * s[2] as u64
            + h[3] as u64 * s[1] as u64
            + h[4] as u64 * s[0] as u64;
        let d1 = h[0] as u64 * r[1] as u64
            + h[1] as u64 * r[0] as u64
            + h[2] as u64 * s[3] as u64
            + h[3] as u64 * s[2] as u64
            + h[4] as u64 * s[1] as u64;
        let d2 = h[0] as u64 * r[2] as u64
            + h[1] as u64 * r[1] as u64
            + h[2] as u64 * r[0] as u64
            + h[3] as u64 * s[3] as u64
            + h[4] as u64 * s[2] as u64;
        let d3 = h[0] as u64 * r[3] as u64
            + h[1] as u64 * r[2] as u64
            + h[2] as u64 * r[1] as u64
            + h[3] as u64 * r[0] as u64
            + h[4] as u64 * s[3] as u64;
        let d4 = h[0] as u64 * r[4] as u64
            + h[1] as u64 * r[3] as u64
            + h[2] as u64 * r[2] as u64
            + h[3] as u64 * r[1] as u64
            + h[4] as u64 * r[0] as u64;

        let mut c = (d0 >> 26) as u32;
        h[0] = d0 as u32 & 0x3ffffff;
        let d1 = d1 + c as u64;
        c = (d1 >> 26) as u32;
        h[1] = d1 as u32 & 0x3ffffff;
        let d2 = d2 + c as u64;
        c = (d2 >> 26) as u32;
        h[2] = d2 as u32 & 0x3ffffff;
        let d3 = d3 + c as u64;
        c = (d3 >> 26) as u32;
        h[3] = d3 as u32 & 0x3ffffff;
        let d4 = d4 + c as u64;
        c = (d4 >> 26) as u32;
        h[4] = d4 as u32 & 0x3ffffff;
        h[0] += c * 5;
        c = h[0] >> 26;
        h[0] &= 0x3ffffff;
        h[1] += c;

        i += n;
    }

    let mut c = h[1] >> 26;
    h[1] &= 0x3ffffff;
    h[2] += c;
    c = h[2] >> 26;
    h[2] &= 0x3ffffff;
    h[3] += c;
    c = h[3] >> 26;
    h[3] &= 0x3ffffff;
    h[4] += c;
    c = h[4] >> 26;
    h[4] &= 0x3ffffff;
    h[0] += c * 5;
    c = h[0] >> 26;
    h[0] &= 0x3ffffff;
    h[1] += c;

    let g0 = h[0] + 5;
    let mut c = g0 >> 26;
    let g0 = g0 & 0x3ffffff;
    let g1 = h[1] + c;
    c = g1 >> 26;
    let g1 = g1 & 0x3ffffff;
    let g2 = h[2] + c;
    c = g2 >> 26;
    let g2 = g2 & 0x3ffffff;
    let g3 = h[3] + c;
    c = g3 >> 26;
    let g3 = g3 & 0x3ffffff;
    let g4 = h[4].wrapping_add(c).wrapping_sub(1 << 26);

    let mask = (g4 >> 31).wrapping_sub(1);
    let g = [
        (g0 & mask),
        (g1 & mask),
        (g2 & mask),
        (g3 & mask),
        (g4 & mask),
    ];
    let not = !mask;
    for j in 0..5 {
        h[j] = (h[j] & not) | g[j];
    }

    let w0 = ((h[0] as u64 | ((h[1] as u64) << 26)) & 0xffffffff) as u32;
    let w1 = (((h[1] as u64) >> 6 | ((h[2] as u64) << 20)) & 0xffffffff) as u32;
    let w2 = (((h[2] as u64) >> 12 | ((h[3] as u64) << 14)) & 0xffffffff) as u32;
    let w3 = (((h[3] as u64) >> 18 | ((h[4] as u64) << 8)) & 0xffffffff) as u32;

    let mut f = w0 as u64 + pad[0] as u64;
    let w0 = f as u32;
    f = w1 as u64 + pad[1] as u64 + (f >> 32);
    let w1 = f as u32;
    f = w2 as u64 + pad[2] as u64 + (f >> 32);
    let w2 = f as u32;
    f = w3 as u64 + pad[3] as u64 + (f >> 32);
    let w3 = f as u32;

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&w0.to_le_bytes());
    out[4..8].copy_from_slice(&w1.to_le_bytes());
    out[8..12].copy_from_slice(&w2.to_le_bytes());
    out[12..16].copy_from_slice(&w3.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::mac;

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn rfc8439_2_5_2() {
        let key = unhex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b");
        let mut k = [0u8; 32];
        k.copy_from_slice(&key);
        let tag = mac(&k, b"Cryptographic Forum Research Group");
        assert_eq!(hex(&tag), "a8061dc1305136c6c22b8baf0c0127a9");
    }

    #[test]
    fn empty_and_partial() {
        let key = [1u8; 32];
        let a = mac(&key, b"");
        let b = mac(&key, b"a");
        assert_ne!(a, b);
        let mut msg = vec![0x42u8; 15];
        let c = mac(&key, &msg);
        msg.push(0);
        let d = mac(&key, &msg);
        assert_ne!(c, d);
        let long = vec![0x11u8; 1000];
        let e = mac(&key, &long);
        let mut bad = long.clone();
        bad[999] ^= 1;
        assert_ne!(e, mac(&key, &bad));
    }
}
