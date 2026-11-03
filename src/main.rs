mod dom;
mod html;

use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| usage());
    let src = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit(1);
    });
    dom::print_tree(&html::parse(&src), 0);
}

fn usage() -> ! {
    eprintln!("usage: breaky <file>");
    exit(2);
}
