use css::{Unit, Value};
use html::{Node, NodeKind};
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
    InlineBlock(&'a StyledNode<'a>),
    Flex(&'a StyledNode<'a>),
    Table(&'a StyledNode<'a>),
    TableRow(&'a StyledNode<'a>),
    TableCell(&'a StyledNode<'a>),
    Anonymous,
}

#[derive(Debug, PartialEq)]
pub struct LayoutBox<'a> {
    pub dims: Dimensions,
    pub kind: BoxKind<'a>,
    pub text: Option<&'a str>,
    pub marker: Option<&'a str>,
    pub marker_img: Option<&'a str>,
    pub children: Vec<LayoutBox<'a>>,
    pub abs: Vec<LayoutBox<'a>>,
    ws: WS,
    align: Align,
    strut: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WS {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Align {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Va {
    Baseline,
    Sub,
    Super,
    Top,
    Bottom,
    Middle,
    TextTop,
    TextBottom,
    Len(f32),
    Pct(f32),
}

struct Item {
    idx: usize,
    w: f32,
    above: f32,
    below: f32,
    va: Va,
    space: bool,
    atomic: bool,
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
    InlineBlock,
    ListItem,
    Table,
    TableRow,
    TableCell,
    RowGroup,
    TableCaption,
    Flex,
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
        BoxKind::InlineBlock(s) => ("inlbk ", s.node.name().unwrap_or("#text")),
        BoxKind::Flex(s) => ("flex ", s.node.name().unwrap_or("#text")),
        BoxKind::Table(s) => ("table", s.node.name().unwrap_or("#text")),
        BoxKind::TableRow(s) => ("trow ", s.node.name().unwrap_or("#text")),
        BoxKind::TableCell(s) => ("tcell", s.node.name().unwrap_or("#text")),
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
    for c in &tree.abs {
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
    let mut root_out: Vec<LayoutBox<'a>> = Vec::new();
    let (mut flow, mut sink) = build_node(styled, &[], &mut root_out, false);
    sink.extend(root_out);
    let mut root = match flow.len() {
        0 => return None,
        1 => flow.pop().unwrap(),
        _ => LayoutBox {
            dims: Dimensions::default(),
            kind: BoxKind::Anonymous,
            text: None,
            abs: Vec::new(),
            children: flow,
            marker: None,
            marker_img: None,
            ws: WS::Normal,
            align: Align::Left,
            strut: 0.0,
        },
    };
    root.abs.append(&mut sink);
    Some(root)
}

fn build_node<'a>(
    styled: &'a StyledNode<'a>,
    anc: &[&'a StyledNode<'a>],
    root_out: &mut Vec<LayoutBox<'a>>,
    inline_ctx: bool,
) -> (Vec<LayoutBox<'a>>, Vec<LayoutBox<'a>>) {
    match &styled.node.kind {
        NodeKind::Text(src) => (text_boxes(styled, src), Vec::new()),
        NodeKind::Element { .. } => {
            if display(styled) == Display::None {
                return (Vec::new(), Vec::new());
            }
            if styled.node.name() == Some("br") {
                return (vec![word_box(styled, "\n", WS::Normal)], Vec::new());
            }
            if !inline_ctx {
                let d = display(styled);
                if d == Display::TableRow {
                    let (row, sink) = build_row(styled, anc, root_out);
                    return (vec![row], sink);
                }
                if matches!(d, Display::Table | Display::RowGroup) {
                    return build_table(styled, anc, root_out);
                }
            }
            let pos = position_kw(styled);
            let oof = pos == "absolute" || pos == "fixed";
            let float = if oof || inline_ctx {
                None
            } else {
                float_side(styled)
            };
            let kind = if oof || float.is_some() {
                BoxKind::Block(styled)
            } else if inline_ctx {
                BoxKind::Inline(styled)
            } else {
                match display(styled) {
                    Display::Block | Display::ListItem | Display::TableCaption => {
                        BoxKind::Block(styled)
                    }
                    Display::Flex => BoxKind::Flex(styled),
                    Display::InlineBlock => BoxKind::InlineBlock(styled),
                    Display::TableCell => BoxKind::TableCell(styled),
                    _ => BoxKind::Inline(styled),
                }
            };
            let is_item = matches!(kind, BoxKind::Block(_)) && display(styled) == Display::ListItem;
            let marker_img = if is_item { list_image(styled) } else { None };
            let marker = if is_item && marker_img.is_none() {
                list_marker(styled, anc)
            } else {
                None
            };
            let kids_inline = matches!(kind, BoxKind::Inline(_));
            let mut child_anc: Vec<&StyledNode<'a>> = anc.to_vec();
            child_anc.push(styled);
            let mut flow: Vec<LayoutBox<'a>> = Vec::new();
            let mut my_abs: Vec<LayoutBox<'a>> = Vec::new();
            let mut sink: Vec<LayoutBox<'a>> = Vec::new();
            for c in &styled.children {
                let (cf, cs) = build_node(c, &child_anc, root_out, kids_inline);
                if pos != "static" {
                    my_abs.extend(cs);
                } else {
                    sink.extend(cs);
                }
                flow.extend(cf);
            }
            if oof {
                let b = LayoutBox {
                    dims: Dimensions::default(),
                    kind: BoxKind::Block(styled),
                    text: None,
                    marker,
                    marker_img,
                    abs: my_abs,
                    children: group_blocks(flow, Some(styled)),
                    ws: ws_of(styled),
                    align: Align::Left,
                    strut: 0.0,
                };
                if pos == "fixed" || !positioned_in(anc) {
                    root_out.push(b);
                } else {
                    sink.push(b);
                }
                (Vec::new(), sink)
            } else {
                let children = if kids_inline {
                    flow
                } else {
                    group_blocks(flow, Some(styled))
                };
                let b = LayoutBox {
                    dims: Dimensions::default(),
                    kind,
                    text: None,
                    marker,
                    marker_img,
                    abs: my_abs,
                    children,
                    ws: ws_of(styled),
                    align: Align::Left,
                    strut: 0.0,
                };
                (vec![b], sink)
            }
        }
        _ => {
            let mut flow: Vec<LayoutBox<'a>> = Vec::new();
            let mut sink: Vec<LayoutBox<'a>> = Vec::new();
            for c in &styled.children {
                let (cf, cs) = build_node(c, anc, root_out, false);
                sink.extend(cs);
                flow.extend(cf);
            }
            (group_blocks(flow, None), sink)
        }
    }
}

fn positioned_in(anc: &[&StyledNode]) -> bool {
    anc.iter().rev().any(|s| positioned(s))
}

fn position_kw(styled: &StyledNode) -> &'static str {
    if !matches!(styled.node.kind, NodeKind::Element { .. }) {
        return "static";
    }
    match styled.computed.get("position").and_then(|v| v.first()) {
        Some(Value::Keyword(k)) => match k.as_str() {
            "relative" => "relative",
            "absolute" => "absolute",
            "fixed" => "fixed",
            _ => "static",
        },
        _ => "static",
    }
}

fn positioned(styled: &StyledNode) -> bool {
    position_kw(styled) != "static"
}

fn text_boxes<'a>(styled: &'a StyledNode<'a>, src: &'a str) -> Vec<LayoutBox<'a>> {
    let ws = ws_of(styled);
    if matches!(ws, WS::Pre | WS::PreWrap) {
        let mut out: Vec<LayoutBox<'a>> = Vec::new();
        let mut first = true;
        for seg in src.split('\n') {
            if !first {
                out.push(word_box(styled, "\n", ws));
            }
            if matches!(ws, WS::Pre) {
                if !seg.is_empty() {
                    out.push(word_box(styled, seg, ws));
                }
            } else {
                for part in runs(seg) {
                    out.push(word_box(styled, part, ws));
                }
            }
            first = false;
        }
        return out;
    }
    let sep = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}');
    let mut out: Vec<LayoutBox<'a>> = Vec::new();
    let words: Vec<&str> = src.split(sep).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        if !src.is_empty() {
            out.push(word_box(styled, " ", ws));
        }
        return out;
    }
    if src.starts_with(sep) {
        out.push(word_box(styled, " ", ws));
    }
    let mut it = words.iter().peekable();
    while let Some(w) = it.next() {
        out.push(word_box(styled, w, ws));
        if it.peek().is_some() {
            out.push(word_box(styled, " ", ws));
        }
    }
    if src.ends_with(sep) {
        out.push(word_box(styled, " ", ws));
    }
    out
}

fn word_box<'a>(styled: &'a StyledNode<'a>, text: &'a str, ws: WS) -> LayoutBox<'a> {
    LayoutBox {
        dims: Dimensions::default(),
        kind: BoxKind::Inline(styled),
        text: Some(text),
        abs: Vec::new(),
        children: Vec::new(),
        marker: None,
        marker_img: None,
        ws,
        align: Align::Left,
        strut: 0.0,
    }
}

fn runs(seg: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = false;
    for (i, c) in seg.char_indices() {
        let sp = c == ' ' || c == '\t';
        if i > start && sp != in_space {
            out.push(&seg[start..i]);
            start = i;
        }
        in_space = sp;
    }
    if start < seg.len() {
        out.push(&seg[start..]);
    }
    out
}

fn ws_of(styled: &StyledNode) -> WS {
    match styled
        .computed
        .get("white-space")
        .and_then(|v| v.first())
    {
        Some(Value::Keyword(k)) => match k.as_str() {
            "nowrap" => WS::NoWrap,
            "pre" => WS::Pre,
            "pre-wrap" => WS::PreWrap,
            _ => WS::Normal,
        },
        _ => WS::Normal,
    }
}

fn align_of(styled: &StyledNode) -> Align {
    match styled.computed.get("text-align").and_then(|v| v.first()) {
        Some(Value::Keyword(k)) => match k.as_str() {
            "center" => Align::Center,
            "right" => Align::Right,
            "justify" => Align::Justify,
            _ => Align::Left,
        },
        _ => Align::Left,
    }
}

fn va_of(styled: &StyledNode, font: f32) -> Va {
    if let Some(v) = styled
        .computed
        .get("vertical-align")
        .and_then(|v| v.first())
    {
        return match v {
            Value::Keyword(k) => match k.as_str() {
                "sub" => Va::Sub,
                "super" => Va::Super,
                "top" => Va::Top,
                "bottom" => Va::Bottom,
                "middle" => Va::Middle,
                "text-top" => Va::TextTop,
                "text-bottom" => Va::TextBottom,
                _ => k
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .map(Va::Pct)
                    .unwrap_or(Va::Baseline),
            },
            Value::Length(n, Unit::Px) => Va::Len(*n),
            Value::Length(n, Unit::Em) => Va::Len(n * font),
            _ => Va::Baseline,
        };
    }
    match styled.node.name() {
        Some("sub") => Va::Sub,
        Some("sup") => Va::Super,
        _ => Va::Baseline,
    }
}

fn strut_of(styled: &StyledNode) -> f32 {
    let font = styled
        .computed
        .get("font-size")
        .and_then(|v| v.first())
        .and_then(|v| px(v, BASE_FONT))
        .unwrap_or(BASE_FONT);
    match styled.computed.get("line-height").and_then(|v| v.first()) {
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

fn node_attr<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    match &node.kind {
        NodeKind::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn list_marker<'a>(styled: &'a StyledNode<'a>, anc: &[&'a StyledNode<'a>]) -> Option<&'a str> {
    let list = anc.last();
    let kind = match styled.computed.get("list-style-type").and_then(|v| v.first()) {
        Some(Value::Keyword(k)) => Some(k.as_str()),
        _ => match list.and_then(|s| s.node.name()) {
            Some("ol") => Some("decimal"),
            Some(n) if n == "ul" || n == "menu" => Some("disc"),
            _ => None,
        },
    };
    match kind {
        Some("none") => return None,
        Some("disc") => return Some("\u{2022}"),
        Some("circle") => return Some("\u{25e6}"),
        Some("square") => return Some("\u{25aa}"),
        _ => {}
    }
    let parent = list?;
    let idx = parent
        .children
        .iter()
        .position(|s| std::ptr::eq(s, styled))
        .unwrap_or(0);
    let idx = parent.children[..idx]
        .iter()
        .filter(|s| s.node.name() == Some("li"))
        .count();
    let start = node_attr(parent.node, "start")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);
    Some(Box::leak(format!("{}.", start + idx as i64).into_boxed_str()))
}

fn caption_bottom(b: &LayoutBox) -> bool {
    b.style()
        .and_then(|s| s.computed.get("caption-side"))
        .and_then(|v| v.first())
        .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "bottom"))
}

fn list_image<'a>(styled: &'a StyledNode<'a>) -> Option<&'a str> {
    match styled
        .computed
        .get("list-style-image")
        .and_then(|v| v.first())
    {
        Some(Value::Url(u)) => Some(u.as_str()),
        _ => None,
    }
}

fn list_inside(styled: &StyledNode) -> bool {
    matches!(
        styled
            .computed
            .get("list-style-position")
            .and_then(|v| v.first()),
        Some(Value::Keyword(k)) if k == "inside"
    )
}

fn build_table<'a>(
    styled: &'a StyledNode<'a>,
    anc: &[&'a StyledNode<'a>],
    root_out: &mut Vec<LayoutBox<'a>>,
) -> (Vec<LayoutBox<'a>>, Vec<LayoutBox<'a>>) {
    let mut rows: Vec<LayoutBox<'a>> = Vec::new();
    let mut cells: Vec<LayoutBox<'a>> = Vec::new();
    let mut top: Vec<LayoutBox<'a>> = Vec::new();
    let mut bottom: Vec<LayoutBox<'a>> = Vec::new();
    let mut sink: Vec<LayoutBox<'a>> = Vec::new();
    let mut sources: Vec<&StyledNode<'a>> = Vec::new();
    for c in &styled.children {
        match c.node.name() {
            Some(n) if n == "thead" || n == "tbody" || n == "tfoot" => {
                for g in &c.children {
                    sources.push(g);
                }
            }
            _ => sources.push(c),
        }
    }
    for src in sources {
        let d = display(src);
        if d == Display::TableCaption {
            let (mut cf, cs) = build_node(src, anc, root_out, false);
            sink.extend(cs);
            if cf.iter().any(caption_bottom) {
                bottom.append(&mut cf);
            } else {
                top.append(&mut cf);
            }
        } else if d == Display::TableRow {
            flush_cells(&mut rows, &mut cells);
            let (row, cs) = build_row(src, anc, root_out);
            sink.extend(cs);
            rows.push(row);
        } else if d == Display::TableCell {
            let (cell, cs) = build_node(src, anc, root_out, false);
            sink.extend(cs);
            cells.extend(cell);
        } else if matches!(d, Display::Table | Display::RowGroup) {
            flush_cells(&mut rows, &mut cells);
            let (t, cs) = build_table(src, anc, root_out);
            sink.extend(cs);
            rows.extend(t);
        } else {
            let (cf, cs) = build_node(src, anc, root_out, false);
            sink.extend(cs);
            cells.extend(cf);
        }
    }
    flush_cells(&mut rows, &mut cells);
    top.extend(rows);
    top.extend(bottom);
    let b = LayoutBox {
        dims: Dimensions::default(),
        kind: BoxKind::Table(styled),
        text: None,
        marker: None,
        marker_img: None,
        abs: Vec::new(),
        children: top,
        ws: ws_of(styled),
        align: Align::Left,
        strut: 0.0,
    };
    (vec![b], sink)
}

fn build_row<'a>(
    styled: &'a StyledNode<'a>,
    anc: &[&'a StyledNode<'a>],
    root_out: &mut Vec<LayoutBox<'a>>,
) -> (LayoutBox<'a>, Vec<LayoutBox<'a>>) {
    let mut cells: Vec<LayoutBox<'a>> = Vec::new();
    let mut sink: Vec<LayoutBox<'a>> = Vec::new();
    for c in &styled.children {
        let (cf, cs) = build_node(c, anc, root_out, false);
        sink.extend(cs);
        if display(c) == Display::TableCell {
            cells.extend(cf);
        } else if !cf.is_empty() {
            cells.push(LayoutBox {
                dims: Dimensions::default(),
                kind: BoxKind::Anonymous,
                text: None,
                marker: None,
                marker_img: None,
                abs: Vec::new(),
                children: group_blocks(cf, None),
                ws: WS::Normal,
                align: Align::Left,
                strut: 0.0,
            });
        }
    }
    let row = LayoutBox {
        dims: Dimensions::default(),
        kind: BoxKind::TableRow(styled),
        text: None,
        marker: None,
        marker_img: None,
        abs: Vec::new(),
        children: cells,
        ws: ws_of(styled),
        align: Align::Left,
        strut: 0.0,
    };
    (row, sink)
}

fn flush_cells<'a>(rows: &mut Vec<LayoutBox<'a>>, cells: &mut Vec<LayoutBox<'a>>) {
    if cells.is_empty() {
        return;
    }
    rows.push(LayoutBox {
        dims: Dimensions::default(),
        kind: BoxKind::Anonymous,
        text: None,
        marker: None,
        marker_img: None,
        abs: Vec::new(),
        children: std::mem::take(cells),
        ws: WS::Normal,
        align: Align::Left,
        strut: 0.0,
    });
}

fn group_blocks<'a>(
    flow: Vec<LayoutBox<'a>>,
    styled: Option<&StyledNode<'a>>,
) -> Vec<LayoutBox<'a>> {
    let mut out: Vec<LayoutBox<'a>> = Vec::new();
    let mut run: Vec<LayoutBox<'a>> = Vec::new();
    for b in flow {
        if matches!(b.kind, BoxKind::Inline(_) | BoxKind::InlineBlock(_)) {
            run.push(b);
        } else {
            flush_run(&mut out, &mut run, styled);
            out.push(b);
        }
    }
    flush_run(&mut out, &mut run, styled);
    out
}

fn flush_run<'a>(
    out: &mut Vec<LayoutBox<'a>>,
    run: &mut Vec<LayoutBox<'a>>,
    styled: Option<&StyledNode<'a>>,
) {
    if run.iter().any(|b| b.text != Some(" ")) {
        out.push(LayoutBox {
            dims: Dimensions::default(),
            kind: BoxKind::Anonymous,
            text: None,
            abs: Vec::new(),
            children: std::mem::take(run),
            marker: None,
            marker_img: None,
            ws: styled.map(ws_of).unwrap_or(WS::Normal),
            align: styled.map(align_of).unwrap_or(Align::Left),
            strut: styled.map(strut_of).unwrap_or(0.0),
        });
    } else {
        run.clear();
    }
}

fn display(styled: &StyledNode) -> Display {
    if let Some(Value::Keyword(k)) = styled.computed.get("display").and_then(|v| v.first()) {
        return match k.as_str() {
            "inline" => Display::Inline,
            "inline-block" => Display::InlineBlock,
            "list-item" => Display::ListItem,
            "table" => Display::Table,
            "table-row" => Display::TableRow,
            "table-cell" => Display::TableCell,
            "table-row-group" | "table-header-group" | "table-footer-group" => Display::RowGroup,
            "table-caption" => Display::TableCaption,
            "flex" | "inline-flex" => Display::Flex,
            "none" => Display::None,
            _ => Display::Block,
        };
    }
    match styled.node.name() {
        Some(n) => default_display(n),
        None => Display::Inline,
    }
}

fn default_display(name: &str) -> Display {
    match name {
        "head" | "title" | "meta" | "link" | "style" | "script" | "noscript" | "template"
        | "col" | "colgroup" => Display::None,
        "a" | "b" | "i" | "s" | "u" | "em" | "strong" | "span" | "code" | "small" | "sub"
        | "sup" | "label" | "br" => Display::Inline,
        "li" => Display::ListItem,
        "table" => Display::Table,
        "tr" => Display::TableRow,
        "td" | "th" => Display::TableCell,
        "thead" | "tbody" | "tfoot" => Display::RowGroup,
        "caption" => Display::TableCaption,
        _ => Display::Block,
    }
}

impl<'a> LayoutBox<'a> {
    fn style(&self) -> Option<&'a StyledNode<'a>> {
        match &self.kind {
            BoxKind::Block(s)
            | BoxKind::Inline(s)
            | BoxKind::InlineBlock(s)
            | BoxKind::Flex(s)
            | BoxKind::Table(s)
            | BoxKind::TableRow(s)
            | BoxKind::TableCell(s) => Some(s),
            BoxKind::Anonymous => None,
        }
    }

    fn element(&self) -> Option<&'a StyledNode<'a>> {
        match &self.kind {
            BoxKind::Block(s)
            | BoxKind::Inline(s)
            | BoxKind::InlineBlock(s)
            | BoxKind::Flex(s)
            | BoxKind::Table(s)
            | BoxKind::TableRow(s)
            | BoxKind::TableCell(s)
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

    fn num_h(&self, key: &str, font: f32, w: f32, h: Option<f32>) -> Option<f32> {
        let v = self.element()?.computed.get(key)?.first()?;
        flex_len(v, font, w, h)
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

    fn is_space(&self) -> bool {
        self.text == Some(" ") && matches!(self.ws, WS::Normal | WS::NoWrap)
    }

    fn extent(&self, parent_font: f32, parent_lh: f32, base: f32) -> (f32, f32, Va, bool) {
        let font = self.font(parent_font);
        let a = font::asc(font);
        let d = font::desc(font);
        let lh = self.line_height(font);
        let hl = (lh - a - d) / 2.0;
        let m = self.edges(Family::Margin, font, base);
        let b = self.edges(Family::Border, font, base);
        let p = self.edges(Family::Padding, font, base);
        let va = self
            .element()
            .map(|s| va_of(s, font))
            .unwrap_or(Va::Baseline);
        let pa = font::asc(parent_font);
        let pd = font::desc(parent_font);
        let phl = (parent_lh - pa - pd) / 2.0;
        let shift = match va {
            Va::Baseline | Va::Top | Va::Bottom => 0.0,
            Va::Len(n) => -n,
            Va::Pct(k) => -k * lh / 100.0,
            Va::Sub => 0.25 * font,
            Va::Super => -0.25 * font,
            Va::Middle => (a - d) / 2.0 - font::x_height(parent_font) / 2.0,
            Va::TextTop => (hl + a) - (phl + pa),
            Va::TextBottom => (phl + pd) - (lh - hl - a),
        };
        if matches!(self.kind, BoxKind::InlineBlock(_)) {
            let above = m.top + b.top + p.top + hl + a - shift;
            let h = self.measure_h(parent_font, base);
            (above, h - above, va, true)
        } else {
            let above = m.top + b.top + p.top + hl + a - shift;
            let below = m.bottom + b.bottom + p.bottom + d + hl + shift;
            (above, below, va, false)
        }
    }

    fn wraps(&self) -> bool {
        match self.text {
            Some(_) => matches!(self.ws, WS::Normal | WS::PreWrap),
            None => true,
        }
    }

    fn place(&mut self, flow: &Flow, floats: &mut Vec<(Side, Rect)>, parent_font: f32) {
        match self.kind {
            BoxKind::Inline(_) => self.place_inline(flow, floats, parent_font),
            BoxKind::Table(_) => self.place_table(flow, floats, parent_font),
            BoxKind::Flex(_) => self.place_flex(flow, floats, parent_font),
            _ => self.place_block(flow, floats, parent_font, false),
        }
    }

    fn table_cols(&self, font: f32, base: f32) -> Vec<f32> {
        let mut cols: Vec<f32> = Vec::new();
        for row in &self.children {
            if matches!(row.kind, BoxKind::TableRow(_) | BoxKind::Anonymous) {
                for (i, cell) in row.children.iter().enumerate() {
                    let w = cell.max_content_w(font, base);
                    if i >= cols.len() {
                        cols.resize(i + 1, 0.0);
                    }
                    cols[i] = cols[i].max(w);
                }
            }
        }
        cols
    }

    fn place_table(&mut self, flow: &Flow, floats: &mut Vec<(Side, Rect)>, parent_font: f32) {
        let font = self.font(parent_font);
        self.dims.margin = self.edges(Family::Margin, font, flow.width);
        self.dims.border = self.edges(Family::Border, font, flow.width);
        self.dims.padding = self.edges(Family::Padding, font, flow.width);
        let collapse = matches!(
            self.style()
                .and_then(|s| s.computed.get("border-collapse"))
                .and_then(|v| v.first()),
            Some(Value::Keyword(k)) if k == "collapse"
        );
        let t = self.dims.border;
        let mut grids: Vec<Vec<Edges>> = Vec::new();
        if collapse {
            for row in &self.children {
                if matches!(row.kind, BoxKind::TableRow(_) | BoxKind::Anonymous) {
                    let mut g: Vec<Edges> = Vec::new();
                    for cell in &row.children {
                        g.push(cell.edges(Family::Border, cell.font(font), 0.0));
                    }
                    grids.push(g);
                }
            }
            let mut tb = Edges::default();
            for (k, g) in grids.iter().enumerate() {
                if g.is_empty() {
                    continue;
                }
                if k == 0 {
                    for e in g {
                        tb.top = tb.top.max(e.top);
                    }
                }
                if k + 1 == grids.len() {
                    for e in g {
                        tb.bottom = tb.bottom.max(e.bottom);
                    }
                }
                tb.left = tb.left.max(g[0].left);
                tb.right = tb.right.max(g[g.len() - 1].right);
            }
            self.dims.border = Edges {
                top: tb.top.max(t.top) / 2.0,
                right: tb.right.max(t.right) / 2.0,
                bottom: tb.bottom.max(t.bottom) / 2.0,
                left: tb.left.max(t.left) / 2.0,
            };
        }
        let ms = self.margin_sides(font, flow.width);
        let ml = ms[3].unwrap_or(0.0);
        let mr = ms[1].unwrap_or(0.0);
        self.dims.margin.left = ml;
        self.dims.margin.right = mr;
        let avail = (flow.width
            - ml
            - mr
            - self.dims.border.left
            - self.dims.border.right
            - self.dims.padding.left
            - self.dims.padding.right)
            .max(0.0);
        let mut cols = self.table_cols(font, avail);
        let ncols = cols.len();
        let sum: f32 = cols.iter().sum();
        let width = match self.num("width", font, flow.width) {
            Some(w) => {
                if ncols > 0 {
                    if w >= sum {
                        let extra = (w - sum) / ncols as f32;
                        for c in cols.iter_mut() {
                            *c += extra;
                        }
                    } else if sum > 0.0 {
                        let k = w / sum;
                        for c in cols.iter_mut() {
                            *c *= k;
                        }
                    }
                }
                w
            }
            None => {
                if sum > avail && sum > 0.0 {
                    let k = avail / sum;
                    for c in cols.iter_mut() {
                        *c *= k;
                    }
                    avail
                } else {
                    sum
                }
            }
        };
        self.dims.content.width = width;
        self.dims.content.x =
            flow.x + ml + self.dims.border.left + self.dims.padding.left;
        self.dims.content.y =
            flow.y + self.dims.margin.top + self.dims.border.top + self.dims.padding.top;
        let explicit = self.num("height", font, flow.height);
        if let Some(h) = explicit {
            self.dims.content.height = h;
        }
        let mut y = self.dims.content.y;
        let mut k = 0usize;
        for row in &mut self.children {
            if matches!(row.kind, BoxKind::TableRow(_) | BoxKind::Anonymous) {
                row.dims.content.x = self.dims.content.x;
                row.dims.content.y = y;
                row.dims.content.width = width;
                let mut x = self.dims.content.x;
                let mut row_h = 0.0f32;
                for (i, cell) in row.children.iter_mut().enumerate() {
                    let cw = if ncols > 0 { cols[i.min(ncols - 1)] } else { 0.0 };
                    let cfont = cell.font(font);
                    cell.dims.margin = cell.edges(Family::Margin, cfont, avail);
                    cell.dims.border = cell.edges(Family::Border, cfont, avail);
                    if collapse {
                        let g = &grids[k];
                        let cur = g[i];
                        let prev = if i > 0 { g[i - 1].right } else { t.left };
                        let next = if i + 1 < g.len() { g[i + 1].left } else { t.right };
                        let above = if k > 0 {
                            grids[k - 1].get(i).map(|e| e.bottom).unwrap_or(0.0)
                        } else {
                            t.top
                        };
                        let below = if k + 1 < grids.len() {
                            grids[k + 1].get(i).map(|e| e.top).unwrap_or(0.0)
                        } else {
                            t.bottom
                        };
                        let top = if k == 0 { cur.top.max(t.top) } else { cur.top.max(above) };
                        let bot = if k + 1 == grids.len() {
                            cur.bottom.max(t.bottom)
                        } else {
                            cur.bottom.max(below)
                        };
                        let left = if i == 0 { cur.left.max(t.left) } else { cur.left.max(prev) };
                        let right = if i + 1 == g.len() {
                            cur.right.max(t.right)
                        } else {
                            cur.right.max(next)
                        };
                        cell.dims.border = Edges {
                            top: top / 2.0,
                            right: right / 2.0,
                            bottom: bot / 2.0,
                            left: left / 2.0,
                        };
                    }
                    cell.dims.padding = cell.edges(Family::Padding, cfont, avail);
                    let fixed = cell.dims.border.left
                        + cell.dims.border.right
                        + cell.dims.padding.left
                        + cell.dims.padding.right;
                    let used = cell.dims.margin.left + cell.dims.margin.right + fixed;
                    cell.dims.content.width = (cw - used).max(0.0);
                    cell.dims.content.x = x
                        + cell.dims.margin.left
                        + cell.dims.border.left
                        + cell.dims.padding.left;
                    cell.dims.content.y = y
                        + cell.dims.margin.top
                        + cell.dims.border.top
                        + cell.dims.padding.top;
                    let ch = cell.num("height", cfont, avail);
                    if let Some(h) = ch {
                        cell.dims.content.height = h;
                    }
                    let mut local: Vec<(Side, Rect)> = Vec::new();
                    cell.stack_children(&mut local, cfont, ch.is_none(), false, false);
                    let mb = cell.dims.margin_box();
                    row_h = row_h.max(mb.height);
                    x += cw;
                }
                row.dims.content.height = row_h;
                k += 1;
            } else {
                let f = Flow {
                    x: self.dims.content.x,
                    y,
                    width,
                    height: 0.0,
                };
                row.place(&f, floats, font);
                row.apply_relative(font, width);
            }
            let rb = row.dims.margin_box();
            y = rb.y + rb.height;
        }
        if explicit.is_none() {
            self.dims.content.height = y - self.dims.content.y;
        }
        self.place_abs_children(font);
    }

    fn place_flex(&mut self, flow: &Flow, floats: &mut Vec<(Side, Rect)>, parent_font: f32) {
        let font = self.font(parent_font);
        self.dims.margin = self.edges(Family::Margin, font, flow.width);
        self.dims.border = self.edges(Family::Border, font, flow.width);
        self.dims.padding = self.edges(Family::Padding, font, flow.width);
        self.resolve_width(flow.width, font);
        let explicit_h = self.num("height", font, flow.height);
        if let Some(h) = explicit_h {
            self.dims.content.height = h;
        }
        self.dims.content.x =
            flow.x + self.dims.margin.left + self.dims.border.left + self.dims.padding.left;
        self.dims.content.y =
            flow.y + self.dims.margin.top + self.dims.border.top + self.dims.padding.top;
        let column = self
            .style()
            .and_then(|s| s.computed.get("flex-direction"))
            .and_then(|v| v.first())
            .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "column"));
        let (row_gap, col_gap) = self.flex_gaps(font, flow.width);
        if column {
            self.flex_column(floats, font, explicit_h.is_some(), row_gap, col_gap);
        } else {
            self.flex_row(floats, font, explicit_h.is_some(), row_gap, col_gap);
        }
        self.place_abs_children(font);
    }

    fn flex_gaps(&self, font: f32, base: f32) -> (f32, f32) {
        let el = match self.element() {
            Some(e) => e,
            None => return (0.0, 0.0),
        };
        let mut row = el
            .computed
            .get("row-gap")
            .and_then(|v| v.first())
            .and_then(|v| len_or(v, font, base));
        let mut col = el
            .computed
            .get("column-gap")
            .and_then(|v| v.first())
            .and_then(|v| len_or(v, font, base));
        if let Some(vals) = el.computed.get("gap") {
            let g: Vec<f32> = vals.iter().filter_map(|v| len_or(v, font, base)).collect();
            if g.len() == 1 {
                row = Some(g[0]);
                col = Some(g[0]);
            } else if g.len() >= 2 {
                row = Some(g[0]);
                col = Some(g[1]);
            }
        }
        (row.unwrap_or(0.0), col.unwrap_or(0.0))
    }

    fn flex_row(
        &mut self,
        floats: &mut Vec<(Side, Rect)>,
        font: f32,
        has_h: bool,
        row_gap: f32,
        col_gap: f32,
    ) {
        let inner_w = self.dims.content.width;
        let n = self.children.len();
        let h_opt = if has_h {
            Some(self.dims.content.height)
        } else {
            None
        };
        let mut bases: Vec<f32> = Vec::new();
        let mut grows: Vec<f32> = Vec::new();
        let mut shrinks: Vec<f32> = Vec::new();
        let mut fixed: Vec<(f32, f32)> = Vec::new();
        for item in &self.children {
            let f = item.font(font);
            let m = item.edges(Family::Margin, f, inner_w);
            let b = item.edges(Family::Border, f, inner_w);
            let p = item.edges(Family::Padding, f, inner_w);
            let fx = m.left + m.right + b.left + b.right + p.left + p.right;
            let fy = m.top + m.bottom + b.top + b.bottom + p.top + p.bottom;
            let basis = flex_basis(item, f, inner_w, Some(inner_w));
            let base = basis.unwrap_or_else(|| (item.max_content_w(f, inner_w) - fx).max(0.0));
            let (g, s) = flex_gs(item);
            bases.push(base);
            grows.push(g);
            shrinks.push(s);
            fixed.push((fx, fy));
        }
        let wrap = flex_wrap_of(self.style());
        let lines = match wrap {
            FlexWrap::NoWrap => vec![(0..n).collect::<Vec<usize>>()],
            _ => split_lines(&bases, &fixed, inner_w, col_gap),
        };
        let k = lines.len();
        for line in &lines {
            let mut bs: Vec<f32> = line.iter().map(|&i| bases[i]).collect();
            let gs: Vec<f32> = line.iter().map(|&i| grows[i]).collect();
            let ss: Vec<f32> = line.iter().map(|&i| shrinks[i]).collect();
            let fs: Vec<f32> = line.iter().map(|&i| fixed[i].0).collect();
            flex_distribute(
                &mut bs,
                &gs,
                &ss,
                &fs,
                inner_w,
                col_gap * (line.len().saturating_sub(1)) as f32,
            );
            for (j, &i) in line.iter().enumerate() {
                bases[i] = bs[j];
            }
        }
        let mut outer_h: Vec<f32> = vec![0.0; n];
        let mut top_y: Vec<f32> = vec![0.0; k];
        let mut line_h: Vec<f32> = vec![0.0; k];
        let mut cursor_y = self.dims.content.y;
        for (c, line) in lines.iter().enumerate() {
            top_y[c] = cursor_y;
            let used: f32 = line.iter().map(|&i| bases[i] + fixed[i].0).sum::<f32>()
                + col_gap * (line.len().saturating_sub(1)) as f32;
            let free = (inner_w - used).max(0.0);
            let (start, extra) = justify_of(self.style(), line.len(), free);
            let mut cursor = self.dims.content.x + start;
            for &i in line {
                let item = &mut self.children[i];
                let f = item.font(font);
                item.dims.margin = item.edges(Family::Margin, f, inner_w);
                item.dims.border = item.edges(Family::Border, f, inner_w);
                item.dims.padding = item.edges(Family::Padding, f, inner_w);
                item.dims.content.width = bases[i];
                item.dims.content.x = cursor
                    + item.dims.margin.left
                    + item.dims.border.left
                    + item.dims.padding.left;
                item.dims.content.y = top_y[c]
                    + item.dims.margin.top
                    + item.dims.border.top
                    + item.dims.padding.top;
                let ih = item.num_h("height", f, inner_w, h_opt).is_some();
                if let Some(h) = item.num_h("height", f, inner_w, h_opt) {
                    item.dims.content.height = h;
                }
                item.stack_children(floats, f, !ih, false, false);
                let mb = item.dims.margin_box();
                outer_h[i] = mb.height;
                line_h[c] = line_h[c].max(mb.height);
                cursor += bases[i] + fixed[i].0 + col_gap + extra;
            }
            cursor_y = top_y[c] + line_h[c] + row_gap;
        }
        let ac = align_content_of(self.style());
        let area: f32 = line_h.iter().sum::<f32>() + row_gap * (k.saturating_sub(1)) as f32;
        let free_h = if has_h {
            (self.dims.content.height - area).max(0.0)
        } else {
            0.0
        };
        if ac == AlignContent::Stretch && has_h {
            for h in line_h.iter_mut() {
                *h += free_h / k as f32;
            }
        }
        let area: f32 = line_h.iter().sum::<f32>() + row_gap * (k.saturating_sub(1)) as f32;
        let free_h = if has_h {
            (self.dims.content.height - area).max(0.0)
        } else {
            0.0
        };
        let step = match ac {
            AlignContent::Between if k > 1 => free_h / (k - 1) as f32,
            AlignContent::Around => free_h / k as f32,
            AlignContent::Evenly => free_h / (k + 1) as f32,
            _ => 0.0,
        };
        let lead = match ac {
            AlignContent::End => free_h,
            AlignContent::Center => free_h / 2.0,
            AlignContent::Around => step / 2.0,
            AlignContent::Evenly => step,
            _ => 0.0,
        };
        let mut prefix = vec![0.0; k];
        for c in 1..k {
            prefix[c] = prefix[c - 1] + line_h[c - 1] + row_gap;
        }
        let mut delta = vec![0.0; k];
        for c in 0..k {
            let ideal = self.dims.content.y + lead + step * c as f32 + prefix[c];
            delta[c] = if wrap == FlexWrap::WrapReverse {
                let band_top = self.dims.content.y + lead;
                band_top + area - (ideal - band_top) - line_h[c]
            } else {
                ideal
            } - top_y[c];
        }
        let cont = self.style();
        for (c, line) in lines.iter().enumerate() {
            for &i in line {
                self.children[i].shift(0.0, delta[c]);
            }
            for &i in line {
                let item = &mut self.children[i];
                let f = item.font(font);
                let ih_item = item.num_h("height", f, inner_w, h_opt).is_some();
                let mut align = align_self(item.element(), cont);
                if wrap == FlexWrap::WrapReverse {
                    align = match align {
                        AlignSelf::Start => AlignSelf::End,
                        AlignSelf::End => AlignSelf::Start,
                        other => other,
                    };
                }
                match align {
                    AlignSelf::Stretch if !ih_item => {
                        item.dims.content.height = (line_h[c] - fixed[i].1).max(0.0);
                    }
                    AlignSelf::End => item.shift(0.0, line_h[c] - outer_h[i]),
                    AlignSelf::Center => item.shift(0.0, (line_h[c] - outer_h[i]) / 2.0),
                    _ => {}
                }
                item.apply_relative(f, inner_w);
            }
        }
        if !has_h {
            self.dims.content.height = area;
        }
    }

    fn flex_column(
        &mut self,
        floats: &mut Vec<(Side, Rect)>,
        font: f32,
        has_h: bool,
        row_gap: f32,
        _col_gap: f32,
    ) {
        let inner_w = self.dims.content.width;
        let n = self.children.len();
        let h_opt = if has_h {
            Some(self.dims.content.height)
        } else {
            None
        };
        let mut bases: Vec<f32> = Vec::new();
        let mut grows: Vec<f32> = Vec::new();
        let mut shrinks: Vec<f32> = Vec::new();
        let mut fixed: Vec<(f32, f32)> = Vec::new();
        let cont = self.style();
        for i in 0..n {
            let item = &mut self.children[i];
            let f = item.font(font);
            item.dims.margin = item.edges(Family::Margin, f, inner_w);
            item.dims.border = item.edges(Family::Border, f, inner_w);
            item.dims.padding = item.edges(Family::Padding, f, inner_w);
            let m = item.dims.margin;
            let b = item.dims.border;
            let p = item.dims.padding;
            let fx = m.left + m.right + b.left + b.right + p.left + p.right;
            let fy = m.top + m.bottom + b.top + b.bottom + p.top + p.bottom;
            let align = align_self(item.element(), cont);
            let has_iw = item.num("width", f, inner_w).is_some();
            let nat = (item.max_content_w(f, inner_w) - fx).max(0.0);
            let w = if align == AlignSelf::Stretch && !has_iw {
                (inner_w - fx).max(0.0)
            } else {
                nat.min((inner_w - fx).max(0.0))
            };
            item.dims.content.width = w;
            item.dims.content.x = match align {
                AlignSelf::End => {
                    self.dims.content.x + (inner_w - fx - w).max(0.0) + m.left + b.left + p.left
                }
                AlignSelf::Center => {
                    self.dims.content.x + (inner_w - fx - w).max(0.0) / 2.0
                        + m.left
                        + b.left
                        + p.left
                }
                _ => self.dims.content.x + m.left + b.left + p.left,
            };
            item.dims.content.y = self.dims.content.y + m.top + b.top + p.top;
            let ih = item.num_h("height", f, inner_w, h_opt).is_some();
            item.stack_children(floats, f, !ih, false, false);
            let (g, s) = flex_gs(item);
            let base = flex_basis(item, f, inner_w, h_opt)
                .or_else(|| item.num_h("height", f, inner_w, h_opt))
                .unwrap_or(item.dims.content.height);
            bases.push(base);
            grows.push(g);
            shrinks.push(s);
            fixed.push((fx, fy));
        }
        if has_h {
            let cross_fixed: Vec<f32> = fixed.iter().map(|f| f.1).collect();
            flex_distribute(
                &mut bases,
                &grows,
                &shrinks,
                &cross_fixed,
                self.dims.content.height,
                row_gap * (n.saturating_sub(1)) as f32,
            );
        }
        let gaps = row_gap * (n.saturating_sub(1)) as f32;
        let used: f32 = (0..n).map(|i| bases[i] + fixed[i].1).sum::<f32>() + gaps;
        let inner_h = if has_h {
            self.dims.content.height
        } else {
            used
        };
        let free = (inner_h - used).max(0.0);
        let (start, extra) = justify_of(self.style(), n, free);
        let mut cursor = self.dims.content.y + start;
        for i in 0..n {
            let item = &mut self.children[i];
            let f = item.font(font);
            let target = cursor
                + item.dims.margin.top
                + item.dims.border.top
                + item.dims.padding.top;
            let dy = target - item.dims.content.y;
            item.dims.content.height = bases[i];
            item.shift(0.0, dy);
            item.apply_relative(f, inner_w);
            cursor += bases[i] + fixed[i].1 + row_gap + extra;
        }
        if !has_h {
            self.dims.content.height = cursor - row_gap - extra - self.dims.content.y;
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
        if self.element().map(list_inside).unwrap_or(false) {
            let mw = match (self.marker, self.marker_img) {
                (Some(m), _) => font::width(m, font),
                (None, Some(_)) => font * 0.5 + 2.0,
                _ => 0.0,
            };
            if mw > 0.0 {
                self.dims.content.x += mw;
                self.dims.content.width = (self.dims.content.width - mw).max(0.0);
            }
        }
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
        let inline_run = matches!(self.kind, BoxKind::Anonymous)
            && self
                .children
                .iter()
                .all(|c| matches!(c.kind, BoxKind::Inline(_) | BoxKind::InlineBlock(_)));
        if inline_run {
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
            child.apply_relative(font, self.dims.content.width);
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
        self.place_abs_children(font);
    }

    fn place_abs_children(&mut self, font: f32) {
        if self.abs.is_empty() {
            return;
        }
        let cb = self.dims.padding_box();
        for a in &mut self.abs {
            a.place_abs(cb, font);
        }
    }

    fn flow_lines(&mut self, floats: &mut Vec<(Side, Rect)>, font: f32) {
        let full_l = self.dims.content.x;
        let full_r = full_l + self.dims.content.width;
        let top = self.dims.content.y;
        let align = self.align;
        let parent_lh = if self.strut > 0.0 {
            self.strut
        } else {
            font * 1.2
        };
        let mut cy = top;
        let mut idx = 0;
        while idx < self.children.len() {
            let (end, items, wrapped) = self.collect(
                idx,
                floats,
                &mut cy,
                full_l,
                full_r,
                font,
                parent_lh,
            );
            if items.is_empty() {
                idx = end;
                continue;
            }
            let mut max_a = f32::NEG_INFINITY;
            let mut max_b = f32::NEG_INFINITY;
            let mut any = false;
            for it in &items {
                if matches!(it.va, Va::Top | Va::Bottom) {
                    continue;
                }
                any = true;
                max_a = max_a.max(it.above);
                max_b = max_b.max(it.below);
            }
            if !any {
                max_a = 0.0;
                max_b = 0.0;
            }
            for it in &items {
                let h = it.above + it.below;
                match it.va {
                    Va::Top => max_b = max_b.max(h - max_a),
                    Va::Bottom => max_a = max_a.max(h - max_b),
                    _ => {}
                }
            }
            let line_h = max_a + max_b;
            let (l, r) = band(floats, cy, line_h, full_l, full_r);
            let width: f32 = items.iter().map(|it| it.w).sum();
            let offset = match align {
                Align::Left | Align::Justify => 0.0,
                Align::Center => ((r - l) - width) / 2.0,
                Align::Right => (r - l) - width,
            };
            let spaces = items.iter().filter(|it| it.space).count();
            let gap = if align == Align::Justify && wrapped && spaces > 0 {
                ((r - l) - width).max(0.0) / spaces as f32
            } else {
                0.0
            };
            let baseline = cy + max_a;
            let mut pen = l + offset;
            let mut first = true;
            let mut restart_idx: Option<usize> = None;
            for it in &items {
                let i = it.idx;
                let y = match it.va {
                    Va::Top => cy,
                    Va::Bottom => cy + line_h - it.above - it.below,
                    _ => baseline - it.above,
                };
                if it.atomic {
                    let avail = (r - pen).max(0.0);
                    self.children[i].layout_atomic(pen, y, avail, floats, font);
                    let mb = self.children[i].dims.margin_box();
                    if !first && mb.x + mb.width > r {
                        cy += line_h;
                        let (l2, r2) =
                            band(floats, cy, it.above + it.below, full_l, full_r);
                        self.children[i].layout_atomic(
                            l2,
                            cy,
                            (r2 - l2).max(0.0),
                            floats,
                            font,
                        );
                        restart_idx = Some(i);
                        break;
                    }
                    pen = mb.x + mb.width;
                } else {
                    let f = Flow {
                        x: pen,
                        y,
                        width: (r - pen).max(0.0),
                        height: 0.0,
                    };
                    self.children[i].place(&f, floats, font);
                    pen += it.w + if it.space { gap } else { 0.0 };
                }
                self.children[i].apply_relative(font, full_r - full_l);
                first = false;
            }
            if let Some(i) = restart_idx {
                idx = i;
            } else {
                cy += line_h;
                idx = end;
            }
        }
        self.dims.content.height = cy - top;
    }

    fn collect(
        &self,
        start: usize,
        floats: &[(Side, Rect)],
        cy: &mut f32,
        full_l: f32,
        full_r: f32,
        font: f32,
        parent_lh: f32,
    ) -> (usize, Vec<Item>, bool) {
        let mut items: Vec<Item> = Vec::new();
        let mut width = 0.0;
        let mut est = 0.0f32;
        let mut prev_space = false;
        let mut wrapped = false;
        let mut j = start;
        while j < self.children.len() {
            let child = &self.children[j];
            if child.text == Some("\n") {
                j += 1;
                if items.is_empty() {
                    *cy += self.strut;
                    continue;
                }
                break;
            }
            let space = child.is_space();
            if space && (items.is_empty() || prev_space) {
                j += 1;
                continue;
            }
            let mw = child.measure_w(font, full_r - full_l);
            let (above, below, va, atomic) = child.extent(font, parent_lh, full_r - full_l);
            let band_h = est.max(above + below);
            let (l, r) = band(floats, *cy, band_h, full_l, full_r);
            if items.is_empty() {
                if mw > r - l {
                    *cy = floats
                        .iter()
                        .map(|(_, rc)| rc.y + rc.height)
                        .fold(*cy, f32::max);
                }
            } else if child.wraps() && width + mw > r - l {
                wrapped = true;
                break;
            }
            items.push(Item {
                idx: j,
                w: mw,
                above,
                below,
                va,
                space,
                atomic,
            });
            width += mw;
            prev_space = space;
            est = band_h;
            j += 1;
        }
        (j, items, wrapped)
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
        let w = self
            .num("width", font, flow.width)
            .or_else(|| self.text.map(|t| font::width(t, font)));
        self.dims.content.width = w.unwrap_or(0.0);
        self.dims.content.height = self.line_height(font);
        let mut cx = self.dims.content.x;
        let mut cy = self.dims.content.y;
        let mut line = 0.0f32;
        let mut line_start = true;
        let mut prev_space = false;
        let limit = w.map(|v| self.dims.content.x + v);
        for child in &mut self.children {
            if child.text == Some("\n") {
                cx = self.dims.content.x;
                cy += line;
                line = 0.0;
                line_start = true;
                prev_space = false;
                continue;
            }
            let space = child.is_space();
            if space && (line_start || prev_space) {
                continue;
            }
            if matches!(child.kind, BoxKind::InlineBlock(_)) {
                let avail = limit
                    .map(|lim| (lim - self.dims.content.x).max(0.0))
                    .unwrap_or(flow.width.max(0.0));
                child.layout_atomic(cx, cy, avail, floats, font);
                let mut mb = child.dims.margin_box();
                if !line_start && limit.map_or(false, |lim| mb.x + mb.width > lim) {
                    cx = self.dims.content.x;
                    cy += line;
                    line = 0.0;
                    child.layout_atomic(cx, cy, avail, floats, font);
                    mb = child.dims.margin_box();
                }
                cx = mb.x + mb.width;
                line = line.max(mb.height);
                line_start = false;
                prev_space = false;
                child.apply_relative(font, flow.width);
                continue;
            }
            let mw = child.measure_w(font, flow.width);
            let mh = child.measure_h(font, flow.width);
            if let Some(lim) = limit {
                if !line_start && child.wraps() && cx + mw > lim {
                    cx = self.dims.content.x;
                    cy += line;
                    line = 0.0;
                    line_start = true;
                    if space {
                        continue;
                    }
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
            line_start = false;
            prev_space = space;
            child.apply_relative(font, flow.width);
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
        self.place_abs_children(font);
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
        if let Some(t) = self.text {
            return font::width(t, font);
        }
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

    fn max_content_w(&self, font: f32, base: f32) -> f32 {
        if let Some(t) = self.text {
            return font::width(t, font);
        }
        let m = self.edges(Family::Margin, font, base);
        let b = self.edges(Family::Border, font, base);
        let p = self.edges(Family::Padding, font, base);
        let inner = match self.num("width", font, base) {
            Some(w) => w,
            None => match self.kind {
                BoxKind::Block(_)
                | BoxKind::InlineBlock(_)
                | BoxKind::TableCell(_) => self
                    .children
                    .iter()
                    .map(|c| c.max_content_w(font, base))
                    .fold(0.0f32, f32::max),
                _ => self
                    .children
                    .iter()
                    .map(|c| c.max_content_w(font, base))
                    .sum::<f32>(),
            },
        };
        m.left + m.right + b.left + b.right + p.left + p.right + inner
    }

    fn layout_atomic(
        &mut self,
        x: f32,
        y: f32,
        avail: f32,
        floats: &mut Vec<(Side, Rect)>,
        parent_font: f32,
    ) {
        let font = self.font(parent_font);
        self.dims.margin = self.edges(Family::Margin, font, avail);
        self.dims.border = self.edges(Family::Border, font, avail);
        self.dims.padding = self.edges(Family::Padding, font, avail);
        let fixed = self.dims.border.left
            + self.dims.border.right
            + self.dims.padding.left
            + self.dims.padding.right;
        let used = self.dims.margin.left + self.dims.margin.right + fixed;
        let room = (avail - used).max(0.0);
        let pref = (self.max_content_w(font, avail) - used).max(0.0);
        self.dims.content.width = self.num("width", font, avail).unwrap_or(pref.min(room));
        self.dims.content.x =
            x + self.dims.margin.left + self.dims.border.left + self.dims.padding.left;
        self.dims.content.y =
            y + self.dims.margin.top + self.dims.border.top + self.dims.padding.top;
        let explicit = self.num("height", font, avail);
        if let Some(h) = explicit {
            self.dims.content.height = h;
        }
        self.stack_children(floats, font, explicit.is_none(), false, false);
    }

    fn place_abs(&mut self, cb: Rect, parent_font: f32) {
        let font = self.font(parent_font);
        let base = cb.width;
        self.dims.margin = self.edges(Family::Margin, font, base);
        self.dims.border = self.edges(Family::Border, font, base);
        self.dims.padding = self.edges(Family::Padding, font, base);
        let ms = self.margin_sides(font, base);
        let ml = ms[3].unwrap_or(0.0);
        let mr = ms[1].unwrap_or(0.0);
        let fixed = self.dims.border.left
            + self.dims.border.right
            + self.dims.padding.left
            + self.dims.padding.right;
        let used = ml + mr + fixed;
        let room = (base - used).max(0.0);
        let pref = (self.max_content_w(font, base) - used).max(0.0);
        let w = self.num("width", font, base).unwrap_or(pref.min(room));
        let left = self.num("left", font, base);
        let right = self.num("right", font, base);
        let mb_w = ml + fixed + w + mr;
        let x_m = match (left, right) {
            (Some(l), _) => cb.x + l,
            (None, Some(r)) => cb.x + cb.width - r - mb_w,
            (None, None) => cb.x + (cb.width - mb_w) / 2.0,
        };
        self.dims.content.width = w;
        self.dims.content.x = x_m + ml + self.dims.border.left + self.dims.padding.left;
        self.dims.content.y = cb.y;
        let explicit = self.num("height", font, base);
        if let Some(h) = explicit {
            self.dims.content.height = h;
        }
        let mut floats: Vec<(Side, Rect)> = Vec::new();
        self.stack_children(&mut floats, font, explicit.is_none(), false, false);
        let top = self.num("top", font, base);
        let bottom = self.num("bottom", font, base);
        let h = self.dims.content.height;
        let mb_h = self.dims.margin.top
            + self.dims.border.top
            + self.dims.padding.top
            + h
            + self.dims.padding.bottom
            + self.dims.border.bottom
            + self.dims.margin.bottom;
        let y_m = match (top, bottom) {
            (Some(t), _) => cb.y + t,
            (None, Some(b)) => cb.y + cb.height - b - mb_h,
            (None, None) => cb.y,
        };
        let dy = y_m + self.dims.margin.top + self.dims.border.top + self.dims.padding.top
            - self.dims.content.y;
        self.shift(0.0, dy);
    }

    fn apply_relative(&mut self, font: f32, base: f32) {
        let skip = match self.style() {
            Some(s) => position_kw(s) != "relative",
            None => true,
        };
        if skip {
            return;
        }
        let l = self.num("left", font, base);
        let r = self.num("right", font, base);
        let t = self.num("top", font, base);
        let b = self.num("bottom", font, base);
        let dx = l.or(r.map(|v| -v)).unwrap_or(0.0);
        let dy = t.or(b.map(|v| -v)).unwrap_or(0.0);
        self.shift(dx, dy);
    }

    fn shift(&mut self, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        self.dims.content.x += dx;
        self.dims.content.y += dy;
        for c in &mut self.children {
            c.shift(dx, dy);
        }
        for c in &mut self.abs {
            c.shift(dx, dy);
        }
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

#[derive(Clone, Copy, PartialEq)]
enum AlignSelf {
    Stretch,
    Start,
    End,
    Center,
}

#[derive(Clone, Copy, PartialEq)]
enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

fn flex_wrap_of(styled: Option<&StyledNode>) -> FlexWrap {
    match styled
        .and_then(|s| s.computed.get("flex-wrap"))
        .and_then(|v| v.first())
    {
        Some(Value::Keyword(k)) if k == "wrap" => FlexWrap::Wrap,
        Some(Value::Keyword(k)) if k == "wrap-reverse" => FlexWrap::WrapReverse,
        _ => FlexWrap::NoWrap,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AlignContent {
    Stretch,
    Start,
    End,
    Center,
    Between,
    Around,
    Evenly,
}

fn align_content_of(styled: Option<&StyledNode>) -> AlignContent {
    match styled
        .and_then(|s| s.computed.get("align-content"))
        .and_then(|v| v.first())
    {
        Some(Value::Keyword(k)) => match k.as_str() {
            "flex-start" | "start" => AlignContent::Start,
            "flex-end" | "end" => AlignContent::End,
            "center" => AlignContent::Center,
            "space-between" => AlignContent::Between,
            "space-around" => AlignContent::Around,
            "space-evenly" => AlignContent::Evenly,
            _ => AlignContent::Stretch,
        },
        _ => AlignContent::Stretch,
    }
}

fn split_lines(bases: &[f32], fixed: &[(f32, f32)], inner: f32, gap: f32) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut used = 0.0;
    for i in 0..bases.len() {
        let w = bases[i] + fixed[i].0;
        if !cur.is_empty() && used + gap + w > inner {
            lines.push(std::mem::take(&mut cur));
            used = 0.0;
        }
        used += if cur.is_empty() { w } else { gap + w };
        cur.push(i);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn justify_of(styled: Option<&StyledNode>, n: usize, free: f32) -> (f32, f32) {
    let kw = styled
        .and_then(|s| s.computed.get("justify-content"))
        .and_then(|v| v.first());
    let k = match kw {
        Some(Value::Keyword(k)) => k.as_str(),
        _ => "flex-start",
    };
    match k {
        "flex-end" | "end" => (free, 0.0),
        "center" => (free / 2.0, 0.0),
        "space-between" => {
            if n > 1 {
                (0.0, free / (n - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        "space-around" => {
            let g = free / n as f32;
            (g / 2.0, g)
        }
        "space-evenly" => {
            let g = free / (n + 1) as f32;
            (g, g)
        }
        _ => (0.0, 0.0),
    }
}

fn align_self(
    item: Option<&StyledNode>,
    container: Option<&StyledNode>,
) -> AlignSelf {
    let own = item
        .and_then(|s| s.computed.get("align-self"))
        .and_then(|v| v.first())
        .filter(|v| !matches!(v, Value::Keyword(k) if k == "auto"));
    let kw = own.or_else(|| {
        container
            .and_then(|s| s.computed.get("align-items"))
            .and_then(|v| v.first())
    });
    match kw {
        Some(Value::Keyword(k)) => match k.as_str() {
            "flex-start" | "start" => AlignSelf::Start,
            "flex-end" | "end" => AlignSelf::End,
            "center" => AlignSelf::Center,
            _ => AlignSelf::Stretch,
        },
        _ => AlignSelf::Stretch,
    }
}

fn flex_num(v: &Value) -> Option<f32> {
    match v {
        Value::Keyword(k) => k.parse::<f32>().ok(),
        Value::Length(n, _) if *n == 0.0 => Some(0.0),
        _ => None,
    }
}

fn flex_gs(item: &LayoutBox) -> (f32, f32) {
    let el = match item.element() {
        Some(e) => e,
        None => return (0.0, 1.0),
    };
    let mut grow = el
        .computed
        .get("flex-grow")
        .and_then(|v| v.first())
        .and_then(flex_num);
    let mut shrink = el
        .computed
        .get("flex-shrink")
        .and_then(|v| v.first())
        .and_then(flex_num);
    if let Some(vals) = el.computed.get("flex") {
        let mut nums: Vec<f32> = Vec::new();
        for v in vals {
            if let Value::Keyword(k) = v {
                if k == "none" {
                    grow = Some(0.0);
                    shrink = Some(0.0);
                    continue;
                }
                if k == "auto" {
                    continue;
                }
            }
            match flex_num(v) {
                Some(x) => nums.push(x),
                None => break,
            }
        }
        if grow.is_none() {
            if let Some(&g) = nums.first() {
                grow = Some(g);
            }
        }
        if shrink.is_none() {
            if let Some(&s) = nums.get(1) {
                shrink = Some(s);
            }
        }
    }
    (grow.unwrap_or(0.0), shrink.unwrap_or(1.0))
}

fn flex_len(v: &Value, font: f32, w: f32, h: Option<f32>) -> Option<f32> {
    match v {
        Value::Keyword(k) if k.ends_with('%') => {
            let p = k[..k.len() - 1].parse::<f32>().ok()?;
            Some(p * h? / 100.0)
        }
        _ => len_or(v, font, w),
    }
}

fn flex_basis(item: &LayoutBox, font: f32, w: f32, pct: Option<f32>) -> Option<f32> {
    let el = item.element()?;
    let resolve = |v: &Value| -> Option<f32> {
        match v {
            Value::Keyword(k) if k.ends_with('%') => {
                let p = k[..k.len() - 1].parse::<f32>().ok()?;
                Some(p * pct? / 100.0)
            }
            _ => len_or(v, font, w),
        }
    };
    if let Some(v) = el.computed.get("flex-basis").and_then(|v| v.first()) {
        if matches!(v, Value::Keyword(k) if k == "auto") {
            return None;
        }
        return resolve(v);
    }
    let vals = el.computed.get("flex")?;
    let mut any_num = false;
    let mut basis: Option<&Value> = None;
    for v in vals {
        if let Value::Keyword(k) = v {
            if k == "none" || k == "auto" {
                return None;
            }
        }
        match flex_num(v) {
            Some(_) => any_num = true,
            None => basis = Some(v),
        }
    }
    match basis {
        Some(v) => resolve(v),
        None => {
            if any_num {
                Some(0.0)
            } else {
                None
            }
        }
    }
}

fn flex_distribute(
    bases: &mut [f32],
    grows: &[f32],
    shrinks: &[f32],
    fixed: &[f32],
    inner: f32,
    gaps: f32,
) {
    let n = bases.len();
    let used: f32 = (0..n).map(|i| bases[i] + fixed[i]).sum::<f32>() + gaps;
    let free = inner - used;
    let total_grow: f32 = grows.iter().sum();
    if free > 0.0 && total_grow > 0.0 {
        for i in 0..n {
            bases[i] += free * grows[i] / total_grow;
        }
        return;
    }
    if free < 0.0 {
        let total_shrink: f32 = shrinks.iter().sum();
        if total_shrink > 0.0 {
            let scaled: Vec<f32> = (0..n).map(|i| bases[i] * shrinks[i]).collect();
            let sum: f32 = scaled.iter().sum();
            if sum > 0.0 {
                for i in 0..n {
                    bases[i] = (bases[i] + free * scaled[i] / sum).max(0.0);
                }
            }
        }
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
            BoxKind::Block(s)
            | BoxKind::Inline(s)
            | BoxKind::InlineBlock(s)
            | BoxKind::Flex(s)
            | BoxKind::Table(s)
            | BoxKind::TableRow(s)
            | BoxKind::TableCell(s) => s.node.name().unwrap_or("#text"),
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
        assert!(eq(
            b.dims.border_box().width,
            4.0 + font::width("x", 16.0)
        ));
    }

    #[test]
    fn text_measured() {
        let dom = parse_html("<html><p>hi</p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let text = &anon.children[0];
        assert!(eq(text.dims.content.width, font::width("hi", 16.0)));
        assert!(eq(p.dims.content.height, 19.2));
    }

    #[test]
    fn text_scales_with_font() {
        let dom = parse_html("<html><p>hi</p></html>");
        let sheet = parse_css("p { font-size: 32px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let p = find(&root, "p");
        let text = &p.children[0].children[0];
        assert!(eq(text.dims.content.width, font::width("hi", 32.0)));
    }

    #[test]
    fn whitespace_collapses() {
        let dom = parse_html("<html><p>a  b</p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        assert_eq!(anon.children.len(), 3);
        assert_eq!(anon.children[0].text, Some("a"));
        assert_eq!(anon.children[1].text, Some(" "));
        assert_eq!(anon.children[2].text, Some("b"));
    }

    #[test]
    fn nbsp_does_not_collapse() {
        let dom = parse_html("<html><p>a\u{a0} b</p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        assert_eq!(anon.children[0].text, Some("a\u{a0}"));
        assert!(eq(
            anon.children[0].dims.content.width,
            font::width("a\u{a0}", 16.0)
        ));
    }

    #[test]
    fn pre_preserves_spaces() {
        let dom = parse_html("<html><pre>a  b</pre></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let pre = find(&root, "pre");
        let anon = &pre.children[0];
        assert_eq!(anon.children.len(), 1);
        assert_eq!(anon.children[0].text, Some("a  b"));
        assert!(eq(
            anon.children[0].dims.content.width,
            font::width("a  b", 16.0)
        ));
    }

    #[test]
    fn pre_line_breaks() {
        let dom = parse_html("<html><pre>ab\ncd</pre></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let pre = find(&root, "pre");
        let anon = &pre.children[0];
        assert_eq!(anon.children.len(), 3);
        let cd = &anon.children[2];
        assert_eq!(cd.text, Some("cd"));
        assert!(eq(cd.dims.content.y, 19.2));
        assert!(eq(anon.dims.content.height, 38.4));
    }

    #[test]
    fn pre_blank_line_uses_strut() {
        let dom = parse_html("<html><pre>a\n\nb</pre></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let pre = find(&root, "pre");
        let anon = &pre.children[0];
        let b = &anon.children[3];
        assert_eq!(b.text, Some("b"));
        assert!(eq(b.dims.content.y, 38.4));
    }

    #[test]
    fn pre_tab_width() {
        let dom = parse_html("<html><pre>a\tb</pre></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 100.0).unwrap();
        let pre = find(&root, "pre");
        let text = &pre.children[0].children[0];
        assert!(eq(
            text.dims.content.width,
            font::width("a", 16.0) + 4.0 * font::width(" ", 16.0) + font::width("b", 16.0)
        ));
    }

    #[test]
    fn text_align_center() {
        let dom = parse_html("<html><p>hi</p></html>");
        let sheet = parse_css("p { width: 100px; text-align: center }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let text = &p.children[0].children[0];
        let want = (100.0 - font::width("hi", 16.0)) / 2.0;
        assert!(eq(text.dims.content.x, want));
    }

    #[test]
    fn text_align_right() {
        let dom = parse_html("<html><p>hi</p></html>");
        let sheet = parse_css("p { width: 100px; text-align: right }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let text = &p.children[0].children[0];
        assert!(eq(text.dims.content.x, 100.0 - font::width("hi", 16.0)));
    }

    #[test]
    fn text_align_inherits() {
        let dom = parse_html("<html><div><b>hi</b></div></html>");
        let sheet = parse_css("div { width: 80px; text-align: center }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let b = find(&div.children[0], "b");
        let want = (80.0 - font::width("hi", 16.0)) / 2.0;
        assert!(eq(b.dims.content.x, want));
    }

    #[test]
    fn text_align_justify_spreads_spaces() {
        let dom = parse_html("<html><p>aa bb cc</p></html>");
        let sheet = parse_css("p { width: 50px; text-align: justify }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let bb = &anon.children[2];
        assert!(eq(bb.dims.content.x, font::width("aa", 16.0) + 10.0));
        let cc = &anon.children[4];
        assert!(eq(cc.dims.content.x, 0.0));
        assert!(eq(cc.dims.content.y, 19.2));
    }

    #[test]
    fn text_align_justify_skips_last_line() {
        let dom = parse_html("<html><p>aa bb</p></html>");
        let sheet = parse_css("p { width: 200px; text-align: justify }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let bb = &p.children[0].children[2];
        assert!(eq(bb.dims.content.x, font::width("aa", 16.0) + 6.0));
    }

    #[test]
    fn vertical_align_super() {
        let dom = parse_html("<html><p>x<b style=\"vertical-align:super\">y</b></p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let x = &anon.children[0];
        let b = find(anon, "b");
        assert!(eq(x.dims.content.y - b.dims.content.y, 4.0));
        assert!(eq(anon.dims.content.height, 23.2));
    }

    #[test]
    fn vertical_align_sub() {
        let dom = parse_html("<html><p>x<span style=\"vertical-align:sub\">y</span></p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let x = &anon.children[0];
        let span = find(anon, "span");
        assert!(eq(span.dims.content.y - x.dims.content.y, 4.0));
    }

    #[test]
    fn vertical_align_length_raises() {
        let dom = parse_html("<html><p>x<span style=\"vertical-align:3px\">y</span></p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let x = &anon.children[0];
        let span = find(anon, "span");
        assert!(eq(x.dims.content.y - span.dims.content.y, 3.0));
    }

    #[test]
    fn vertical_align_percent_of_line_height() {
        let dom = parse_html("<html><p>x<span style=\"vertical-align:50%\">y</span></p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let x = &anon.children[0];
        let span = find(anon, "span");
        assert!(eq(x.dims.content.y - span.dims.content.y, 9.6));
    }

    #[test]
    fn sup_tag_defaults_to_super() {
        let dom = parse_html("<html><p>x<sup>y</sup></p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let x = &anon.children[0];
        let sup = find(anon, "sup");
        assert!(eq(x.dims.content.y - sup.dims.content.y, 4.0));
    }

    #[test]
    fn vertical_align_bottom() {
        let dom = parse_html("<html><p>x<b style=\"line-height:20px; vertical-align:bottom\">y</b></p></html>");
        let sheet = parse_css("p { line-height: 40px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let b = find(anon, "b");
        assert!(eq(b.dims.content.y, 20.0));
        assert!(eq(anon.dims.content.height, 40.0));
    }

    #[test]
    fn vertical_align_top() {
        let dom = parse_html("<html><p>x<b style=\"line-height:40px; vertical-align:top\">y</b></p></html>");
        let sheet = parse_css("p { line-height: 20px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let b = find(anon, "b");
        assert!(eq(b.dims.content.y, 0.0));
        assert!(eq(anon.dims.content.height, 40.0));
    }

    #[test]
    fn vertical_align_text_top() {
        let dom = parse_html("<html><p>x<span style=\"line-height:20px; vertical-align:text-top\">y</span></p></html>");
        let sheet = parse_css("p { line-height: 40px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let x = &anon.children[0];
        let span = find(anon, "span");
        assert!(eq(span.dims.content.y, x.dims.content.y));
    }

    #[test]
    fn vertical_align_middle() {
        let dom = parse_html("<html><p>x<span style=\"vertical-align:middle\">y</span></p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let x = &anon.children[0];
        let span = find(anon, "span");
        assert!(eq(x.dims.content.y - span.dims.content.y, 1.0));
    }

    #[test]
    fn ul_bullet_marker() {
        let dom = parse_html("<html><ul><li>aa</li></ul></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let ul = find(&root, "ul");
        assert!(eq(ul.dims.padding.left, 40.0));
        let li = find(ul, "li");
        assert_eq!(li.marker, Some("\u{2022}"));
        assert!(eq(li.dims.content.x, 40.0));
    }

    #[test]
    fn ol_decimal_markers() {
        let dom = parse_html("<html><ol><li>a</li><li>b</li></ol></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let ol = find(&root, "ol");
        assert_eq!(ol.children[0].marker, Some("1."));
        assert_eq!(ol.children[1].marker, Some("2."));
        assert!(eq(ol.children[1].dims.content.y, 19.2));
    }

    #[test]
    fn ol_start_attribute() {
        let dom = parse_html("<html><ol start=\"5\"><li>a</li></ol></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let ol = find(&root, "ol");
        assert_eq!(ol.children[0].marker, Some("5."));
    }

    #[test]
    fn list_style_none() {
        let dom = parse_html("<html><ul><li>a</li></ul></html>");
        let sheet = parse_css("li { list-style-type: none }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let li = find(find(&root, "ul"), "li");
        assert_eq!(li.marker, None);
    }

    #[test]
    fn list_style_type_variants() {
        let dom = parse_html(
            "<html><ul style=\"list-style-type: square\"><li>a</li></ul><ol style=\"list-style-type: circle\"><li>b</li></ol></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let ul = find(&root, "ul");
        assert_eq!(find(ul, "li").marker, Some("\u{25aa}"));
        let ol = find(&root, "ol");
        assert_eq!(find(ol, "li").marker, Some("\u{25e6}"));
    }

    #[test]
    fn list_style_image_marker() {
        let dom = parse_html("<html><ul><li>a</li></ul></html>");
        let sheet = parse_css("ul { list-style-image: url(b.png) }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let li = find(find(&root, "ul"), "li");
        assert_eq!(li.marker, None);
        assert_eq!(li.marker_img, Some("b.png"));
    }

    #[test]
    fn list_style_image_none_keeps_glyph() {
        let dom = parse_html("<html><ul><li>a</li></ul></html>");
        let sheet = parse_css("li { list-style-image: none }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let li = find(find(&root, "ul"), "li");
        assert_eq!(li.marker, Some("\u{2022}"));
        assert_eq!(li.marker_img, None);
    }

    #[test]
    fn list_style_image_inside_indents() {
        let dom = parse_html(
            "<html><ul style=\"list-style-position: inside; list-style-image: url(b.png)\"><li>a</li></ul></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let li = find(find(&root, "ul"), "li");
        assert!(eq(li.dims.content.x, 50.0));
        assert!(eq(li.dims.content.width, 150.0));
    }

    #[test]
    fn list_inside_indents_content() {
        let dom = parse_html("<html><ul><li style=\"list-style-position: inside\">a</li></ul></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let li = find(find(&root, "ul"), "li");
        assert!(eq(
            li.dims.content.x,
            40.0 + font::width("\u{2022}", 16.0)
        ));
    }

    #[test]
    fn display_block_li_has_no_marker() {
        let dom = parse_html("<html><ul><li style=\"display: block\">a</li></ul></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let li = find(find(&root, "ul"), "li");
        assert_eq!(li.marker, None);
    }

    #[test]
    fn table_cell_geometry() {
        let dom = parse_html("<html><table><tr><td>a</td><td>b</td></tr></table></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        assert!(matches!(table.kind, BoxKind::Table(_)));
        let w = font::width("a", 16.0);
        assert!(eq(table.dims.content.width, w * 2.0));
        assert!(eq(table.dims.content.height, 19.2));
        let row = &table.children[0];
        let c1 = &row.children[0];
        let c2 = &row.children[1];
        assert!(matches!(c1.kind, BoxKind::TableCell(_)));
        assert!(eq(c1.dims.content.x, 0.0));
        assert!(eq(c1.dims.content.width, w));
        assert!(eq(c2.dims.content.x, w));
    }

    #[test]
    fn table_two_rows() {
        let dom = parse_html(
            "<html><table><tr><td>a</td></tr><tr><td>b</td></tr></table></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        assert_eq!(table.children.len(), 2);
        assert!(eq(table.children[1].dims.content.y, 19.2));
        assert!(eq(table.dims.content.height, 38.4));
    }

    #[test]
    fn table_explicit_width_distributes() {
        let dom = parse_html("<html><table style=\"width: 100px\"><tr><td>ab</td><td>cd</td></tr></table></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let row = &table.children[0];
        assert!(eq(row.children[0].dims.content.width, 50.0));
        assert!(eq(row.children[1].dims.content.x, 50.0));
        assert!(eq(table.dims.content.width, 100.0));
    }

    #[test]
    fn table_column_takes_max_cell() {
        let dom = parse_html(
            "<html><table><tr><td>i</td><td>a</td></tr><tr><td>wide</td><td>b</td></tr></table></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let w = font::width("wide", 16.0);
        assert!(eq(table.dims.content.width, w + font::width("a", 16.0)));
        assert!(eq(table.children[0].children[1].dims.content.x, w));
    }

    #[test]
    fn table_cell_box_model() {
        let dom = parse_html("<html><table><tr><td>a</td></tr></table></html>");
        let sheet = parse_css("td { padding: 2px; border: 1px solid #000000 }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let cell = &find(&root, "table").children[0].children[0];
        let w = font::width("a", 16.0);
        assert!(eq(cell.dims.content.x, 3.0));
        assert!(eq(cell.dims.content.width, w));
        assert!(eq(cell.dims.content.y, 3.0));
    }

    #[test]
    fn table_shrinks_to_available() {
        let dom = parse_html(
            "<html><table><tr><td>aaaa</td><td>bbbb</td></tr></table></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 50.0).unwrap();
        let table = find(&root, "table");
        assert!(eq(table.dims.content.width, 50.0));
        let row = &table.children[0];
        assert!(eq(row.children[0].dims.content.width, 25.0));
        assert!(eq(row.children[1].dims.content.x, 25.0));
    }

    #[test]
    fn table_tbody_flattens() {
        let dom = parse_html(
            "<html><table><tbody><tr><td>a</td></tr></tbody></table></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        assert_eq!(table.children.len(), 1);
        let cell = &table.children[0].children[0];
        assert!(matches!(cell.kind, BoxKind::TableCell(_)));
        assert!(eq(cell.dims.content.width, font::width("a", 16.0)));
    }

    #[test]
    fn table_anonymous_cell_for_text() {
        let dom = parse_html("<html><table><tr>text<td>b</td></tr></table></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let row = &find(&root, "table").children[0];
        assert_eq!(row.children.len(), 2);
        assert!(matches!(row.children[0].kind, BoxKind::Anonymous));
        let w = font::width("text", 16.0);
        assert!(eq(row.children[0].dims.content.width, w));
        assert!(eq(row.children[1].dims.content.x, w));
    }

    #[test]
    fn table_display_css_keywords() {
        let dom = parse_html(
            "<html><div style=\"display: table\"><p style=\"display: table-row\"><b style=\"display: table-cell\">x</b></p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(matches!(div.kind, BoxKind::Table(_)));
        let p = find(div, "p");
        assert!(matches!(p.kind, BoxKind::TableRow(_)));
        let b = find(p, "b");
        assert!(matches!(b.kind, BoxKind::TableCell(_)));
        assert!(eq(b.dims.content.width, font::width("x", 16.0)));
    }

    #[test]
    fn table_caption_above_rows() {
        let dom = parse_html(
            "<html><table><caption>hi</caption><tr><td>a</td></tr></table></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let cap = &table.children[0];
        assert_eq!(tag(cap), "caption");
        let w = font::width("a", 16.0);
        assert!(eq(cap.dims.content.y, 0.0));
        assert!(eq(cap.dims.content.width, w));
        assert!(eq(cap.dims.content.height, 19.2));
        let row = &table.children[1];
        assert!(eq(row.dims.content.y, 19.2));
        assert!(eq(table.dims.content.height, 38.4));
    }

    #[test]
    fn table_caption_side_bottom() {
        let dom = parse_html(
            "<html><table><caption style=\"caption-side: bottom\">hi</caption><tr><td>a</td></tr></table></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let row = &table.children[0];
        assert!(eq(row.dims.content.y, 0.0));
        let cap = &table.children[1];
        assert!(eq(cap.dims.content.y, 19.2));
        assert!(eq(table.dims.content.height, 38.4));
    }

    #[test]
    fn table_caption_display_keyword() {
        let dom = parse_html(
            "<html><div style=\"display: table\"><p style=\"display: table-caption\">c</p><p style=\"display: table-row\"><b style=\"display: table-cell\">x</b></p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let cap = &div.children[0];
        assert_eq!(tag(cap), "p");
        assert!(eq(cap.dims.content.height, 19.2));
        let row = &div.children[1];
        assert!(matches!(row.kind, BoxKind::TableRow(_)));
        assert!(eq(row.dims.content.y, 19.2));
    }

    #[test]
    fn table_collapse_shares_adjacent_border() {
        let dom = parse_html(
            "<html><table style=\"border-collapse: collapse\"><tr><td>a</td><td>b</td></tr></table></html>",
        );
        let sheet = parse_css("td { border: 2px solid #000000 }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let w = font::width("a", 16.0);
        let row = &table.children[0];
        let c1 = &row.children[0];
        let c2 = &row.children[1];
        assert!(eq(c1.dims.border.left, 1.0));
        assert!(eq(c1.dims.border.right, 1.0));
        assert!(eq(c1.dims.content.x, 2.0));
        assert!(eq(c2.dims.content.x, w + 6.0));
        assert!(eq(table.dims.border.left, 1.0));
        assert!(eq(table.dims.content.width, 2.0 * w + 8.0));
        assert!(eq(table.dims.content.height, 21.2));
    }

    #[test]
    fn table_collapse_takes_max_border() {
        let dom = parse_html(
            "<html><table style=\"border-collapse: collapse\"><tr><td class=\"w\">a</td><td>b</td></tr></table></html>",
        );
        let sheet = parse_css("td { border: 2px solid #000000 } .w { border: 4px solid #000000 }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let w = font::width("a", 16.0);
        let row = &table.children[0];
        let c1 = &row.children[0];
        let c2 = &row.children[1];
        assert!(eq(c1.dims.border.left, 2.0));
        assert!(eq(c1.dims.border.right, 2.0));
        assert!(eq(c2.dims.border.left, 2.0));
        assert!(eq(c2.dims.border.right, 1.0));
        assert!(eq(c2.dims.content.x, w + 12.0));
    }

    #[test]
    fn table_collapse_outer_edge() {
        let dom = parse_html(
            "<html><table style=\"border: 6px solid #000000; border-collapse: collapse\"><tr><td>a</td></tr></table></html>",
        );
        let sheet = parse_css("td { border: 2px solid #000000 }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let c1 = &table.children[0].children[0];
        assert!(eq(table.dims.border.left, 3.0));
        assert!(eq(table.dims.content.x, 3.0));
        assert!(eq(c1.dims.border.left, 3.0));
        assert!(eq(c1.dims.border.right, 3.0));
        assert!(eq(c1.dims.content.x, 6.0));
    }

    #[test]
    fn table_collapse_vertical_borders() {
        let dom = parse_html(
            "<html><table style=\"border-collapse: collapse\"><tr><td>a</td></tr><tr><td>b</td></tr></table></html>",
        );
        let sheet = parse_css("td { border: 2px solid #000000 }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let table = find(&root, "table");
        let c1 = &table.children[0].children[0];
        let c2 = &table.children[1].children[0];
        assert!(eq(c1.dims.border.bottom, 1.0));
        assert!(eq(c2.dims.border.top, 1.0));
        assert!(eq(c2.dims.content.y, 23.2));
        assert!(eq(table.dims.content.height, 42.4));
    }

    #[test]
    fn nowrap_keeps_one_line() {
        let dom = parse_html("<html><p>aa bb</p></html>");
        let sheet = parse_css("p { width: 30px; white-space: nowrap }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let bb = &anon.children[2];
        assert!(eq(bb.dims.content.x, font::width("aa", 16.0) + 6.0));
        assert!(eq(bb.dims.content.y, 0.0));
        assert!(eq(anon.dims.content.height, 19.2));
    }

    #[test]
    fn pre_wrap_breaks_and_preserves() {
        let dom = parse_html(
            "<html><p style=\"white-space:pre-wrap\">aa  bb  cc</p></html>",
        );
        let sheet = parse_css("p { width: 50px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        assert_eq!(anon.children.len(), 5);
        assert_eq!(anon.children[1].text, Some("  "));
        let cc = &anon.children[4];
        assert_eq!(cc.text, Some("cc"));
        assert!(eq(cc.dims.content.x, 0.0));
        assert!(eq(cc.dims.content.y, 38.4));
    }

    #[test]
    fn word_wrap() {
        let dom = parse_html("<html><p>Hello world</p></html>");
        let sheet = parse_css("p { width: 60px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        assert_eq!(anon.children.len(), 3);
        let world = &anon.children[2];
        assert!(eq(world.dims.content.x, 0.0));
        assert!(eq(world.dims.content.y, 19.2));
        assert!(eq(anon.dims.content.height, 38.4));
    }

    #[test]
    fn wrap_skips_leading_space() {
        let dom = parse_html("<html><p>aa bb cc</p></html>");
        let sheet = parse_css("p { width: 50px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let cc = &anon.children[4];
        assert_eq!(cc.text, Some("cc"));
        assert!(eq(cc.dims.content.x, 0.0));
        assert!(eq(cc.dims.content.y, 19.2));
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

    #[test]
    fn space_before_inline_kept() {
        let dom = parse_html("<html><p>ab <b>cd</b></p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let anon = &p.children[0];
        let b = find(anon, "b");
        let want = font::width("ab", 16.0) + font::width(" ", 16.0);
        assert!(eq(b.dims.content.x, want));
    }

    #[test]
    fn ws_only_run_between_blocks_dropped() {
        let dom = parse_html("<html><div>\n<p>a</p>\n<p>b</p>\n</div></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert_eq!(div.children.len(), 2);
        assert!(eq(find(div, "p").dims.content.y, 0.0));
    }

    #[test]
    fn inline_block_shrink_to_fit() {
        let dom = parse_html("<html><div><p style=\"display:inline-block\">hi</p></div></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(matches!(div.children[0].kind, BoxKind::Anonymous));
        let ib = find(&div.children[0], "p");
        assert!(matches!(ib.kind, BoxKind::InlineBlock(_)));
        assert!(eq(ib.dims.content.width, font::width("hi", 16.0)));
        assert!(eq(ib.dims.content.height, 19.2));
    }

    #[test]
    fn inline_block_explicit_width() {
        let dom = parse_html("<html><div><p style=\"display:inline-block\">hi</p></div></html>");
        let sheet = parse_css("p { width: 40px; height: 10px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let ib = find(&div.children[0], "p");
        assert!(eq(ib.dims.content.width, 40.0));
        assert!(eq(ib.dims.content.height, 10.0));
    }

    #[test]
    fn inline_block_side_by_side() {
        let dom = parse_html("<html><div><a style=\"display:inline-block\">aa</a> <a style=\"display:inline-block\">bb</a></div></html>");
        let sheet = parse_css("a { padding: 2px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let anon = &div.children[0];
        let a1 = find(anon, "a");
        let gap = font::width(" ", 16.0);
        let a2 = &anon.children[2];
        assert!(eq(
            a2.dims.content.x - a1.dims.content.x,
            a1.dims.margin_box().width + gap
        ));
        assert!(eq(a1.dims.content.height, 19.2));
    }

    #[test]
    fn inline_block_wraps_whole() {
        let dom = parse_html("<html><div><a style=\"display:inline-block\">aaaa</a> <a style=\"display:inline-block\">bbbb</a></div></html>");
        let sheet = parse_css("a { padding: 2px }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 60.0).unwrap();
        let div = find(&root, "div");
        let anon = &div.children[0];
        let a2 = &anon.children[2];
        assert!(eq(a2.dims.border_box().x, 0.0));
        assert!(eq(a2.dims.border_box().y, 23.2));
        assert!(eq(a2.dims.content.width, font::width("bbbb", 16.0)));
    }

    #[test]
    fn relative_offset() {
        let dom = parse_html("<html><div style=\"position:relative; left:5px; top:3px\"><p>hi</p></div></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.dims.content.x, 5.0));
        assert!(eq(div.dims.content.y, 3.0));
        let p = find(div, "p");
        assert!(eq(p.dims.content.y, 3.0));
    }

    #[test]
    fn absolute_against_positioned_ancestor() {
        let dom = parse_html("<html><div style=\"position:relative; height:40px\"><p style=\"position:absolute; top:10px; left:4px\">hi</p></div></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(div.children.is_empty());
        assert_eq!(div.abs.len(), 1);
        let p = &div.abs[0];
        assert!(eq(p.dims.content.x, 4.0));
        assert!(eq(p.dims.content.y, 10.0));
        assert!(eq(p.dims.content.width, font::width("hi", 16.0)));
        assert!(eq(div.dims.content.height, 40.0));
    }

    #[test]
    fn absolute_bottom_right() {
        let dom = parse_html("<html><div style=\"position:relative; height:40px\"><p style=\"position:absolute; right:6px; bottom:5px; width:30px; height:10px\">hi</p></div></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let p = &div.abs[0];
        assert!(eq(p.dims.content.x, 164.0));
        assert!(eq(p.dims.content.y, 25.0));
    }

    #[test]
    fn fixed_at_root() {
        let dom = parse_html("<html><p style=\"position:fixed; top:2px; left:3px\">hi</p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        assert_eq!(root.abs.len(), 1);
        let p = &root.abs[0];
        assert!(eq(p.dims.content.x, 3.0));
        assert!(eq(p.dims.content.y, 2.0));
    }

    #[test]
    fn absolute_no_positioned_ancestor() {
        let dom = parse_html("<html><div><p style=\"position:absolute; top:2px; left:3px\">hi</p></div></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(div.children.is_empty());
        assert!(div.abs.is_empty());
        assert_eq!(root.abs.len(), 1);
        let p = &root.abs[0];
        assert!(eq(p.dims.content.x, 3.0));
        assert!(eq(p.dims.content.y, 2.0));
    }

    #[test]
    fn flex_row_items() {
        let dom = parse_html(
            "<html><div style=\"display: flex\"><p>a</p><p>b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        let w = font::width("a", 16.0);
        let p1 = &div.children[0];
        let p2 = &div.children[1];
        assert!(eq(p1.dims.content.x, 0.0));
        assert!(eq(p1.dims.content.width, w));
        assert!(eq(p2.dims.content.x, w));
        assert!(eq(div.dims.content.height, 19.2));
    }

    #[test]
    fn flex_grow_fills_free_space() {
        let dom = parse_html(
            "<html><div style=\"display: flex\"><p style=\"flex: 1\">a</p><p style=\"flex: 1\">b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.width, 100.0));
        assert!(eq(div.children[1].dims.content.x, 100.0));
    }

    #[test]
    fn flex_justify_center() {
        let dom = parse_html(
            "<html><div style=\"display: flex; justify-content: center\"><p style=\"width: 50px\">a</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.x, 75.0));
    }

    #[test]
    fn flex_justify_between() {
        let dom = parse_html(
            "<html><div style=\"display: flex; justify-content: space-between\"><p style=\"width: 50px\">a</p><p style=\"width: 50px\">b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.x, 0.0));
        assert!(eq(div.children[1].dims.content.x, 150.0));
    }

    #[test]
    fn flex_justify_end() {
        let dom = parse_html(
            "<html><div style=\"display: flex; justify-content: flex-end\"><p style=\"width: 50px\">a</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.x, 150.0));
    }

    #[test]
    fn flex_column_stacks() {
        let dom = parse_html(
            "<html><div style=\"display: flex; flex-direction: column\"><p>a</p><p>b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.y, 0.0));
        assert!(eq(div.children[0].dims.content.width, 200.0));
        assert!(eq(div.children[1].dims.content.y, 19.2));
        assert!(eq(div.dims.content.height, 38.4));
    }

    #[test]
    fn flex_column_grow() {
        let dom = parse_html(
            "<html><div style=\"display: flex; flex-direction: column; height: 100px\"><p>a</p><p style=\"flex: 1\">b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[1].dims.content.y, 19.2));
        assert!(eq(div.children[1].dims.content.height, 80.8));
        assert!(eq(div.dims.content.height, 100.0));
    }

    #[test]
    fn flex_gap_separates_items() {
        let dom = parse_html(
            "<html><div style=\"display: flex; gap: 10px\"><p>a</p><p>b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[1].dims.content.x, font::width("a", 16.0) + 10.0));
    }

    #[test]
    fn flex_align_items_center() {
        let dom = parse_html(
            "<html><div style=\"display: flex; height: 60px; align-items: center\"><p>a</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.y, 20.4));
    }

    #[test]
    fn flex_align_items_stretch() {
        let dom = parse_html(
            "<html><div style=\"display: flex; height: 60px\"><p>a</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.height, 60.0));
    }

    #[test]
    fn flex_align_self_end() {
        let dom = parse_html(
            "<html><div style=\"display: flex; height: 60px; align-items: flex-start\"><p style=\"align-self: flex-end\">a</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.y, 40.8));
    }

    #[test]
    fn flex_shrinks_overflow() {
        let dom = parse_html(
            "<html><div style=\"display: flex\"><p style=\"width: 150px\">a</p><p style=\"width: 150px\">b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.width, 100.0));
        assert!(eq(div.children[1].dims.content.x, 100.0));
    }

    #[test]
    fn flex_shrink_proportional() {
        let dom = parse_html(
            "<html><div style=\"display: flex\"><p style=\"width: 150px\">a</p><p style=\"width: 150px; flex-shrink: 3\">b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.width, 125.0));
        assert!(eq(div.children[1].dims.content.width, 75.0));
    }

    #[test]
    fn flex_basis_length() {
        let dom = parse_html(
            "<html><div style=\"display: flex\"><p style=\"flex: 0 0 80px\">a</p><p>b</p></div></html>",
        );
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let div = find(&root, "div");
        assert!(eq(div.children[0].dims.content.width, 80.0));
        assert!(eq(div.children[1].dims.content.x, 80.0));
    }

    #[test]
    fn br_forces_line_break() {
        let dom = parse_html("<html><p>a<br>b</p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let run = &p.children[0];
        assert_eq!(run.children.len(), 3);
        assert!(eq(run.children[0].dims.content.y, 0.0));
        assert!(eq(run.children[2].dims.content.y, 19.2));
        assert!(eq(p.dims.content.height, 38.4));
    }

    #[test]
    fn br_double_makes_blank_line() {
        let dom = parse_html("<html><p>a<br><br>b</p></html>");
        let sheet = parse_css("");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let run = &p.children[0];
        assert_eq!(run.children.len(), 4);
        assert!(eq(run.children[3].dims.content.y, 38.4));
        assert!(eq(p.dims.content.height, 57.6));
    }

    #[test]
    fn br_display_none_ignored() {
        let dom = parse_html("<html><p>a<br>b</p></html>");
        let sheet = parse_css("br { display: none }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let p = find(&root, "p");
        let run = &p.children[0];
        assert_eq!(run.children.len(), 2);
        assert!(eq(run.children[1].dims.content.y, 0.0));
        assert!(eq(p.dims.content.height, 19.2));
    }

    #[test]
    fn flex_basis_pct_row() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; width: 200px } .a { flex-basis: 25% } .b { flex-basis: 75% }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.width, 50.0));
        assert!(eq(f.children[1].dims.content.width, 150.0));
    }

    #[test]
    fn flex_basis_pct_grows() {
        let dom = parse_html("<html><div class=\"f\"><div class=\"a\"></div></div></html>");
        let sheet = parse_css(".f { display: flex; width: 200px } .a { flex-basis: 25%; flex-grow: 1 }");
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.width, 200.0));
    }

    #[test]
    fn flex_basis_pct_column() {
        let dom = parse_html("<html><div class=\"f\"><div class=\"a\"></div></div></html>");
        let sheet = parse_css(
            ".f { display: flex; flex-direction: column; width: 100px; height: 100px } .a { flex-basis: 50% }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.height, 50.0));
    }

    #[test]
    fn flex_item_pct_height() {
        let dom = parse_html("<html><div class=\"f\"><div class=\"a\"></div></div></html>");
        let sheet = parse_css(
            ".f { display: flex; width: 100px; height: 80px; align-items: flex-start } .a { width: 20px; height: 50% }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.height, 40.0));
    }

    #[test]
    fn flex_item_pct_height_auto_container() {
        let dom = parse_html("<html><div class=\"f\"><div class=\"a\"></div></div></html>");
        let sheet = parse_css(
            ".f { display: flex; width: 100px; align-items: flex-start } .a { width: 20px; height: 50% }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.height, 0.0));
    }

    #[test]
    fn flex_shrink_pct_basis() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; width: 100px } .a { flex-basis: 60% } .b { flex-basis: 60% }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.width, 50.0));
        assert!(eq(f.children[1].dims.content.width, 50.0));
    }

    #[test]
    fn flex_wrap_two_lines() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; flex-wrap: wrap; width: 100px } .a { width: 60px; height: 20px } .b { width: 60px; height: 30px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.y, 0.0));
        assert!(eq(f.children[1].dims.content.x, 0.0));
        assert!(eq(f.children[1].dims.content.y, 20.0));
        assert!(eq(f.dims.content.height, 50.0));
    }

    #[test]
    fn flex_wrap_reverse_order() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; flex-wrap: wrap-reverse; width: 100px } .a { width: 60px; height: 20px } .b { width: 60px; height: 30px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.y, 30.0));
        assert!(eq(f.children[1].dims.content.y, 0.0));
        assert!(eq(f.dims.content.height, 50.0));
    }

    #[test]
    fn flex_wrap_gap_between_lines() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; flex-wrap: wrap; width: 100px; gap: 10px } .a { width: 60px; height: 20px } .b { width: 60px; height: 30px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[1].dims.content.y, 30.0));
        assert!(eq(f.dims.content.height, 60.0));
    }

    #[test]
    fn flex_align_content_stretch() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; flex-wrap: wrap; width: 100px; height: 100px } .a { width: 60px } .b { width: 60px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.height, 50.0));
        assert!(eq(f.children[1].dims.content.y, 50.0));
    }

    #[test]
    fn flex_align_content_end() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; flex-wrap: wrap; width: 100px; height: 100px; align-content: flex-end } .a { width: 60px; height: 20px } .b { width: 60px; height: 30px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.y, 50.0));
        assert!(eq(f.children[1].dims.content.y, 70.0));
    }

    #[test]
    fn flex_align_content_between() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; flex-wrap: wrap; width: 100px; height: 100px; align-content: space-between } .a { width: 60px; height: 20px } .b { width: 60px; height: 30px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[0].dims.content.y, 0.0));
        assert!(eq(f.children[1].dims.content.y, 70.0));
    }

    #[test]
    fn flex_wrap_three_items() {
        let dom = parse_html(
            "<html><div class=\"f\"><div class=\"a\"></div><div class=\"b\"></div><div class=\"c\"></div></div></html>",
        );
        let sheet = parse_css(
            ".f { display: flex; flex-wrap: wrap; width: 100px } .a, .b, .c { width: 40px; height: 20px }",
        );
        let styled = style_build(&dom, &sheet);
        let root = layout(&styled, 200.0).unwrap();
        let f = find(&root, "div");
        assert!(eq(f.children[1].dims.content.x, 40.0));
        assert!(eq(f.children[2].dims.content.x, 0.0));
        assert!(eq(f.children[2].dims.content.y, 20.0));
    }
}
