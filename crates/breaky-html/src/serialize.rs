use crate::dom::{Node, NodeKind};

const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const RAW: [&str; 2] = ["script", "style"];

pub fn inner_html(node: &Node) -> String {
    let mut out = String::new();
    if let NodeKind::Element { name, .. } = &node.kind {
        if RAW.contains(&name.as_str()) {
            for c in &node.children {
                if let NodeKind::Text(t) = &c.kind {
                    out.push_str(t);
                }
            }
            return out;
        }
    }
    for c in &node.children {
        write(c, &mut out);
    }
    out
}

fn write(node: &Node, out: &mut String) {
    match &node.kind {
        NodeKind::Document => {
            for c in &node.children {
                write(c, out);
            }
        }
        NodeKind::Doctype => {}
        NodeKind::Element { name, attrs } => {
            out.push('<');
            out.push_str(name);
            for (k, v) in attrs {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                escape(v, out, true);
                out.push('"');
            }
            out.push('>');
            if VOID.contains(&name.as_str()) {
                return;
            }
            if RAW.contains(&name.as_str()) {
                for c in &node.children {
                    if let NodeKind::Text(t) = &c.kind {
                        out.push_str(t);
                    }
                }
            } else {
                for c in &node.children {
                    write(c, out);
                }
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        NodeKind::Text(t) => escape(t, out, false),
    }
}

fn escape(s: &str, out: &mut String, attr: bool) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn find_el<'a>(node: &'a Node, name: &str) -> &'a Node {
        node.children
            .iter()
            .find(|c| c.name() == Some(name))
            .unwrap()
    }

    #[test]
    fn text_and_void() {
        let doc = parse("<p>Hi<br>there</p>");
        assert_eq!(inner_html(find_el(&doc, "p")), "Hi<br>there");
    }

    #[test]
    fn nested() {
        let doc = parse("<div><p class=\"x\">a<b>c</b></p></div>");
        assert_eq!(
            inner_html(find_el(&doc, "div")),
            "<p class=\"x\">a<b>c</b></p>"
        );
    }

    #[test]
    fn escapes_text_and_attrs() {
        let doc = parse("<p title=\"a&quot;b\">x &amp; y &lt;z&gt;</p>");
        assert_eq!(inner_html(find_el(&doc, "p")), "x &amp; y &lt;z&gt;");
        let doc = parse("<div><a title=\"a&quot;b&amp;c\">t</a></div>");
        assert_eq!(
            inner_html(find_el(&doc, "div")),
            "<a title=\"a&quot;b&amp;c\">t</a>"
        );
    }

    #[test]
    fn unknown_element_roundtrips() {
        let doc = parse("<div>x <z></z></div>");
        assert_eq!(inner_html(find_el(&doc, "div")), "x <z></z>");
    }

    #[test]
    fn raw_text_untouched() {
        let doc = parse("<script>if (a < b) { x(\"&amp;\"); }</script>");
        assert_eq!(
            inner_html(find_el(&doc, "script")),
            "if (a < b) { x(\"&amp;\"); }"
        );
    }

    #[test]
    fn empty_children() {
        let doc = parse("<div></div>");
        assert_eq!(inner_html(find_el(&doc, "div")), "");
    }
}
