use css::{Color, Unit, Value};
use img::Image;
use layout::{BoxKind, LayoutBox, Rect};

pub struct Canvas {
    pub width: u32,
    pub height: u32,
    clip: Option<Rect>,
    pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        let w = width.max(1);
        let h = height.max(1);
        Canvas {
            width: w,
            height: h,
            clip: None,
            pixels: vec![255; (w * h * 3) as usize],
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        if x >= self.width || y >= self.height {
            return [255, 255, 255];
        }
        let i = ((y * self.width + x) * 3) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
        ]
    }

    fn fill(&mut self, r: Rect, c: Color) {
        let x0 = r.x.floor().max(0.0) as i64;
        let y0 = r.y.floor().max(0.0) as i64;
        let x1 = (r.x + r.width).ceil().min(self.width as f32) as i64;
        let y1 = (r.y + r.height).ceil().min(self.height as f32) as i64;
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend(x, y, c);
            }
        }
    }

    pub fn text(&mut self, x: f32, y: f32, s: &str, c: Color, size: f32) {
        let sc = font::scale(size);
        let top = y - font::ASCENT * sc;
        let mut pen = x;
        for ch in s.chars() {
            let g = font::glyph(ch);
            for (r, row) in g.rows.iter().enumerate() {
                for col in 0..g.width {
                    if (row >> col) & 1 == 1 {
                        self.fill(
                            Rect {
                                x: pen + col as f32 * sc,
                                y: top + r as f32 * sc,
                                width: sc,
                                height: sc,
                            },
                            c,
                        );
                    }
                }
            }
            pen += (g.width as f32 + 1.0) * sc;
        }
    }

    fn blend(&mut self, x: i64, y: i64, c: Color) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        if let Some(cl) = self.clip {
            let (fx, fy) = (x as f32, y as f32);
            if fx < cl.x || fy < cl.y || fx >= cl.x + cl.width || fy >= cl.y + cl.height {
                return;
            }
        }
        let i = ((y as u32 * self.width + x as u32) * 3) as usize;
        let a = c.a as f32 / 255.0;
        for (k, ch) in [c.r, c.g, c.b].iter().enumerate() {
            self.pixels[i + k] =
                (*ch as f32 * a + self.pixels[i + k] as f32 * (1.0 - a)).round() as u8;
        }
    }

    pub fn image(&mut self, x: f32, y: f32, img: &Image, clip: Rect) {
        for py in 0..img.height {
            for px in 0..img.width {
                let dx = x + px as f32;
                let dy = y + py as f32;
                if dx < clip.x
                    || dx >= clip.x + clip.width
                    || dy < clip.y
                    || dy >= clip.y + clip.height
                {
                    continue;
                }
                let ix = dx as i64;
                let iy = dy as i64;
                if ix < 0 || iy < 0 || ix >= self.width as i64 || iy >= self.height as i64 {
                    continue;
                }
                let [r, g, b, a] = img.pixel(px, py);
                if a == 0 {
                    continue;
                }
                self.blend(ix, iy, Color { r, g, b, a });
            }
        }
    }

    pub fn image_scaled(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        img: &Image,
        clip: Rect,
        smooth: bool,
    ) {
        if w <= 0.0 || h <= 0.0 || img.width == 0 || img.height == 0 {
            return;
        }
        let x0 = x.floor().max(0.0) as i64;
        let y0 = y.floor().max(0.0) as i64;
        let x1 = (x + w).ceil().min(self.width as f32) as i64;
        let y1 = (y + h).ceil().min(self.height as f32) as i64;
        for dy in y0..y1 {
            for dx in x0..x1 {
                let fx = dx as f32;
                let fy = dy as f32;
                if fx < clip.x
                    || fx >= clip.x + clip.width
                    || fy < clip.y
                    || fy >= clip.y + clip.height
                {
                    continue;
                }
                let sx = (fx - x + 0.5) * img.width as f32 / w - 0.5;
                let sy = (fy - y + 0.5) * img.height as f32 / h - 0.5;
                let c = if smooth {
                    sample_bilinear(img, sx, sy)
                } else {
                    sample_nearest(img, sx, sy)
                };
                if c.a == 0 {
                    continue;
                }
                self.blend(dx, dy, c);
            }
        }
    }

    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut ihdr = Vec::new();
        ihdr.extend(self.width.to_be_bytes());
        ihdr.extend(self.height.to_be_bytes());
        ihdr.extend([8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        let stride = (self.width * 3) as usize;
        let mut raw = Vec::with_capacity((stride + 1) * self.height as usize);
        for y in 0..self.height as usize {
            raw.push(0);
            raw.extend_from_slice(&self.pixels[y * stride..(y + 1) * stride]);
        }
        chunk(&mut out, b"IDAT", &zlib_store(&raw));
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

pub fn paint(root: &LayoutBox, viewport_width: f32) -> Canvas {
    paint_with(root, viewport_width, &img::Store::new())
}

pub fn paint_with(root: &LayoutBox, viewport_width: f32, imgs: &img::Store) -> Canvas {
    let bb = root.dims.border_box();
    let height = ((bb.y + bb.height).max(max_bottom(root))
        .ceil()
        .max(1.0)) as u32;
    let mut canvas = Canvas::new(viewport_width.max(1.0) as u32, height);
    if let Some(c) = computed(root).and_then(bg_color) {
        canvas.fill(
            Rect {
                x: 0.0,
                y: 0.0,
                width: canvas.width as f32,
                height: canvas.height as f32,
            },
            c,
        );
    }
    paint_box(&mut canvas, root, imgs);
    canvas
}

fn max_bottom(b: &LayoutBox) -> f32 {
    let bb = b.dims.border_box();
    if clips(b) {
        return bb.y + bb.height;
    }
    let mut m = bb.y + bb.height;
    for c in &b.children {
        m = m.max(max_bottom(c));
    }
    for c in &b.abs {
        m = m.max(max_bottom(c));
    }
    m
}

fn clips(b: &LayoutBox) -> bool {
    match computed(b).and_then(|m| m.get("overflow")).and_then(|v| v.first()) {
        Some(Value::Keyword(k)) => matches!(k.as_str(), "hidden" | "clip" | "scroll" | "auto"),
        _ => false,
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    Rect {
        x: x0,
        y: y0,
        width: (x1 - x0).max(0.0),
        height: (y1 - y0).max(0.0),
    }
}

fn z_index(b: &LayoutBox) -> i32 {
    match computed(b)
        .and_then(|m| m.get("z-index"))
        .and_then(|v| v.first())
    {
        Some(Value::Length(n, _)) => *n as i32,
        Some(Value::Keyword(k)) => k.parse::<i32>().unwrap_or(0),
        _ => 0,
    }
}

fn sample_nearest(img: &Image, sx: f32, sy: f32) -> Color {
    let x = sx.round().clamp(0.0, img.width as f32 - 1.0) as u32;
    let y = sy.round().clamp(0.0, img.height as f32 - 1.0) as u32;
    let [r, g, b, a] = img.pixel(x, y);
    Color { r, g, b, a }
}

fn sample_bilinear(img: &Image, sx: f32, sy: f32) -> Color {
    let sx = sx.clamp(0.0, img.width as f32 - 1.0);
    let sy = sy.clamp(0.0, img.height as f32 - 1.0);
    let x0 = sx.floor() as u32;
    let y0 = sy.floor() as u32;
    let x1 = (x0 + 1).min(img.width - 1);
    let y1 = (y0 + 1).min(img.height - 1);
    let fx = sx - x0 as f32;
    let fy = sy - y0 as f32;
    let ws = [
        (img.pixel(x0, y0), (1.0 - fx) * (1.0 - fy)),
        (img.pixel(x1, y0), fx * (1.0 - fy)),
        (img.pixel(x0, y1), (1.0 - fx) * fy),
        (img.pixel(x1, y1), fx * fy),
    ];
    let mut a = 0f32;
    let mut rgb = [0f32; 3];
    for (c, wgt) in ws {
        let al = c[3] as f32 / 255.0 * wgt;
        a += al;
        for k in 0..3 {
            rgb[k] += c[k] as f32 * al;
        }
    }
    if a <= 0.0 {
        return Color { r: 0, g: 0, b: 0, a: 0 };
    }
    Color {
        r: (rgb[0] / a).round() as u8,
        g: (rgb[1] / a).round() as u8,
        b: (rgb[2] / a).round() as u8,
        a: (a * 255.0).round() as u8,
    }
}

fn paint_img(canvas: &mut Canvas, b: &LayoutBox, imgs: &img::Store) {
    let (src, ..) = match b.img {
        Some(v) => v,
        None => return,
    };
    let im = match imgs.get(src) {
        Some(im) => im,
        None => return,
    };
    let c = b.dims.content;
    if c.width <= 0.0 || c.height <= 0.0 {
        return;
    }
    let m = computed(b);
    let nearest = m.map(pixelated).unwrap_or(false);
    let (iw, ih) = (im.width as f32, im.height as f32);
    let fit = m.map(object_fit).unwrap_or(Fit::Fill);
    let placed = |w: f32, h: f32| (c.x + (c.width - w) / 2.0, c.y + (c.height - h) / 2.0);
    let (dx, dy, dw, dh) = match fit {
        Fit::Fill => (c.x, c.y, c.width, c.height),
        Fit::Contain => {
            let s = (c.width / iw).min(c.height / ih);
            let (dw, dh) = (iw * s, ih * s);
            let (dx, dy) = placed(dw, dh);
            (dx, dy, dw, dh)
        }
        Fit::Cover => {
            let s = (c.width / iw).max(c.height / ih);
            let (dw, dh) = (iw * s, ih * s);
            let (dx, dy) = placed(dw, dh);
            (dx, dy, dw, dh)
        }
        Fit::None => {
            let (dx, dy) = placed(iw, ih);
            (dx, dy, iw, ih)
        }
        Fit::ScaleDown => {
            let s = ((c.width / iw).min(c.height / ih)).min(1.0);
            let (dw, dh) = (iw * s, ih * s);
            let (dx, dy) = placed(dw, dh);
            (dx, dy, dw, dh)
        }
    };
    canvas.image_scaled(dx, dy, dw, dh, im, c, !nearest);
}

fn paint_box(canvas: &mut Canvas, b: &LayoutBox, imgs: &img::Store) {
    paint_bg(canvas, b, imgs);
    paint_img(canvas, b, imgs);
    let saved = canvas.clip;
    if clips(b) {
        let r = b.dims.padding_box();
        canvas.clip = Some(match saved {
            Some(c) => intersect(c, r),
            None => r,
        });
    }
    paint_abs_group(canvas, b, imgs, true);
    paint_text(canvas, b);
    paint_marker(canvas, b, imgs);
    for child in &b.children {
        if !is_float(child) {
            paint_box(canvas, child, imgs);
        }
    }
    for child in &b.children {
        if is_float(child) {
            paint_box(canvas, child, imgs);
        }
    }
    paint_abs_group(canvas, b, imgs, false);
    canvas.clip = saved;
}

fn paint_abs_group(canvas: &mut Canvas, b: &LayoutBox, imgs: &img::Store, below: bool) {
    let mut idx: Vec<usize> = (0..b.abs.len())
        .filter(|&i| (z_index(&b.abs[i]) < 0) == below)
        .collect();
    idx.sort_by_key(|&i| z_index(&b.abs[i]));
    for i in idx {
        paint_box(canvas, &b.abs[i], imgs);
    }
}

fn paint_text(canvas: &mut Canvas, b: &LayoutBox) {
    let t = match b.text {
        Some(t) if t != "\n" => t,
        _ => return,
    };
    let map = computed(b);
    let color = map
        .and_then(|m| token_color(m, "color"))
        .unwrap_or(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        });
    let (size, lh) = text_metrics(map);
    let sc = font::scale(size);
    let baseline =
        b.dims.content.y + (lh - (font::ASCENT + font::DESCENT) * sc) / 2.0 + font::ASCENT * sc;
    canvas.text(b.dims.content.x, baseline, t, color, size);
}

fn paint_marker(canvas: &mut Canvas, b: &LayoutBox, imgs: &img::Store) {
    if b.marker.is_none() && b.marker_img.is_none() {
        return;
    }
    let map = computed(b);
    let color = map
        .and_then(|m| token_color(m, "color"))
        .unwrap_or(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        });
    let (size, lh) = text_metrics(map);
    let sc = font::scale(size);
    let baseline =
        b.dims.content.y + (lh - (font::ASCENT + font::DESCENT) * sc) / 2.0 + font::ASCENT * sc;
    if let Some(u) = b.marker_img {
        if let Some(im) = imgs.get(u) {
            canvas.image(
                b.dims.content.x - 2.0 - im.width as f32,
                baseline - im.height as f32,
                im,
                Rect {
                    x: f32::NEG_INFINITY,
                    y: f32::NEG_INFINITY,
                    width: f32::INFINITY,
                    height: f32::INFINITY,
                },
            );
            return;
        }
        let side = b.marker_size.0.max(1.0);
        canvas.fill(
            Rect {
                x: b.dims.content.x - 2.0 - side,
                y: baseline - side,
                width: side,
                height: side,
            },
            color,
        );
        return;
    }
    let m = b.marker.unwrap();
    canvas.text(b.dims.content.x - font::width(m, size), baseline, m, color, size);
}

fn text_metrics(map: Option<&std::collections::HashMap<String, Vec<Value>>>) -> (f32, f32) {
    const BASE_FONT: f32 = 16.0;
    let size = map
        .and_then(|m| m.get("font-size"))
        .and_then(|v| v.first())
        .and_then(|v| match v {
            Value::Length(n, _) => Some(*n),
            _ => None,
        })
        .unwrap_or(BASE_FONT);
    let lh = map
        .and_then(|m| m.get("line-height"))
        .and_then(|v| v.first())
        .and_then(|v| match v {
            Value::Length(n, Unit::Px) => Some(*n),
            Value::Length(n, Unit::Em) => Some(n * size),
            Value::Keyword(k) => k
                .trim_end_matches('%')
                .parse::<f32>()
                .ok()
                .map(|m| m * size),
            _ => None,
        })
        .unwrap_or(size * 1.2);
    (size, lh)
}

fn paint_bg(canvas: &mut Canvas, b: &LayoutBox, imgs: &img::Store) {
    let d = &b.dims;
    let bb = d.border_box();
    if let Some(c) = computed(b).and_then(bg_color) {
        canvas.fill(bb, c);
    }
    if let Some(m) = computed(b) {
        if let Some(u) = bg_url(m) {
            if let Some(im) = imgs.get(&u) {
                let pb = d.padding_box();
                let font = text_metrics(Some(m)).0;
                let (rw, rh) = bg_size(
                    m,
                    pb.width,
                    pb.height,
                    im.width as f32,
                    im.height as f32,
                    font,
                );
                let (ox, oy) = bg_offset(m, pb.width, pb.height, rw, rh);
                let xs = match bg_repeat(m) {
                    Repeat::Y | Repeat::None => vec![ox],
                    _ => span(ox, pb.width, rw),
                };
                let ys = match bg_repeat(m) {
                    Repeat::X | Repeat::None => vec![oy],
                    _ => span(oy, pb.height, rh),
                };
                for y in ys {
                    for x in &xs {
                        canvas.image_scaled(pb.x + x, pb.y + y, rw, rh, im, pb, !pixelated(m));
                    }
                }
            }
        }
    }
    if !borders_hidden(b) && d.border.top + d.border.right + d.border.bottom + d.border.left > 0.0
    {
        let c = computed(b)
            .and_then(|m| token_color(m, "border-color"))
            .unwrap_or(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            });
        canvas.fill(
            Rect {
                x: bb.x,
                y: bb.y,
                width: bb.width,
                height: d.border.top,
            },
            c,
        );
        canvas.fill(
            Rect {
                x: bb.x,
                y: bb.y + bb.height - d.border.bottom,
                width: bb.width,
                height: d.border.bottom,
            },
            c,
        );
        canvas.fill(
            Rect {
                x: bb.x,
                y: bb.y + d.border.top,
                width: d.border.left,
                height: (bb.height - d.border.top - d.border.bottom).max(0.0),
            },
            c,
        );
        canvas.fill(
            Rect {
                x: bb.x + bb.width - d.border.right,
                y: bb.y + d.border.top,
                width: d.border.right,
                height: (bb.height - d.border.top - d.border.bottom).max(0.0),
            },
            c,
        );
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Repeat {
    Both,
    X,
    Y,
    None,
}

enum Sz {
    Len(f32),
    Pct(f32),
    Auto,
    Cover,
    Contain,
}

fn span(start: f32, total: f32, tile: f32) -> Vec<f32> {
    if tile <= 0.0 || total <= 0.0 {
        return vec![start];
    }
    let mut out = Vec::new();
    let mut x = start - tile * (start / tile).floor() - tile;
    while x < total {
        out.push(x);
        x += tile;
    }
    out
}

#[derive(Clone, Copy)]
enum Pos {
    Len(f32),
    Pct(f32),
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

fn pos_of(v: &Value, font: f32) -> Option<Pos> {
    match v {
        Value::Length(n, Unit::Px) => Some(Pos::Len(*n)),
        Value::Length(n, Unit::Em) => Some(Pos::Len(n * font)),
        Value::Keyword(k) => {
            if let Some(p) = k.strip_suffix('%') {
                return p.parse::<f32>().ok().map(Pos::Pct);
            }
            match k.as_str() {
                "left" => Some(Pos::Left),
                "right" => Some(Pos::Right),
                "top" => Some(Pos::Top),
                "bottom" => Some(Pos::Bottom),
                "center" => Some(Pos::Center),
                _ => None,
            }
        }
        _ => None,
    }
}

fn bg_axis(p: Pos, extent: f32, tile: f32) -> f32 {
    match p {
        Pos::Len(n) => n,
        Pos::Pct(k) => (extent - tile) * k / 100.0,
        Pos::Left | Pos::Top => 0.0,
        Pos::Right | Pos::Bottom => extent - tile,
        Pos::Center => (extent - tile) / 2.0,
    }
}

fn split_slash(vals: &[Value]) -> (&[Value], &[Value]) {
    match vals.iter().position(|v| matches!(v, Value::Keyword(k) if k == "/")) {
        Some(i) => (&vals[..i], &vals[i + 1..]),
        None => (vals, &[]),
    }
}

fn bg_offset(
    m: &std::collections::HashMap<String, Vec<Value>>,
    cw: f32,
    ch: f32,
    rw: f32,
    rh: f32,
) -> (f32, f32) {
    let font = text_metrics(Some(m)).0;
    let vals: &[Value] = if let Some(v) = m.get("background-position") {
        v
    } else if let Some(all) = m.get("background") {
        split_slash(all).0
    } else {
        &[]
    };
    let mut x: Option<Pos> = None;
    let mut y: Option<Pos> = None;
    for v in vals {
        if let Some(p) = pos_of(v, font) {
            if x.is_none() && !matches!(p, Pos::Top | Pos::Bottom) {
                x = Some(p);
            } else if y.is_none() {
                y = Some(p);
            }
        }
    }
    let ox = x.map(|p| bg_axis(p, cw, rw)).unwrap_or(0.0);
    let oy = match y {
        Some(p) => bg_axis(p, ch, rh),
        None if x.is_some() => (ch - rh) / 2.0,
        None => 0.0,
    };
    (ox, oy)
}

fn bg_url(m: &std::collections::HashMap<String, Vec<Value>>) -> Option<String> {
    let pick = |vals: &[Value]| {
        vals.iter().find_map(|v| match v {
            Value::Url(u) => Some(u.clone()),
            _ => None,
        })
    };
    if let Some(u) = m.get("background-image").and_then(|v| pick(v)) {
        return Some(u);
    }
    m.get("background").and_then(|v| pick(v))
}

fn bg_color(m: &std::collections::HashMap<String, Vec<Value>>) -> Option<Color> {
    let pick = |vals: &[Value]| {
        vals.iter().find_map(|v| match v {
            Value::Color(c) => Some(*c),
            _ => None,
        })
    };
    if let Some(c) = m.get("background-color").and_then(|v| pick(v)) {
        return Some(c);
    }
    m.get("background").and_then(|v| pick(v))
}

fn bg_repeat(m: &std::collections::HashMap<String, Vec<Value>>) -> Repeat {
    let of = |vals: &[Value]| -> Option<Repeat> {
        vals.iter().find_map(|v| match v {
            Value::Keyword(k) => match k.as_str() {
                "no-repeat" => Some(Repeat::None),
                "repeat-x" => Some(Repeat::X),
                "repeat-y" => Some(Repeat::Y),
                "repeat" => Some(Repeat::Both),
                _ => None,
            },
            _ => None,
        })
    };
    if let Some(r) = m.get("background-repeat").and_then(|v| of(v)) {
        return r;
    }
    m.get("background")
        .and_then(|v| of(v))
        .unwrap_or(Repeat::Both)
}

fn size_of(v: &Value, font: f32) -> Option<Sz> {
    match v {
        Value::Length(n, Unit::Px) => Some(Sz::Len(*n)),
        Value::Length(n, Unit::Em) => Some(Sz::Len(n * font)),
        Value::Keyword(k) => {
            if let Some(p) = k.strip_suffix('%') {
                return p.parse::<f32>().ok().map(Sz::Pct);
            }
            match k.as_str() {
                "cover" => Some(Sz::Cover),
                "contain" => Some(Sz::Contain),
                "auto" => Some(Sz::Auto),
                _ => None,
            }
        }
        _ => None,
    }
}

fn bg_size(
    m: &std::collections::HashMap<String, Vec<Value>>,
    pw: f32,
    ph: f32,
    iw: f32,
    ih: f32,
    font: f32,
) -> (f32, f32) {
    let vals: Vec<Value> = match m.get("background-size") {
        Some(v) => v.clone(),
        None => match m.get("background") {
            Some(all) => split_slash(all).1.to_vec(),
            None => Vec::new(),
        },
    };
    let n: Vec<Sz> = vals.iter().filter_map(|v| size_of(v, font)).collect();
    if n.iter().any(|s| matches!(s, Sz::Cover)) {
        let s = (pw / iw).max(ph / ih);
        return (iw * s, ih * s);
    }
    if n.iter().any(|s| matches!(s, Sz::Contain)) {
        let s = (pw / iw).min(ph / ih);
        return (iw * s, ih * s);
    }
    let ratio = if ih > 0.0 { iw / ih } else { 1.0 };
    let axis = |s: Option<&Sz>, base: f32| match s {
        Some(Sz::Len(v)) => Some(*v),
        Some(Sz::Pct(k)) => Some(base * k / 100.0),
        _ => None,
    };
    let w = axis(n.first(), pw);
    let h = axis(n.get(1), ph);
    let (w, h) = match (w, h) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w / ratio),
        (None, Some(h)) => (h * ratio, h),
        (None, None) => (iw, ih),
    };
    if w > 0.0 && h > 0.0 {
        (w, h)
    } else {
        (iw, ih)
    }
}

fn pixelated(m: &std::collections::HashMap<String, Vec<Value>>) -> bool {
    matches!(
        m.get("image-rendering").and_then(|v| v.first()),
        Some(Value::Keyword(k)) if k == "pixelated"
    )
}

enum Fit {
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

fn object_fit(m: &std::collections::HashMap<String, Vec<Value>>) -> Fit {
    match m.get("object-fit").and_then(|v| v.first()) {
        Some(Value::Keyword(k)) => match k.as_str() {
            "contain" => Fit::Contain,
            "cover" => Fit::Cover,
            "none" => Fit::None,
            "scale-down" => Fit::ScaleDown,
            _ => Fit::Fill,
        },
        _ => Fit::Fill,
    }
}

fn is_float(b: &LayoutBox) -> bool {
    match &b.kind {
        BoxKind::Block(s) => matches!(
            s.computed.get("float").and_then(|v| v.first()),
            Some(Value::Keyword(k)) if k == "left" || k == "right"
        ),
        _ => false,
    }
}

fn computed<'a>(b: &'a LayoutBox<'a>) -> Option<&'a std::collections::HashMap<String, Vec<Value>>> {
    match &b.kind {
        BoxKind::Block(s)
        | BoxKind::Inline(s)
        | BoxKind::InlineBlock(s)
        | BoxKind::Flex(s)
        | BoxKind::Table(s)
        | BoxKind::TableRow(s)
        | BoxKind::TableCell(s) => Some(&s.computed),
        BoxKind::Anonymous => None,
    }
}

fn token_color(m: &std::collections::HashMap<String, Vec<Value>>, key: &str) -> Option<Color> {
    m.get(key)?.iter().find_map(|v| match v {
        Value::Color(c) => Some(*c),
        _ => None,
    })
}

fn borders_hidden(b: &LayoutBox) -> bool {
    computed(b)
        .map(|m| {
            ["border", "border-style"].iter().any(|k| {
                m.get(*k)
                    .map(|vals| {
                        vals.iter()
                            .any(|v| matches!(v, Value::Keyword(k) if k == "none" || k == "hidden"))
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(tag);
    out.extend(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend(tag);
    crc_input.extend(data);
    out.extend(crc32(&crc_input).to_be_bytes());
}

fn zlib_store(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let mut i = 0;
    loop {
        let n = (raw.len() - i).min(65535);
        let last = i + n >= raw.len();
        z.push(if last { 1 } else { 0 });
        z.extend((n as u16).to_le_bytes());
        z.extend((!(n as u16)).to_le_bytes());
        z.extend(&raw[i..i + n]);
        i += n;
        if last {
            break;
        }
    }
    z.extend(adler32(raw).to_be_bytes());
    z
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::parse as parse_css;
    use html::parse as parse_html;
    use layout::{layout, layout_with};
    use style::build as style_build;

    fn eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn background_fills() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { background-color: #ff0000; width: 50px; height: 20px }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(10, 10), [255, 0, 0]);
        assert_eq!(canvas.pixel(60, 10), [255, 255, 255]);
        assert_eq!(canvas.pixel(10, 25), [255, 255, 255]);
    }

    #[test]
    fn border_ring() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 20px; height: 20px; background-color: white; border: 3px solid #000000 }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(1, 1), [0, 0, 0]);
        assert_eq!(canvas.pixel(10, 10), [255, 255, 255]);
        assert_eq!(canvas.pixel(24, 10), [0, 0, 0]);
        assert_eq!(canvas.pixel(10, 1), [0, 0, 0]);
    }

    #[test]
    fn nested_offset() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css(
            "div { padding: 5px; background-color: #00ff00 } p { width: 10px; height: 10px; background-color: #0000ff; margin: 0 }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(2, 2), [0, 255, 0]);
        assert_eq!(canvas.pixel(7, 7), [0, 0, 255]);
    }

    #[test]
    fn root_background_propagates() {
        let dom = parse_html("<html><body></body></html>");
        let sheet = parse_css("html { background-color: #0000ff } body { width: 10px; height: 10px }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 50.0).unwrap();
        let canvas = paint(&tree, 50.0);
        assert_eq!(canvas.pixel(0, 0), [0, 0, 255]);
        assert_eq!(canvas.pixel(40, 5), [0, 0, 255]);
    }

    #[test]
    fn transparent_keeps_background() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { width: 30px; height: 10px; background-color: transparent }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(5, 5), [255, 255, 255]);
    }

    #[test]
    fn alpha_blend() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "html { background-color: #000000 } div { width: 10px; height: 10px; background-color: #ffffff80 }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        let p = canvas.pixel(5, 5);
        assert!(p[0] > 110 && p[0] < 140);
    }

    #[test]
    fn border_none_skipped() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 20px; height: 20px; background-color: white; border: 5px solid #000000; border-style: none }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(1, 1), [255, 255, 255]);
    }

    #[test]
    fn float_paints_over_sibling_background() {
        let dom = parse_html("<html><div class=\"f\"></div><p></p></html>");
        let sheet = parse_css(
            ".f { float: left; width: 50px; height: 40px; background-color: #00ff00 } p { width: 100px; height: 30px; background-color: #ffffff; margin: 0 }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(10, 10), [0, 255, 0]);
        assert_eq!(canvas.pixel(60, 10), [255, 255, 255]);
        assert_eq!(canvas.pixel(10, 35), [0, 255, 0]);
    }

    #[test]
    fn png_roundtrip() {
        let mut canvas = Canvas::new(3, 2);
        canvas.pixels[0] = 250;
        canvas.pixels[1] = 10;
        canvas.pixels[2] = 60;
        let png = canvas.to_png();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let (w, h, idat) = parse_png(&png);
        assert!(eq(w as f32, 3.0));
        assert!(eq(h as f32, 2.0));
        let raw = inflate_stored(&idat);
        assert_eq!(raw.len(), 2 * (1 + 9));
        assert_eq!(raw[0], 0);
        assert_eq!(&raw[1..10], &canvas.pixels[0..9]);
        assert_eq!(raw[10], 0);
        assert_eq!(&raw[11..20], &canvas.pixels[9..18]);
    }

    #[test]
    fn text_draws_glyphs() {
        let dom = parse_html("<html><p>l</p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(0, 11), [0, 0, 0]);
        assert_eq!(canvas.pixel(1, 5), [0, 0, 0]);
        assert_eq!(canvas.pixel(4, 11), [255, 255, 255]);
        assert_eq!(canvas.pixel(0, 15), [255, 255, 255]);
    }

    #[test]
    fn text_color() {
        let dom = parse_html("<html><p>l</p></html>");
        let sheet = parse_css("p { color: #ff0000 }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(0, 11), [255, 0, 0]);
    }

    #[test]
    fn text_inherited_color() {
        let dom = parse_html("<html><div><p>l</p></div></html>");
        let sheet = parse_css("div { color: #0000ff }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(0, 11), [0, 0, 255]);
    }

    #[test]
    fn text_font_size_scales() {
        let dom = parse_html("<html><p>l</p></html>");
        let sheet = parse_css("p { font-size: 32px }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(0, 25), [0, 0, 0]);
        assert_eq!(canvas.pixel(5, 25), [255, 255, 255]);
    }

    #[test]
    fn bullet_marker_paints() {
        let dom = parse_html("<html><ul><li>a</li></ul></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 200.0).unwrap();
        let canvas = paint(&tree, 200.0);
        assert_eq!(canvas.pixel(34, 5), [0, 0, 0]);
        assert_eq!(canvas.pixel(20, 5), [255, 255, 255]);
    }

    #[test]
    fn table_collapse_paints_single_border() {
        let dom = parse_html(
            "<html><table style=\"border-collapse: collapse\"><tr><td>a</td><td>b</td></tr></table></html>",
        );
        let sheet = parse_css("td { border: 2px solid #000000 }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 200.0).unwrap();
        let canvas = paint(&tree, 200.0);
        let x = font::width("a", 16.0) as u32 + 5;
        assert_eq!(canvas.pixel(x - 1, 2), [0, 0, 0]);
        assert_eq!(canvas.pixel(x, 2), [0, 0, 0]);
        assert_eq!(canvas.pixel(x - 2, 2), [255, 255, 255]);
        assert_eq!(canvas.pixel(x + 1, 2), [255, 255, 255]);
    }

    #[test]
    fn list_style_image_paints_square() {
        let dom = parse_html("<html><ul><li>a</li></ul></html>");
        let sheet = parse_css("li { list-style-image: url(b.png); color: #ff0000 }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 200.0).unwrap();
        let canvas = paint(&tree, 200.0);
        assert_eq!(canvas.pixel(32, 8), [255, 0, 0]);
        assert_eq!(canvas.pixel(28, 8), [255, 255, 255]);
        assert_eq!(canvas.pixel(39, 8), [255, 255, 255]);
        assert_eq!(canvas.pixel(32, 4), [255, 0, 0]);
        assert_eq!(canvas.pixel(32, 12), [255, 0, 0]);
        assert_eq!(canvas.pixel(32, 3), [255, 255, 255]);
        assert_eq!(canvas.pixel(32, 13), [255, 255, 255]);
    }

    #[test]
    fn list_style_image_paints_bitmap() {
        let dom = parse_html("<html><ul><li>a</li></ul></html>");
        let sheet = parse_css("li { list-style-image: url(b.png); color: #ff0000 }");
        let styled = style_build(&dom, &sheet);
        let mut imgs = img::Store::new();
        imgs.insert(
            "b.png".into(),
            img::Image {
                width: 2,
                height: 2,
                data: vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
                ],
            },
        );
        let tree = layout_with(&styled, 200.0, &imgs).unwrap();
        let canvas = paint_with(&tree, 200.0, &imgs);
        let li = find_box(&tree, "li").expect("missing li");
        let lh = 19.2;
        let sc = font::scale(16.0);
        let baseline = li.dims.content.y
            + (lh - (font::ASCENT + font::DESCENT) * sc) / 2.0
            + font::ASCENT * sc;
        let mx = li.dims.content.x - 4.0;
        let by = baseline as u32;
        assert_eq!(canvas.pixel(mx as u32, by - 2), [255, 0, 0]);
        assert_eq!(canvas.pixel(mx as u32 + 1, by - 2), [0, 255, 0]);
        assert_eq!(canvas.pixel(mx as u32, by - 1), [0, 0, 255]);
        assert_eq!(canvas.pixel(mx as u32 + 1, by - 1), [255, 255, 0]);
        assert_eq!(canvas.pixel(mx as u32 - 1, by - 1), [255, 255, 255]);
        assert_eq!(canvas.pixel(mx as u32 + 2, by - 1), [255, 255, 255]);
    }

    #[test]
    fn background_image_tiles() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { width: 10px; height: 10px; background-image: url(t.png) }");
        let styled = style_build(&dom, &sheet);
        let mut imgs = img::Store::new();
        imgs.insert(
            "t.png".into(),
            img::Image {
                width: 2,
                height: 2,
                data: vec![
                    255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255, 0, 0, 0, 255,
                ],
            },
        );
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(1, 0), [0, 0, 255]);
        assert_eq!(canvas.pixel(0, 1), [0, 255, 0]);
        assert_eq!(canvas.pixel(2, 2), [255, 0, 0]);
        assert_eq!(canvas.pixel(9, 9), [0, 0, 0]);
        assert_eq!(canvas.pixel(9, 10), [255, 255, 255]);
    }

    #[test]
    fn background_image_no_repeat() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat }",
        );
        let styled = style_build(&dom, &sheet);
        let mut imgs = img::Store::new();
        imgs.insert(
            "t.png".into(),
            img::Image {
                width: 2,
                height: 2,
                data: vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255],
            },
        );
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(1, 1), [255, 0, 0]);
        assert_eq!(canvas.pixel(2, 2), [255, 255, 255]);
    }

    #[test]
    fn background_image_repeat_x() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) repeat-x }",
        );
        let styled = style_build(&dom, &sheet);
        let mut imgs = img::Store::new();
        imgs.insert(
            "t.png".into(),
            img::Image {
                width: 2,
                height: 2,
                data: vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255],
            },
        );
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(5, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(5, 2), [255, 255, 255]);
    }

    fn tile_store() -> img::Store {
        let mut imgs = img::Store::new();
        imgs.insert(
            "t.png".into(),
            img::Image {
                width: 2,
                height: 2,
                data: vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
                ],
            },
        );
        imgs
    }

    #[test]
    fn background_position_offsets() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat; background-position: 6px 4px }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(6, 4), [255, 0, 0]);
        assert_eq!(canvas.pixel(7, 4), [0, 255, 0]);
        assert_eq!(canvas.pixel(6, 5), [0, 0, 255]);
        assert_eq!(canvas.pixel(5, 4), [255, 255, 255]);
        assert_eq!(canvas.pixel(6, 3), [255, 255, 255]);
        assert_eq!(canvas.pixel(8, 6), [255, 255, 255]);
    }

    #[test]
    fn background_position_percent() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat; background-position: 100% 100% }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(8, 8), [255, 0, 0]);
        assert_eq!(canvas.pixel(9, 9), [255, 255, 0]);
        assert_eq!(canvas.pixel(7, 7), [255, 255, 255]);
    }

    #[test]
    fn background_position_center_and_keywords() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat; background-position: 50% 50% }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(4, 4), [255, 0, 0]);
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat; background-position: right bottom }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(8, 8), [255, 0, 0]);
    }

    #[test]
    fn background_position_one_value_wraps() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png); background-position: 3px }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 4), [0, 255, 0]);
        assert_eq!(canvas.pixel(1, 4), [255, 0, 0]);
        assert_eq!(canvas.pixel(9, 5), [0, 0, 255]);
        assert_eq!(canvas.pixel(2, 4), [0, 255, 0]);
        assert_eq!(canvas.pixel(1, 0), [255, 0, 0]);
    }

    #[test]
    fn background_size_cover() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat; background-size: cover }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(9, 9), [255, 255, 0]);
        assert_eq!(canvas.pixel(5, 5), [133, 153, 61]);
    }

    #[test]
    fn background_size_contain() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 4px; background: url(t.png) no-repeat; background-size: contain }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(3, 0), [0, 255, 0]);
        assert_eq!(canvas.pixel(0, 3), [0, 0, 255]);
        assert_eq!(canvas.pixel(4, 0), [255, 255, 255]);
        assert_eq!(canvas.pixel(0, 4), [255, 255, 255]);
    }

    #[test]
    fn background_size_lengths_tile() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) repeat; background-size: 4px 2px }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(4, 2), [255, 0, 0]);
        assert_eq!(canvas.pixel(8, 6), [255, 0, 0]);
    }

    #[test]
    fn background_size_percent() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat; background-size: 100% 100% }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(9, 9), [255, 255, 0]);
    }

    #[test]
    fn background_size_one_keeps_ratio() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat; background-size: 4px }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(3, 3), [255, 255, 0]);
        assert_eq!(canvas.pixel(4, 4), [255, 255, 255]);
    }

    #[test]
    fn background_size_shorthand_slash() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 10px; height: 10px; background: url(t.png) no-repeat center / 6px 6px }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(2, 2), [255, 0, 0]);
        assert_eq!(canvas.pixel(1, 2), [255, 255, 255]);
        assert_eq!(canvas.pixel(7, 7), [255, 255, 0]);
    }

    #[test]
    fn img_paints_scaled() {
        let dom = parse_html("<html><body><img src=\"q.png\" width=\"4\" height=\"2\"></body></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let mut imgs = img::Store::new();
        imgs.insert(
            "q.png".into(),
            img::Image {
                width: 2,
                height: 1,
                data: vec![255, 0, 0, 255, 0, 0, 255, 255],
            },
        );
        let tree = layout_with(&styled, 50.0, &imgs).unwrap();
        let canvas = paint_with(&tree, 50.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(3, 0), [0, 0, 255]);
        let mid = canvas.pixel(1, 0);
        assert!(mid[0] > 100 && mid[0] < 255 && mid[2] > 0 && mid[2] < 100);
        assert_eq!(canvas.pixel(0, 1), [255, 0, 0]);
        assert_eq!(canvas.pixel(4, 0), [255, 255, 255]);
    }

    #[test]
    fn img_paints_nearest_when_pixelated() {
        let dom = parse_html(
            "<html><body><img src=\"q.png\" width=\"4\" height=\"2\"></body></html>",
        );
        let sheet = parse_css("img { image-rendering: pixelated }");
        let styled = style_build(&dom, &sheet);
        let mut imgs = img::Store::new();
        imgs.insert(
            "q.png".into(),
            img::Image {
                width: 2,
                height: 1,
                data: vec![255, 0, 0, 255, 0, 0, 255, 255],
            },
        );
        let tree = layout_with(&styled, 50.0, &imgs).unwrap();
        let canvas = paint_with(&tree, 50.0, &imgs);
        assert_eq!(canvas.pixel(0, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(1, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(2, 0), [0, 0, 255]);
        assert_eq!(canvas.pixel(3, 0), [0, 0, 255]);
    }

    #[test]
    fn img_bilinear_alpha_no_bleed() {
        let dom = parse_html(
            "<html><body><img src=\"q.png\" width=\"4\" height=\"4\"></body></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let mut imgs = img::Store::new();
        imgs.insert(
            "q.png".into(),
            img::Image {
                width: 2,
                height: 2,
                data: vec![
                    255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 255, 255, 0, 0, 0, 0,
                ],
            },
        );
        let tree = layout_with(&styled, 50.0, &imgs).unwrap();
        let canvas = paint_with(&tree, 50.0, &imgs);
        let p = canvas.pixel(1, 0);
        assert_eq!(p[0], 255);
        assert!(p[1] > 0 && p[1] < 128, "green must come only from coverage");
    }

    fn quad_store() -> img::Store {
        let mut imgs = img::Store::new();
        imgs.insert(
            "q.png".into(),
            img::Image {
                width: 2,
                height: 2,
                data: vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
                ],
            },
        );
        imgs
    }

    fn paint_img_fit(css: &str, w: u32, h: u32) -> Canvas {
        let dom = parse_html(format!(
            "<html><body><img src=\"q.png\" width=\"{}\" height=\"{}\"></body></html>",
            w, h
        )
        .as_str());
        let sheet = parse_css(css);
        let styled = style_build(&dom, &sheet);
        let imgs = quad_store();
        let tree = layout_with(&styled, 50.0, &imgs).unwrap();
        paint_with(&tree, 50.0, &imgs)
    }

    #[test]
    fn object_fit_contain_letterboxes() {
        let canvas = paint_img_fit("img { object-fit: contain }", 4, 2);
        assert_eq!(canvas.pixel(0, 0), [255, 255, 255]);
        assert_eq!(canvas.pixel(1, 0), [255, 0, 0]);
        assert_eq!(canvas.pixel(2, 0), [0, 255, 0]);
        assert_eq!(canvas.pixel(3, 0), [255, 255, 255]);
        assert_eq!(canvas.pixel(1, 1), [0, 0, 255]);
        assert_eq!(canvas.pixel(2, 1), [255, 255, 0]);
    }

    #[test]
    fn object_fit_cover_clips() {
        let canvas = paint_img_fit("img { object-fit: cover }", 4, 2);
        assert_eq!(canvas.pixel(0, 0), [191, 0, 64]);
        assert_eq!(canvas.pixel(3, 1), [191, 255, 0]);
    }

    #[test]
    fn object_fit_none_keeps_natural() {
        let canvas = paint_img_fit("img { object-fit: none }", 4, 4);
        assert_eq!(canvas.pixel(0, 0), [255, 255, 255]);
        assert_eq!(canvas.pixel(1, 1), [255, 0, 0]);
        assert_eq!(canvas.pixel(2, 2), [255, 255, 0]);
        assert_eq!(canvas.pixel(3, 3), [255, 255, 255]);
    }

    #[test]
    fn object_fit_scale_down_never_grows() {
        let canvas = paint_img_fit("img { object-fit: scale-down }", 4, 4);
        assert_eq!(canvas.pixel(0, 0), [255, 255, 255]);
        assert_eq!(canvas.pixel(1, 1), [255, 0, 0]);
        assert_eq!(canvas.pixel(3, 3), [255, 255, 255]);
        let small = paint_img_fit("img { object-fit: scale-down }", 1, 1);
        assert_eq!(small.pixel(0, 0), [128, 128, 64]);
    }

    #[test]
    fn background_position_em() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css(
            "div { width: 30px; height: 30px; background: url(t.png) no-repeat; background-position: 1em }",
        );
        let styled = style_build(&dom, &sheet);
        let imgs = tile_store();
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint_with(&tree, 100.0, &imgs);
        assert_eq!(canvas.pixel(16, 14), [255, 0, 0]);
        assert_eq!(canvas.pixel(16, 13), [255, 255, 255]);
    }

    #[test]
    fn background_shorthand_color() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { background: #0000ff; width: 20px; height: 10px }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(5, 5), [0, 0, 255]);
        assert_eq!(canvas.pixel(25, 5), [255, 255, 255]);
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { background: red; width: 20px; height: 10px }");
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(5, 5), [255, 0, 0]);
    }

    fn overflow_page(css: &str) -> Canvas {
        let dom = parse_html("<html><body><div class=\"a\"><div class=\"b\"></div></div></body></html>");
        let sheet = parse_css(css);
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        paint(&tree, 100.0)
    }

    #[test]
    fn overflow_hidden_clips_children() {
        let canvas = overflow_page(
            ".a { width: 30px; height: 10px; overflow: hidden; background: #0000ff } .b { width: 50px; height: 20px; background: #ff0000 }",
        );
        assert_eq!(canvas.pixel(20, 5), [255, 0, 0]);
        assert_eq!(canvas.pixel(20, 9), [255, 0, 0]);
        assert_eq!(canvas.pixel(35, 5), [255, 255, 255]);
        assert_eq!(canvas.pixel(20, 12), [255, 255, 255]);
        assert_eq!(canvas.pixel(2, 2), [255, 0, 0]);
    }

    #[test]
    fn overflow_visible_bleeds() {
        let canvas = overflow_page(
            ".a { width: 30px; height: 10px; background: #0000ff } .b { width: 50px; height: 20px; background: #ff0000 }",
        );
        assert_eq!(canvas.pixel(35, 5), [255, 0, 0]);
        assert_eq!(canvas.pixel(20, 12), [255, 0, 0]);
    }

    #[test]
    fn overflow_clip_intersects_ancestors() {
        let dom = parse_html(
            "<html><body><div class=\"a\"><div class=\"b\"><div class=\"c\"></div></div></div></body></html>",
        );
        let sheet = parse_css(
            ".a { width: 30px; height: 30px; overflow: hidden } .b { margin-left: 10px; width: 50px; height: 50px; overflow: hidden } .c { width: 60px; height: 60px; background: #ff0000 }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(29, 5), [255, 0, 0]);
        assert_eq!(canvas.pixel(31, 5), [255, 255, 255]);
        assert_eq!(canvas.pixel(15, 5), [255, 0, 0]);
        assert_eq!(canvas.pixel(9, 5), [255, 255, 255]);
    }

    #[test]
    fn overflow_hidden_bounds_canvas_height() {
        let dom = parse_html("<html><body><div class=\"a\"><p class=\"x\">t</p></div></body></html>");
        let sheet = parse_css(
            ".a { position: relative; width: 30px; height: 10px; overflow: hidden } .x { position: absolute; top: 50px; left: 0; width: 5px; height: 5px }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.height, 10);
    }

    #[test]
    fn z_index_negative_behind_content() {
        let dom = parse_html(
            "<html><body><div class=\"p\"><div class=\"a\"></div><div class=\"b\"></div></div></body></html>",
        );
        let sheet = parse_css(
            ".p { position: relative; width: 40px; height: 20px; background-color: #0000ff } .a { position: absolute; z-index: -1; width: 40px; height: 20px; background-color: #ff0000 } .b { width: 40px; height: 20px; background-color: #00ff00 }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(20, 10), [0, 255, 0]);
        let sheet = parse_css(
            ".p { position: relative; width: 40px; height: 20px; background-color: #0000ff } .a { position: absolute; width: 40px; height: 20px; background-color: #ff0000 } .b { width: 40px; height: 20px; background-color: #00ff00 }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(20, 10), [255, 0, 0]);
    }

    #[test]
    fn z_index_orders_abs_siblings() {
        let dom = parse_html(
            "<html><body><div class=\"p\"><div class=\"a\"></div><div class=\"b\"></div></div></body></html>",
        );
        let sheet = parse_css(
            ".p { position: relative; width: 40px; height: 20px } .a { position: absolute; z-index: 1; width: 40px; height: 20px; background-color: #ffff00 } .b { position: absolute; z-index: 2; width: 20px; height: 20px; background-color: #00ffff }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(10, 10), [0, 255, 255]);
        assert_eq!(canvas.pixel(30, 10), [255, 255, 0]);
        let sheet = parse_css(
            ".p { position: relative; width: 40px; height: 20px } .a { position: absolute; z-index: 2; width: 40px; height: 20px; background-color: #ffff00 } .b { position: absolute; z-index: 1; width: 20px; height: 20px; background-color: #00ffff }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(10, 10), [255, 255, 0]);
    }

    #[test]
    fn z_index_auto_keeps_tree_order() {
        let dom = parse_html(
            "<html><body><div class=\"p\"><div class=\"a\"></div><div class=\"b\"></div></div></body></html>",
        );
        let sheet = parse_css(
            ".p { position: relative; width: 40px; height: 20px } .a { position: absolute; width: 40px; height: 20px; background-color: #ffff00 } .b { position: absolute; width: 20px; height: 20px; background-color: #00ffff }",
        );
        let styled = style_build(&dom, &sheet);
        let tree = layout(&styled, 100.0).unwrap();
        let canvas = paint(&tree, 100.0);
        assert_eq!(canvas.pixel(10, 10), [0, 255, 255]);
    }

    fn find_box<'a>(b: &'a LayoutBox<'a>, name: &str) -> Option<&'a LayoutBox<'a>> {
        for c in &b.children {
            let t = match &c.kind {
                BoxKind::Block(s)
                | BoxKind::Inline(s)
                | BoxKind::InlineBlock(s)
                | BoxKind::Flex(s)
                | BoxKind::Table(s)
                | BoxKind::TableRow(s)
                | BoxKind::TableCell(s) => s.node.name().unwrap_or("#text"),
                BoxKind::Anonymous => "-",
            };
            if t == name {
                return Some(c);
            }
        }
        for c in &b.children {
            if let Some(f) = find_box(c, name) {
                return Some(f);
            }
        }
        None
    }

    fn parse_png(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut i = 8;
        let mut wh = (0u32, 0u32);
        let mut idat = Vec::new();
        while i < png.len() {
            let len = u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]) as usize;
            let tag = &png[i + 4..i + 8];
            let data = &png[i + 8..i + 8 + len];
            if tag == b"IHDR" {
                wh = (
                    u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
                    u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
                );
                assert_eq!(data[8], 8);
                assert_eq!(data[9], 2);
            }
            if tag == b"IDAT" {
                idat.extend(data);
            }
            i += 12 + len;
        }
        (wh.0, wh.1, idat)
    }

    fn inflate_stored(z: &[u8]) -> Vec<u8> {
        let mut i = 2;
        let mut out = Vec::new();
        loop {
            let h = z[i];
            i += 1;
            assert_eq!((h >> 1) & 3, 0);
            let last = h & 1 == 1;
            let len = u16::from_le_bytes([z[i], z[i + 1]]) as usize;
            i += 4;
            out.extend(&z[i..i + len]);
            i += len;
            if last {
                break;
            }
        }
        out
    }
}
