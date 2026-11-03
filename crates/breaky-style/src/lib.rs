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
    let children = node
        .children
        .iter()
        .map(|c| build_rec(c, sheet, &chain, Some(&computed), font))
        .collect();
    StyledNode {
        node,
        computed,
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
