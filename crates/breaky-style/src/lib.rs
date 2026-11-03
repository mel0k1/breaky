use std::collections::HashMap;

use css::{Color, Rule, Selector, SimpleSelector, Stylesheet, Unit, Value};
use html::{Node, NodeKind};

const INHERITED: [&str; 9] = [
    "color",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "line-height",
    "letter-spacing",
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
    build_rec(node, sheet, &[], None)
}

fn build_rec<'a>(
    node: &'a Node,
    sheet: &Stylesheet,
    ancestors: &[&'a Node],
    parent: Option<&HashMap<String, Vec<Value>>>,
) -> StyledNode<'a> {
    let mut computed = HashMap::new();
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
            for key in INHERITED {
                if !computed.contains_key(key) {
                    if let Some(v) = parent.and_then(|m| m.get(key)) {
                        computed.insert(key.to_string(), v.clone());
                    }
                }
            }
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
        .map(|c| build_rec(c, sheet, &chain, Some(&computed)))
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
