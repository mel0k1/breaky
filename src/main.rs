use std::env;
use std::fs;
use std::process::exit;

use html::NodeKind;
use style::{StyledNode, build};

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).unwrap_or_else(|| usage());
    let src = fs::read_to_string(path).unwrap_or_else(fail);
    let dom = html::parse(&src);
    html::print_tree(&dom, 0);
    if let Some(css) = args.get(2) {
        let sheet = css::parse(&fs::read_to_string(css).unwrap_or_else(fail));
        print_styled(&build(&dom, &sheet), 0);
    }
}

fn print_styled(node: &StyledNode, depth: usize) {
    if let NodeKind::Element { name, .. } = &node.node.kind {
        let mut props: Vec<_> = node.specified.iter().collect();
        props.sort();
        let p: Vec<String> = props.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        let head = if p.is_empty() {
            format!("<{}>", name)
        } else {
            format!("<{} {}>", name, p.join(" "))
        };
        println!("{}{}", "  ".repeat(depth), head);
    }
    for c in &node.children {
        print_styled(c, depth + 1);
    }
}

fn fail<T>(e: std::io::Error) -> T {
    eprintln!("{}", e);
    exit(1);
}

fn usage() -> ! {
    eprintln!("usage: breaky <file> [style.css]");
    exit(2);
}
