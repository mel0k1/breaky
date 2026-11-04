use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

use css::Value;
use html::{Node, NodeKind};
use net::Url;
use style::{StyledNode, build, build_selected};

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).unwrap_or_else(|| usage());
    if path == "fetch" {
        fetch(&args);
        return;
    }
    if args.len() >= 6 {
        transition_page(&args);
        return;
    }
    let src = fs::read_to_string(path).unwrap_or_else(fail);
    let dom = html::parse(&src);
    html::print_tree(&dom, 0);
    if let Some(css) = args.get(2) {
        let mut css_text = fs::read_to_string(css).unwrap_or_else(fail);
        let mut doc_css = String::new();
        style::collect_css(&dom, &mut doc_css);
        css_text.push_str(&doc_css);
        let sheet = css::parse(&css_text);
        let selected = args.get(5).map(|s| s.as_str()) == Some("sel");
        let styled = if selected {
            build_selected(&dom, &sheet)
        } else {
            build(&dom, &sheet)
        };
        println!();
        print_styled(&styled, 0);
        let viewport = args
            .get(3)
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(800.0);
        let mut imgs = img::Store::new();
        let mut urls: Vec<String> = Vec::new();
        gather_images(&styled, &mut urls);
        let base_css = Path::new(css).parent().unwrap_or(Path::new("."));
        load_local_images(&urls, base_css, &mut imgs);
        let mut srcs: Vec<String> = Vec::new();
        gather_dom_images(&dom, &mut srcs);
        let base_html = Path::new(path).parent().unwrap_or(Path::new("."));
        load_local_images(&srcs, base_html, &mut imgs);
        if let Some(tree) = layout::layout_with(&styled, viewport, &imgs) {
            println!();
            layout::print_tree(&tree, 0);
            if let Some(out) = args.get(4) {
                let canvas = render::paint_with(&tree, viewport, &imgs);
                fs::write(out, canvas.to_png()).unwrap_or_else(fail);
            }
        }
    }
}

fn transition_page(args: &[String]) {
    let src = fs::read_to_string(&args[1]).unwrap_or_else(fail);
    let dom = html::parse(&src);
    html::print_tree(&dom, 0);
    let mut doc_css = String::new();
    style::collect_css(&dom, &mut doc_css);
    let mut base = fs::read_to_string(&args[2]).unwrap_or_else(fail);
    base.push_str(&doc_css);
    let mut target = fs::read_to_string(&args[3]).unwrap_or_else(fail);
    target.push_str(&doc_css);
    let sheet_from = css::parse(&base);
    let sheet_to = css::parse(&target);
    let from = build(&dom, &sheet_from);
    let to = build(&dom, &sheet_to);
    let t = args
        .get(6)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(f32::INFINITY);
    let merged = style::transition_merge(&from, &to, t);
    println!();
    print_styled(&merged, 0);
    let viewport = args[4].parse::<f32>().unwrap_or(800.0);
    let mut imgs = img::Store::new();
    let mut urls: Vec<String> = Vec::new();
    gather_images(&merged, &mut urls);
    let base_css = Path::new(&args[2]).parent().unwrap_or(Path::new("."));
    load_local_images(&urls, base_css, &mut imgs);
    let mut srcs: Vec<String> = Vec::new();
    gather_dom_images(&dom, &mut srcs);
    let base_html = Path::new(&args[1]).parent().unwrap_or(Path::new("."));
    load_local_images(&srcs, base_html, &mut imgs);
    if let Some(tree) = layout::layout_with(&merged, viewport, &imgs) {
        println!();
        layout::print_tree(&tree, 0);
        let canvas = render::paint_with(&tree, viewport, &imgs);
        fs::write(&args[5], canvas.to_png()).unwrap_or_else(fail);
        println!("saved {}", args[5]);
    }
}

fn fetch(args: &[String]) {
    let url = args.get(2).unwrap_or_else(|| usage());
    let viewport = args
        .get(3)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(800.0);
    let out = args.get(4).map(|s| s.as_str()).unwrap_or("page.png");
    let base = net::parse_url(url).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit(1);
    });
    let resp = net::get(url).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit(1);
    });
    if resp.status != 200 {
        eprintln!("HTTP {}", resp.status);
        exit(1);
    }
    let src = String::from_utf8_lossy(&resp.body).into_owned();
    let dom = html::parse(&src);
    html::print_tree(&dom, 0);
    let mut css_text = String::new();
    collect_links(&dom, &base, &mut css_text);
    style::collect_css(&dom, &mut css_text);
    let sheet = css::parse(&css_text);
    let styled = build(&dom, &sheet);
    println!();
    print_styled(&styled, 0);
    let mut imgs = img::Store::new();
    let mut urls: Vec<String> = Vec::new();
    gather_images(&styled, &mut urls);
    let mut srcs: Vec<String> = Vec::new();
    gather_dom_images(&dom, &mut srcs);
    for s in srcs {
        if !urls.iter().any(|u| u == &s) {
            urls.push(s);
        }
    }
    load_remote_images(&urls, &base, &mut imgs);
    if let Some(tree) = layout::layout_with(&styled, viewport, &imgs) {
        println!();
        layout::print_tree(&tree, 0);
        let canvas = render::paint_with(&tree, viewport, &imgs);
        fs::write(out, canvas.to_png()).unwrap_or_else(fail);
        println!("saved {}", out);
    }
}

fn gather_images(node: &StyledNode, out: &mut Vec<String>) {
    for key in ["list-style-image", "background-image", "background"] {
        if let Some(vals) = node.computed.get(key) {
            for v in vals {
                if let Value::Url(u) = v {
                    if !out.iter().any(|x| x == u) {
                        out.push(u.clone());
                    }
                }
            }
        }
    }
    for c in &node.children {
        gather_images(c, out);
    }
}

fn gather_dom_images(node: &Node, out: &mut Vec<String>) {
    if let NodeKind::Element { name, attrs } = &node.kind {
        if name == "img" {
            if let Some((_, src)) = attrs.iter().find(|(k, _)| k == "src") {
                if !src.is_empty() && !out.iter().any(|x| x == src) {
                    out.push(src.clone());
                }
            }
        }
    }
    for c in &node.children {
        gather_dom_images(c, out);
    }
}

fn load_local_images(urls: &[String], base: &Path, store: &mut img::Store) {
    for u in urls {
        match fs::read(base.join(u)) {
            Ok(bytes) => match img::decode(&bytes) {
                Some(im) => store.insert(u.clone(), im),
                None => eprintln!("{}: unsupported image", u),
            },
            Err(e) => eprintln!("{}: {}", u, e),
        }
    }
}

fn load_remote_images(urls: &[String], base: &Url, store: &mut img::Store) {
    for u in urls {
        match net::resolve(base, u) {
            Ok(full) => match net::get_url(&full, 0) {
                Ok(resp) if resp.status == 200 => match img::decode(&resp.body) {
                    Some(im) => store.insert(u.clone(), im),
                    None => eprintln!("{}: unsupported image", u),
                },
                Ok(resp) => eprintln!("HTTP {} for {}", resp.status, full),
                Err(e) => eprintln!("{}: {}", full, e),
            },
            Err(e) => eprintln!("{}: {}", u, e),
        }
    }
}

fn collect_links(node: &Node, base: &Url, out: &mut String) {
    if let NodeKind::Element { name, attrs } = &node.kind {
        if name == "link" {
            let rel = attrs.iter().find(|(k, _)| k == "rel").map(|(_, v)| v);
            let href = attrs.iter().find(|(k, _)| k == "href").map(|(_, v)| v);
            let is_sheet = rel
                .map(|r| r.split_whitespace().any(|p| p.eq_ignore_ascii_case("stylesheet")))
                .unwrap_or(false);
            if is_sheet {
                if let Some(h) = href {
                    match net::resolve(base, h) {
                        Ok(u) => match net::get_url(&u, 0) {
                            Ok(resp) if resp.status == 200 => {
                                out.push_str(&String::from_utf8_lossy(&resp.body));
                                out.push('\n');
                            }
                            Ok(resp) => eprintln!("HTTP {} for {}", resp.status, u),
                            Err(e) => eprintln!("{}: {}", u, e),
                        },
                        Err(e) => eprintln!("{}: {}", h, e),
                    }
                }
            }
        }
    }
    for c in &node.children {
        collect_links(c, base, out);
    }
}

fn print_styled(node: &StyledNode, depth: usize) {
    if let NodeKind::Element { name, .. } = &node.node.kind {
        let mut props: Vec<_> = node.computed.iter().collect();
        props.sort_by_key(|(k, _)| *k);
        let p: Vec<String> = props
            .iter()
            .map(|(k, v)| format!("{}={}", k, vals(v)))
            .collect();
        let tag = match &node.pseudo {
            Some((kind, _)) => format!("{}::{}", name, kind),
            None => name.clone(),
        };
        let head = if p.is_empty() {
            format!("<{}>", tag)
        } else {
            format!("<{} {}>", tag, p.join(" "))
        };
        println!("{}{}", "  ".repeat(depth), head);
    }
    for c in &node.children {
        print_styled(c, depth + 1);
    }
}

fn vals(v: &[Value]) -> String {
    v.iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn fail<T>(e: std::io::Error) -> T {
    eprintln!("{}", e);
    exit(1);
}

fn usage() -> ! {
    eprintln!("usage: breaky <file> [style.css] [width] [out.png] [sel]");
    eprintln!("       breaky <file> <style.css> <target.css> <width> <out.png> [t]");
    eprintln!("       breaky fetch <url> [width] [out.png]");
    exit(2);
}
