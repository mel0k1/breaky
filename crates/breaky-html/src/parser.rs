use crate::dom::{Node, NodeKind};

const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const RAW: [&str; 2] = ["script", "style"];

pub fn parse(src: &str) -> Node {
    let chars: Vec<char> = src.chars().collect();
    let mut doc = Node::new(NodeKind::Document);
    let mut stack: Vec<Node> = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' && tag_start(chars.get(i + 1)) {
            flush(&mut stack, &mut doc, &mut text);
            i = tag(&chars, i, &mut stack, &mut doc);
        } else {
            text.push(chars[i]);
            i += 1;
        }
    }
    flush(&mut stack, &mut doc, &mut text);
    while let Some(el) = stack.pop() {
        match stack.last_mut() {
            Some(t) => t.children.push(el),
            None => doc.children.push(el),
        }
    }
    doc
}

fn tag_start(c: Option<&char>) -> bool {
    match c {
        Some('!') | Some('/') => true,
        Some(c) => c.is_ascii_alphabetic(),
        None => false,
    }
}

fn tag(chars: &[char], i: usize, stack: &mut Vec<Node>, doc: &mut Node) -> usize {
    match chars.get(i + 1) {
        Some('!') => bang(chars, i, stack, doc),
        Some('/') => close(chars, i, stack, doc),
        _ => open(chars, i, stack, doc),
    }
}

fn bang(chars: &[char], i: usize, stack: &mut Vec<Node>, doc: &mut Node) -> usize {
    if chars[i + 2..].starts_with(&['-', '-']) {
        find(chars, i + 4, &['-', '-', '>']).map_or(chars.len(), |e| e + 3)
    } else {
        match find(chars, i + 2, &['>']) {
            Some(e) => {
                attach_top(stack, doc, Node::new(NodeKind::Doctype));
                e + 1
            }
            None => chars.len(),
        }
    }
}

fn close(chars: &[char], i: usize, stack: &mut Vec<Node>, doc: &mut Node) -> usize {
    let end = find(chars, i + 2, &['>']).unwrap_or(chars.len());
    let name = chars[i + 2..end]
        .iter()
        .collect::<String>()
        .trim()
        .to_lowercase();
    if let Some(pos) = stack.iter().rposition(|n| n.name() == Some(name.as_str())) {
        let mut tail = stack.split_off(pos);
        let mut el = tail.remove(0);
        el.children.extend(tail);
        attach_top(stack, doc, el);
    }
    end + 1
}

fn open(chars: &[char], i: usize, stack: &mut Vec<Node>, doc: &mut Node) -> usize {
    let end = find(chars, i + 1, &['>']).unwrap_or(chars.len());
    let inner: String = chars[i + 1..end].iter().collect();
    let (name, attrs, self_close) = parse_tag(&inner);
    let lower = name.to_lowercase();
    let kind = NodeKind::Element {
        name: lower.clone(),
        attrs,
    };
    if self_close || VOID.contains(&lower.as_str()) {
        attach_top(stack, doc, Node::new(kind));
        return end + 1;
    }
    stack.push(Node::new(kind));
    if RAW.contains(&lower.as_str()) {
        let pat: Vec<char> = format!("</{}", lower).chars().collect();
        return match find(chars, end + 1, &pat) {
            Some(j) => {
                let raw: String = chars[end + 1..j].iter().collect();
                let raw = raw.trim().to_string();
                if !raw.is_empty() {
                    let t = Node::new(NodeKind::Text(raw));
                    stack.last_mut().unwrap().children.push(t);
                }
                j
            }
            None => chars.len(),
        };
    }
    end + 1
}

fn parse_tag(inner: &str) -> (String, Vec<(String, String)>, bool) {
    let trimmed = inner.trim_end();
    let body = trimmed.trim_end_matches('/');
    let self_close = body.len() < trimmed.len();
    let body = body.trim();
    let (name, rest) = match body.find(char::is_whitespace) {
        Some(p) => (body[..p].to_string(), &body[p..]),
        None => (body.to_string(), ""),
    };
    (name, parse_attrs(rest), self_close)
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while !rest.is_empty() {
        let eq = rest.find('=').unwrap_or(rest.len());
        let sp = rest.find(char::is_whitespace).unwrap_or(rest.len());
        if eq < sp {
            let key = rest[..eq].trim().to_lowercase();
            rest = rest[eq + 1..].trim_start();
            let (val, tail) = match rest.chars().next() {
                Some(q @ ('"' | '\'')) => match rest[1..].find(q) {
                    Some(p) => (rest[1..1 + p].to_string(), &rest[2 + p..]),
                    None => (rest[1..].to_string(), ""),
                },
                _ => {
                    let p = rest.find(char::is_whitespace).unwrap_or(rest.len());
                    (rest[..p].to_string(), &rest[p..])
                }
            };
            if !key.is_empty() {
                out.push((key, val));
            }
            rest = tail.trim();
        } else {
            let key = rest[..sp].trim().to_lowercase();
            if !key.is_empty() {
                out.push((key, String::new()));
            }
            rest = rest[sp..].trim();
        }
    }
    out
}

fn attach_top(stack: &mut Vec<Node>, doc: &mut Node, node: Node) {
    match stack.last_mut() {
        Some(t) => t.children.push(node),
        None => doc.children.push(node),
    }
}

fn flush(stack: &mut Vec<Node>, doc: &mut Node, text: &mut String) {
    let t = std::mem::take(text);
    if !t.is_empty() {
        attach_top(stack, doc, Node::new(NodeKind::Text(t)));
    }
}

fn find(chars: &[char], from: usize, pat: &[char]) -> Option<usize> {
    if pat.is_empty() || chars.len() < pat.len() {
        return None;
    }
    (from..=chars.len() - pat.len()).find(|&j| chars[j..j + pat.len()] == *pat)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_el<'a>(node: &'a Node, name: &str) -> &'a Node {
        node.children
            .iter()
            .find(|c| c.name() == Some(name))
            .unwrap()
    }

    #[test]
    fn tree() {
        let doc = parse("<!DOCTYPE html><html><body><p class=\"a\">Hi<br>there</p></body></html>");
        assert!(matches!(doc.children[0].kind, NodeKind::Doctype));
        let p = find_el(find_el(find_el(&doc, "html"), "body"), "p");
        let NodeKind::Element { attrs, .. } = &p.kind else {
            panic!()
        };
        assert_eq!(attrs, &[("class".into(), "a".into())]);
        assert_eq!(p.children.len(), 3);
        assert_eq!(p.children[0], Node::new(NodeKind::Text("Hi".into())));
        assert_eq!(p.children[1].name(), Some("br"));
        assert_eq!(p.children[2], Node::new(NodeKind::Text("there".into())));
    }

    #[test]
    fn implicit_close() {
        let doc = parse("<ul><li>a<li>b</ul>");
        let ul = find_el(&doc, "ul");
        assert_eq!(ul.children.len(), 2);
        assert_eq!(
            ul.children[1].children[0],
            Node::new(NodeKind::Text("b".into()))
        );
    }

    #[test]
    fn keeps_edge_spaces() {
        let doc = parse("<p>a <b> b </b> c</p>");
        let p = find_el(&doc, "p");
        assert_eq!(p.children[0], Node::new(NodeKind::Text("a ".into())));
        assert_eq!(p.children[2], Node::new(NodeKind::Text(" c".into())));
        assert_eq!(
            p.children[1].children[0],
            Node::new(NodeKind::Text(" b ".into()))
        );
    }

    #[test]
    fn attrs() {
        let doc = parse("<a href=\"https://x.io\" target=_blank data-q='y'>t</a>");
        let a = find_el(&doc, "a");
        let NodeKind::Element { attrs, .. } = &a.kind else {
            panic!()
        };
        assert_eq!(attrs.len(), 3);
        assert_eq!(attrs[0], ("href".into(), "https://x.io".into()));
        assert_eq!(attrs[1], ("target".into(), "_blank".into()));
        assert_eq!(attrs[2], ("data-q".into(), "y".into()));
    }

    #[test]
    fn raw_text() {
        let doc = parse("<body><script>var a=1<2;</script><p>x</p></body>");
        let body = find_el(&doc, "body");
        assert_eq!(body.children.len(), 2);
        assert_eq!(
            body.children[0].children[0],
            Node::new(NodeKind::Text("var a=1<2;".into()))
        );
        assert_eq!(body.children[1].name(), Some("p"));
    }

    #[test]
    fn comments_and_void() {
        let doc = parse("<head><!--c--><meta charset=\"utf-8\"><title>T</title></head>");
        let head = find_el(&doc, "head");
        assert_eq!(head.children.len(), 2);
        assert_eq!(head.children[0].name(), Some("meta"));
        let NodeKind::Element { attrs, .. } = &head.children[0].kind else {
            panic!()
        };
        assert_eq!(attrs[0], ("charset".into(), "utf-8".into()));
        assert_eq!(head.children[1].children[0], Node::new(NodeKind::Text("T".into())));
    }
}
