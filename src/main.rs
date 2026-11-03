mod css;
mod dom;
mod html;

use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).unwrap_or_else(|| usage());
    let src = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit(1);
    });
    dom::print_tree(&html::parse(&src), 0);
    if let Some(css) = args.get(2) {
        let sheet = css::parse(&fs::read_to_string(css).unwrap_or_else(|e| {
            eprintln!("{}", e);
            exit(1);
        }));
        for rule in &sheet.rules {
            println!("{:?}", rule);
        }
    }
}

fn usage() -> ! {
    eprintln!("usage: breaky <file>");
    exit(2);
}
