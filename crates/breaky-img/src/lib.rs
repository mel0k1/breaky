use std::collections::HashMap;

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Image {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }
}

pub struct Store {
    map: HashMap<String, Image>,
}

impl Store {
    pub fn new() -> Store {
        Store {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, url: String, img: Image) {
        self.map.insert(url, img);
    }

    pub fn get(&self, url: &str) -> Option<&Image> {
        self.map.get(url)
    }
}

pub fn decode(bytes: &[u8]) -> Option<Image> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return decode_png(bytes);
    }
    None
}

const CHANNELS: [usize; 8] = [1, 0, 3, 1, 2, 0, 4, 0];

fn decode_png(bytes: &[u8]) -> Option<Image> {
    let mut i = 8usize;
    let mut head: Option<(u32, u32, u8, u8, u8)> = None;
    let mut plte: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let tag = [bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]];
        let start = i + 8;
        let end = start.checked_add(len)?;
        if end + 4 > bytes.len() {
            return None;
        }
        let data = &bytes[start..end];
        match &tag {
            b"IHDR" => {
                if data.len() < 13 {
                    return None;
                }
                let w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                head = Some((w, h, data[8], data[9], data[12]));
            }
            b"PLTE" => {
                plte = data.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            b"tRNS" => trns = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        i = end + 4;
    }
    let (w, h, depth, ctype, interlace) = head?;
    if w == 0 || h == 0 || interlace != 0 {
        return None;
    }
    let ch = *CHANNELS.get(ctype as usize)?;
    if ch == 0 {
        return None;
    }
    let depth_ok = match ctype {
        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
        2 | 4 | 6 => matches!(depth, 8 | 16),
        3 => matches!(depth, 1 | 2 | 4 | 8),
        _ => false,
    };
    if !depth_ok {
        return None;
    }
    let raw = inflate(&idat)?;
    let bits_pp = ch * depth as usize;
    let stride = (w as usize * bits_pp + 7) / 8;
    let bpp = (bits_pp + 7) / 8;
    let flat = unfilter(&raw, h, stride, bpp)?;
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        let row = y * stride;
        for x in 0..w as usize {
            let px = match ctype {
                0 => {
                    let v = sample(&flat, row, x, depth)?;
                    let g = scale(v, depth);
                    let a = if trns.len() >= 2 && v == trns[0] { 0 } else { 255 };
                    [g, g, g, a]
                }
                2 => {
                    let r = scale(sample(&flat, row, x * 3, depth)?, depth);
                    let g = scale(sample(&flat, row, x * 3 + 1, depth)?, depth);
                    let b = scale(sample(&flat, row, x * 3 + 2, depth)?, depth);
                    let a = if trns.len() >= 6
                        && r == trns[0] && g == trns[2] && b == trns[4]
                    {
                        0
                    } else {
                        255
                    };
                    [r, g, b, a]
                }
                3 => {
                    let idx = sample(&flat, row, x, depth)? as usize;
                    let p = plte.get(idx)?;
                    let a = trns.get(idx).copied().unwrap_or(255);
                    [p[0], p[1], p[2], a]
                }
                4 => {
                    let g = scale(sample(&flat, row, x * 2, depth)?, depth);
                    let a = scale(sample(&flat, row, x * 2 + 1, depth)?, depth);
                    [g, g, g, a]
                }
                _ => {
                    let r = scale(sample(&flat, row, x * 4, depth)?, depth);
                    let g = scale(sample(&flat, row, x * 4 + 1, depth)?, depth);
                    let b = scale(sample(&flat, row, x * 4 + 2, depth)?, depth);
                    let a = scale(sample(&flat, row, x * 4 + 3, depth)?, depth);
                    [r, g, b, a]
                }
            };
            data.extend_from_slice(&px);
        }
    }
    Some(Image {
        width: w,
        height: h,
        data,
    })
}

fn sample(flat: &[u8], row: usize, index: usize, depth: u8) -> Option<u8> {
    match depth {
        8 => flat.get(row + index).copied(),
        16 => flat.get(row + index * 2).copied(),
        d => {
            let per = 8 / d as usize;
            let byte = flat.get(row + index / per).copied()?;
            let shift = 8 - d as usize - (index % per) * d as usize;
            Some((byte >> shift) & ((1u16 << d) - 1) as u8)
        }
    }
}

fn scale(v: u8, depth: u8) -> u8 {
    match depth {
        1 => v * 255,
        2 => v * 85,
        4 => v * 17,
        _ => v,
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (a as i32, b as i32, c as i32);
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    (if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }) as u8
}

fn unfilter(raw: &[u8], h: u32, stride: usize, bpp: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; stride * h as usize];
    let mut pos = 0usize;
    for y in 0..h as usize {
        let ft = *raw.get(pos)?;
        pos += 1;
        if pos + stride > raw.len() {
            return None;
        }
        let line = &raw[pos..pos + stride];
        pos += stride;
        let row = y * stride;
        for x in 0..stride {
            let a = if x >= bpp { out[row + x - bpp] } else { 0 };
            let b = if y > 0 { out[row - stride + x] } else { 0 };
            let c = if y > 0 && x >= bpp {
                out[row - stride + x - bpp]
            } else {
                0
            };
            let v = line[x] as i32
                + match ft {
                    0 => 0,
                    1 => a as i32,
                    2 => b as i32,
                    3 => (a as i32 + b as i32) / 2,
                    4 => paeth(a, b, c) as i32,
                    _ => return None,
                };
            out[row + x] = v as u8;
        }
    }
    Some(out)
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn bit(&mut self) -> Option<u32> {
        let byte = *self.data.get(self.pos >> 3)?;
        let b = (byte >> (self.pos & 7)) & 1;
        self.pos += 1;
        Some(b as u32)
    }

    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= self.bit()? << i;
        }
        Some(v)
    }

    fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }
}

struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

fn huff_build(lengths: &[u8]) -> Option<Huff> {
    let mut count = [0u16; 16];
    for &l in lengths {
        let l = l as usize;
        if l > 15 {
            return None;
        }
        count[l] += 1;
    }
    let mut left = 1i32;
    for l in 1..16 {
        left <<= 1;
        left -= count[l] as i32;
        if left < 0 {
            return None;
        }
    }
    let mut offs = [0u16; 16];
    for l in 1..15 {
        offs[l + 1] = offs[l] + count[l];
    }
    let mut symbol = vec![0u16; lengths.len()];
    for (sym, &l) in lengths.iter().enumerate() {
        if l != 0 {
            symbol[offs[l as usize] as usize] = sym as u16;
            offs[l as usize] += 1;
        }
    }
    Some(Huff { count, symbol })
}

fn huff_decode(bits: &mut Bits, h: &Huff) -> Option<u16> {
    let mut code = 0i32;
    let mut first = 0i32;
    let mut index = 0i32;
    for len in 1..16 {
        code |= bits.bit()? as i32;
        let count = h.count[len] as i32;
        if code - first < count {
            return Some(h.symbol[(index + (code - first)) as usize]);
        }
        index += count;
        first = (first + count) << 1;
        code <<= 1;
    }
    None
}

fn fixed_tables() -> (Huff, Huff) {
    let mut lit = [0u8; 288];
    for (i, l) in lit.iter_mut().enumerate() {
        *l = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    let dist = [5u8; 30];
    (
        huff_build(&lit).unwrap_or(Huff {
            count: [0; 16],
            symbol: Vec::new(),
        }),
        huff_build(&dist).unwrap_or(Huff {
            count: [0; 16],
            symbol: Vec::new(),
        }),
    )
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn inflate_block(out: &mut Vec<u8>, bits: &mut Bits, lit: &Huff, dist: &Huff) -> Option<()> {
    loop {
        let sym = huff_decode(bits, lit)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Some(()),
            257..=285 => {
                let i = sym as usize - 257;
                let len = LEN_BASE[i] as usize + bits.bits(LEN_EXTRA[i] as u32)? as usize;
                let dsym = huff_decode(bits, dist)? as usize;
                if dsym >= 30 {
                    return None;
                }
                let d = DIST_BASE[dsym] as usize + bits.bits(DIST_EXTRA[dsym] as u32)? as usize;
                if d == 0 || d > out.len() {
                    return None;
                }
                let start = out.len() - d;
                for j in 0..len {
                    let b = out[start + j];
                    out.push(b);
                }
            }
            _ => return None,
        }
    }
}

fn dynamic_tables(bits: &mut Bits) -> Option<(Huff, Huff)> {
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let hlit = bits.bits(5)? as usize + 257;
    let hdist = bits.bits(5)? as usize + 1;
    let hclen = bits.bits(4)? as usize + 4;
    if hlit > 286 || hdist > 30 {
        return None;
    }
    let mut cl = [0u8; 19];
    for &slot in ORDER.iter().take(hclen) {
        cl[slot] = bits.bits(3)? as u8;
    }
    let clh = huff_build(&cl)?;
    let mut lengths = vec![0u8; hlit + hdist];
    let mut i = 0usize;
    while i < lengths.len() {
        let sym = huff_decode(bits, &clh)?;
        match sym {
            0..=15 => {
                lengths[i] = sym as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return None;
                }
                let prev = lengths[i - 1];
                let rep = 3 + bits.bits(2)? as usize;
                for _ in 0..rep {
                    if i >= lengths.len() {
                        break;
                    }
                    lengths[i] = prev;
                    i += 1;
                }
            }
            17 => i = (i + 3 + bits.bits(3)? as usize).min(lengths.len()),
            18 => i = (i + 11 + bits.bits(7)? as usize).min(lengths.len()),
            _ => return None,
        }
    }
    let lit = huff_build(&lengths[..hlit])?;
    let dist = huff_build(&lengths[hlit..])?;
    Some((lit, dist))
}

fn inflate(z: &[u8]) -> Option<Vec<u8>> {
    if z.len() < 2 || z[0] & 0x0F != 8 {
        return None;
    }
    if (z[0] as u16 * 256 + z[1] as u16) % 31 != 0 {
        return None;
    }
    let mut bits = Bits { data: z, pos: 16 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = bits.bit()?;
        match bits.bits(2)? {
            0 => {
                bits.align();
                let p = bits.pos / 8;
                if p + 4 > bits.data.len() {
                    return None;
                }
                let len = u16::from_le_bytes([bits.data[p], bits.data[p + 1]]) as usize;
                let nlen = u16::from_le_bytes([bits.data[p + 2], bits.data[p + 3]]);
                if len as u16 != !nlen || p + 4 + len > bits.data.len() {
                    return None;
                }
                out.extend_from_slice(&bits.data[p + 4..p + 4 + len]);
                bits.pos = (p + 4 + len) * 8;
            }
            1 => {
                let (l, d) = fixed_tables();
                inflate_block(&mut out, &mut bits, &l, &d)?;
            }
            2 => {
                let (l, d) = dynamic_tables(&mut bits)?;
                inflate_block(&mut out, &mut bits, &l, &d)?;
            }
            _ => return None,
        }
        if last == 1 {
            break;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(s: &str) -> Vec<u8> {
        const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut acc = 0u32;
        let mut n = 0usize;
        for c in s.bytes() {
            let v = match TBL.iter().position(|&t| t == c) {
                Some(v) => v as u32,
                None => continue,
            };
            acc = (acc << 6) | v;
            n += 1;
            if n == 4 {
                out.extend_from_slice(&[(acc >> 16) as u8, (acc >> 8) as u8, acc as u8]);
                acc = 0;
                n = 0;
            }
        }
        match n {
            2 => out.push((acc >> 4) as u8),
            3 => {
                out.push((acc >> 10) as u8);
                out.push((acc >> 2) as u8);
            }
            _ => {}
        }
        out
    }

    const RGB_STORED: &str = "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAIAAAASFvFNAAAAH0lEQVR4AQEUAOv/AP8AAAD/AAAA/wD//wD///8AAABMxwf57mft8wAAAABJRU5ErkJggg==";
    const RGB_FIXED: &str = "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAIAAAASFvFNAAAAE0lEQVR4AWP4z8DAAMH/QQjEBABMxwf5PRlDLgAAAABJRU5ErkJggg==";
    const RGB_DYNAMIC: &str = "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAIAAAASFvFNAAAAE0lEQVR4nGP4z8DAAMH/QQjEBABMxwf5Tno2XAAAAABJRU5ErkJggg==";
    const RGB_BIG: &str = "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";
    const PAL4: &str = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAACBAMAAACNhmBQAAAADFBMVEX/AAAA/wAAAP///wDWAo97AAAABHRSTlP/gABA+n86ewAAAA5JREFUeJxjYFRmYFQGAAC8AElaOxIaAAAAAElFTkSuQmCC";
    const GRAY1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAACAQAAAABN76BAAAAADElEQVR4nGPYxLAJAALMAWXBEXpYAAAAAElFTkSuQmCC";
    const GA8: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAQAAABeK7cBAAAADUlEQVR4nGP4/5+hAQAHfgJ/pSPAfwAAAABJRU5ErkJggg==";
    const RGBA16: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABEAYAAABPhRjKAAAAEUlEQVR4nGMQMll9luF/fSMADoQDvrFASgkAAAAASUVORK5CYII=";
    const FILTERS: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAAECAIAAAArjXluAAAAH0lEQVR4AWPkEpEDAqYoMGCeNnuBo6Mji6WBOjMzMwBTPQXqOFjHQwAAAABJRU5ErkJggg==";

    fn eq4(a: [u8; 4], b: [u8; 4]) -> bool {
        a == b
    }

    #[test]
    fn decode_stored() {
        let img = decode(&b64(RGB_STORED)).unwrap();
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
        assert!(eq4(img.pixel(0, 0), [255, 0, 0, 255]));
        assert!(eq4(img.pixel(1, 0), [0, 255, 0, 255]));
        assert!(eq4(img.pixel(2, 0), [0, 0, 255, 255]));
        assert!(eq4(img.pixel(0, 1), [255, 255, 0, 255]));
        assert!(eq4(img.pixel(1, 1), [255, 255, 255, 255]));
        assert!(eq4(img.pixel(2, 1), [0, 0, 0, 255]));
    }

    #[test]
    fn decode_fixed_huffman() {
        let img = decode(&b64(RGB_FIXED)).unwrap();
        assert!(eq4(img.pixel(0, 0), [255, 0, 0, 255]));
        assert!(eq4(img.pixel(2, 1), [0, 0, 0, 255]));
    }

    #[test]
    fn decode_dynamic_huffman() {
        let img = decode(&b64(RGB_DYNAMIC)).unwrap();
        assert!(eq4(img.pixel(0, 0), [255, 0, 0, 255]));
        assert!(eq4(img.pixel(1, 1), [255, 255, 255, 255]));
    }

    #[test]
    fn decode_dynamic_matches() {
        let img = decode(&b64(RGB_BIG)).unwrap();
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 10);
        let px = |x: u32, y: u32| -> [u8; 4] {
            let i = (y * 100 + x) as usize;
            [
                (i % 251) as u8,
                ((i * 7) % 256) as u8,
                ((i * 13) % 256) as u8,
                255,
            ]
        };
        for (x, y) in [(0u32, 0u32), (57u32, 3u32), (99u32, 9u32), (42u32, 7u32)] {
            assert!(eq4(img.pixel(x, y), px(x, y)), "at {},{}", x, y);
        }
    }

    #[test]
    fn decode_palette_trns() {
        let img = decode(&b64(PAL4)).unwrap();
        assert!(eq4(img.pixel(0, 0), [255, 0, 0, 255]));
        assert!(eq4(img.pixel(1, 0), [0, 255, 0, 128]));
        assert!(eq4(img.pixel(2, 0), [0, 0, 255, 0]));
        assert!(eq4(img.pixel(3, 0), [255, 255, 0, 64]));
        assert!(eq4(img.pixel(3, 1), [255, 255, 0, 64]));
    }

    #[test]
    fn decode_gray1() {
        let img = decode(&b64(GRAY1)).unwrap();
        assert!(eq4(img.pixel(0, 0), [255, 255, 255, 255]));
        assert!(eq4(img.pixel(1, 0), [0, 0, 0, 255]));
        assert!(eq4(img.pixel(7, 0), [0, 0, 0, 255]));
        assert!(eq4(img.pixel(6, 1), [255, 255, 255, 255]));
    }

    #[test]
    fn decode_gray_alpha() {
        let img = decode(&b64(GA8)).unwrap();
        assert!(eq4(img.pixel(0, 0), [255, 255, 255, 255]));
        assert!(eq4(img.pixel(1, 0), [0, 0, 0, 128]));
    }

    #[test]
    fn decode_rgba16() {
        let img = decode(&b64(RGBA16)).unwrap();
        assert!(eq4(img.pixel(0, 0), [0x12, 0xAB, 0x00, 0x7F]));
    }

    #[test]
    fn decode_all_filters() {
        let img = decode(&b64(FILTERS)).unwrap();
        assert!(eq4(img.pixel(0, 0), [10, 20, 30, 255]));
        assert!(eq4(img.pixel(1, 1), [130, 140, 150, 255]));
        assert!(eq4(img.pixel(0, 2), [200, 210, 220, 255]));
        assert!(eq4(img.pixel(1, 3), [4, 5, 6, 255]));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(decode(b"not a png at all").is_none());
        let full = b64(RGB_STORED);
        assert!(decode(&full[..20]).is_none());
        assert!(decode(&full[..60]).is_none());
        assert!(decode(&[]).is_none());
    }

    #[test]
    fn store_roundtrip() {
        let mut store = Store::new();
        assert!(store.get("a.png").is_none());
        store.insert(
            "a.png".into(),
            Image {
                width: 1,
                height: 1,
                data: vec![1, 2, 3, 4],
            },
        );
        let img = store.get("a.png").unwrap();
        assert!(eq4(img.pixel(0, 0), [1, 2, 3, 4]));
    }
}
