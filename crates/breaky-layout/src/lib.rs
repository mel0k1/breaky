use css::{Unit, Value};
use html::NodeKind;
use style::StyledNode;

const BASE_FONT: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dimensions {
    pub content: Rect,
    pub padding: Edges,
    pub border: Edges,
    pub margin: Edges,
}

impl Dimensions {
    pub fn padding_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left,
            y: self.content.y - self.padding.top,
            width: self.content.width + self.padding.left + self.padding.right,
            height: self.content.height + self.padding.top + self.padding.bottom,
        }
    }

    pub fn border_box(&self) -> Rect {
        let p = self.padding_box();
        Rect {
            x: p.x - self.border.left,
            y: p.y - self.border.top,
            width: p.width + self.border.left + self.border.right,
            height: p.height + self.border.top + self.border.bottom,
        }
    }

    pub fn margin_box(&self) -> Rect {
        let b = self.border_box();
        Rect {
            x: b.x - self.margin.left,
            y: b.y - self.margin.top,
            width: b.width + self.margin.left + self.margin.right,
            height: b.height + self.margin.top + self.margin.bottom,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum BoxKind<'a> {
    Block(&'a StyledNode<'a>),
    Inline(&'a StyledNode<'a>),
    Anonymous,
}

#[derive(Debug, PartialEq)]
pub struct LayoutBox<'a> {
    pub dims: Dimensions,
    pub kind: BoxKind<'a>,
    pub children: Vec<LayoutBox<'a>>,
}

struct Flow {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy)]
enum Family {
    Margin,
    Padding,
    Border,
}

#[derive(PartialEq)]
enum Display {
    Block,
    Inline,
    None,
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

pub fn layout<'a>(styled: &'a StyledNode<'a>, viewport_width: f32) -> Option<LayoutBox<'a>> {
    let mut root = build_root(styled)?;
    root.place(
        &Flow {
            x: 0.0,
            y: 0.0,
            width: viewport_width,
            height: 0.0,
        },
        &mut Vec::new(),
        BASE_FONT,
    );
    Some(root)
}

pub fn print_tree(tree: &LayoutBox, depth: usize) {
    let (label, tag) = match &tree.kind {
        BoxKind::Block(s) => ("block ", s.node.name().unwrap_or("#text")),
        BoxKind::Inline(s) => ("inline", s.node.name().unwrap_or("#text")),
        BoxKind::Anonymous => ("anon  ", "-"),
    };
    let d = &tree.dims.content;
    println!(
        "{}{} <{}> {},{} {}x{}",
        "  ".repeat(depth),
        label,
        tag,
        num(d.x),
        num(d.y),
        num(d.width),
        num(d.height)
    );
    for c in &tree.children {
        print_tree(c, depth + 1);
    }
}

fn num(x: f32) -> String {
    if (x - x.trunc()).abs() < 1e-4 {
        format!("{}", x as i64)
    } else {
        format!("{:.1}", x)
    }
}

fn build_root<'a>(styled: &'a StyledNode<'a>) -> Option<LayoutBox<'a>> {
    if matches!(styled.node.kind, NodeKind::Element { .. }) {
        return build_box(styled);
    }
    let mut kids = block_children(styled);
    match kids.len() {
        0 => None,
        1 => kids.pop(),
        _ => Some(LayoutBox {
            dims: Dimensions::default(),
            kind: BoxKind::Anonymous,
            children: kids,
        }),
    }
}

fn build_box<'a>(styled: &'a StyledNode<'a>) -> Option<LayoutBox<'a>> {
    match &styled.node.kind {
        NodeKind::Element { .. } => {
            if float_side(styled).is_some() {
                return Some(LayoutBox {
                    dims: Dimensions::default(),
                    kind: BoxKind::Block(styled),
                    children: block_children(styled),
                });
            }
            match display(styled) {
                Display::None => None,
                Display::Inline => Some(LayoutBox {
                    dims: Dimensions::default(),
                    kind: BoxKind::Inline(styled),
                    children: styled.children.iter().filter_map(build_inline).collect(),
                }),
                Display::Block => Some(LayoutBox {
                    dims: Dimensions::default(),
                    kind: BoxKind::Block(styled),
                    children: block_children(styled),
                }),
            }
        }
        NodeKind::Text(_) => Some(LayoutBox {
            dims: Dimensions::default(),
            kind: BoxKind::Inline(styled),
            children: Vec::new(),
        }),
        _ => None,
    }
}

fn build_inline<'a>(styled: &'a StyledNode<'a>) -> Option<LayoutBox<'a>> {
    match &styled.node.kind {
        NodeKind::Element { .. } if display(styled) != Display::None => Some(LayoutBox {
            dims: Dimensions::default(),
            kind: BoxKind::Inline(styled),
            children: styled.children.iter().filter_map(build_inline).collect(),
        }),
        NodeKind::Text(_) => Some(LayoutBox {
            dims: Dimensions::default(),
            kind: BoxKind::Inline(styled),
            children: Vec::new(),
        }),
        _ => None,
    }
}

fn float_side(styled: &StyledNode) -> Option<Side> {
    if !matches!(styled.node.kind, NodeKind::Element { .. }) {
        return None;
    }
    match styled.computed.get("float").and_then(|v| v.first()) {
        Some(Value::Keyword(k)) if k == "left" => Some(Side::Left),
        Some(Value::Keyword(k)) if k == "right" => Some(Side::Right),
        _ => None,
    }
}

fn block_children<'a>(styled: &'a StyledNode<'a>) -> Vec<LayoutBox<'a>> {
    let mut out: Vec<LayoutBox<'a>> = Vec::new();
    for c in &styled.children {
        let b = match build_box(c) {
            Some(b) => b,
            None => continue,
        };
        if matches!(b.kind, BoxKind::Inline(_)) {
            match out.last_mut() {
                Some(last) if matches!(last.kind, BoxKind::Anonymous) => last.children.push(b),
                _ => out.push(LayoutBox {
                    dims: Dimensions::default(),
                    kind: BoxKind::Anonymous,
                    children: vec![b],
                }),
            }
        } else {
            out.push(b);
        }
    }
    out
}

fn display(styled: &StyledNode) -> Display {
    if let Some(Value::Keyword(k)) = styled.computed.get("display").and_then(|v| v.first()) {
        if k == "inline" {
            return Display::Inline;
        }
        if k == "none" {
            return Display::None;
        }
    }
    match styled.node.name() {
        Some(n) => default_display(n),
        None => Display::Inline,
    }
}

fn default_display(name: &str) -> Display {
    match name {
        "a" | "b" | "i" | "s" | "u" | "em" | "strong" | "span" | "code" | "small" | "sub"
        | "sup" | "label" => Display::Inline,
        _ => Display::Block,
    }
}

impl<'a> LayoutBox<'a> {
    fn style(&self) -> Option<&'a StyledNode<'a>> {
        match &self.kind {
            BoxKind::Block(s) | BoxKind::Inline(s) => Some(s),
            BoxKind::Anonymous => None,
        }
    }

    fn element(&self) -> Option<&'a StyledNode<'a>> {
        match &self.kind {
            BoxKind::Block(s) | BoxKind::Inline(s)
                if matches!(s.node.kind, NodeKind::Element { .. }) =>
            {
                Some(s)
            }
            _ => None,
        }
    }

    fn font(&self, parent: f32) -> f32 {
        match self
            .style()
            .and_then(|s| s.computed.get("font-size"))
            .and_then(|v| v.first())
        {
            Some(Value::Length(n, Unit::Px)) => *n,
            Some(Value::Length(n, Unit::Em)) => n * parent,
            Some(Value::Keyword(k)) => k
                .trim_end_matches('%')
                .parse::<f32>()
                .map(|p| p * parent / 100.0)
                .unwrap_or(parent),
            _ => parent,
        }
    }

    fn line_height(&self, font: f32) -> f32 {
        match self
            .style()
            .and_then(|s| s.computed.get("line-height"))
            .and_then(|v| v.first())
        {
            Some(Value::Length(n, Unit::Px)) => *n,
            Some(Value::Length(n, Unit::Em)) => n * font,
            Some(Value::Keyword(k)) => k
                .trim_end_matches('%')
                .parse::<f32>()
                .map(|m| m * font)
                .unwrap_or(font * 1.2),
            _ => font * 1.2,
        }
    }

    fn num(&self, key: &str, font: f32, base: f32) -> Option<f32> {
        len_or(self.element()?.computed.get(key)?.first()?, font, base)
    }

    fn edges(&self, family: Family, font: f32, base: f32) -> Edges {
        let mut base4 = self.shorthand_base(family, font, base);
        let sides = ["top", "right", "bottom", "left"];
        let mut e = Edges::default();
        for (i, side) in sides.iter().enumerate() {
            let long = match family {
                Family::Margin => format!("margin-{}", side),
                Family::Padding => format!("padding-{}", side),
                Family::Border => format!("border-{}-width", side),
            };
            if let Some(v) = self.num(&long, font, base) {
                base4[i] = Some(v);
            } else if matches!(family, Family::Border) {
                let side_key = format!("border-{}", side);
                if let Some(v) = self
                    .element()
                    .and_then(|s| s.computed.get(&side_key))
                    .and_then(|vals| vals.iter().filter_map(|t| px(t, font)).next())
                {
                    base4[i] = Some(v);
                }
            }
            let v = base4[i].unwrap_or(0.0);
            match i {
                0 => e.top = v,
                1 => e.right = v,
                2 => e.bottom = v,
                _ => e.left = v,
            }
        }
        e
    }

    fn shorthand_base(&self, family: Family, font: f32, base: f32) -> [Option<f32>; 4] {
        let keys: &[&str] = match family {
            Family::Margin => &["margin"],
            Family::Padding => &["padding"],
            Family::Border => &["border", "border-width"],
        };
        for key in keys {
            let vals = match self.element().and_then(|s| s.computed.get(*key)) {
                Some(v) => v,
                None => continue,
            };
            let slots: Vec<Option<f32>> = if matches!(family, Family::Border) {
                vals.iter()
                    .filter_map(|t| len_or(t, font, base))
                    .map(Some)
                    .collect()
            } else {
                vals.iter().map(|t| len_or(t, font, base)).collect()
            };
            let v: [Option<f32>; 4] = match slots.len() {
                1 => [slots[0], slots[0], slots[0], slots[0]],
                2 => [slots[0], slots[1], slots[0], slots[1]],
                3 => [slots[0], slots[1], slots[2], slots[1]],
                4 => [slots[0], slots[1], slots[2], slots[3]],
                _ => continue,
            };
            return v;
        }
        [None; 4]
    }

    fn margin_sides(&self, font: f32, base: f32) -> [Option<f32>; 4] {
        let el = match self.element() {
            Some(e) => e,
            None => return [Some(0.0); 4],
        };
        let mut base4 = self.shorthand_base(Family::Margin, font, base);
        let sides = ["top", "right", "bottom", "left"];
        let mut any = el.computed.contains_key("margin");
        for (i, side) in sides.iter().enumerate() {
            if let Some(v) = self.num(&format!("margin-{}", side), font, base) {
                base4[i] = Some(v);
                any = true;
            }
        }
        if !any {
            base4 = [Some(0.0); 4];
        }
        base4
    }

    fn float_side(&self) -> Option<Side> {
        match &self.kind {
            BoxKind::Block(s) => float_side(s),
            _ => None,
        }
    }

    fn place(&mut self, flow: &Flow, floats: &mut Vec<(Side, Rect)>, parent_font: f32) {
        match self.kind {
            BoxKind::Inline(_) => self.place_inline(flow, floats, parent_font),
            _ => self.place_block(flow, floats, parent_font, false),
        }
    }

    fn place_block(
        &mut self,
        flow: &Flow,
        floats: &mut Vec<(Side, Rect)>,
        parent_font: f32,
        root: bool,
    ) {
        let font = self.font(parent_font);
        self.dims.margin = self.edges(Family::Margin, font, flow.width);
        self.dims.border = self.edges(Family::Border, font, flow.width);
        self.dims.padding = self.edges(Family::Padding, font, flow.width);
        self.resolve_width(flow.width, font);
        let explicit = self.num("height", font, flow.height);
        if let Some(h) = explicit {
            self.dims.content.height = h;
        }
        let can_top = !root && self.dims.border.top == 0.0 && self.dims.padding.top == 0.0;
        if can_top {
            if let Some(c) = self.children.iter().find(|c| c.float_side().is_none()) {
                let cmt = c.peek_margin_top(font, flow.width);
                self.dims.margin.top = self.dims.margin.top.max(cmt);
            }
        }
        self.dims.content.x =
            flow.x + self.dims.margin.left + self.dims.border.left + self.dims.padding.left;
        self.dims.content.y =
            flow.y + self.dims.margin.top + self.dims.border.top + self.dims.padding.top;
        self.stack_children(floats, font, explicit.is_none(), can_top, can_top);
    }

    fn stack_children(
        &mut self,
        floats: &mut Vec<(Side, Rect)>,
        font: f32,
        set_height: bool,
        can_bottom: bool,
        top_collapsed: bool,
    ) {
        if matches!(self.kind, BoxKind::Anonymous) {
            self.flow_lines(floats, font);
            return;
        }
        let mut prev: Option<(f32, f32)> = None;
        for child in &mut self.children {
            if let Some(s) = child.float_side() {
                let y = match prev {
                    Some((bb, pmb)) => bb + pmb,
                    None => self.dims.content.y,
                };
                let f = Flow {
                    x: self.dims.content.x,
                    y,
                    width: self.dims.content.width,
                    height: self.dims.content.height,
                };
                child.place_float(&f, floats, font, s);
                continue;
            }
            let cmt = child.peek_margin_top(font, self.dims.content.width);
            let (b, g) = match prev {
                Some((bb, pmb)) => (bb, collapse(pmb, cmt)),
                None => (
                    self.dims.content.y,
                    if top_collapsed { 0.0 } else { cmt },
                ),
            };
            let f = Flow {
                x: self.dims.content.x,
                y: b + g - cmt,
                width: self.dims.content.width,
                height: self.dims.content.height,
            };
            child.place(&f, floats, font);
            let bb = child.dims.border_box();
            prev = Some((bb.y + bb.height, child.dims.margin.bottom));
        }
        if set_height {
            self.dims.content.height = match prev {
                Some((bb, mb)) => {
                    if can_bottom
                        && self.dims.border.bottom == 0.0
                        && self.dims.padding.bottom == 0.0
                    {
                        self.dims.margin.bottom = self.dims.margin.bottom.max(mb);
                        bb - self.dims.content.y
                    } else {
                        bb + mb - self.dims.content.y
                    }
                }
                None => 0.0,
            };
        }
    }

    fn flow_lines(&mut self, floats: &mut Vec<(Side, Rect)>, font: f32) {
        let full_l = self.dims.content.x;
        let full_r = full_l + self.dims.content.width;
        let top = self.dims.content.y;
        let mut cx = full_l;
        let mut cy = top;
        let mut line = 0.0f32;
        let mut empty = true;
        for child in &mut self.children {
            let mw = child.measure_w(font, full_r - full_l);
            let mh = child.measure_h(font, full_r - full_l);
            let (l, r) = band(floats, cy, mh.max(line), full_l, full_r);
            if empty {
                cx = l;
                if mw > r - l {
                    cy = floats
                        .iter()
                        .map(|(_, rc)| rc.y + rc.height)
                        .fold(cy, f32::max);
                    cx = band(floats, cy, mh, full_l, full_r).0;
                }
            } else if cx + mw > r {
                cy += line;
                line = 0.0;
                let b2 = band(floats, cy, mh, full_l, full_r);
                cx = b2.0;
                if mw > b2.1 - b2.0 {
                    cy = floats
                        .iter()
                        .map(|(_, rc)| rc.y + rc.height)
                        .fold(cy, f32::max);
                    cx = band(floats, cy, mh, full_l, full_r).0;
                }
            }
            let f = Flow {
                x: cx,
                y: cy,
                width: 0.0,
                height: 0.0,
            };
            child.place(&f, floats, font);
            cx += mw;
            line = line.max(mh);
            empty = false;
        }
        self.dims.content.height = cy + line - top;
    }

    fn place_inline(
        &mut self,
        flow: &Flow,
        floats: &mut Vec<(Side, Rect)>,
        parent_font: f32,
    ) {
        let font = self.font(parent_font);
        self.dims.margin = self.edges(Family::Margin, font, flow.width);
        self.dims.border = self.edges(Family::Border, font, flow.width);
        self.dims.padding = self.edges(Family::Padding, font, flow.width);
        self.dims.content.x =
            flow.x + self.dims.margin.left + self.dims.border.left + self.dims.padding.left;
        self.dims.content.y =
            flow.y + self.dims.margin.top + self.dims.border.top + self.dims.padding.top;
        let w = self.num("width", font, flow.width);
        self.dims.content.width = w.unwrap_or(0.0);
        self.dims.content.height = self.line_height(font);
        let mut cx = self.dims.content.x;
        let mut cy = self.dims.content.y;
        let mut line = 0.0f32;
        let limit = w.map(|v| self.dims.content.x + v);
        for child in &mut self.children {
            let mw = child.measure_w(font, flow.width);
            let mh = child.measure_h(font, flow.width);
            if let Some(lim) = limit {
                if cx > self.dims.content.x && cx + mw > lim {
                    cx = self.dims.content.x;
                    cy += line;
                    line = 0.0;
                }
            }
            let f = Flow {
                x: cx,
                y: cy,
                width: mw,
                height: 0.0,
            };
            child.place(&f, floats, font);
            cx += mw;
            line = line.max(mh);
        }
        if w.is_none() && !self.children.is_empty() {
            self.dims.content.width = self
                .children
                .iter()
                .map(|c| {
                    let b = c.dims.border_box();
                    b.x + b.width
                })
                .fold(self.dims.content.x, f32::max)
                - self.dims.content.x;
        }
        self.dims.content.height = self
            .children
            .iter()
            .map(|c| {
                let b = c.dims.border_box();
                b.y + b.height
            })
            .fold(self.dims.content.y + self.dims.content.height, f32::max)
            - self.dims.content.y;
    }

    fn place_float(
        &mut self,
        flow: &Flow,
        floats: &mut Vec<(Side, Rect)>,
        parent_font: f32,
        side: Side,
    ) {
        let font = self.font(parent_font);
        self.dims.margin = self.edges(Family::Margin, font, flow.width);
        self.dims.border = self.edges(Family::Border, font, flow.width);
        self.dims.padding = self.edges(Family::Padding, font, flow.width);
        let w = self.num("width", font, flow.width);
        self.dims.content.width = w.unwrap_or_else(|| {
            let pref = self
                .children
                .iter()
                .map(|c| c.measure_w(font, flow.width))
                .fold(0.0f32, f32::max);
            let room = flow.width
                - self.dims.margin.left
                - self.dims.margin.right
                - self.dims.border.left
                - self.dims.border.right
                - self.dims.padding.left
                - self.dims.padding.right;
            pref.min(room.max(0.0))
        });
        let bb_w = self.dims.margin.left
            + self.dims.border.left
            + self.dims.padding.left
            + self.dims.content.width
            + self.dims.padding.right
            + self.dims.border.right
            + self.dims.margin.right;
        let (bx, by) = match side {
            Side::Left => (
                floats
                    .iter()
                    .filter(|(s, _)| *s == Side::Left)
                    .map(|(_, r)| r.x + r.width)
                    .fold(flow.x, f32::max),
                flow.y + self.dims.margin.top,
            ),
            Side::Right => (
                floats
                    .iter()
                    .filter(|(s, _)| *s == Side::Right)
                    .map(|(_, r)| r.x)
                    .fold(flow.x + flow.width, f32::min)
                    - bb_w,
                flow.y + self.dims.margin.top,
            ),
        };
        self.dims.content.x =
            bx + self.dims.margin.left + self.dims.border.left + self.dims.padding.left;
        self.dims.content.y = by + self.dims.border.top + self.dims.padding.top;
        let explicit = self.num("height", font, flow.height);
        if let Some(h) = explicit {
            self.dims.content.height = h;
        }
        self.stack_children(floats, font, explicit.is_none(), false, false);
        floats.push((side, self.dims.border_box()));
    }

    fn peek_margin_top(&self, parent_font: f32, base: f32) -> f32 {
        self.margin_sides(self.font(parent_font), base)[0].unwrap_or(0.0)
    }

    fn resolve_width(&mut self, avail: f32, font: f32) {
        let w = self.num("width", font, avail);
        let m = self.margin_sides(font, avail);
        let fixed = self.dims.border.left
            + self.dims.border.right
            + self.dims.padding.left
            + self.dims.padding.right;
        let rest = |used: f32| avail - fixed - used;
        let (left, right, width) = match (w, m[3], m[1]) {
            (None, a, b) => {
                let l = a.unwrap_or(0.0);
                let r = b.unwrap_or(0.0);
                (l, r, rest(l + r).max(0.0))
            }
            (Some(w), None, None) => {
                let half = rest(w) / 2.0;
                (half, half, w)
            }
            (Some(w), None, Some(r)) => (rest(w + r), r, w),
            (Some(w), Some(l), _) => (l, rest(w + l), w),
        };
        self.dims.margin.left = left;
        self.dims.margin.right = right;
        self.dims.content.width = width;
    }

    fn measure_w(&self, parent_font: f32, base: f32) -> f32 {
        let font = self.font(parent_font);
        let m = self.edges(Family::Margin, font, base);
        let b = self.edges(Family::Border, font, base);
        let p = self.edges(Family::Padding, font, base);
        let w = self.num("width", font, base).unwrap_or_else(|| {
            self.children
                .iter()
                .map(|c| c.measure_w(font, base))
                .sum::<f32>()
        });
        m.left + m.right + b.left + b.right + p.left + p.right + w
    }

    fn measure_h(&self, parent_font: f32, base: f32) -> f32 {
        let font = self.font(parent_font);
        let m = self.edges(Family::Margin, font, base);
        let b = self.edges(Family::Border, font, base);
        let p = self.edges(Family::Padding, font, base);
        m.top + m.bottom + b.top + b.bottom + p.top + p.bottom + self.line_height(font)
    }
}

fn px(v: &Value, font: f32) -> Option<f32> {
    match v {
        Value::Length(n, Unit::Px) => Some(*n),
        Value::Length(n, Unit::Em) => Some(n * font),
        _ => None,
    }
}

fn pct_val(v: &Value, base: f32) -> Option<f32> {
    match v {
        Value::Keyword(k) if k.ends_with('%') => k[..k.len() - 1]
            .parse::<f32>()
            .ok()
            .map(|p| p * base / 100.0),
        _ => None,
    }
}

fn len_or(v: &Value, font: f32, base: f32) -> Option<f32> {
    px(v, font).or_else(|| pct_val(v, base))
}

fn band(floats: &[(Side, Rect)], y: f32, h: f32, l0: f32, r0: f32) -> (f32, f32) {
    let mut l = l0;
    let mut r = r0;
    for (side, rc) in floats {
        if rc.y < y + h && rc.y + rc.height > y {
            match side {
                Side::Left => l = l.max(rc.x + rc.width),
                Side::Right => r = r.min(rc.x),
            }
        }
    }
    (l, r)
}

fn collapse(a: f32, b: f32) -> f32 {
    if a >= 0.0 && b >= 0.0 {
        a.max(b)
    } else if a <= 0.0 && b <= 0.0 {
        a.min(b)
    } else {
        a + b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::parse as parse_css;
    use html::parse as parse_html;
    use style::build as style_build;

    fn eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn tag<'a>(b: &'a LayoutBox<'a>) -> &'a str {
        match &b.kind {
            BoxKind::Block(s) | BoxKind::Inline(s) => s.node.name().unwrap_or("#text"),
            BoxKind::Anonymous => "-",
        }
    }

    fn find<'a>(b: &'a LayoutBox<'a>, name: &str) -> &'a LayoutBox<'a> {
        b.children
            .iter()
            .find(|c| tag(c) == name)
            .unwrap_or_else(|| panic!("missing {}", name))
    }

    #[test]
    fn block_stack() {
        let dom = parse_html("<html><div></div><div></div></html>");
        let sheet = parse_css("div { height: 10px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        assert!(eq(root.children[1].dims.content.y, 10.0));
        assert!(eq(root.dims.content.height, 20.0));
    }

    #[test]
    fn box_model_offsets() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { padding: 10px; border: 2px solid black; margin: 4px } p { margin: 5px; height: 5px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let p = find(div, "p");
        assert!(eq(div.dims.content.x, 16.0));
        assert!(eq(div.dims.content.y, 16.0));
        assert!(eq(p.dims.content.x, 21.0));
        assert!(eq(p.dims.content.y, 21.0));
        assert!(eq(p.dims.content.width, 158.0));
        assert!(eq(p.dims.content.height, 5.0));
    }

    #[test]
    fn anonymous_grouping() {
        let dom = parse_html("<html><span></span><div></div><span></span></html>");
        let sheet = parse_css("div { height: 5px } span { line-height: 0 }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        assert!(matches!(root.children[0].kind, BoxKind::Anonymous));
        assert!(matches!(root.children[1].kind, BoxKind::Block(_)));
        assert!(matches!(root.children[2].kind, BoxKind::Anonymous));
        assert!(matches!(
            root.children[0].children[0].kind,
            BoxKind::Inline(_)
        ));
        assert!(eq(root.children[2].dims.content.y, 5.0));
    }

    #[test]
    fn display_none() {
        let dom = parse_html("<html><div></div><p></p></html>");
        let sheet = parse_css("p { display: none }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        assert_eq!(root.children.len(), 1);
        assert_eq!(tag(&root.children[0]), "div");
    }

    #[test]
    fn inline_flow() {
        let dom = parse_html("<html><div><span></span><span></span></div></html>");
        let sheet = parse_css("div { width: 120px } span { width: 50px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let anon = &div.children[0];
        assert!(matches!(anon.kind, BoxKind::Anonymous));
        assert!(eq(anon.children[0].dims.content.x, 0.0));
        assert!(eq(anon.children[1].dims.content.x, 50.0));
        assert!(eq(anon.dims.content.height, 19.2));
    }

    #[test]
    fn inline_wrap() {
        let dom = parse_html("<html><div><span></span><span></span><span></span></div></html>");
        let sheet = parse_css("div { width: 120px } span { width: 50px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let anon = &div.children[0];
        assert!(eq(anon.children[2].dims.content.x, 0.0));
        assert!(eq(anon.children[2].dims.content.y, 19.2));
        assert!(eq(anon.dims.content.height, 38.4));
    }

    #[test]
    fn auto_width_fill() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { margin: 5px; padding: 10px } p { height: 3px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        let p = find(div, "p");
        assert!(eq(div.dims.content.width, 70.0));
        assert!(eq(div.dims.content.x, 15.0));
        assert!(eq(p.dims.content.width, 70.0));
    }

    #[test]
    fn auto_margins_center() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { width: 50px; margin: 0 auto }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.margin.left, 75.0));
        assert!(eq(div.dims.margin.right, 75.0));
        assert!(eq(div.dims.content.x, 75.0));
    }

    #[test]
    fn em_units() {
        let dom = parse_html("<html><div><p>hi</p></div></html>");
        let sheet = parse_css("div { font-size: 10px; padding: 2em }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.padding.top, 20.0));
        assert!(eq(div.dims.content.y, 20.0));
    }

    #[test]
    fn explicit_height_wins() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { height: 50px } p { height: 5px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.content.height, 50.0));
    }

    #[test]
    fn percent_width_padding() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { width: 50%; padding-left: 10% }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.content.width, 100.0));
        assert!(eq(div.dims.padding.left, 20.0));
        assert!(eq(div.dims.content.x, 20.0));
    }

    #[test]
    fn percent_margins() {
        let dom = parse_html("<html><div></div></html>");
        let sheet = parse_css("div { width: 40px; margin-left: 25% }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.margin.left, 50.0));
        assert!(eq(div.dims.content.x, 50.0));
    }

    #[test]
    fn percent_font_size() {
        let dom = parse_html("<html><div><span>hi</span></div></html>");
        let sheet = parse_css("div { font-size: 200% }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        let anon = &div.children[0];
        assert!(eq(anon.dims.content.height, 38.4));
    }

    #[test]
    fn percent_height_explicit_parent() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { height: 100px } p { height: 50% }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        let p = find(div, "p");
        assert!(eq(p.dims.content.height, 50.0));
    }

    #[test]
    fn float_left_line_avoid() {
        let dom = parse_html("<html><div class=\"f\"></div><p>hi</p></html>");
        let sheet = parse_css(".f { float: left; width: 50px; height: 30px } p { height: 20px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = &root.children[0];
        let fb = f.dims.border_box();
        assert!(eq(fb.x, 0.0));
        assert!(eq(fb.y, 0.0));
        assert!(eq(fb.width, 50.0));
        assert!(eq(fb.height, 30.0));
        let p = &root.children[1];
        assert!(eq(p.dims.content.x, 0.0));
        assert!(eq(p.dims.content.width, 200.0));
        let text = &p.children[0].children[0];
        assert!(eq(text.dims.content.x, 50.0));
    }

    #[test]
    fn float_right() {
        let dom = parse_html("<html><div class=\"f\"></div></html>");
        let sheet = parse_css(".f { float: right; width: 50px; height: 10px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = &root.children[0];
        assert!(eq(f.dims.border_box().x, 150.0));
        assert!(eq(f.dims.content.x, 150.0));
    }

    #[test]
    fn floats_stack() {
        let dom = parse_html("<html><div class=\"f\"></div><div class=\"f\"></div></html>");
        let sheet = parse_css(".f { float: left; width: 50px; height: 10px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        assert!(eq(root.children[0].dims.border_box().x, 0.0));
        assert!(eq(root.children[1].dims.border_box().x, 50.0));
    }

    #[test]
    fn wrap_below_float() {
        let dom = parse_html("<html><div class=\"f\"></div><p><span class=\"s\"></span></p></html>");
        let sheet = parse_css(".f { float: left; width: 150px; height: 30px } .s { width: 60px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = &root.children[1];
        let span = &p.children[0].children[0];
        assert!(eq(span.dims.content.x, 0.0));
        assert!(eq(span.dims.content.y, 30.0));
    }

    #[test]
    fn float_not_parent_height() {
        let dom = parse_html("<html><div class=\"f\"></div></html>");
        let sheet = parse_css(".f { float: left; width: 50px; height: 40px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        assert!(eq(root.dims.content.height, 0.0));
    }

    #[test]
    fn float_avoids_other_float() {
        let dom = parse_html(
            "<html><div class=\"f\"></div><p><span class=\"s\"></span></p></html>",
        );
        let sheet = parse_css(
            ".f { float: left; width: 150px; height: 30px } .s { width: 40px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = &root.children[1];
        let span = &p.children[0].children[0];
        assert!(eq(span.dims.content.x, 150.0));
        assert!(eq(span.dims.content.y, 0.0));
    }

    #[test]
    fn sibling_margins_collapse() {
        let dom = parse_html("<html><div id=\"a\"></div><div id=\"b\"></div></html>");
        let sheet = parse_css("#a { height: 5px; margin-bottom: 40px } #b { margin-top: 10px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let b = &root.children[1];
        assert!(eq(b.dims.content.y, 45.0));
    }

    #[test]
    fn parent_child_collapse() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { margin-top: 20px } p { margin-top: 10px; height: 4px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        let p = find(div, "p");
        assert!(eq(root.dims.content.y, 20.0));
        assert!(eq(div.dims.content.y, 20.0));
        assert!(eq(p.dims.content.y, 20.0));
    }

    #[test]
    fn no_collapse_with_padding() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { padding-top: 10px } p { margin-top: 6px; height: 2px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        let p = find(div, "p");
        assert!(eq(div.dims.content.y, 10.0));
        assert!(eq(p.dims.content.y, 16.0));
    }

    #[test]
    fn negative_margins_collapse() {
        let dom = parse_html("<html><div id=\"a\"></div><div id=\"b\"></div></html>");
        let sheet = parse_css("#a { height: 4px; margin-bottom: -5px } #b { margin-top: 10px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let b = &root.children[1];
        assert!(eq(b.dims.content.y, 9.0));
    }

    #[test]
    fn bottom_collapse() {
        let dom = parse_html("<html><div><p></p></div><div id=\"s\"></div></html>");
        let sheet = parse_css(
            "div { margin-bottom: 30px } p { height: 5px; margin-bottom: 12px } #s { height: 2px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let s = &root.children[1];
        assert!(eq(s.dims.content.y, 35.0));
    }

    #[test]
    fn border_side_shorthand() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { border-bottom: 4px solid black } p { height: 2px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.border.bottom, 4.0));
        assert!(eq(div.dims.border.top, 0.0));
        assert!(eq(div.dims.border.left, 0.0));
    }

    #[test]
    fn text_line_height() {
        let dom = parse_html("<html><p>hi</p></html>");
        let sheet = parse_css("p { font-size: 20px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let p = find(&root, "p");
        assert!(eq(p.dims.content.height, 24.0));
        let anon = &p.children[0];
        assert!(matches!(anon.kind, BoxKind::Anonymous));
        let text = &anon.children[0];
        assert!(matches!(text.kind, BoxKind::Inline(_)));
        assert_eq!(tag(text), "#text");
    }

    #[test]
    fn inline_padding_offset() {
        let dom = parse_html("<html><p><b>x</b></p></html>");
        let sheet = parse_css("b { padding: 2px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let b = find(anon, "b");
        assert!(eq(b.dims.content.x, 2.0));
        assert!(eq(b.dims.border_box().width, 4.0));
    }

    #[test]
    fn shorthand_sides() {
        let dom = parse_html("<html><div><p></p></div></html>");
        let sheet = parse_css("div { padding: 1px 2px 3px 4px } p { margin: 6px 8px; height: 2px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.padding.top, 1.0));
        assert!(eq(div.dims.padding.right, 2.0));
        assert!(eq(div.dims.padding.bottom, 3.0));
        assert!(eq(div.dims.padding.left, 4.0));
        let p = find(div, "p");
        assert!(eq(p.dims.content.x, 12.0));
    }
}
