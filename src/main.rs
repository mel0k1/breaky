use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).unwrap_or_else(|| usage());
    let src = fs::read_to_string(path).unwrap_or_else(fail);
    let dom = html::parse(&src);
    html::print_tree(&dom, 0);
    if let Some(css) = args.get(2) {
        let sheet = css::parse(&fs::read_to_string(css).unwrap_or_else(fail));
        for rule in &sheet.rules {
            println!("{:?}", rule);
        }
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
