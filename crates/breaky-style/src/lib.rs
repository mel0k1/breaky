use std::collections::HashMap;

use css::{Rule, Selector, SimpleSelector, Stylesheet, Unit, Value};
use html::{Node, NodeKind};

const BASE_FONT: f32 = 16.0;

const INHERITED: [&str; 12] = [
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
    build_rec(node, sheet, &[], None, BASE_FONT)
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
                    computed.insert(d.name.clone(), d.value.clone());
                }
            }
            if let Some(s) = attr(node, "style") {
                for d in css::declarations(s) {
                    computed.insert(d.name, d.value);
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
        }
        _ => {}
    }
    let mut chain: Vec<&Node> = ancestors.to_vec();
    chain.push(node);
    let mut children: Vec<StyledNode<'a>> = node
        .children
        .iter()
        .map(|c| build_rec(c, sheet, &chain, Some(&computed), font))
        .collect();
    if matches!(node.kind, NodeKind::Element { .. }) {
        if let Some(b) = pseudo_node(node, sheet, ancestors, &computed, font, "before") {
            children.insert(0, b);
        }
        if let Some(a) = pseudo_node(node, sheet, ancestors, &computed, font, "after") {
            children.push(a);
        }
    }
    StyledNode {
        node,
        computed,
        children,
        pseudo: None,
    }
}

fn pseudo_of(s: &Selector) -> Option<&str> {
    let last = s.compounds.last()?.last()?;
    match last {
        SimpleSelector::Pseudo(p) => Some(p.as_str()),
        _ => None,
    }
}

fn pseudo_node<'a>(
    node: &'a Node,
    sheet: &Stylesheet,
    ancestors: &[&'a Node],
    parent: &HashMap<String, Vec<Value>>,
    parent_font: f32,
    kind: &'static str,
) -> Option<StyledNode<'a>> {
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
    let mut computed = HashMap::new();
    for (_, _, rule) in &matched {
        for d in &rule.declarations {
            computed.insert(d.name.clone(), d.value.clone());
        }
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
}
