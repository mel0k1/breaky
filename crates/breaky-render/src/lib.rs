use css::{Color, Value};
use layout::{BoxKind, LayoutBox, Rect};

pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        let w = width.max(1);
        let h = height.max(1);
        Canvas {
            width: w,
            height: h,
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

    fn blend(&mut self, x: i64, y: i64, c: Color) {
        let i = ((y as u32 * self.width + x as u32) * 3) as usize;
        let a = c.a as f32 / 255.0;
        for (k, ch) in [c.r, c.g, c.b].iter().enumerate() {
            self.pixels[i + k] =
                (*ch as f32 * a + self.pixels[i + k] as f32 * (1.0 - a)).round() as u8;
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
    let bb = root.dims.border_box();
    let height = ((bb.y + bb.height).max(max_bottom(root))
        .ceil()
        .max(1.0)) as u32;
    let mut canvas = Canvas::new(viewport_width.max(1.0) as u32, height);
    if let Some(c) = computed(root).and_then(|m| token_color(m, "background-color")) {
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
    paint_box(&mut canvas, root);
    canvas
}

fn max_bottom(b: &LayoutBox) -> f32 {
    let bb = b.dims.border_box();
    let mut m = bb.y + bb.height;
    for c in &b.children {
        m = m.max(max_bottom(c));
    }
    m
}

fn paint_box(canvas: &mut Canvas, b: &LayoutBox) {
    paint_bg(canvas, b);
    for child in &b.children {
        if !is_float(child) {
            paint_box(canvas, child);
        }
    }
    for child in &b.children {
        if is_float(child) {
            paint_box(canvas, child);
        }
    }
}

fn paint_bg(canvas: &mut Canvas, b: &LayoutBox) {
    let d = &b.dims;
    let bb = d.border_box();
    if let Some(c) = computed(b).and_then(|m| token_color(m, "background-color")) {
        canvas.fill(bb, c);
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
        BoxKind::Block(s) | BoxKind::Inline(s) => Some(&s.computed),
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
    use layout::layout;
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
