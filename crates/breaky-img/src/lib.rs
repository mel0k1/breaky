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
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return decode_jpeg(bytes);
    }
    None
}

const ZZ: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

struct JBits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> JBits<'a> {
    fn bit(&mut self) -> Option<u32> {
        let b = self.pos >> 3;
        let off = self.pos & 7;
        let byte = *self.d.get(b)?;
        if byte == 0xFF && *self.d.get(b + 1)? != 0x00 {
            return None;
        }
        let bit = (byte >> (7 - off)) & 1;
        self.pos += 1;
        if off == 7 && byte == 0xFF && self.d.get(b + 1) == Some(&0x00) {
            self.pos += 8;
        }
        Some(bit as u32)
    }

    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit()?;
        }
        Some(v)
    }

    fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }
}

fn jhuff(bits: &mut JBits, h: &Huff) -> Option<u16> {
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

fn extend(v: u32, s: u32) -> i32 {
    if s == 0 {
        return 0;
    }
    let v = v as i32;
    if v < (1 << (s - 1)) {
        v - (1 << s) + 1
    } else {
        v
    }
}

struct JComp {
    id: u8,
    h: u8,
    v: u8,
    tq: usize,
    td: usize,
    ta: usize,
    pred: i32,
    cw: usize,
    ch: usize,
    stride: usize,
    coef_w: usize,
    coef: Vec<i16>,
    plane: Vec<u8>,
}

struct Frame {
    w: u32,
    h: u32,
    prog: bool,
    comps: Vec<JComp>,
}

fn decode_jpeg(bytes: &[u8]) -> Option<Image> {
    let mut i = 2usize;
    let mut qt: Vec<Option<[u16; 64]>> = vec![None; 4];
    let mut ht: Vec<Option<Huff>> = (0..8).map(|_| None).collect();
    let mut frame: Option<Frame> = None;
    let mut dri = 0usize;
    let mut adobe_rgb = false;
    while i + 1 < bytes.len() {
        while i + 1 < bytes.len() && (bytes[i] != 0xFF || bytes[i + 1] == 0x00) {
            i += 1;
        }
        while i + 1 < bytes.len() && bytes[i + 1] == 0xFF {
            i += 1;
        }
        if i + 1 >= bytes.len() {
            return None;
        }
        let m = bytes[i + 1];
        i += 2;
        match m {
            0xD9 => break,
            0x01 | 0xD0..=0xD7 => {}
            _ => {
                if i + 2 > bytes.len() {
                    return None;
                }
                let len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
                if len < 2 || i + len > bytes.len() {
                    return None;
                }
                let seg = &bytes[i + 2..i + len];
                match m {
                    0xC0 | 0xC1 => frame = Some(sof(seg, false)?),
                    0xC2 => frame = Some(sof(seg, true)?),
                    0xC4 => dht(seg, &mut ht)?,
                    0xDB => dqt(seg, &mut qt)?,
                    0xDD => {
                        if seg.len() != 2 {
                            return None;
                        }
                        dri = u16::from_be_bytes([seg[0], seg[1]]) as usize;
                    }
                    0xEE => {
                        if seg.len() >= 12 && &seg[0..5] == b"Adobe" {
                            adobe_rgb = seg[11] == 0;
                        }
                    }
                    0xDA => {
                        let f = frame.as_mut()?;
                        if std::env::var("BKDBG").is_ok() {
                            eprintln!("SOS hdr={:02x?}", seg);
                        }
                        let next = sos(f, seg, &bytes[i + len..], &ht, dri);
                        if std::env::var("BKDBG").is_ok() {
                            eprintln!("SOS -> {:?}", next);
                        }
                        let next = next?;
                        i = i + len + next;
                        continue;
                    }
                    _ => {}
                }
                i += len;
            }
        }
    }
    let mut f = frame?;
    let mut cos = [0f32; 64];
    for u in 0..8 {
        for x in 0..8 {
            cos[u * 8 + x] = ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }
    finish(&mut f, &qt, &cos)?;
    render_frame(&f, adobe_rgb)
}

fn sof(seg: &[u8], prog: bool) -> Option<Frame> {
    if seg.len() < 6 || seg[0] != 8 {
        return None;
    }
    let h = u16::from_be_bytes([seg[1], seg[2]]) as u32;
    let w = u16::from_be_bytes([seg[3], seg[4]]) as u32;
    let n = seg[5] as usize;
    if w == 0 || h == 0 || seg.len() < 6 + n * 3 || n > 3 {
        return None;
    }
    let mut comps = Vec::new();
    for k in 0..n {
        let hv = seg[7 + k * 3];
        let (ch, cv) = (hv >> 4, hv & 15);
        if ch == 0 || cv == 0 || ch > 4 || cv > 4 {
            return None;
        }
        let tq = seg[8 + k * 3] as usize;
        if tq > 3 {
            return None;
        }
        comps.push(JComp {
            id: seg[6 + k * 3],
            h: ch,
            v: cv,
            tq,
            td: 0,
            ta: 0,
            pred: 0,
            cw: 0,
            ch: 0,
            stride: 0,
            coef_w: 0,
            coef: Vec::new(),
            plane: Vec::new(),
        });
    }
    let hmax = comps.iter().map(|c| c.h).max()?;
    let vmax = comps.iter().map(|c| c.v).max()?;
    let mcu_x = (w + 8 * hmax as u32 - 1) / (8 * hmax as u32);
    let mcu_y = (h + 8 * vmax as u32 - 1) / (8 * vmax as u32);
    for c in comps.iter_mut() {
        c.cw = ((w * c.h as u32 + hmax as u32 - 1) / hmax as u32) as usize;
        c.ch = ((h * c.v as u32 + vmax as u32 - 1) / vmax as u32) as usize;
        c.coef_w = mcu_x as usize * c.h as usize;
        let rows = mcu_y as usize * c.v as usize;
        c.stride = c.coef_w * 8;
        c.coef = vec![0i16; c.coef_w * rows * 64];
        c.plane = vec![0u8; c.stride * rows * 8];
    }
    Some(Frame { w, h, prog, comps })
}

fn dqt(seg: &[u8], qt: &mut Vec<Option<[u16; 64]>>) -> Option<()> {
    let mut i = 0usize;
    while i < seg.len() {
        let pq = seg[i] >> 4;
        let tq = (seg[i] & 15) as usize;
        i += 1;
        if tq > 3 {
            return None;
        }
        let mut t = [0u16; 64];
        for k in 0..64 {
            if pq == 1 {
                if i + 1 >= seg.len() {
                    return None;
                }
                t[k] = u16::from_be_bytes([seg[i], seg[i + 1]]);
                i += 2;
            } else {
                t[k] = *seg.get(i)? as u16;
                i += 1;
            }
        }
        qt[tq] = Some(t);
    }
    Some(())
}

fn dht(seg: &[u8], ht: &mut Vec<Option<Huff>>) -> Option<()> {
    let mut i = 0usize;
    while i < seg.len() {
        let tc = seg[i] >> 4;
        let th = (seg[i] & 15) as usize;
        i += 1;
        if tc > 1 || th > 3 {
            return None;
        }
        if i + 16 > seg.len() {
            return None;
        }
        let mut total = 0usize;
        for k in 0..16 {
            total += seg[i + k] as usize;
        }
        i += 16;
        if i + total > seg.len() {
            return None;
        }
        let mut lengths = vec![0u8; 256];
        let mut j = 0usize;
        for (li, &c) in seg[i - 16..i].iter().enumerate() {
            for _ in 0..c {
                if j >= total {
                    return None;
                }
                lengths[seg[i + j] as usize] = li as u8 + 1;
                j += 1;
            }
        }
        i += total;
        ht[th as usize + tc as usize * 4] = Some(huff_build(&lengths)?);
    }
    Some(())
}

fn sos(f: &mut Frame, hdr: &[u8], ent: &[u8], ht: &[Option<Huff>], dri: usize) -> Option<usize> {
    if hdr.is_empty() {
        return None;
    }
    let ns = hdr[0] as usize;
    if hdr.len() < 1 + ns * 2 + 3 {
        return None;
    }
    let mut order: Vec<usize> = Vec::new();
    for k in 0..ns {
        let cs = hdr[1 + k * 2];
        let tdta = hdr[2 + k * 2];
        let n = f.comps.iter().position(|c| c.id == cs)?;
        f.comps[n].td = (tdta >> 4) as usize;
        f.comps[n].ta = (tdta & 15) as usize;
        order.push(n);
    }
    let ss = hdr[1 + ns * 2] as usize;
    let se = hdr[2 + ns * 2] as usize;
    let ahal = hdr[3 + ns * 2];
    let (ah, al) = (ahal >> 4, ahal & 15);
    if se > 63 || ss > se || al > 15 {
        return None;
    }
    if f.prog && ss == 0 && se != 0 {
        return None;
    }
    for &n in &order {
        if f.comps[n].td > 3 || f.comps[n].ta > 3 {
            return None;
        }
        ht.get(f.comps[n].td)?.as_ref()?;
        if ss > 0 || !f.prog {
            ht.get(4 + f.comps[n].ta)?.as_ref()?;
        }
    }
    for c in f.comps.iter_mut() {
        c.pred = 0;
    }
    let mut bits = JBits { d: ent, pos: 0 };
    let mut eobrun = 0usize;
    if std::env::var("BKDBG").is_ok() {
        let _ = &mut bits;
        eprintln!("sos ent[0..8]={:02x?}", &ent[0..8.min(ent.len())]);
    }
    let mut todo = if dri > 0 { dri } else { usize::MAX };
    let interleave = !f.prog || ns > 1;
    let (total_x, total_y) = if interleave {
        let hmax = f.comps.iter().map(|c| c.h).max()?;
        let vmax = f.comps.iter().map(|c| c.v).max()?;
        (
            (f.w + 8 * hmax as u32 - 1) / (8 * hmax as u32),
            (f.h + 8 * vmax as u32 - 1) / (8 * vmax as u32),
        )
    } else {
        let c = &f.comps[order[0]];
        (((c.cw + 7) / 8) as u32, ((c.ch + 7) / 8) as u32)
    };
    for j in 0..total_y {
        for i in 0..total_x {
            if interleave {
                for &n in &order {
                    let (ch, cv) = (f.comps[n].h, f.comps[n].v);
                    for y in 0..cv {
                        for x in 0..ch {
                            let bx = i as usize * ch as usize + x as usize;
                            let by = j as usize * cv as usize + y as usize;
                            decode_unit(
                                &mut bits,
                                &mut f.comps[n],
                                bx,
                                by,
                                f.prog,
                                ss,
                                se,
                                ah,
                                al,
                                &ht,
                                &mut eobrun,
                            )?;
                        }
                    }
                }
            } else {
                let n = order[0];
                decode_unit(
                    &mut bits,
                    &mut f.comps[n],
                    i as usize,
                    j as usize,
                    f.prog,
                    ss,
                    se,
                    ah,
                    al,
                    &ht,
                    &mut eobrun,
                )?;
            }
            todo -= 1;
            if todo == 0 {
                bits.align();
                match find_marker(ent, bits.pos / 8) {
                    Some((p, m)) if (0xD0..=0xD7).contains(&m) => {
                        bits.pos = (p + 2) * 8;
                        for c in f.comps.iter_mut() {
                            c.pred = 0;
                        }
                        eobrun = 0;
                        todo = if dri > 0 { dri } else { usize::MAX };
                    }
                    Some((p, _)) => return Some(p),
                    None => {}
                }
            }
        }
    }
    Some(bits.pos / 8)
}

fn find_marker(d: &[u8], from: usize) -> Option<(usize, u8)> {
    let mut i = from;
    while i + 1 < d.len() {
        if d[i] == 0xFF {
            let m = d[i + 1];
            if m == 0xFF {
                i += 1;
                continue;
            }
            if m == 0x00 {
                i += 2;
                continue;
            }
            return Some((i, m));
        }
        i += 1;
    }
    None
}

fn decode_unit(
    bits: &mut JBits,
    comp: &mut JComp,
    bx: usize,
    by: usize,
    prog: bool,
    ss: usize,
    se: usize,
    ah: u8,
    al: u8,
    ht: &[Option<Huff>],
    eobrun: &mut usize,
) -> Option<()> {
    let start = (by * comp.coef_w + bx) * 64;
    let block = &mut comp.coef[start..start + 64];
    if !prog {
        let dc = ht.get(comp.td)?.as_ref()?;
        let ac = ht.get(4 + comp.ta)?.as_ref()?;
        let t = jhuff(bits, dc)? as u32;
        if t > 15 {
            return None;
        }
        let diff = if t > 0 { extend(bits.bits(t)?, t) } else { 0 };
        comp.pred += diff;
        block[0] = comp.pred as i16;
        let mut k = 1usize;
        while k <= 63 {
            let rs = jhuff(bits, ac)? as u32;
            let s = rs & 15;
            let r = (rs >> 4) as usize;
            if s == 0 {
                if r == 15 {
                    k += 16;
                    continue;
                }
                break;
            }
            k += r;
            if k > 63 {
                return None;
            }
            let v = extend(bits.bits(s)?, s);
            block[ZZ[k]] = v as i16;
            k += 1;
        }
        return Some(());
    }
    if ss == 0 {
        let dc = ht.get(comp.td)?.as_ref()?;
        if ah == 0 {
            let t = jhuff(bits, dc)? as u32;
            if t > 15 {
                return None;
            }
            let diff = if t > 0 { extend(bits.bits(t)?, t) } else { 0 };
            comp.pred += diff;
            block[0] = (comp.pred << al) as i16;
        } else if bits.bit()? == 1 {
            block[0] = ((block[0] as i32) + (1i32 << al)) as i16;
        }
        return Some(());
    }
    let ac = ht.get(4 + comp.ta)?.as_ref()?;
    let m = 1i32 << al;
    if ah == 0 {
        if *eobrun > 0 {
            *eobrun -= 1;
            return Some(());
        }
        let mut k = ss;
        loop {
            let rs = match jhuff(bits, ac) {
                Some(v) => v as u32,
                None => {
                    if std::env::var("BKDBG").is_ok() {
                        eprintln!("acf jhuff fail at k={} pos={}", k, bits.pos);
                    }
                    return None;
                }
            };
            let s = rs & 15;
            let r = (rs >> 4) as usize;
            if s == 0 {
                if r < 15 {
                    *eobrun = (1usize << r)
                        + if r > 0 {
                            bits.bits(r as u32)? as usize
                        } else {
                            0
                        };
                    *eobrun -= 1;
                    break;
                }
                k += 16;
            } else {
                k += r;
                if k > se {
                    if std::env::var("BKDBG").is_ok() {
                        eprintln!(
                            "acf overrun r={} s={} k={} se={} pos={} byte={:02x?}",
                            r,
                            s,
                            k,
                            se,
                            bits.pos,
                            &bits.d[bits.pos / 8..bits.pos / 8 + 8]
                        );
                    }
                    return None;
                }
                let v = extend(bits.bits(s)?, s) << al;
                block[ZZ[k]] = v as i16;
                k += 1;
            }
            if k > se {
                break;
            }
        }
        return Some(());
    }
    if *eobrun > 0 {
        *eobrun -= 1;
        for kk in ss..=se {
            let idx = ZZ[kk];
            let p = block[idx] as i32;
            if p != 0 && bits.bit()? == 1 && (p & m) == 0 {
                block[idx] = if p > 0 {
                    (p + m) as i16
                } else {
                    (p - m) as i16
                };
            }
        }
        return Some(());
    }
    let mut k = ss;
    loop {
        let rs = match jhuff(bits, ac) {
            Some(v) => v as u32,
            None => {
                if std::env::var("BKDBG").is_ok() {
                    eprintln!("acr jhuff fail k={} pos={}", k, bits.pos);
                }
                return None;
            }
        };
        let mut r = (rs >> 4) as usize;
        let s = rs & 15;
        let mut val = 0i32;
        if s == 0 {
            if r < 15 {
                *eobrun = (1usize << r) - 1
                    + if r > 0 {
                        bits.bits(r as u32)? as usize
                    } else {
                        0
                    };
                r = 64;
            } else {
                r = 16;
            }
        } else {
            if s != 1 {
                if std::env::var("BKDBG").is_ok() {
                    eprintln!("acr s={} rs={:x} k={} pos={}", s, rs, k, bits.pos);
                }
                return None;
            }
            val = if bits.bit()? == 1 { m } else { -m };
        }
        while k <= se {
            let idx = ZZ[k];
            k += 1;
            let p = block[idx] as i32;
            if p != 0 {
                if bits.bit()? == 1 && (p & m) == 0 {
                    block[idx] = if p > 0 {
                        (p + m) as i16
                    } else {
                        (p - m) as i16
                    };
                }
            } else {
                if r == 0 {
                    block[idx] = val as i16;
                    break;
                }
                r -= 1;
            }
        }
        if k > se {
            break;
        }
    }
    Some(())
}

const C2: [f32; 8] = [0.353_553_4, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5];

fn finish(f: &mut Frame, qt: &[Option<[u16; 64]>], cos: &[f32; 64]) -> Option<()> {
    for ci in 0..f.comps.len() {
        let q = *qt.get(f.comps[ci].tq)?.as_ref()?;
        let blocks_w = (f.comps[ci].cw + 7) / 8;
        let blocks_h = (f.comps[ci].ch + 7) / 8;
        for by in 0..blocks_h {
            for bx in 0..blocks_w {
                let comp = &mut f.comps[ci];
                let start = (by * comp.coef_w + bx) * 64;
                let block = &comp.coef[start..start + 64];
                let mut d = [0f32; 64];
                for k in 0..64 {
                    d[ZZ[k]] = block[ZZ[k]] as f32 * q[k] as f32;
                }
                let mut tmp = [0f32; 64];
                for py in 0..8 {
                    for ux in 0..8 {
                        let mut s = 0f32;
                        for vy in 0..8 {
                            s += C2[vy] * d[vy * 8 + ux] * cos[vy * 8 + py];
                        }
                        tmp[py * 8 + ux] = s;
                    }
                }
                for py in 0..8 {
                    for px in 0..8 {
                        let mut s = 0f32;
                        for ux in 0..8 {
                            s += C2[ux] * tmp[py * 8 + ux] * cos[ux * 8 + px];
                        }
                        let v = (s + 128.0).round().clamp(0.0, 255.0) as u8;
                        comp.plane[(by * 8 + py) * comp.stride + bx * 8 + px] = v;
                    }
                }
            }
        }
    }
    Some(())
}

fn bilinear(pl: &[u8], stride: usize, cw: usize, ch: usize, sx: f32, sy: f32) -> u8 {
    let sx = sx.clamp(0.0, cw as f32 - 1.0);
    let sy = sy.clamp(0.0, ch as f32 - 1.0);
    let x0 = sx.floor() as usize;
    let y0 = sy.floor() as usize;
    let x1 = (x0 + 1).min(cw - 1);
    let y1 = (y0 + 1).min(ch - 1);
    let fx = sx - x0 as f32;
    let fy = sy - y0 as f32;
    let a = pl[y0 * stride + x0] as f32;
    let b = pl[y0 * stride + x1] as f32;
    let c = pl[y1 * stride + x0] as f32;
    let e = pl[y1 * stride + x1] as f32;
    let v = a * (1.0 - fx) * (1.0 - fy) + b * fx * (1.0 - fy) + c * (1.0 - fx) * fy + e * fx * fy;
    v.round() as u8
}

fn render_frame(f: &Frame, adobe_rgb: bool) -> Option<Image> {
    let w = f.w as usize;
    let h = f.h as usize;
    let mut data = Vec::with_capacity(w * h * 4);
    if f.comps.len() == 1 {
        let c = &f.comps[0];
        for y in 0..h {
            for x in 0..w {
                let v = c.plane[y * c.stride + x];
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
    } else {
        let hmax = f.comps.iter().map(|c| c.h).max()?;
        let vmax = f.comps.iter().map(|c| c.v).max()?;
        for y in 0..h {
            for x in 0..w {
                let mut s = [0u8; 3];
                for (i, c) in f.comps.iter().enumerate() {
                    let sx = (x as f32 + 0.5) * (c.h as f32 / hmax as f32) - 0.5;
                    let sy = (y as f32 + 0.5) * (c.v as f32 / vmax as f32) - 0.5;
                    s[i] = bilinear(&c.plane, c.stride, c.cw, c.ch, sx, sy);
                }
                let (r, g, b) = if adobe_rgb {
                    (s[0], s[1], s[2])
                } else {
                    ycc(s[0], s[1], s[2])
                };
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    Some(Image {
        width: f.w,
        height: f.h,
        data,
    })
}

fn ycc(y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
    let y = y as f32;
    let cb = cb as f32 - 128.0;
    let cr = cr as f32 - 128.0;
    let r = y + 1.402 * cr;
    let g = y - 0.344136 * cb - 0.714136 * cr;
    let b = y + 1.772 * cb;
    (
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
    )
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
                    let a = if trns.len() >= 2 && v == trns[0] {
                        0
                    } else {
                        255
                    };
                    [g, g, g, a]
                }
                2 => {
                    let r = scale(sample(&flat, row, x * 3, depth)?, depth);
                    let g = scale(sample(&flat, row, x * 3 + 1, depth)?, depth);
                    let b = scale(sample(&flat, row, x * 3 + 2, depth)?, depth);
                    let a = if trns.len() >= 6 && r == trns[0] && g == trns[2] && b == trns[4] {
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
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
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

    struct Jw {
        out: Vec<u8>,
        scan: Vec<u8>,
        acc: u32,
        nb: u32,
    }

    impl Jw {
        fn new() -> Jw {
            Jw {
                out: vec![0xFF, 0xD8],
                scan: Vec::new(),
                acc: 0,
                nb: 0,
            }
        }

        fn marker(&mut self, m: u8) {
            self.flush_scan();
            self.out.push(0xFF);
            self.out.push(m);
        }

        fn seg(&mut self, m: u8, body: &[u8]) {
            self.flush_scan();
            self.out.push(0xFF);
            self.out.push(m);
            self.out.extend(((body.len() + 2) as u16).to_be_bytes());
            self.out.extend(body);
        }

        fn bits(&mut self, v: u32, n: u32) {
            for i in (0..n).rev() {
                self.acc = (self.acc << 1) | ((v >> i) & 1);
                self.nb += 1;
                if self.nb == 8 {
                    self.scan.push(self.acc as u8);
                    self.acc = 0;
                    self.nb = 0;
                }
            }
        }

        fn flush(&mut self) {
            if self.nb > 0 {
                let pad = 8 - self.nb;
                self.acc = (self.acc << pad) | ((1 << pad) - 1);
                self.scan.push(self.acc as u8);
                self.acc = 0;
                self.nb = 0;
            }
        }

        fn flush_scan(&mut self) {
            self.flush();
            for b in std::mem::take(&mut self.scan) {
                self.out.push(b);
                if b == 0xFF {
                    self.out.push(0x00);
                }
            }
        }
    }

    fn cat(v: i32) -> u32 {
        let a = v.unsigned_abs() as u64;
        let mut s = 0u32;
        while (1u64 << s) <= a {
            s += 1;
        }
        s
    }

    fn val_bits(v: i32, s: u32) -> u32 {
        if v < 0 {
            (v + (1i32 << s) - 1) as u32
        } else {
            v as u32
        }
    }

    const AC_SYMS: [u8; 7] = [0x00, 0x06, 0x07, 0x10, 0x11, 0x15, 0x17];

    fn ac_code(sym: u8) -> (u32, u32) {
        let i = AC_SYMS.iter().position(|&s| s == sym).unwrap();
        (i as u32, 3)
    }

    fn put_tables(j: &mut Jw) {
        let mut dqt = vec![0u8];
        dqt.extend([1u8; 64]);
        j.seg(0xDB, &dqt);
        let mut dht = vec![0x00u8];
        let mut counts = [0u8; 16];
        counts[3] = 16;
        dht.extend(counts);
        dht.extend(0..16u8);
        dht.push(0x10);
        let mut acounts = [0u8; 16];
        acounts[2] = AC_SYMS.len() as u8;
        dht.extend(acounts);
        dht.extend(AC_SYMS);
        j.seg(0xC4, &dht);
    }

    fn put_sof(j: &mut Jw, prog: bool, w: u16, h: u16, comps: &[(u8, u8)]) {
        let mut sof = vec![8u8];
        sof.extend(h.to_be_bytes());
        sof.extend(w.to_be_bytes());
        sof.push(comps.len() as u8);
        for (i, (ch, cv)) in comps.iter().enumerate() {
            sof.push(i as u8 + 1);
            sof.push(ch << 4 | cv);
            sof.push(0);
        }
        j.seg(if prog { 0xC2 } else { 0xC0 }, &sof);
    }

    fn put_sos(j: &mut Jw, comps: &[(u8, u8, u8)], ss: u8, se: u8, ah: u8, al: u8) {
        let mut sos = vec![comps.len() as u8];
        for (id, td, ta) in comps {
            sos.push(*id);
            sos.push(td << 4 | ta);
        }
        sos.extend([ss, se, ah << 4 | al]);
        j.seg(0xDA, &sos);
    }

    fn restart(j: &mut Jw, unit: usize, total: usize, dri: u16, rstn: &mut u8) -> bool {
        if dri > 0 && unit % dri as usize == 0 && unit < total {
            j.marker(0xD0 | *rstn);
            *rstn = (*rstn + 1) & 7;
            true
        } else {
            false
        }
    }

    fn scan_units(comps: &[(u8, u8)], ns: usize, w: u16, h: u16) -> Vec<(usize, usize, usize)> {
        let hmax = comps.iter().map(|c| c.0).max().unwrap() as usize;
        let vmax = comps.iter().map(|c| c.1).max().unwrap() as usize;
        let mcux = (w as usize + 8 * hmax - 1) / (8 * hmax);
        let mcuy = (h as usize + 8 * vmax - 1) / (8 * vmax);
        let mut out = Vec::new();
        if ns > 1 {
            for mj in 0..mcuy {
                for mi in 0..mcux {
                    for c in 0..comps.len() {
                        for y in 0..comps[c].1 as usize {
                            for x in 0..comps[c].0 as usize {
                                out.push((
                                    c,
                                    mi * comps[c].0 as usize + x,
                                    mj * comps[c].1 as usize + y,
                                ));
                            }
                        }
                    }
                }
            }
        } else {
            let cw = (w as usize * comps[0].0 as usize + hmax - 1) / hmax;
            let ch = (h as usize * comps[0].1 as usize + vmax - 1) / vmax;
            for by in 0..(ch + 7) / 8 {
                for bx in 0..(cw + 7) / 8 {
                    out.push((0, bx, by));
                }
            }
        }
        out
    }

    fn block_index(comps: &[(u8, u8)], c: usize, w: u16, bx: usize, by: usize) -> usize {
        let hmax = comps.iter().map(|x| x.0).max().unwrap() as usize;
        let mcux = (w as usize + 8 * hmax - 1) / (8 * hmax);
        by * mcux * comps[c].0 as usize + bx
    }

    fn baseline_file(
        w: u16,
        h: u16,
        comps: &[(u8, u8)],
        blocks: &[Vec<(i32, Vec<(usize, i32)>)>],
        dri: u16,
    ) -> Vec<u8> {
        let mut j = Jw::new();
        put_tables(&mut j);
        put_sof(&mut j, false, w, h, comps);
        if dri > 0 {
            j.seg(0xDD, &dri.to_be_bytes());
        }
        let sos: Vec<(u8, u8, u8)> = (0..comps.len()).map(|i| (i as u8 + 1, 0, 0)).collect();
        put_sos(&mut j, &sos, 0, 63, 0, 0);
        let units = scan_units(comps, comps.len(), w, h);
        let total = units.len();
        let mut prev = vec![0i32; comps.len()];
        let mut rstn = 0u8;
        for (u, &(c, bx, by)) in units.iter().enumerate() {
            let bi = block_index(comps, c, w, bx, by);
            let (dc, acs) = &blocks[c][bi];
            let diff = dc - prev[c];
            prev[c] = *dc;
            let s = cat(diff);
            j.bits(s, 4);
            if s > 0 {
                j.bits(val_bits(diff, s), s);
            }
            let mut last = 0usize;
            for &(k, v) in acs {
                let sz = cat(v);
                let sym = (((k - last - 1) as u8) << 4) | sz as u8;
                let (code, len) = ac_code(sym);
                j.bits(code, len);
                j.bits(val_bits(v, sz), sz);
                last = k;
            }
            let (code, len) = ac_code(0x00);
            j.bits(code, len);
            if restart(&mut j, u + 1, total, dri, &mut rstn) {
                for p in prev.iter_mut() {
                    *p = 0;
                }
            }
        }
        j.marker(0xD9);
        j.out
    }

    enum Ps {
        Dc {
            al: u8,
            vals: Vec<Vec<i32>>,
        },
        DcR {
            al: u8,
            vals: Vec<Vec<i32>>,
        },
        AcF {
            ss: u8,
            se: u8,
            al: u8,
            coefs: Vec<Vec<(usize, i32)>>,
        },
        AcR {
            ss: u8,
            se: u8,
            al: u8,
            coefs: Vec<Vec<(usize, i32)>>,
        },
    }

    fn prog_file(w: u16, h: u16, comps: &[(u8, u8)], dri: u16, scans: &[Ps]) -> Vec<u8> {
        let mut j = Jw::new();
        put_tables(&mut j);
        put_sof(&mut j, true, w, h, comps);
        if dri > 0 {
            j.seg(0xDD, &dri.to_be_bytes());
        }
        for sc in scans {
            match sc {
                Ps::Dc { al, vals } => {
                    let sos: Vec<(u8, u8, u8)> =
                        (0..comps.len()).map(|i| (i as u8 + 1, 0, 0)).collect();
                    put_sos(&mut j, &sos, 0, 0, 0, *al);
                    let units = scan_units(comps, comps.len(), w, h);
                    let total = units.len();
                    let mut prev = vec![0i32; comps.len()];
                    let mut rstn = 0u8;
                    for (u, &(c, bx, by)) in units.iter().enumerate() {
                        let bi = block_index(comps, c, w, bx, by);
                        let diff = (vals[c][bi] >> *al) - prev[c];
                        prev[c] = vals[c][bi] >> *al;
                        let s = cat(diff);
                        j.bits(s, 4);
                        if s > 0 {
                            j.bits(val_bits(diff, s), s);
                        }
                        restart(&mut j, u + 1, total, dri, &mut rstn);
                        if u + 1 < total && dri > 0 && (u + 1) % dri as usize == 0 {
                            for p in prev.iter_mut() {
                                *p = 0;
                            }
                        }
                    }
                }
                Ps::DcR { al, vals } => {
                    let sos: Vec<(u8, u8, u8)> =
                        (0..comps.len()).map(|i| (i as u8 + 1, 0, 0)).collect();
                    put_sos(&mut j, &sos, 0, 0, 1, *al);
                    let units = scan_units(comps, comps.len(), w, h);
                    let total = units.len();
                    let mut rstn = 0u8;
                    for (u, &(c, bx, by)) in units.iter().enumerate() {
                        let bi = block_index(comps, c, w, bx, by);
                        j.bits(((vals[c][bi] >> *al) & 1) as u32, 1);
                        restart(&mut j, u + 1, total, dri, &mut rstn);
                    }
                }
                Ps::AcF { ss, se, al, coefs } => {
                    put_sos(&mut j, &[(1, 0, 0)], *ss, *se, 0, *al);
                    let units = scan_units(comps, 1, w, h);
                    let total = units.len();
                    let mut rstn = 0u8;
                    for (u, &(_, bx, by)) in units.iter().enumerate() {
                        let bi = block_index(comps, 0, w, bx, by);
                        let mut list: Vec<(usize, i32)> = coefs[bi]
                            .iter()
                            .filter(|(k, _)| *k >= *ss as usize && *k <= *se as usize)
                            .copied()
                            .collect();
                        list.sort_by_key(|(k, _)| *k);
                        let mut last = *ss as usize;
                        for (k, v) in list {
                            let sent = if v < 0 { -((-v) >> *al) } else { v >> *al };
                            let sz = cat(sent);
                            let sym = (((k - last) as u8) << 4) | sz as u8;
                            let (code, len) = ac_code(sym);
                            j.bits(code, len);
                            j.bits(val_bits(sent, sz), sz);
                            last = k + 1;
                        }
                        let (code, len) = ac_code(0x00);
                        j.bits(code, len);
                        restart(&mut j, u + 1, total, dri, &mut rstn);
                    }
                }
                Ps::AcR { ss, se, al, coefs } => {
                    put_sos(&mut j, &[(1, 0, 0)], *ss, *se, 1, *al);
                    let units = scan_units(comps, 1, w, h);
                    let n = units.len();
                    let r = (n as f32).log2().floor() as u32;
                    let extra = (n - 1) - ((1usize << r) - 1);
                    let (code, len) = ac_code((r as u8) << 4);
                    j.bits(code, len);
                    j.bits(extra as u32, r);
                    for &(_, bx, by) in units.iter() {
                        let bi = block_index(comps, 0, w, bx, by);
                        let mut list: Vec<(usize, i32)> = coefs[bi]
                            .iter()
                            .filter(|(k, _)| *k >= *ss as usize && *k <= *se as usize)
                            .copied()
                            .collect();
                        list.sort_by_key(|(k, _)| *k);
                        for (_, v) in list {
                            j.bits(((v.abs() >> *al) & 1) as u32, 1);
                        }
                    }
                }
            }
        }
        j.marker(0xD9);
        j.out
    }

    #[test]
    fn jpeg_baseline_gray() {
        let bytes = baseline_file(8, 16, &[(1, 1)], &[vec![(416, vec![]), (-704, vec![])]], 0);
        let img = decode(&bytes).unwrap();
        assert_eq!(img.width, 8);
        assert_eq!(img.height, 16);
        assert!(eq4(img.pixel(0, 0), [180, 180, 180, 255]));
        assert!(eq4(img.pixel(7, 7), [180, 180, 180, 255]));
        assert!(eq4(img.pixel(0, 15), [40, 40, 40, 255]));
        assert!(eq4(img.pixel(7, 8), [40, 40, 40, 255]));
    }

    #[test]
    fn jpeg_baseline_color() {
        let y = 8 * 22;
        let cb = 8 * -18;
        let cr = 8 * 2;
        let bytes = baseline_file(
            8,
            8,
            &[(1, 1), (1, 1), (1, 1)],
            &[vec![(y, vec![])], vec![(cb, vec![])], vec![(cr, vec![])]],
            0,
        );
        let img = decode(&bytes).unwrap();
        let (r, g, b) = ycc(150, 110, 130);
        assert!(eq4(img.pixel(0, 0), [r, g, b, 255]));
        assert!(eq4(img.pixel(7, 7), [r, g, b, 255]));
    }

    #[test]
    fn jpeg_subsampled_420() {
        let y = 8 * 72;
        let cb = 8 * -18;
        let cr = 8 * 2;
        let bytes = baseline_file(
            16,
            16,
            &[(2, 2), (1, 1), (1, 1)],
            &[vec![(y, vec![]); 4], vec![(cb, vec![])], vec![(cr, vec![])]],
            0,
        );
        let img = decode(&bytes).unwrap();
        let (r, g, b) = ycc(200, 110, 130);
        assert!(eq4(img.pixel(0, 0), [r, g, b, 255]));
        assert!(eq4(img.pixel(8, 8), [r, g, b, 255]));
        assert!(eq4(img.pixel(15, 15), [r, g, b, 255]));
        assert!(eq4(img.pixel(4, 12), [r, g, b, 255]));
    }

    #[test]
    fn jpeg_restart_markers() {
        let bytes = baseline_file(8, 16, &[(1, 1)], &[vec![(416, vec![]), (-704, vec![])]], 1);
        let img = decode(&bytes).unwrap();
        assert!(eq4(img.pixel(0, 0), [180, 180, 180, 255]));
        assert!(eq4(img.pixel(0, 15), [40, 40, 40, 255]));
    }

    #[test]
    fn jpeg_progressive_flat() {
        let bytes = prog_file(
            8,
            8,
            &[(1, 1)],
            0,
            &[
                Ps::Dc {
                    al: 0,
                    vals: vec![vec![416]],
                },
                Ps::AcF {
                    ss: 1,
                    se: 63,
                    al: 0,
                    coefs: vec![vec![]],
                },
            ],
        );
        let img = decode(&bytes).unwrap();
        assert!(eq4(img.pixel(0, 0), [180, 180, 180, 255]));
        assert!(eq4(img.pixel(7, 7), [180, 180, 180, 255]));
    }

    #[test]
    fn jpeg_progressive_dc_approx() {
        let bytes = prog_file(
            8,
            8,
            &[(1, 1)],
            0,
            &[
                Ps::Dc {
                    al: 1,
                    vals: vec![vec![417]],
                },
                Ps::DcR {
                    al: 0,
                    vals: vec![vec![417]],
                },
            ],
        );
        let img = decode(&bytes).unwrap();
        assert!(eq4(img.pixel(0, 0), [180, 180, 180, 255]));
        let bytes = prog_file(
            8,
            8,
            &[(1, 1)],
            0,
            &[
                Ps::Dc {
                    al: 2,
                    vals: vec![vec![417]],
                },
                Ps::DcR {
                    al: 1,
                    vals: vec![vec![417]],
                },
                Ps::DcR {
                    al: 0,
                    vals: vec![vec![417]],
                },
            ],
        );
        let img = decode(&bytes).unwrap();
        assert!(eq4(img.pixel(0, 0), [180, 180, 180, 255]));
    }

    #[test]
    fn jpeg_progressive_ac_coef() {
        let bytes = prog_file(
            8,
            8,
            &[(1, 1)],
            0,
            &[
                Ps::Dc {
                    al: 0,
                    vals: vec![vec![0]],
                },
                Ps::AcF {
                    ss: 1,
                    se: 63,
                    al: 0,
                    coefs: vec![vec![(1, 64)]],
                },
            ],
        );
        let img = decode(&bytes).unwrap();
        let e =
            (128.0 + 64.0 * 0.353_553_4 * 0.5 * (std::f32::consts::PI / 16.0).cos()).round() as u8;
        assert!(eq4(img.pixel(0, 0), [e, e, e, 255]));
        assert!(eq4(img.pixel(0, 7), [e, e, e, 255]));
        let e7 =
            (128.0 - 64.0 * 0.353_553_4 * 0.5 * (std::f32::consts::PI / 16.0).cos()).round() as u8;
        assert!(eq4(img.pixel(7, 0), [e7, e7, e7, 255]));
        assert!(eq4(img.pixel(7, 7), [e7, e7, e7, 255]));
    }

    #[test]
    fn jpeg_progressive_ac_run_and_refine() {
        let bytes = prog_file(
            8,
            16,
            &[(1, 1)],
            0,
            &[
                Ps::Dc {
                    al: 0,
                    vals: vec![vec![0, 0]],
                },
                Ps::AcF {
                    ss: 1,
                    se: 5,
                    al: 1,
                    coefs: vec![vec![(2, -63)], vec![]],
                },
                Ps::AcF {
                    ss: 6,
                    se: 63,
                    al: 0,
                    coefs: vec![vec![], vec![]],
                },
                Ps::AcR {
                    ss: 1,
                    se: 5,
                    al: 0,
                    coefs: vec![vec![(2, -63)], vec![]],
                },
            ],
        );
        let img = decode(&bytes).unwrap();
        let e =
            (128.0 - 63.0 * 0.353_553_4 * 0.5 * (std::f32::consts::PI / 16.0).cos()).round() as u8;
        let e7 =
            (128.0 + 63.0 * 0.353_553_4 * 0.5 * (std::f32::consts::PI / 16.0).cos()).round() as u8;
        assert!(eq4(img.pixel(0, 0), [e, e, e, 255]));
        assert!(eq4(img.pixel(0, 7), [e7, e7, e7, 255]));
        assert!(eq4(img.pixel(0, 15), [128, 128, 128, 255]));
    }

    #[test]
    fn jpeg_progressive_restart() {
        let bytes = prog_file(
            8,
            16,
            &[(1, 1)],
            1,
            &[
                Ps::Dc {
                    al: 0,
                    vals: vec![vec![416, -704]],
                },
                Ps::AcF {
                    ss: 1,
                    se: 63,
                    al: 0,
                    coefs: vec![vec![], vec![]],
                },
            ],
        );
        let img = decode(&bytes).unwrap();
        assert!(eq4(img.pixel(0, 0), [180, 180, 180, 255]));
        assert!(eq4(img.pixel(0, 15), [40, 40, 40, 255]));
    }

    #[test]
    fn jpeg_rejects_bad_input() {
        assert!(decode(b"not a jpeg at all").is_none());
        let full = baseline_file(8, 8, &[(1, 1)], &[vec![(416, vec![])]], 0);
        assert!(decode(&full[..24]).is_none());
        assert!(decode(&full[..40]).is_none());
        assert!(decode(&[]).is_none());
        let mut cut = full.clone();
        let n = cut.len();
        cut.truncate(n - 3);
        assert!(decode(&cut).is_none());
    }

    #[test]
    fn jpeg_logo_decodes() {
        let bytes = include_bytes!("../../../docs/logo.jpg");
        let img = decode(bytes).expect("logo must decode");
        assert_eq!(img.width, 736);
        assert_eq!(img.height, 552);
        let a = img.pixel(0, 0);
        let b = img.pixel(368, 276);
        let c = img.pixel(735, 551);
        assert_eq!(a[3], 255);
        assert_eq!(c[3], 255);
        assert!(a != b || b != c);
    }
}
