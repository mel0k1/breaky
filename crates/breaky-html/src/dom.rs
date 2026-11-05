#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Document,
    Doctype,
    Element {
        name: String,
        attrs: Vec<(String, String)>,
    },
    Text(String),
}

#[derive(Debug, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(kind: NodeKind) -> Self {
        Self {
            kind,
            children: Vec::new(),
        }
    }

    pub fn name(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Element { name, .. } => Some(name),
            _ => None,
        }
    }
}

pub fn print_tree(node: &Node, depth: usize) {
    match &node.kind {
        NodeKind::Document => {}
        NodeKind::Doctype => println!("{}<!doctype>", pad(depth)),
        NodeKind::Element { name, attrs } => {
            let mut line = format!("{}<{}", pad(depth), name);
            for (k, v) in attrs {
                line.push_str(&format!(" {}=\"{}\"", k, v));
            }
            line.push('>');
            println!("{}", line);
        }
        NodeKind::Text(t) => println!("{}{}", pad(depth), t),
    }
    for c in &node.children {
        print_tree(c, depth + 1);
    }
}

fn pad(depth: usize) -> String {
    "  ".repeat(depth)
}
