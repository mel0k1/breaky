use std::collections::HashMap;

use css::{Color, Rule, Selector, SimpleSelector, Stylesheet, Unit, Value};
use html::{Node, NodeKind};

const BASE_FONT: f32 = 16.0;

const INHERITED: [&str; 13] = [
    "color",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "line-height",
    "letter-spacing",
    "list-style-image",
    "list-style-position",
    "list-style-type",
    "text-align",
    "text-shadow",
    "white-space",
];

#[derive(Debug, PartialEq)]
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub computed: HashMap<String, Vec<Value>>,
    pub children: Vec<StyledNode<'a>>,
    pub pseudo: Option<(&'static str, String)>,
}

pub fn build<'a>(node: &'a Node, sheet: &Stylesheet) -> StyledNode<'a> {
    build_rec(node, sheet, &[], None, BASE_FONT, false)
}

pub fn build_selected<'a>(node: &'a Node, sheet: &Stylesheet) -> StyledNode<'a> {
    build_rec(node, sheet, &[], None, BASE_FONT, true)
}

pub fn collect_css(node: &Node, out: &mut String) {
    if let NodeKind::Element { name, .. } = &node.kind {
        if name == "style" {
            for c in &node.children {
                if let NodeKind::Text(t) = &c.kind {
                    out.push_str(t);
                    out.push('\n');
                }
            }
        }
    }
    for c in &node.children {
        collect_css(c, out);
    }
}

fn build_rec<'a>(
    node: &'a Node,
    sheet: &Stylesheet,
    ancestors: &[&'a Node],
    parent: Option<&HashMap<String, Vec<Value>>>,
    parent_font: f32,
    sel: bool,
) -> StyledNode<'a> {
    let mut computed = HashMap::new();
    let mut font = parent_font;
    match &node.kind {
        NodeKind::Element { .. } => {
            let mut matched: Vec<((usize, usize, usize), usize, &Rule)> = Vec::new();
            for (i, rule) in sheet.rules.iter().enumerate() {
                let spec = rule
                    .selectors
                    .iter()
                    .filter(|s| selector_matches(node, s, ancestors))
                    .map(|s| s.specificity())
                    .max();
                if let Some(spec) = spec {
                    matched.push((spec, i, rule));
                }
            }
            matched.sort_by_key(|(spec, i, _)| (*spec, *i));
            for (_, _, rule) in &matched {
                for d in &rule.declarations {
                    apply_decl(&mut computed, &d.name, d.value.clone());
                }
            }
            if let Some(s) = attr(node, "style") {
                for d in css::declarations(s) {
                    apply_decl(&mut computed, &d.name, d.value);
                }
            }
            if matches!(node.name(), Some("pre") | Some("textarea"))
                && !computed.contains_key("white-space")
            {
                computed.insert(
                    "white-space".to_string(),
                    vec![Value::Keyword("pre".to_string())],
                );
            }
            if matches!(node.name(), Some("ul") | Some("ol") | Some("menu"))
                && !computed.contains_key("padding-left")
            {
                computed.insert(
                    "padding-left".to_string(),
                    vec![Value::Length(40.0, Unit::Px)],
                );
            }
            if matches!(node.name(), Some("ul") | Some("menu"))
                && !computed.contains_key("list-style-type")
            {
                computed.insert(
                    "list-style-type".to_string(),
                    vec![Value::Keyword("disc".to_string())],
                );
            }
            if node.name() == Some("ol") && !computed.contains_key("list-style-type") {
                computed.insert(
                    "list-style-type".to_string(),
                    vec![Value::Keyword("decimal".to_string())],
                );
            }
            for key in INHERITED {
                if !computed.contains_key(key) {
                    if let Some(v) = parent.and_then(|m| m.get(key)) {
                        computed.insert(key.to_string(), v.clone());
                    }
                }
            }
            font = match computed.get("font-size").and_then(|v| v.first()) {
                Some(Value::Length(n, Unit::Px)) => *n,
                Some(Value::Length(n, Unit::Em)) => n * parent_font,
                Some(Value::Keyword(k)) => k
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .map(|p| p * parent_font / 100.0)
                    .unwrap_or(parent_font),
                _ => parent_font,
            };
            computed.insert(
                "font-size".to_string(),
                vec![Value::Length(font, Unit::Px)],
            );
        }
        NodeKind::Text(_) => {
            if let Some(m) = parent {
                computed = m.clone();
            }
            if sel {
                if let Some(pe) = ancestors.last() {
                    if matches!(pe.kind, NodeKind::Element { .. }) {
                        let decls = pseudo_rules(
                            pe,
                            sheet,
                            &ancestors[..ancestors.len() - 1],
                            "selection",
                        )
                        .unwrap_or_default();
                        apply_selection(&mut computed, &decls);
                    }
                }
            }
        }
        _ => {}
    }
    let mut chain: Vec<&Node> = ancestors.to_vec();
    chain.push(node);
    let mut children: Vec<StyledNode<'a>> = node
        .children
        .iter()
        .map(|c| build_rec(c, sheet, &chain, Some(&computed), font, sel))
        .collect();
    if matches!(node.kind, NodeKind::Element { .. }) {
        if let Some(b) = pseudo_node(node, sheet, ancestors, &computed, font, "before", sel) {
            children.insert(0, b);
        }
        if let Some(a) = pseudo_node(node, sheet, ancestors, &computed, font, "after", sel) {
            children.push(a);
        }
        if block_container(node, &computed) {
            apply_first_letter(node, sheet, ancestors, font, &mut children);
        }
    }
    StyledNode {
        node,
        computed,
        children,
        pseudo: None,
    }
}

pub fn family(computed: &HashMap<String, Vec<Value>>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for v in computed.get("font-family").into_iter().flatten() {
        match v {
            Value::Keyword(k) if k == "," => break,
            Value::Keyword(k) => parts.push(k.clone()),
            _ => {}
        }
    }
    parts.join(" ")
}

fn apply_decl(computed: &mut HashMap<String, Vec<Value>>, name: &str, value: Vec<Value>) {
    if name == "gap" {
        let mut vals = value.into_iter();
        if let Some(row) = vals.next() {
            computed.insert("row-gap".to_string(), vec![row.clone()]);
            computed.insert("column-gap".to_string(), vec![vals.next().unwrap_or(row)]);
        }
        return;
    }
    computed.insert(name.to_string(), value);
}

fn pseudo_of(s: &Selector) -> Option<&str> {
    let last = s.compounds.last()?.last()?;
    match last {
        SimpleSelector::Pseudo(p) => Some(p.as_str()),
        _ => None,
    }
}

fn pseudo_rules<'a>(
    node: &'a Node,
    sheet: &'a Stylesheet,
    ancestors: &[&'a Node],
    kind: &str,
) -> Option<Vec<&'a css::Declaration>> {
    let mut matched: Vec<((usize, usize, usize), usize, &Rule)> = Vec::new();
    for (i, rule) in sheet.rules.iter().enumerate() {
        let spec = rule
            .selectors
            .iter()
            .filter(|s| pseudo_of(s) == Some(kind))
            .filter(|s| {
                let n = s.compounds.len();
                let mut rest: Vec<Vec<SimpleSelector>> = s.compounds[..n - 1].to_vec();
                let lastc: Vec<SimpleSelector> = s.compounds[n - 1]
                    .iter()
                    .filter(|p| !matches!(p, SimpleSelector::Pseudo(_)))
                    .cloned()
                    .collect();
                if !lastc.is_empty() {
                    rest.push(lastc);
                }
                rest.is_empty() || chain_matches(node, ancestors, &rest)
            })
            .map(|s| s.specificity())
            .max();
        if let Some(spec) = spec {
            matched.push((spec, i, rule));
        }
    }
    if matched.is_empty() {
        return None;
    }
    matched.sort_by_key(|(spec, i, _)| (*spec, *i));
    Some(
        matched
            .iter()
            .flat_map(|(_, _, r)| r.declarations.iter())
            .collect(),
    )
}

fn pseudo_node<'a>(
    node: &'a Node,
    sheet: &Stylesheet,
    ancestors: &[&'a Node],
    parent: &HashMap<String, Vec<Value>>,
    parent_font: f32,
    kind: &'static str,
    sel: bool,
) -> Option<StyledNode<'a>> {
    let decls = pseudo_rules(node, sheet, ancestors, kind)?;
    let mut computed = HashMap::new();
    for d in &decls {
        apply_decl(&mut computed, &d.name, d.value.clone());
    }
    if sel {
        let sdecls = pseudo_rules(node, sheet, ancestors, "selection").unwrap_or_default();
        apply_selection(&mut computed, &sdecls);
    }
    for key in INHERITED {
        if !computed.contains_key(key) {
            if let Some(v) = parent.get(key) {
                computed.insert(key.to_string(), v.clone());
            }
        }
    }
    let font = match computed.get("font-size").and_then(|v| v.first()) {
        Some(Value::Length(n, Unit::Px)) => *n,
        Some(Value::Length(n, Unit::Em)) => n * parent_font,
        _ => parent_font,
    };
    computed.insert(
        "font-size".to_string(),
        vec![Value::Length(font, Unit::Px)],
    );
    let content = match computed.get("content").and_then(|v| v.first()) {
        Some(Value::Keyword(k)) if !k.is_empty() && k != "none" && k != "normal" => k.clone(),
        _ => return None,
    };
    Some(StyledNode {
        node,
        computed,
        children: Vec::new(),
        pseudo: Some((kind, content)),
    })
}

fn block_container(node: &Node, computed: &HashMap<String, Vec<Value>>) -> bool {
    let d = match computed.get("display").and_then(|v| v.first()) {
        Some(Value::Keyword(k)) => k.clone(),
        _ => match node.name() {
            Some("a" | "b" | "i" | "s" | "u" | "em" | "strong" | "span" | "code"
            | "small" | "sub" | "sup" | "label" | "br" | "img") => "inline".to_string(),
            Some("head" | "title" | "meta" | "link" | "style" | "script" | "noscript"
            | "template" | "col" | "colgroup") => "none".to_string(),
            Some("table") => "table".to_string(),
            Some("tr") => "table-row".to_string(),
            Some("td" | "th") => "table-cell".to_string(),
            Some("thead" | "tbody" | "tfoot") => "table-row-group".to_string(),
            Some("caption") => "table-caption".to_string(),
            _ => "block".to_string(),
        },
    };
    matches!(
        d.as_str(),
        "block"
            | "list-item"
            | "table-cell"
            | "table-caption"
            | "inline-block"
            | "flex"
            | "flow-root"
            | "inline-flex"
    )
}

fn apply_first_letter<'a>(
    node: &'a Node,
    sheet: &Stylesheet,
    ancestors: &[&'a Node],
    parent_font: f32,
    children: &mut Vec<StyledNode<'a>>,
) {
    let decls = match pseudo_rules(node, sheet, ancestors, "first-letter") {
        Some(d) => d,
        None => return,
    };
    insert_letter(children, &decls, parent_font);
}

fn insert_letter<'a>(
    children: &mut Vec<StyledNode<'a>>,
    decls: &[&css::Declaration],
    parent_font: f32,
) -> bool {
    for idx in 0..children.len() {
        if children[idx].pseudo.is_some() {
            continue;
        }
        match &children[idx].node.kind {
            NodeKind::Text(src) => {
                let (byte_i, ch) = match src.char_indices().find(|(_, c)| !c.is_whitespace()) {
                    Some(p) => p,
                    None => continue,
                };
                let letter = ch.to_string();
                let rest = src[byte_i + ch.len_utf8()..].to_string();
                let mut computed = children[idx].computed.clone();
                for d in decls {
                    apply_decl(&mut computed, &d.name, d.value.clone());
                }
                let font = match computed.get("font-size").and_then(|v| v.first()) {
                    Some(Value::Length(n, Unit::Px)) => *n,
                    Some(Value::Length(n, Unit::Em)) => n * parent_font,
                    _ => parent_font,
                };
                computed.insert(
                    "font-size".to_string(),
                    vec![Value::Length(font, Unit::Px)],
                );
                let letter_node = StyledNode {
                    node: children[idx].node,
                    computed,
                    children: Vec::new(),
                    pseudo: Some(("first-letter", letter)),
                };
                children[idx].pseudo = Some(("text", rest));
                children.insert(idx, letter_node);
                return true;
            }
            _ => {
                if insert_letter(&mut children[idx].children, decls, parent_font) {
                    return true;
                }
            }
        }
    }
    false
}

fn apply_selection(computed: &mut HashMap<String, Vec<Value>>, decls: &[&css::Declaration]) {
    for d in decls {
        match d.name.as_str() {
            "color" => {
                computed.insert("::selection-color".to_string(), d.value.clone());
            }
            "background-color" => {
                computed.insert("::selection-background".to_string(), d.value.clone());
            }
            "background" => {
                if let Some(v) = d.value.iter().find_map(|v| match v {
                    Value::Color(c) => Some(vec![Value::Color(*c)]),
                    _ => None,
                }) {
                    computed.insert("::selection-background".to_string(), v);
                }
            }
            _ => {}
        }
    }
}

fn font_px(computed: &HashMap<String, Vec<Value>>) -> f32 {
    match computed.get("font-size").and_then(|v| v.first()) {
        Some(Value::Length(n, _)) => *n,
        _ => BASE_FONT,
    }
}

fn px_of(v: &Value, font: f32) -> f32 {
    match v {
        Value::Length(n, Unit::Px) => *n,
        Value::Length(n, Unit::Em) => n * font,
        _ => 0.0,
    }
}

fn ease_of(v: &Value) -> Option<[f32; 4]> {
    match v {
        Value::Bezier(b) => Some(*b),
        Value::Keyword(k) => match k.as_str() {
            "linear" => Some([0.0, 0.0, 1.0, 1.0]),
            "ease" => Some([0.25, 0.1, 0.25, 1.0]),
            "ease-in" => Some([0.42, 0.0, 1.0, 1.0]),
            "ease-out" => Some([0.0, 0.0, 0.58, 1.0]),
            "ease-in-out" => Some([0.42, 0.0, 0.58, 1.0]),
            _ => None,
        },
        _ => None,
    }
}

fn transition_items(m: &HashMap<String, Vec<Value>>) -> Vec<(String, f32, f32, [f32; 4])> {
    let mut props: Vec<Option<String>> = Vec::new();
    let mut durs: Vec<Option<f32>> = Vec::new();
    let mut eases: Vec<Option<[f32; 4]>> = Vec::new();
    let mut delays: Vec<Option<f32>> = Vec::new();
    if let Some(vals) = m.get("transition") {
        for g in css::groups(vals) {
            let mut prop = None;
            let mut dur = None;
            let mut ease = None;
            let mut delay = None;
            for v in g {
                match v {
                    Value::Time(s) => {
                        if dur.is_none() {
                            dur = Some(*s);
                        } else if delay.is_none() {
                            delay = Some(*s);
                        }
                    }
                    Value::Bezier(_) => ease = ease_of(v),
                    Value::Keyword(k) => match k.as_str() {
                        "none" => {}
                        _ => {
                            if ease.is_none() {
                                if let Some(b) = ease_of(v) {
                                    ease = Some(b);
                                    continue;
                                }
                            }
                            if prop.is_none() {
                                prop = Some(k.clone());
                            }
                        }
                    },
                    _ => {}
                }
            }
            props.push(prop);
            durs.push(dur);
            eases.push(ease);
            delays.push(delay);
        }
    }
    let pick = |key: &str| -> Option<Vec<&Value>> {
        m.get(key).map(|vals| {
            css::groups(vals)
                .into_iter()
                .filter_map(|g| g.first())
                .collect()
        })
    };
    if let Some(list) = pick("transition-property") {
        props = list.iter().map(|v| match v {
            Value::Keyword(k) => Some(k.clone()),
            _ => None,
        }).collect();
    }
    if let Some(list) = pick("transition-duration") {
        durs = list.iter().map(|v| match v {
            Value::Time(s) => Some(*s),
            _ => None,
        }).collect();
    }
    if let Some(list) = pick("transition-timing-function") {
        eases = list.iter().map(|v| ease_of(v)).collect();
    }
    if let Some(list) = pick("transition-delay") {
        delays = list.iter().map(|v| match v {
            Value::Time(s) => Some(*s),
            _ => None,
        }).collect();
    }
    let n = props.len();
    let at = |v: &Vec<Option<String>>, i: usize| -> String {
        match v.get(i % v.len().max(1)).and_then(|x| x.clone()) {
            Some(k) => k,
            None => "all".to_string(),
        }
    };
    (0..n)
        .filter(|i| props[*i].as_deref() != Some("none"))
        .map(|i| {
            (
                at(&props, i),
                durs.get(i % durs.len().max(1)).copied().flatten().unwrap_or(0.0),
                delays.get(i % delays.len().max(1)).copied().flatten().unwrap_or(0.0),
                eases
                    .get(i % eases.len().max(1))
                    .copied()
                    .flatten()
                    .unwrap_or([0.25, 0.1, 0.25, 1.0]),
            )
        })
        .collect()
}

fn easing(b: [f32; 4], x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let cx = 3.0 * b[0];
    let bx = 3.0 * (b[2] - b[0]) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * b[1];
    let by = 3.0 * (b[3] - b[1]) - cy;
    let ay = 1.0 - cy - by;
    let sx = |s: f32| ((ax * s + bx) * s + cx) * s;
    let sy = |s: f32| ((ay * s + by) * s + cy) * s;
    let dx = |s: f32| (3.0 * ax * s + 2.0 * bx) * s + cx;
    let mut s = x;
    for _ in 0..8 {
        let e = sx(s) - x;
        if e.abs() < 1e-6 {
            return sy(s);
        }
        let d = dx(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= e / d;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    s = x;
    for _ in 0..20 {
        let e = sx(s);
        if (e - x).abs() < 1e-6 {
            break;
        }
        if e < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    sy(s)
}

fn mix_color(a: &Color, b: &Color, k: f32) -> Color {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color {
        r: mix(a.r, b.r),
        g: mix(a.g, b.g),
        b: mix(a.b, b.b),
        a: mix(a.a, b.a),
    }
}

fn interpolate(
    fv: &[Value],
    tv: &[Value],
    ff: f32,
    tf: f32,
    k: f32,
) -> Option<Vec<Value>> {
    let f = fv.first()?;
    let t = tv.first()?;
    match (f, t) {
        (Value::Length(_, _), Value::Length(_, _)) => {
            let a = px_of(f, ff);
            let b = px_of(t, tf);
            Some(vec![Value::Length(a + (b - a) * k, Unit::Px)])
        }
        (Value::Color(a), Value::Color(b)) => Some(vec![Value::Color(mix_color(a, b, k))]),
        _ => None,
    }
}

fn same_box<'a>(a: &StyledNode<'a>, b: &StyledNode<'a>) -> bool {
    std::ptr::eq(a.node as *const Node, b.node as *const Node)
        && a.pseudo.as_ref().map(|(k, _)| *k) == b.pseudo.as_ref().map(|(k, _)| *k)
}

fn clone_tree<'a>(n: &StyledNode<'a>) -> StyledNode<'a> {
    StyledNode {
        node: n.node,
        computed: n.computed.clone(),
        pseudo: n.pseudo.clone(),
        children: n.children.iter().map(clone_tree).collect(),
    }
}

pub fn transition_merge<'a>(
    from: &StyledNode<'a>,
    to: &StyledNode<'a>,
    t: f32,
) -> StyledNode<'a> {
    let mut computed = to.computed.clone();
    let mut items = transition_items(&to.computed);
    if items.is_empty() {
        items = transition_items(&from.computed);
    }
    for (prop, dur, delay, bez) in items {
        let targets: Vec<String> = if prop == "all" {
            to.computed
                .keys()
                .filter(|k| {
                    from.computed.contains_key(*k) && !k.starts_with("transition")
                })
                .cloned()
                .collect()
        } else if from.computed.contains_key(&prop) && to.computed.contains_key(&prop) {
            vec![prop]
        } else {
            Vec::new()
        };
        for p in targets {
            let fv = &from.computed[&p];
            let tv = &to.computed[&p];
            if fv == tv {
                continue;
            }
            let k = if dur <= 0.0 {
                if t >= delay {
                    1.0
                } else {
                    0.0
                }
            } else {
                ((t - delay) / dur).clamp(0.0, 1.0)
            };
            let e = easing(bez, k);
            let v = interpolate(fv, tv, font_px(&from.computed), font_px(&to.computed), e)
                .unwrap_or_else(|| if e < 0.5 { fv.clone() } else { tv.clone() });
            computed.insert(p, v);
        }
    }
    let mut children: Vec<StyledNode<'a>> = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < from.children.len() && j < to.children.len() {
        if same_box(&from.children[i], &to.children[j]) {
            children.push(transition_merge(&from.children[i], &to.children[j], t));
            i += 1;
            j += 1;
            continue;
        }
        match to.children[j..]
            .iter()
            .position(|c| same_box(c, &from.children[i]))
        {
            Some(off) => {
                for c in &to.children[j..j + off] {
                    children.push(clone_tree(c));
                }
                j += off;
            }
            None => {
                i += 1;
            }
        }
    }
    for c in &to.children[j..] {
        children.push(clone_tree(c));
    }
    StyledNode {
        node: to.node,
        computed,
        pseudo: to.pseudo.clone(),
        children,
    }
}

fn selector_matches(node: &Node, s: &Selector, ancestors: &[&Node]) -> bool {
    chain_matches(node, ancestors, &s.compounds)
}

fn chain_matches(node: &Node, ancestors: &[&Node], compounds: &[Vec<SimpleSelector>]) -> bool {
    let (subject, rest) = compounds.split_last().unwrap();
    if !compound_matches(node, subject) {
        return false;
    }
    if rest.is_empty() {
        return true;
    }
    for k in (0..ancestors.len()).rev() {
        if matches!(ancestors[k].kind, NodeKind::Element { .. })
            && chain_matches(ancestors[k], &ancestors[..k], rest)
        {
            return true;
        }
    }
    false
}

fn compound_matches(node: &Node, compound: &[SimpleSelector]) -> bool {
    compound.iter().all(|s| simple_matches(node, s))
}

fn simple_matches(node: &Node, s: &SimpleSelector) -> bool {
    match s {
        SimpleSelector::Universal => true,
        SimpleSelector::Tag(t) => node.name() == Some(t.as_str()),
        SimpleSelector::Id(i) => attr(node, "id") == Some(i.as_str()),
        SimpleSelector::Class(c) => attr(node, "class")
            .map(|v| v.split_whitespace().any(|p| p == c))
            .unwrap_or(false),
        SimpleSelector::Pseudo(_) => false,
    }
}

fn attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match &node.kind {
        NodeKind::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::Color;
    use css::Unit;
    use css::parse as parse_css;
    use html::parse as parse_html;

    fn el<'a>(s: &'a StyledNode<'a>, name: &str) -> &'a StyledNode<'a> {
        s.children
            .iter()
            .find(|c| c.node.name() == Some(name))
            .unwrap()
    }

    fn rgb(r: u8, g: u8, b: u8) -> Value {
        Value::Color(Color { r, g, b, a: 255 })
    }

    fn px(n: f32) -> Value {
        Value::Length(n, Unit::Px)
    }

    #[test]
    fn specificity_and_order() {
        let dom = parse_html("<p id=\"a\" class=\"x\">t</p>");
        let sheet = parse_css("p { color: red } #a { color: green } .x { margin: 0 }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.computed["color"], vec![rgb(0, 128, 0)]);
        assert_eq!(p.computed["margin"], vec![px(0.0)]);
    }

    #[test]
    fn later_wins() {
        let dom = parse_html("<p>t</p>");
        let sheet = parse_css("p { color: red } p { color: blue }");
        let s = build(&dom, &sheet);
        assert_eq!(el(&s, "p").computed["color"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn inline_style_wins() {
        let dom = parse_html("<p style=\"color: green\">t</p>");
        let sheet = parse_css("p { color: red }");
        let s = build(&dom, &sheet);
        assert_eq!(el(&s, "p").computed["color"], vec![rgb(0, 128, 0)]);
    }

    #[test]
    fn style_element_collected() {
        let dom = parse_html("<html><head><style>p { color: red }</style></head><body><p>t</p></body></html>");
        let mut css = String::new();
        collect_css(&dom, &mut css);
        assert_eq!(css.trim(), "p { color: red }");
        let sheet = parse_css(&css);
        let s = build(&dom, &sheet);
        let body = el(el(&s, "html"), "body");
        assert_eq!(el(body, "p").computed["color"], vec![rgb(255, 0, 0)]);
    }

    #[test]
    fn descendant() {
        let dom = parse_html("<div><p>a</p></div><p>b</p>");
        let sheet = parse_css("div p { color: red } p { color: blue }");
        let s = build(&dom, &sheet);
        assert_eq!(el(el(&s, "div"), "p").computed["color"], vec![rgb(255, 0, 0)]);
        assert_eq!(s.children[1].computed["color"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn class_list() {
        let dom = parse_html("<p class=\"a b\">t</p>");
        let sheet = parse_css(".b { color: red }");
        let s = build(&dom, &sheet);
        assert_eq!(el(&s, "p").computed["color"], vec![rgb(255, 0, 0)]);
    }

    #[test]
    fn text_inherits() {
        let dom = parse_html("<p>t</p>");
        let sheet = parse_css("* { color: red }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.computed["color"], vec![rgb(255, 0, 0)]);
        assert_eq!(p.children[0].computed["color"], vec![rgb(255, 0, 0)]);
    }

    #[test]
    fn inherited_down() {
        let dom = parse_html("<div><p><b>t</b></p></div>");
        let sheet = parse_css("div { color: red; font-size: 14px }");
        let s = build(&dom, &sheet);
        let b = el(el(el(&s, "div"), "p"), "b");
        assert_eq!(b.computed["color"], vec![rgb(255, 0, 0)]);
        assert_eq!(b.computed["font-size"], vec![px(14.0)]);
    }

    #[test]
    fn not_inherited() {
        let dom = parse_html("<div><p>t</p></div>");
        let sheet = parse_css("div { padding: 4px; color: red }");
        let s = build(&dom, &sheet);
        let p = el(el(&s, "div"), "p");
        assert!(!p.computed.contains_key("padding"));
        assert_eq!(p.computed["color"], vec![rgb(255, 0, 0)]);
    }

    #[test]
    fn own_beats_inherited() {
        let dom = parse_html("<div><p>t</p></div>");
        let sheet = parse_css("div { color: red } p { color: blue }");
        let s = build(&dom, &sheet);
        let p = el(el(&s, "div"), "p");
        assert_eq!(p.computed["color"], vec![rgb(0, 0, 255)]);
        assert_eq!(el(&s, "div").computed["color"], vec![rgb(255, 0, 0)]);
    }

    #[test]
    fn gap_expands_to_longhands() {
        let dom = parse_html("<div style=\"gap: 5px 10px\">t</div>");
        let sheet = parse_css("");
        let s = build(&dom, &sheet);
        let d = el(&s, "div");
        assert_eq!(d.computed["row-gap"], vec![px(5.0)]);
        assert_eq!(d.computed["column-gap"], vec![px(10.0)]);
        assert!(!d.computed.contains_key("gap"));
    }

    #[test]
    fn gap_longhand_overrides_after_shorthand() {
        let dom = parse_html("<div style=\"gap: 5px; row-gap: 15px\">t</div>");
        let sheet = parse_css("div { gap: 20px 30px }");
        let s = build(&dom, &sheet);
        let d = el(&s, "div");
        assert_eq!(d.computed["row-gap"], vec![px(15.0)]);
        assert_eq!(d.computed["column-gap"], vec![px(5.0)]);
    }

    #[test]
    fn gap_single_value_sets_both() {
        let dom = parse_html("<div>t</div>");
        let sheet = parse_css("div { gap: 7px }");
        let s = build(&dom, &sheet);
        let d = el(&s, "div");
        assert_eq!(d.computed["row-gap"], vec![px(7.0)]);
        assert_eq!(d.computed["column-gap"], vec![px(7.0)]);
    }

    #[test]
    fn family_takes_first_list_entry() {
        let dom = parse_html("<p style=\"font-family: Arial, sans-serif\">t</p>");
        let sheet = parse_css("");
        let s = build(&dom, &sheet);
        assert_eq!(family(&el(&s, "p").computed), "arial");
    }

    #[test]
    fn family_joins_multi_word() {
        let dom = parse_html("<p style=\"font-family: Comic Sans MS, cursive\">t</p>");
        let sheet = parse_css("");
        let s = build(&dom, &sheet);
        assert_eq!(family(&el(&s, "p").computed), "comic sans ms");
    }

    #[test]
    fn font_size_resolved() {
        let dom = parse_html("<div><p><b>t</b></p></div>");
        let sheet = parse_css("div { font-size: 2em }");
        let s = build(&dom, &sheet);
        let div = el(&s, "div");
        let p = el(div, "p");
        assert_eq!(div.computed["font-size"], vec![px(32.0)]);
        assert_eq!(p.computed["font-size"], vec![px(32.0)]);
        let b = el(p, "b");
        assert_eq!(b.computed["font-size"], vec![px(32.0)]);
    }

    #[test]
    fn before_after_nodes() {
        let dom = parse_html("<p>hi</p>");
        let sheet = parse_css("p::before { content: \"<\" } p::after { content: \">!\" }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.children.len(), 3);
        let b = &p.children[0];
        assert_eq!(b.pseudo.as_ref().unwrap().0, "before");
        assert_eq!(b.pseudo.as_ref().unwrap().1, "<");
        assert!(p.children[1].pseudo.is_none());
        assert_eq!(p.children[2].pseudo.as_ref().unwrap().0, "after");
        assert_eq!(p.children[2].pseudo.as_ref().unwrap().1, ">!");
    }

    #[test]
    fn pseudo_inherits_and_styles() {
        let dom = parse_html("<p>hi</p>");
        let sheet = parse_css(
            "p { color: red; font-size: 10px } p::before { content: \"x\"; font-size: 2em }",
        );
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        let b = &p.children[0];
        assert_eq!(b.computed["color"], vec![rgb(255, 0, 0)]);
        assert_eq!(b.computed["font-size"], vec![px(20.0)]);
        assert_eq!(p.computed["font-size"], vec![px(10.0)]);
    }

    #[test]
    fn pseudo_without_content_skipped() {
        let dom = parse_html("<p>hi</p>");
        let sheet = parse_css("p::before { color: red } p::after { content: none }");
        let s = build(&dom, &sheet);
        assert_eq!(el(&s, "p").children.len(), 1);
    }

    #[test]
    fn pseudo_scoped_to_matching() {
        let dom = parse_html("<div><p>t</p></div>");
        let sheet = parse_css("div::before { content: \"d\" } span::before { content: \"s\" }");
        let s = build(&dom, &sheet);
        let div = el(&s, "div");
        assert_eq!(div.children.len(), 2);
        assert_eq!(div.children[0].pseudo.as_ref().unwrap().1, "d");
        assert_eq!(el(div, "p").children.len(), 1);
    }

    #[test]
    fn pseudo_specificity_order() {
        let dom = parse_html("<p class=\"a\">t</p>");
        let sheet = parse_css(
            "p::before { content: \"one\"; color: red } .a::before { content: \"two\"; color: blue }",
        );
        let s = build(&dom, &sheet);
        let b = &el(&s, "p").children[0];
        assert_eq!(b.pseudo.as_ref().unwrap().1, "two");
        assert_eq!(b.computed["color"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn deep_ancestor() {
        let dom = parse_html("<div><section><p>t</p></section></div>");
        let sheet = parse_css("div p { color: red }");
        let s = build(&dom, &sheet);
        assert_eq!(
            el(el(el(&s, "div"), "section"), "p").computed["color"],
            vec![rgb(255, 0, 0)]
        );
    }

    #[test]
    fn first_letter_extracts_and_rest() {
        let dom = parse_html("<p>Hello</p>");
        let sheet = parse_css("p::first-letter { font-size: 2em; color: red }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.children.len(), 2);
        let l = &p.children[0];
        assert_eq!(l.pseudo.as_ref().unwrap().0, "first-letter");
        assert_eq!(l.pseudo.as_ref().unwrap().1, "H");
        assert_eq!(l.computed["font-size"], vec![px(32.0)]);
        assert_eq!(l.computed["color"], vec![rgb(255, 0, 0)]);
        let rest = &p.children[1];
        assert_eq!(rest.pseudo.as_ref().unwrap().0, "text");
        assert_eq!(rest.pseudo.as_ref().unwrap().1, "ello");
        assert_eq!(rest.computed["font-size"], vec![px(16.0)]);
    }

    #[test]
    fn first_letter_inside_inline_child() {
        let dom = parse_html("<p><b>Hi</b></p>");
        let sheet = parse_css("p::first-letter { color: red }");
        let s = build(&dom, &sheet);
        let b = el(el(&s, "p"), "b");
        assert_eq!(b.children.len(), 2);
        assert_eq!(b.children[0].pseudo.as_ref().unwrap(), &("first-letter", "H".to_string()));
        assert_eq!(b.children[1].pseudo.as_ref().unwrap(), &("text", "i".to_string()));
    }

    #[test]
    fn first_letter_skips_whitespace_and_elements() {
        let dom = parse_html("<p><img src=\"x.png\"> Hi</p>");
        let sheet = parse_css("p::first-letter { color: red }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        let rest = p.children.iter().find(|c| c.pseudo.as_ref().map(|p| p.0) == Some("text")).unwrap();
        assert_eq!(rest.pseudo.as_ref().unwrap().1, "i");
    }

    #[test]
    fn first_letter_needs_block_container() {
        let dom = parse_html("<p>hi <b>bold</b></p>");
        let sheet = parse_css("b::first-letter { color: red }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        let b = el(p, "b");
        assert_eq!(b.children.len(), 1);
        assert!(b.children[0].pseudo.is_none());
    }

    #[test]
    fn first_letter_no_match_untouched() {
        let dom = parse_html("<p>Hello</p>");
        let sheet = parse_css("div::first-letter { color: red }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.children.len(), 1);
        assert!(p.children[0].pseudo.is_none());
    }

    #[test]
    fn first_letter_single_char_rest_empty() {
        let dom = parse_html("<p>A</p>");
        let sheet = parse_css("p::first-letter { color: red }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.children[0].pseudo.as_ref().unwrap().1, "A");
        assert_eq!(p.children[1].pseudo.as_ref().unwrap().1, "");
    }

    #[test]
    fn first_letter_cascade_specificity() {
        let dom = parse_html("<p class=\"a\">Hi</p>");
        let sheet = parse_css(
            "p::first-letter { color: red } .a::first-letter { color: blue }",
        );
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.children[0].computed["color"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn selection_overlay_on_text() {
        let dom = parse_html("<p>hi</p>");
        let sheet = parse_css("p::selection { color: white; background-color: blue }");
        let s = build_selected(&dom, &sheet);
        let t = &el(&s, "p").children[0];
        assert_eq!(t.computed["::selection-color"], vec![rgb(255, 255, 255)]);
        assert_eq!(t.computed["::selection-background"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn selection_absent_in_plain_build() {
        let dom = parse_html("<p>hi</p>");
        let sheet = parse_css("p::selection { color: white }");
        let s = build(&dom, &sheet);
        let t = &el(&s, "p").children[0];
        assert!(!t.computed.contains_key("::selection-color"));
    }

    #[test]
    fn selection_bare_and_scoped() {
        let dom = parse_html("<div><p>a</p></div><p>b</p>");
        let sheet = parse_css("div p::selection { color: red }");
        let s = build_selected(&dom, &sheet);
        let inner = &el(el(&s, "div"), "p").children[0];
        assert_eq!(inner.computed["::selection-color"], vec![rgb(255, 0, 0)]);
        let outer = &s.children[1].children[0];
        assert!(!outer.computed.contains_key("::selection-color"));
    }

    #[test]
    fn selection_background_shorthand() {
        let dom = parse_html("<p>hi</p>");
        let sheet = parse_css("p::selection { background: #00ff00 }");
        let s = build_selected(&dom, &sheet);
        let t = &el(&s, "p").children[0];
        assert_eq!(t.computed["::selection-background"], vec![rgb(0, 255, 0)]);
    }

    #[test]
    fn selection_applies_to_pseudo_content() {
        let dom = parse_html("<p>hi</p>");
        let sheet = parse_css(
            "p::before { content: \"x\" } p::selection { color: red; background-color: blue }",
        );
        let s = build_selected(&dom, &sheet);
        let p = el(&s, "p");
        let b = &p.children[0];
        assert_eq!(b.pseudo.as_ref().unwrap().0, "before");
        assert_eq!(b.computed["::selection-color"], vec![rgb(255, 0, 0)]);
        assert_eq!(b.computed["::selection-background"], vec![rgb(0, 0, 255)]);
    }

    fn near_c(v: &Value, r: u8, g: u8, b: u8) -> bool {
        match v {
            Value::Color(c) => {
                (c.r as i32 - r as i32).abs() <= 2
                    && (c.g as i32 - g as i32).abs() <= 2
                    && (c.b as i32 - b as i32).abs() <= 2
            }
            _ => false,
        }
    }

    #[test]
    fn easing_endpoints_and_monotonic() {
        assert_eq!(easing([0.25, 0.1, 0.25, 1.0], 0.0), 0.0);
        assert_eq!(easing([0.25, 0.1, 0.25, 1.0], 1.0), 1.0);
        assert_eq!(easing([0.0, 0.0, 1.0, 1.0], 0.5), 0.5);
        assert!(easing([0.42, 0.0, 1.0, 1.0], 0.5) < 0.5);
        assert!(easing([0.0, 0.0, 0.58, 1.0], 0.5) > 0.5);
        let mut prev = -1.0;
        for i in 0..=20 {
            let e = easing([0.25, 0.1, 0.25, 1.0], i as f32 / 20.0);
            assert!(e >= prev);
            prev = e;
        }
    }

    #[test]
    fn transition_color_midpoint() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css("p { color: red; transition: color 1s linear }");
        let target = parse_css("p { color: blue }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let mid = transition_merge(&from, &to, 0.5);
        let p = el(&mid, "p");
        assert!(near_c(&p.computed["color"][0], 128, 0, 128));
        let start = transition_merge(&from, &to, 0.0);
        assert_eq!(el(&start, "p").computed["color"], vec![rgb(255, 0, 0)]);
        let end = transition_merge(&from, &to, 1.0);
        assert_eq!(el(&end, "p").computed["color"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn transition_delay_and_clamp() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css("p { color: red; transition: color 1s 0.5s linear }");
        let target = parse_css("p { color: blue }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let before = transition_merge(&from, &to, 0.25);
        assert_eq!(el(&before, "p").computed["color"], vec![rgb(255, 0, 0)]);
        let quarter = transition_merge(&from, &to, 0.75);
        assert!(near_c(&el(&quarter, "p").computed["color"][0], 191, 0, 64));
        let after = transition_merge(&from, &to, 5.0);
        assert_eq!(el(&after, "p").computed["color"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn transition_unlisted_prop_jumps() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css("p { color: red; margin-left: 10px; transition: color 1s linear }");
        let target = parse_css("p { color: blue; margin-left: 40px }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let mid = transition_merge(&from, &to, 0.5);
        let p = el(&mid, "p");
        assert!(near_c(&p.computed["color"][0], 128, 0, 128));
        assert_eq!(p.computed["margin-left"], vec![Value::Length(40.0, Unit::Px)]);
    }

    #[test]
    fn transition_em_resolves_against_own_font() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css(
            "p { font-size: 16px; margin-left: 1em; transition: margin-left 1s linear }",
        );
        let target = parse_css("p { font-size: 16px; margin-left: 3em }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let mid = transition_merge(&from, &to, 0.5);
        assert_eq!(
            el(&mid, "p").computed["margin-left"],
            vec![Value::Length(32.0, Unit::Px)]
        );
    }

    #[test]
    fn transition_discrete_keyword_flips() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css("p { text-align: left; transition: text-align 1s linear }");
        let target = parse_css("p { text-align: center }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let early = transition_merge(&from, &to, 0.25);
        assert_eq!(el(&early, "p").computed["text-align"], vec![Value::Keyword("left".into())]);
        let late = transition_merge(&from, &to, 0.75);
        assert_eq!(
            el(&late, "p").computed["text-align"],
            vec![Value::Keyword("center".into())]
        );
    }

    #[test]
    fn transition_zero_duration_is_instant() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css("p { color: red; transition: color 0s }");
        let target = parse_css("p { color: blue }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let merged = transition_merge(&from, &to, 0.0);
        assert_eq!(el(&merged, "p").computed["color"], vec![rgb(0, 0, 255)]);
    }

    #[test]
    fn transition_multiple_props_and_easing() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css(
            "p { color: red; margin-left: 0px; transition: color 2s linear, margin-left 1s ease-in-out }",
        );
        let target = parse_css("p { color: blue; margin-left: 100px }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let m = transition_merge(&from, &to, 0.5);
        let p = el(&m, "p");
        assert!(near_c(&p.computed["color"][0], 191, 0, 64));
        let ml = match &p.computed["margin-left"][0] {
            Value::Length(n, _) => *n,
            _ => panic!("not a length"),
        };
        assert!(ml > 40.0 && ml < 60.0);
    }

    #[test]
    fn transition_longhands_override() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css(
            "p { color: red; transition-property: color; transition-duration: 1s; transition-timing-function: linear }",
        );
        let target = parse_css("p { color: blue }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let mid = transition_merge(&from, &to, 0.5);
        assert!(near_c(&el(&mid, "p").computed["color"][0], 128, 0, 128));
    }

    #[test]
    fn transition_adds_pseudo_child() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css("p { color: red }");
        let target = parse_css("p { color: blue } p::before { content: \"!\" }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let merged = transition_merge(&from, &to, 0.5);
        let p = el(&merged, "p");
        assert_eq!(p.children.len(), 2);
        assert_eq!(p.children[0].pseudo.as_ref().unwrap().0, "before");
        assert!(p.children[1].pseudo.is_none());
    }

    #[test]
    fn transition_declared_in_target_sheet() {
        let dom = parse_html("<p>t</p>");
        let base = parse_css("p { color: red }");
        let target = parse_css("p { color: blue; transition: color 1s linear }");
        let from = build(&dom, &base);
        let to = build(&dom, &target);
        let mid = transition_merge(&from, &to, 0.5);
        assert!(near_c(&el(&mid, "p").computed["color"][0], 128, 0, 128));
    }
}
