use std::collections::HashMap;
use std::sync::OnceLock;

pub struct Bitmap {
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub cover: Vec<u8>,
}

impl Bitmap {
    fn empty() -> Bitmap {
        Bitmap {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            cover: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct Contour {
    x: Vec<f32>,
    y: Vec<f32>,
    on: Vec<bool>,
}

#[derive(Clone)]
struct Outline {
    contours: Vec<Contour>,
}

struct Font {
    data: Vec<u8>,
    extra: Vec<Outline>,
    upem: f32,
    ascent: f32,
    descent: f32,
    xh: f32,
    advances: Vec<u16>,
    map: HashMap<u32, u16>,
    kern: HashMap<u32, i16>,
    glyf_off: usize,
    loca: Vec<usize>,
    nglyphs: usize,
}

fn rd_u8(b: &[u8], o: usize) -> Option<u8> {
    b.get(o).copied()
}

fn rd_i8(b: &[u8], o: usize) -> Option<i8> {
    b.get(o).map(|v| *v as i8)
}

fn rd_u16(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}

fn rd_i16(b: &[u8], o: usize) -> Option<i16> {
    rd_u16(b, o).map(|v| v as i16)
}

fn rd_u32(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn f2dot14(v: i16) -> f32 {
    v as f32 / 16384.0
}

fn table(data: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    let num = rd_u16(data, 4)? as usize;
    for i in 0..num {
        let e = 12 + i * 16;
        if data.get(e..e + 4)? == tag {
            let off = rd_u32(data, e + 8)? as usize;
            let len = rd_u32(data, e + 12)? as usize;
            return Some((off, len));
        }
    }
    None
}

impl Font {
    fn parse(data: &[u8]) -> Option<Font> {
        let version = rd_u32(data, 0)?;
        if version != 0x00010000 && version != 0x74727565 {
            return None;
        }
        let head = table(data, b"head")?;
        let upem = rd_u16(data, head.0 + 18)? as f32;
        if !(16.0..=16384.0).contains(&upem) {
            return None;
        }
        let itl = rd_i16(data, head.0 + 50)?;
        let hhea = table(data, b"hhea")?;
        let ascent = rd_i16(data, hhea.0 + 4)? as f32;
        let descent = rd_i16(data, hhea.0 + 6)?.abs() as f32;
        let nhm = rd_u16(data, hhea.0 + 34)? as usize;
        let maxp = table(data, b"maxp")?;
        let nglyphs = rd_u16(data, maxp.0 + 4)? as usize;
        if nglyphs == 0 {
            return None;
        }
        let hmtx = table(data, b"hmtx")?;
        let mut advances = Vec::with_capacity(nglyphs);
        for i in 0..nglyphs {
            let a = i.min(nhm.saturating_sub(1));
            advances.push(rd_u16(data, hmtx.0 + a * 4)?);
        }
        let mut map = HashMap::new();
        if let Some(cm) = table(data, b"cmap") {
            parse_cmap(data, cm.0, &mut map);
        }
        let glyf = table(data, b"glyf");
        let mut loca = vec![0usize; nglyphs + 1];
        if let (Some(lo), Some(gl)) = (table(data, b"loca"), glyf) {
            for i in 0..=nglyphs {
                let off = if itl == 0 {
                    rd_u16(data, lo.0 + i * 2)? as usize * 2
                } else {
                    rd_u32(data, lo.0 + i * 4)? as usize
                };
                loca[i] = gl.0 + off.min(gl.1);
            }
        }
        let mut kern = HashMap::new();
        if let Some(k) = table(data, b"kern") {
            parse_kern(data, k.0, &mut kern);
        }
        let f = Font {
            data: data.to_vec(),
            extra: Vec::new(),
            upem,
            ascent,
            descent,
            xh: 0.0,
            advances,
            map,
            kern,
            glyf_off: glyf.map(|g| g.0).unwrap_or(0),
            loca,
            nglyphs,
        };
        let xh = table(data, b"OS/2").and_then(|o| {
            let v = rd_i16(data, o.0)?;
            if v >= 2 {
                rd_i16(data, o.0 + 86).map(|x| x.max(0) as f32)
            } else {
                None
            }
        });
        let f = match xh {
            Some(x) if x > 0.0 => Font { xh: x, ..f },
            None => Font {
                xh: f.measure_xh().unwrap_or(upem * 0.5),
                ..f
            },
            Some(_) => Font { xh: upem * 0.5, ..f },
        };
        Some(f)
    }

    fn measure_xh(&self) -> Option<f32> {
        let o = self.outline(self.gid('x') as usize, 0)?;
        let mut m = f32::MIN;
        for c in &o.contours {
            for &y in &c.y {
                m = m.max(y);
            }
        }
        if m.is_finite() && m > 0.0 {
            Some(m)
        } else {
            None
        }
    }

    fn gid(&self, c: char) -> u16 {
        self.map.get(&(c as u32)).copied().unwrap_or(0)
    }

    fn kern_of(&self, a: char, b: char) -> i16 {
        let key = ((self.gid(a) as u32) << 16) | self.gid(b) as u32;
        self.kern.get(&key).copied().unwrap_or(0)
    }

    fn outline(&self, gid: usize, depth: usize) -> Option<Outline> {
        if depth > 5 || gid >= self.nglyphs {
            return None;
        }
        if self.data.is_empty() {
            return self.extra.get(gid).cloned();
        }
        let a = self.loca[gid];
        let b = self.loca[gid + 1];
        if b <= a {
            return None;
        }
        let g = self.data.get(a..b)?;
        let nc = rd_i16(g, 0)?;
        if nc >= 0 {
            simple_outline(g, nc as usize)
        } else {
            let mut o = Outline {
                contours: Vec::new(),
            };
            let mut p = 10usize;
            loop {
                let flags = rd_u16(g, p)?;
                let cgid = rd_u16(g, p + 2)? as usize;
                let mut q = p + 4;
                let (arg1, arg2) = if flags & 1 != 0 {
                    let r = (rd_i16(g, q)?, rd_i16(g, q + 2)?);
                    q += 4;
                    (r.0 as f32, r.1 as f32)
                } else {
                    let r = (rd_i8(g, q)?, rd_i8(g, q + 1)?);
                    q += 2;
                    (r.0 as f32, r.1 as f32)
                };
                let (dx, dy) = if flags & 2 != 0 {
                    (arg1, arg2)
                } else {
                    (0.0, 0.0)
                };
                let (ma, mb, mc, md) = if flags & 8 != 0 {
                    let sc = f2dot14(rd_i16(g, q)?);
                    q += 2;
                    (sc, 0.0, 0.0, sc)
                } else if flags & 0x40 != 0 {
                    let sx = f2dot14(rd_i16(g, q)?);
                    let sy = f2dot14(rd_i16(g, q + 2)?);
                    q += 4;
                    (sx, 0.0, 0.0, sy)
                } else if flags & 0x80 != 0 {
                    let a = f2dot14(rd_i16(g, q)?);
                    let b = f2dot14(rd_i16(g, q + 2)?);
                    let c = f2dot14(rd_i16(g, q + 4)?);
                    let d = f2dot14(rd_i16(g, q + 6)?);
                    q += 8;
                    (a, b, c, d)
                } else {
                    (1.0, 0.0, 0.0, 1.0)
                };
                if let Some(comp) = self.outline(cgid, depth + 1) {
                    for ct in comp.contours {
                        let mut c = Contour {
                            x: Vec::with_capacity(ct.x.len()),
                            y: Vec::with_capacity(ct.y.len()),
                            on: ct.on.clone(),
                        };
                        for i in 0..ct.x.len() {
                            c.x.push(ma * ct.x[i] + mc * ct.y[i] + dx);
                            c.y.push(mb * ct.x[i] + md * ct.y[i] + dy);
                        }
                        o.contours.push(c);
                    }
                }
                p = q;
                if flags & 0x20 == 0 {
                    break;
                }
            }
            Some(o)
        }
    }
}

fn simple_outline(g: &[u8], nc: usize) -> Option<Outline> {
    if nc == 0 {
        return Some(Outline {
            contours: Vec::new(),
        });
    }
    let mut ends = Vec::with_capacity(nc);
    for i in 0..nc {
        ends.push(rd_u16(g, 10 + i * 2)? as usize);
    }
    let npts = *ends.last()? + 1;
    let ilen = rd_u16(g, 10 + nc * 2)? as usize;
    let mut p = 12 + nc * 2 + ilen;
    let mut flags = Vec::with_capacity(npts);
    while flags.len() < npts {
        let f = rd_u8(g, p)?;
        p += 1;
        flags.push(f);
        if f & 8 != 0 {
            let r = rd_u8(g, p)?;
            p += 1;
            for _ in 0..r {
                if flags.len() < npts {
                    flags.push(f);
                }
            }
        }
    }
    let mut xs = Vec::with_capacity(npts);
    let mut x = 0i32;
    for &f in &flags {
        if f & 2 != 0 {
            let d = rd_u8(g, p)? as i32;
            p += 1;
            x += if f & 16 != 0 { d } else { -d };
        } else if f & 16 == 0 {
            x += rd_i16(g, p)? as i32;
            p += 2;
        }
        xs.push(x as f32);
    }
    let mut ys = Vec::with_capacity(npts);
    let mut y = 0i32;
    for &f in &flags {
        if f & 4 != 0 {
            let d = rd_u8(g, p)? as i32;
            p += 1;
            y += if f & 32 != 0 { d } else { -d };
        } else if f & 32 == 0 {
            y += rd_i16(g, p)? as i32;
            p += 2;
        }
        ys.push(y as f32);
    }
    let mut o = Outline {
        contours: Vec::new(),
    };
    let mut s = 0usize;
    for e in ends {
        let e = e + 1;
        if e > npts || s >= e {
            return None;
        }
        let mut c = Contour {
            x: Vec::new(),
            y: Vec::new(),
            on: Vec::new(),
        };
        for i in s..e {
            c.x.push(xs[i]);
            c.y.push(ys[i]);
            c.on.push(flags[i] & 1 != 0);
        }
        if c.x.len() >= 2 {
            o.contours.push(c);
        }
        s = e;
    }
    Some(o)
}

fn parse_cmap(data: &[u8], base: usize, map: &mut HashMap<u32, u16>) {
    let n = rd_u16(data, base + 2).unwrap_or(0) as usize;
    let mut best: Option<(u32, usize)> = None;
    for i in 0..n {
        let r = base + 4 + i * 8;
        let pid = rd_u16(data, r).unwrap_or(0);
        let eid = rd_u16(data, r + 2).unwrap_or(0);
        let off = base + rd_u32(data, r + 4).unwrap_or(0) as usize;
        let fmt = rd_u16(data, off).unwrap_or(0);
        let score = match fmt {
            12 => 4,
            4 => match (pid, eid) {
                (3, 0) => 1,
                _ => 3,
            },
            _ => 0,
        };
        if score > best.map(|b| b.0).unwrap_or(0) {
            best = Some((score, off));
        }
    }
    if let Some((_, off)) = best {
        match rd_u16(data, off).unwrap_or(0) {
            4 => cmap4(data, off, map),
            12 => cmap12(data, off, map),
            _ => {}
        }
    }
}

fn cmap4(data: &[u8], off: usize, map: &mut HashMap<u32, u16>) {
    let seg = rd_u16(data, off + 6).unwrap_or(0) as usize / 2;
    if seg == 0 {
        return;
    }
    let ends = off + 14;
    let starts = ends + seg * 2 + 2;
    let deltas = starts + seg * 2;
    let ranges = deltas + seg * 2;
    for i in 0..seg {
        let end = rd_u16(data, ends + i * 2).unwrap_or(0) as u32;
        let start = rd_u16(data, starts + i * 2).unwrap_or(0) as u32;
        let delta = rd_i16(data, deltas + i * 2).unwrap_or(0);
        let ro = rd_u16(data, ranges + i * 2).unwrap_or(0) as usize;
        for c in start..=end {
            let gid = if ro == 0 {
                ((c as i32 + delta as i32) & 0xFFFF) as u16
            } else {
                let addr = ranges + i * 2 + ro + (c as usize - start as usize) * 2;
                let g = rd_u16(data, addr).unwrap_or(0);
                if g == 0 {
                    0
                } else {
                    ((g as i32 + delta as i32) & 0xFFFF) as u16
                }
            };
            map.insert(c, gid);
        }
    }
}

fn cmap12(data: &[u8], off: usize, map: &mut HashMap<u32, u16>) {
    let n = rd_u32(data, off + 12).unwrap_or(0) as usize;
    for i in 0..n {
        let g = off + 16 + i * 12;
        let start = rd_u32(data, g).unwrap_or(0);
        let end = rd_u32(data, g + 4).unwrap_or(0);
        let gid = rd_u32(data, g + 8).unwrap_or(0);
        if end < start || end - start > 0x10FFFF {
            continue;
        }
        for c in start..=end {
            let id = gid + (c - start);
            if id <= 0xFFFF {
                map.insert(c, id as u16);
            }
        }
    }
}

fn parse_kern(data: &[u8], base: usize, kern: &mut HashMap<u32, i16>) {
    if rd_u16(data, base).unwrap_or(1) != 0 {
        return;
    }
    let n = rd_u16(data, base + 2).unwrap_or(0) as usize;
    let mut p = base + 4;
    for _ in 0..n {
        let ver = rd_u16(data, p).unwrap_or(0);
        let len = rd_u16(data, p + 2).unwrap_or(0) as usize;
        let cov = rd_u16(data, p + 4).unwrap_or(0);
        if len == 0 {
            break;
        }
        if ver == 0 && cov & 1 != 0 && cov & 4 == 0 && cov >> 8 == 0 {
            let np = rd_u16(data, p + 6).unwrap_or(0) as usize;
            for i in 0..np {
                let q = p + 14 + i * 6;
                let l = rd_u16(data, q).unwrap_or(0) as u32;
                let r = rd_u16(data, q + 2).unwrap_or(0) as u32;
                let v = rd_i16(data, q + 4).unwrap_or(0);
                kern.insert((l << 16) | r, v);
            }
        }
        p += len;
    }
}

static FONT: OnceLock<Font> = OnceLock::new();

pub fn set(data: &[u8]) -> bool {
    match Font::parse(data) {
        Some(f) => FONT.set(f).is_ok(),
        None => false,
    }
}

fn font() -> &'static Font {
    FONT.get_or_init(builtin)
}

fn kernable(c: char) -> bool {
    !matches!(c, '\t' | '\n' | '\r')
}

pub fn scale(size: f32) -> f32 {
    size / font().upem
}

pub fn asc(size: f32) -> f32 {
    font().ascent * scale(size)
}

pub fn desc(size: f32) -> f32 {
    font().descent * scale(size)
}

pub fn x_height(size: f32) -> f32 {
    font().xh * scale(size)
}

pub fn advance(c: char, size: f32) -> f32 {
    match c {
        '\n' | '\r' => 0.0,
        '\t' => 4.0 * advance(' ', size),
        _ => {
            let f = font();
            f.advances[f.gid(c) as usize] as f32 * scale(size)
        }
    }
}

pub fn width(text: &str, size: f32) -> f32 {
    let f = font();
    let sc = scale(size);
    let mut w = 0.0;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        if let Some(p) = prev {
            if kernable(p) && kernable(c) {
                w += f.kern_of(p, c) as f32 * sc;
            }
        }
        w += advance(c, size);
        prev = Some(c);
    }
    w
}

pub fn offsets(text: &str, size: f32) -> Vec<f32> {
    let chars: Vec<char> = text.chars().collect();
    let mut v = Vec::with_capacity(chars.len());
    let mut pen = 0.0;
    for (i, &c) in chars.iter().enumerate() {
        v.push(pen);
        pen += advance(c, size);
        if let Some(&n) = chars.get(i + 1) {
            if kernable(c) && kernable(n) {
                pen += font().kern_of(c, n) as f32 * scale(size);
            }
        }
    }
    v
}

pub fn rasterize(c: char, size: f32) -> Bitmap {
    raster(font(), c, size)
}

fn raster(f: &Font, c: char, size: f32) -> Bitmap {
    let s = size / f.upem;
    if !(s > 0.0) || !s.is_finite() {
        return Bitmap::empty();
    }
    let o = match f.outline(f.gid(c) as usize, 0) {
        Some(o) => o,
        None => return Bitmap::empty(),
    };
    render(&o, s)
}

fn contour_pts(ct: &Contour, s: f32) -> Vec<(f32, f32)> {
    let n = ct.x.len();
    let mut out = Vec::new();
    if n < 2 {
        return out;
    }
    match ct.on.iter().position(|&b| b) {
        Some(si) => {
            out.push((ct.x[si] * s, -ct.y[si] * s));
            let (mut px, mut py) = (ct.x[si], ct.y[si]);
            let mut ctrl: Option<(f32, f32)> = None;
            for k in 1..=n {
                let i = (si + k) % n;
                let (cx, cy, con) = (ct.x[i], ct.y[i], ct.on[i]);
                if con {
                    match ctrl {
                        Some((qx, qy)) => quad(&mut out, px, py, qx, qy, cx, cy, s),
                        None => out.push((cx * s, -cy * s)),
                    }
                    px = cx;
                    py = cy;
                    ctrl = None;
                } else {
                    match ctrl {
                        Some((qx, qy)) => {
                            let mx = (qx + cx) * 0.5;
                            let my = (qy + cy) * 0.5;
                            quad(&mut out, px, py, qx, qy, mx, my, s);
                            px = mx;
                            py = my;
                            ctrl = Some((cx, cy));
                        }
                        None => ctrl = Some((cx, cy)),
                    }
                }
            }
        }
        None => {
            let mid = |i: usize| {
                (
                    (ct.x[i] + ct.x[(i + 1) % n]) * 0.5,
                    (ct.y[i] + ct.y[(i + 1) % n]) * 0.5,
                )
            };
            let (mut px, mut py) = mid(0);
            out.push((px * s, -py * s));
            for i in 1..n {
                let (mx, my) = mid(i);
                quad(&mut out, px, py, ct.x[i], ct.y[i], mx, my, s);
                px = mx;
                py = my;
            }
            quad(&mut out, px, py, ct.x[0], ct.y[0], mid(0).0, mid(0).1, s);
        }
    }
    out
}

fn quad(out: &mut Vec<(f32, f32)>, x0: f32, y0: f32, cx: f32, cy: f32, x1: f32, y1: f32, s: f32) {
    let dev = ((cx - (x0 + x1) * 0.5).abs() + (cy - (y0 + y1) * 0.5).abs()) * s;
    let n = ((dev / 0.25).sqrt().ceil() as usize).clamp(1, 16);
    for k in 1..n {
        let t = k as f32 / n as f32;
        let u = 1.0 - t;
        out.push((
            (u * u * x0 + 2.0 * u * t * cx + t * t * x1) * s,
            -(u * u * y0 + 2.0 * u * t * cy + t * t * y1) * s,
        ));
    }
    out.push((x1 * s, -y1 * s));
}

fn render(o: &Outline, s: f32) -> Bitmap {
    let mut pts_all = Vec::new();
    for ct in &o.contours {
        pts_all.push(contour_pts(ct, s));
    }
    let mut mnx = f32::MAX;
    let mut mny = f32::MAX;
    let mut mxx = f32::MIN;
    let mut mxy = f32::MIN;
    for pts in &pts_all {
        for &(x, y) in pts {
            mnx = mnx.min(x);
            mny = mny.min(y);
            mxx = mxx.max(x);
            mxy = mxy.max(y);
        }
    }
    if pts_all.is_empty() || mnx > mxx {
        return Bitmap::empty();
    }
    let left = mnx.floor() as i32 - 1;
    let top = mny.floor() as i32 - 1;
    let w = (mxx.ceil() as i32 + 1 - left).max(1) as usize;
    let h = (mxy.ceil() as i32 + 1 - top).max(1) as usize;
    let mut acc = vec![0.0f32; w * h];
    let mut pieces: Vec<(usize, f32)> = Vec::new();
    for pts in &pts_all {
        let n = pts.len();
        for i in 0..n {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            scan_edge(&mut acc, &mut pieces, w, h, left as f32, top as f32, a, b);
        }
    }
    for row in 0..h {
        let mut t = 0.0f32;
        for v in &mut acc[row * w..(row + 1) * w] {
            t += *v;
            *v = t;
        }
    }
    for (i, v) in pieces {
        acc[i] += v;
    }
    let cover = acc
        .iter()
        .map(|v| (v.abs().min(1.0) * 255.0).round() as u8)
        .collect();
    Bitmap {
        left,
        top,
        width: w,
        height: h,
        cover,
    }
}

fn scan_edge(
    acc: &mut [f32],
    pieces: &mut Vec<(usize, f32)>,
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    a: (f32, f32),
    b: (f32, f32),
) {
    let (mut x0, mut y0, mut x1, mut y1) = (a.0 - ox, a.1 - oy, b.0 - ox, b.1 - oy);
    if y0 == y1 {
        return;
    }
    let mut s = 1.0f32;
    if y1 < y0 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
        s = -1.0;
    }
    let dxdy = (x1 - x0) / (y1 - y0);
    let ya = y0.max(0.0);
    let yb = y1.min(h as f32);
    if ya >= yb {
        return;
    }
    for j in ya.floor() as usize..(yb.ceil() as usize).min(h) {
        let yt = ya.max(j as f32);
        let yb2 = yb.min(j as f32 + 1.0);
        let hh = yb2 - yt;
        if hh <= 0.0 {
            continue;
        }
        let xt = x0 + (yt - y0) * dxdy;
        let xb = x0 + (yb2 - y0) * dxdy;
        let row = j * w;
        acc[row] += s * hh;
        let ilo = xt.min(xb).floor() as i64;
        if ilo < w as i64 {
            acc[row + ilo.clamp(0, w as i64 - 1) as usize] -= s * hh;
        }
        if dxdy == 0.0 {
            let p = xt.floor() as i64;
            if p >= 0 && p < w as i64 {
                pieces.push((row + p as usize, s * (hh - (p as f32 + 1.0 - xt) * hh)));
            }
        } else {
            let (xa, xb2) = if xt <= xb { (xt, xb) } else { (xb, xt) };
            let mut cur = xa;
            let mut p = xa.floor() as i64;
            let mut cum = 0.0f32;
            let mut k = p + 1;
            while (k as f32) < xb2 {
                let kx = k as f32;
                let hp = (kx - cur) / dxdy.abs();
                if p >= 0 && p < w as i64 {
                    pieces.push((
                        row + p as usize,
                        s * (hh - cum - (p as f32 + 1.0 - (cur + kx) * 0.5) * hp),
                    ));
                }
                cum += hp;
                cur = kx;
                p = k;
                k += 1;
            }
            let hp = (xb2 - cur) / dxdy.abs();
            if p >= 0 && p < w as i64 {
                pieces.push((
                    row + p as usize,
                    s * (hh - cum - (p as f32 + 1.0 - (cur + xb2) * 0.5) * hp),
                ));
            }
        }
    }
}

const RING: [(f32, f32); 8] = [
    (630.0, 312.0),
    (548.0, 533.0),
    (350.0, 624.0),
    (152.0, 533.0),
    (70.0, 312.0),
    (152.0, 91.0),
    (350.0, 0.0),
    (548.0, 91.0),
];

const HOLE: [(f32, f32); 8] = [
    (210.0, 312.0),
    (251.0, 439.0),
    (350.0, 492.0),
    (449.0, 439.0),
    (490.0, 312.0),
    (449.0, 185.0),
    (350.0, 132.0),
    (251.0, 185.0),
];

const DOT: [(f32, f32); 8] = [
    (470.0, 250.0),
    (414.0, 384.0),
    (280.0, 440.0),
    (146.0, 384.0),
    (90.0, 250.0),
    (146.0, 116.0),
    (280.0, 60.0),
    (414.0, 116.0),
];

fn builtin() -> Font {
    let glyphs: [(f32, &[&[(f32, f32)]]); 9] = [
        (300.0, &[]),
        (500.0, &[]),
        (300.0, &[&[(100.0, 0.0), (250.0, 0.0), (250.0, 150.0), (100.0, 150.0)]]),
        (
            500.0,
            &[
                &[(90.0, 0.0), (190.0, 0.0), (410.0, 500.0), (310.0, 500.0)],
                &[(310.0, 0.0), (410.0, 0.0), (190.0, 500.0), (90.0, 500.0)],
            ],
        ),
        (
            660.0,
            &[
                &[(0.0, 625.0), (100.0, 0.0), (190.0, 0.0), (90.0, 625.0)],
                &[(190.0, 0.0), (280.0, 0.0), (370.0, 350.0), (280.0, 350.0)],
                &[(280.0, 350.0), (370.0, 350.0), (460.0, 0.0), (370.0, 0.0)],
                &[(460.0, 0.0), (550.0, 0.0), (640.0, 625.0), (550.0, 625.0)],
            ],
        ),
        (400.0, &[&[(130.0, 0.0), (270.0, 0.0), (270.0, 625.0), (130.0, 625.0)]]),
        (
            500.0,
            &[&[
                (130.0, 0.0),
                (270.0, 0.0),
                (270.0, 485.0),
                (460.0, 485.0),
                (460.0, 625.0),
                (130.0, 625.0),
            ]],
        ),
        (700.0, &[&RING[..], &HOLE[..]]),
        (560.0, &[&DOT[..]]),
    ];
    let mut extra = Vec::new();
    let mut advances = Vec::new();
    for (adv, cs) in &glyphs {
        advances.push(*adv as u16);
        extra.push(Outline {
            contours: cs.iter().map(|c| poly(c)).collect(),
        });
    }
    let mut map = HashMap::new();
    for (c, g) in [
        (' ', 1u16),
        ('\u{a0}', 1),
        ('.', 2),
        ('x', 3),
        ('W', 4),
        ('I', 5),
        ('L', 6),
        ('O', 7),
        ('\u{2022}', 8),
    ] {
        map.insert(c as u32, g);
    }
    Font {
        data: Vec::new(),
        extra,
        upem: 1000.0,
        ascent: 625.0,
        descent: 250.0,
        xh: 500.0,
        advances,
        map,
        kern: HashMap::new(),
        glyf_off: 0,
        loca: vec![0; 10],
        nglyphs: 9,
    }
}

fn poly(pts: &[(f32, f32)]) -> Contour {
    Contour {
        x: pts.iter().map(|p| p.0).collect(),
        y: pts.iter().map(|p| p.1).collect(),
        on: vec![true; pts.len()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_ttf(tables: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let n = tables.len();
        let mut out = Vec::new();
        out.extend(0x00010000u32.to_be_bytes());
        out.extend((n as u16).to_be_bytes());
        out.extend(0u16.to_be_bytes());
        out.extend(0u16.to_be_bytes());
        out.extend(0u16.to_be_bytes());
        let mut off = 12 + n * 16;
        let mut bodies = Vec::new();
        for (tag, body) in tables {
            out.extend_from_slice(*tag);
            out.extend(0u32.to_be_bytes());
            out.extend((off as u32).to_be_bytes());
            out.extend((body.len() as u32).to_be_bytes());
            let mut b = body.clone();
            while b.len() % 4 != 0 {
                b.push(0);
            }
            off += b.len();
            bodies.push(b);
        }
        for b in bodies {
            out.extend(b);
        }
        out
    }

    fn simple_glyph(contours: &[&[(i16, i16)]]) -> Vec<u8> {
        let mut g = Vec::new();
        g.extend((contours.len() as i16).to_be_bytes());
        let (mut mnx, mut mny, mut mxx, mut mxy) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
        for c in contours {
            for &(x, y) in c.iter() {
                mnx = mnx.min(x);
                mny = mny.min(y);
                mxx = mxx.max(x);
                mxy = mxy.max(y);
            }
        }
        g.extend(mnx.to_be_bytes());
        g.extend(mny.to_be_bytes());
        g.extend(mxx.to_be_bytes());
        g.extend(mxy.to_be_bytes());
        let mut npts = 0u16;
        for c in contours {
            npts += c.len() as u16;
            g.extend((npts - 1).to_be_bytes());
        }
        g.extend(0u16.to_be_bytes());
        for _ in 0..npts {
            g.push(1u8);
        }
        let mut px = 0i16;
        let mut py = 0i16;
        for c in contours {
            for &(x, _) in c.iter() {
                g.extend((x - px).to_be_bytes());
                px = x;
            }
        }
        for c in contours {
            for &(_, y) in c.iter() {
                g.extend((y - py).to_be_bytes());
                py = y;
            }
        }
        g
    }

    fn composite_glyph(parts: &[(u16, i16, i16)]) -> Vec<u8> {
        let mut g = Vec::new();
        g.extend((-1i16).to_be_bytes());
        for _ in 0..4 {
            g.extend(0i16.to_be_bytes());
        }
        for (i, &(gid, dx, dy)) in parts.iter().enumerate() {
            let more = if i + 1 < parts.len() { 0x0020u16 } else { 0 };
            g.extend((0x0003 | more).to_be_bytes());
            g.extend(gid.to_be_bytes());
            g.extend(dx.to_be_bytes());
            g.extend(dy.to_be_bytes());
        }
        g
    }

    fn head(upem: u16) -> Vec<u8> {
        let mut t = vec![0u8; 54];
        t[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        t[12..16].copy_from_slice(&0x5F0F3CF5u32.to_be_bytes());
        t[18..20].copy_from_slice(&upem.to_be_bytes());
        t[50..52].copy_from_slice(&0i16.to_be_bytes());
        t
    }

    fn hhea(asc: i16, desc: i16, nhm: u16) -> Vec<u8> {
        let mut t = vec![0u8; 36];
        t[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        t[4..6].copy_from_slice(&asc.to_be_bytes());
        t[6..8].copy_from_slice(&desc.to_be_bytes());
        t[34..36].copy_from_slice(&nhm.to_be_bytes());
        t
    }

    fn hmtx(advances: &[u16]) -> Vec<u8> {
        let mut t = Vec::new();
        for &a in advances {
            t.extend(a.to_be_bytes());
            t.extend(0i16.to_be_bytes());
        }
        t
    }

    fn cmap4(segs: &[(u16, u16, i16, u16)], glyphs: &[u16]) -> Vec<u8> {
        let seg = segs.len();
        let mut t = Vec::new();
        t.extend(4u16.to_be_bytes());
        t.extend(0u16.to_be_bytes());
        t.extend(0u16.to_be_bytes());
        t.extend(((seg * 2) as u16).to_be_bytes());
        t.extend(0u16.to_be_bytes());
        t.extend(0u16.to_be_bytes());
        t.extend(0u16.to_be_bytes());
        for s in segs {
            t.extend(s.1.to_be_bytes());
        }
        t.extend(0u16.to_be_bytes());
        for s in segs {
            t.extend(s.0.to_be_bytes());
        }
        for s in segs {
            t.extend(s.2.to_be_bytes());
        }
        let ranges = t.len();
        for s in segs {
            t.extend(s.3.to_be_bytes());
        }
        let arr = t.len();
        for g in glyphs {
            t.extend(g.to_be_bytes());
        }
        t[ranges] = ((arr - ranges) as u16).to_be_bytes()[0];
        t[ranges + 1] = ((arr - ranges) as u16).to_be_bytes()[1];
        t
    }

    fn kern_table(pairs: &[(u16, u16, i16)]) -> Vec<u8> {
        let mut t = Vec::new();
        t.extend(0u16.to_be_bytes());
        t.extend(1u16.to_be_bytes());
        let sub = 14 + pairs.len() * 6;
        t.extend(0u16.to_be_bytes());
        t.extend((sub as u16).to_be_bytes());
        t.extend(1u16.to_be_bytes());
        t.extend((pairs.len() as u16).to_be_bytes());
        t.extend(0u16.to_be_bytes());
        t.extend(0u16.to_be_bytes());
        t.extend(0u16.to_be_bytes());
        for p in pairs {
            t.extend(p.0.to_be_bytes());
            t.extend(p.1.to_be_bytes());
            t.extend(p.2.to_be_bytes());
        }
        t
    }

    fn rect() -> Vec<u8> {
        simple_glyph(&[&[(100, 0), (300, 0), (300, 600), (100, 600)]])
    }

    fn ring() -> Vec<u8> {
        simple_glyph(&[
            &[(100, 0), (400, 0), (400, 600), (100, 600)],
            &[(200, 150), (200, 450), (300, 450), (300, 150)],
        ])
    }

    fn sample_font() -> Vec<u8> {
        let glyphs = [
            Vec::new(),
            rect(),
            ring(),
            composite_glyph(&[(2, 0, 700), (1, 0, 700)]),
        ];
        let mut glyf = Vec::new();
        let mut loca = Vec::new();
        for g in &glyphs {
            loca.push((glyf.len() / 2) as u16);
            glyf.extend(g);
            if glyf.len() % 2 != 0 {
                glyf.push(0);
            }
        }
        loca.push((glyf.len() / 2) as u16);
        let loca_bytes: Vec<u8> = loca.iter().flat_map(|v| v.to_be_bytes()).collect();
        let cmap = {
            let mut t = Vec::new();
            t.extend(0u16.to_be_bytes());
            t.extend(1u16.to_be_bytes());
            t.extend(3u16.to_be_bytes());
            t.extend(1u16.to_be_bytes());
            t.extend(12u32.to_be_bytes());
            t.extend(cmap4(
                &[
                    ('A' as u16, 'V' as u16, 0, 4),
                    (0xC4, 0xC4, -193, 0),
                    (0xFFFF, 0xFFFF, 1, 0),
                ],
                &[
                    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ],
            ));
            t
        };
        build_ttf(&[
            (b"cmap", cmap),
            (b"glyf", glyf),
            (b"head", head(1000)),
            (b"hhea", hhea(800, -200, 4)),
            (b"hmtx", hmtx(&[250, 300, 400, 450])),
            (b"kern", kern_table(&[(1, 2, -100)])),
            (b"loca", loca_bytes),
            (b"maxp", {
                let mut t = vec![0u8; 6];
                t[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
                t[4..6].copy_from_slice(&4u16.to_be_bytes());
                t
            }),
        ])
    }

    #[test]
    fn parses_tables_and_cmap() {
        let f = Font::parse(&sample_font()).unwrap();
        assert_eq!(f.upem, 1000.0);
        assert_eq!(f.nglyphs, 4);
        assert_eq!(f.gid('A'), 1);
        assert_eq!(f.gid('V'), 2);
        assert_eq!(f.gid('\u{c4}'), 3);
        assert_eq!(f.gid('Z'), 0);
        assert_eq!(f.advances[1], 300);
        assert_eq!(f.advances[3], 450);
        assert_eq!(f.ascent, 800.0);
        assert_eq!(f.descent, 200.0);
    }

    #[test]
    fn rasterizes_rect_with_coverage() {
        let f = Font::parse(&sample_font()).unwrap();
        let b = raster(&f, 'A', 100.0);
        assert!(b.width > 20 && b.height > 60);
        let inside = |px: i32, py: i32| -> u8 {
            let x = (px - b.left) as usize;
            let y = (py - b.top) as usize;
            b.cover[y * b.width + x]
        };
        assert_eq!(inside(9, -30), 0);
        assert_eq!(inside(10, -30), 255);
        assert_eq!(inside(20, -30), 255);
        assert_eq!(inside(29, -30), 255);
        assert_eq!(inside(30, -30), 0);
        assert_eq!(inside(20, 0), 0);
        assert_eq!(inside(20, -1), 255);
    }

    #[test]
    fn ring_has_hole_by_winding() {
        let f = Font::parse(&sample_font()).unwrap();
        let b = raster(&f, 'V', 100.0);
        let at = |px: i32, py: i32| -> u8 {
            let x = (px - b.left) as usize;
            let y = (py - b.top) as usize;
            b.cover[y * b.width + x]
        };
        assert_eq!(at(25, -30), 0);
        assert_eq!(at(12, -30), 255);
        assert_eq!(at(38, -30), 255);
        assert_eq!(at(25, -57), 255);
    }

    #[test]
    fn kerning_shrinks_pair() {
        let f = Font::parse(&sample_font()).unwrap();
        let sc = 100.0 / f.upem;
        let plain = (f.advances[1] + f.advances[2]) as f32 * sc;
        assert!((raster_width(&f, "AV", 100.0) - (plain - 100.0 * sc)).abs() < 1e-3);
    }

    fn raster_width(f: &Font, t: &str, size: f32) -> f32 {
        let sc = size / f.upem;
        let mut w = 0.0;
        let chars: Vec<char> = t.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            w += f.advances[f.gid(c) as usize] as f32 * sc;
            if let Some(&n) = chars.get(i + 1) {
                w += f.kern_of(c, n) as f32 * sc;
            }
        }
        w
    }

    #[test]
    fn composite_resolves_components() {
        let f = Font::parse(&sample_font()).unwrap();
        let o = f.outline(3, 0).unwrap();
        assert_eq!(o.contours.len(), 3);
        let mut ymax = f32::MIN;
        for c in &o.contours {
            for &y in &c.y {
                ymax = ymax.max(y);
            }
        }
        assert_eq!(ymax, 700.0 + 600.0);
        let b = raster(&f, '\u{c4}', 100.0);
        assert!(b.cover.iter().any(|v| *v == 255));
    }

    #[test]
    fn offsets_and_width_agree() {
        let f = Font::parse(&sample_font()).unwrap();
        let chars: Vec<char> = "AVA".chars().collect();
        let offs = global_offsets(&f, "AVA", 100.0);
        let last = *offs.last().unwrap();
        let total = last + f.advances[f.gid(chars[2]) as usize] as f32 * (100.0 / f.upem);
        assert!((global_width(&f, "AVA", 100.0) - total).abs() < 1e-4);
        assert!(offs[1] > 0.0);
    }

    fn global_width(f: &Font, t: &str, size: f32) -> f32 {
        let sc = size / f.upem;
        let mut w = 0.0;
        let chars: Vec<char> = t.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if i > 0 {
                w += f.kern_of(chars[i - 1], c) as f32 * sc;
            }
            w += f.advances[f.gid(c) as usize] as f32 * sc;
        }
        w
    }

    fn global_offsets(f: &Font, t: &str, size: f32) -> Vec<f32> {
        let sc = size / f.upem;
        let chars: Vec<char> = t.chars().collect();
        let mut v = Vec::new();
        let mut pen = 0.0;
        for (i, &c) in chars.iter().enumerate() {
            v.push(pen);
            pen += f.advances[f.gid(c) as usize] as f32 * sc;
            if let Some(&n) = chars.get(i + 1) {
                pen += f.kern_of(c, n) as f32 * sc;
            }
        }
        v
    }

    #[test]
    fn antialiasing_gives_partial_coverage() {
        let f = Font::parse(&sample_font()).unwrap();
        let b = raster(&f, 'A', 33.0);
        assert!(b.cover.iter().any(|v| *v > 0 && *v < 255));
        assert!(b.cover.iter().any(|v| *v == 255));
    }

    #[test]
    fn invalid_fonts_rejected() {
        assert!(Font::parse(&[]).is_none());
        assert!(Font::parse(&[1, 2, 3, 4]).is_none());
        assert!(!set(&[1, 2, 3, 4]));
    }

    #[test]
    fn global_falls_back_to_builtin() {
        assert!(width(" ", 16.0) > 0.0);
        assert!((asc(16.0) - 10.0).abs() < 1e-4);
        assert!((desc(16.0) - 4.0).abs() < 1e-4);
        assert!((x_height(16.0) - 8.0).abs() < 1e-4);
        assert!((advance('\t', 16.0) - 4.0 * advance(' ', 16.0)).abs() < 1e-4);
        assert_eq!(advance('\n', 16.0), 0.0);
        assert_eq!(advance('\r', 16.0), 0.0);
        assert!((advance('\u{a0}', 16.0) - advance(' ', 16.0)).abs() < 1e-4);
        assert!(width("I", 16.0) < width("W", 16.0));
    }

    #[test]
    fn unknown_char_is_notdef() {
        let b = rasterize('\u{436}', 16.0);
        assert_eq!(b.width, 0);
        assert!(advance('\u{436}', 16.0) > 0.0);
    }

    #[test]
    fn builtin_ring_winding() {
        let b = rasterize('O', 100.0);
        let at = |px: i32, py: i32| -> u8 {
            let x = (px - b.left) as usize;
            let y = (py - b.top) as usize;
            b.cover[y * b.width + x]
        };
        assert_eq!(at(35, -31), 0);
        assert_eq!(at(9, -31), 255);
        assert_eq!(at(35, -6), 255);
        assert_eq!(at(35, -55), 255);
    }

    #[test]
    fn real_dejavu_parses() {
        let data = match std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") {
            Ok(d) => d,
            Err(_) => return,
        };
        let f = Font::parse(&data).unwrap();
        assert_eq!(f.upem, 2048.0);
        assert!(f.gid('A') > 0);
        assert!(f.gid('\u{436}') > 0);
        let b = raster(&f, 'A', 16.0);
        assert!(b.width > 5 && b.cover.iter().any(|v| *v == 255));
        let bc = raster(&f, '\u{436}', 16.0);
        assert!(bc.cover.iter().any(|v| *v == 255));
        let b2 = raster(&f, 'O', 48.0);
        let at = |px: i32, py: i32| -> u8 {
            b2.cover[(py - b2.top) as usize * b2.width + (px - b2.left) as usize]
        };
        assert_eq!(at(19, -24), 0);
        assert_eq!(at(4, -24), 255);
    }

    #[test]
    fn empty_text_metrics() {
        assert_eq!(width("", 16.0), 0.0);
        assert!(offsets("", 16.0).is_empty());
    }
}
