use std::collections::HashMap;

use css::{Rule, Selector, SimpleSelector, Stylesheet};
use html::{Node, NodeKind};

#[derive(Debug, PartialEq)]
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub specified: HashMap<String, String>,
    pub children: Vec<StyledNode<'a>>,
}

pub fn build<'a>(node: &'a Node, sheet: &Stylesheet) -> StyledNode<'a> {
    build_rec(node, sheet, &[])
}

fn build_rec<'a>(node: &'a Node, sheet: &Stylesheet, ancestors: &[&'a Node]) -> StyledNode<'a> {
    let mut specified = HashMap::new();
    if matches!(node.kind, NodeKind::Element { .. }) {
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
                specified.insert(d.name.clone(), d.value.clone());
            }
        }
    }
    let mut chain: Vec<&Node> = ancestors.to_vec();
    chain.push(node);
    let children = node
        .children
        .iter()
        .map(|c| build_rec(c, sheet, &chain))
        .collect();
    StyledNode {
        node,
        specified,
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

    #[test]
    fn specificity_and_order() {
        let dom = parse_html("<p id=\"a\" class=\"x\">t</p>");
        let sheet = parse_css("p { color: red } #a { color: green } .x { margin: 0 }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.specified["color"], "green");
        assert_eq!(p.specified["margin"], "0");
    }

    #[test]
    fn later_wins() {
        let dom = parse_html("<p>t</p>");
        let sheet = parse_css("p { color: red } p { color: blue }");
        let s = build(&dom, &sheet);
        assert_eq!(el(&s, "p").specified["color"], "blue");
    }

    #[test]
    fn descendant() {
        let dom = parse_html("<div><p>a</p></div><p>b</p>");
        let sheet = parse_css("div p { color: red } p { color: blue }");
        let s = build(&dom, &sheet);
        assert_eq!(el(el(&s, "div"), "p").specified["color"], "red");
        assert_eq!(s.children[1].specified["color"], "blue");
    }

    #[test]
    fn class_list() {
        let dom = parse_html("<p class=\"a b\">t</p>");
        let sheet = parse_css(".b { color: red }");
        let s = build(&dom, &sheet);
        assert_eq!(el(&s, "p").specified["color"], "red");
    }

    #[test]
    fn text_not_styled() {
        let dom = parse_html("<p>t</p>");
        let sheet = parse_css("* { color: red }");
        let s = build(&dom, &sheet);
        let p = el(&s, "p");
        assert_eq!(p.specified["color"], "red");
        assert!(p.children[0].specified.is_empty());
    }

    #[test]
    fn deep_ancestor() {
        let dom = parse_html("<div><section><p>t</p></section></div>");
        let sheet = parse_css("div p { color: red }");
        let s = build(&dom, &sheet);
        assert_eq!(el(el(el(&s, "div"), "section"), "p").specified["color"], "red");
    }
}
