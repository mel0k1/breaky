use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

use css::Value;
use html::{Node, NodeKind};
use breaky_js as js;
use net::Url;
use style::{build, build_selected, StyledNode};

fn main() {
    load_font();
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).unwrap_or_else(|| usage());
    if path == "fetch" {
        fetch(&args);
        return;
    }
    if path == "anim" {
        anim_cmd(&args);
        return;
    }
    if path == "animate" {
        animate_cmd(&args);
        return;
    }
    if path == "click" {
        click_cmd(&args);
        return;
    }
    if args.len() >= 6 {
        transition_page(&args);
        return;
    }
    let src = fs::read_to_string(path).unwrap_or_else(fail);
    let mut dom = html::parse(&src);
    html::print_tree(&dom, 0);
    if let Some(css) = args.get(2) {
        let mut css_text = fs::read_to_string(css).unwrap_or_else(fail);
        let mut doc_css = String::new();
        style::collect_css(&dom, &mut doc_css);
        css_text.push_str(&doc_css);
        let sheet = css::parse(&css_text);
        let selected = args.get(5).map(|s| s.as_str()) == Some("sel");
        let base_html = Path::new(path).parent().unwrap_or(Path::new("."));
        let mut scripts: Vec<Script> = Vec::new();
        gather_scripts(&dom, &mut scripts);
        run_scripts(&scripts, &mut dom, base_html, None);
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
        let mut anims = img::AnimStore::new();
        let mut urls: Vec<String> = Vec::new();
        gather_images(&styled, &mut urls);
        let base_css = Path::new(css).parent().unwrap_or(Path::new("."));
        load_local_images(&urls, base_css, &mut imgs, &mut anims);
        let mut srcs: Vec<String> = Vec::new();
        gather_dom_images(&dom, &mut srcs);
        load_local_images(&srcs, base_html, &mut imgs, &mut anims);
        load_faces(&sheet, &styled, base_css, None);
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

fn click_cmd(args: &[String]) {
    let src = args.get(2).unwrap_or_else(|| usage());
    let remote = src.contains("://");
    let (si, vi, oi) = if remote { (3, 4, 5) } else { (4, 5, 6) };
    let selector = args.get(si).unwrap_or_else(|| usage());
    let viewport = args
        .get(vi)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(800.0);
    let out = args.get(oi).map(|s| s.as_str()).unwrap_or("click.png");
    let mut client = net::Client::new();
    let (mut dom, css_text, base) = if remote {
        let (b, d, c) = remote_page(&mut client, src);
        (d, c, Some(b))
    } else {
        let html_text = fs::read_to_string(src).unwrap_or_else(fail);
        let dom = html::parse(&html_text);
        let mut css_text = args
            .get(3)
            .map(|p| fs::read_to_string(p).unwrap_or_else(fail))
            .unwrap_or_default();
        style::collect_css(&dom, &mut css_text);
        (dom, css_text, None)
    };
    let sheet = css::parse(&css_text);
    let mut scripts: Vec<Script> = Vec::new();
    gather_scripts(&dom, &mut scripts);
    let base_html = Path::new(src).parent().unwrap_or(Path::new("."));
    let mut rt = match &base {
        Some(b) => run_scripts(&scripts, &mut dom, Path::new("."), Some((&mut client, b))),
        None => run_scripts(&scripts, &mut dom, base_html, None),
    };
    match style::query_first(&dom, selector) {
        Some(p) => match rt.dispatch_event("click", &p, 0.0, 0.0) {
            Ok(true) => {}
            Ok(false) => eprintln!("{}: no click handler", selector),
            Err(e) => eprintln!("{}", e),
        },
        None => eprintln!("{}: no match", selector),
    }
    let styled = build(&dom, &sheet);
    let mut imgs = img::Store::new();
    let mut anims = img::AnimStore::new();
    let mut urls: Vec<String> = Vec::new();
    gather_images(&styled, &mut urls);
    let mut srcs: Vec<String> = Vec::new();
    gather_dom_images(&dom, &mut srcs);
    match &base {
        Some(b) => {
            load_remote_images(&mut client, &urls, b, &mut imgs, &mut anims);
            load_remote_images(&mut client, &srcs, b, &mut imgs, &mut anims);
            load_faces(&sheet, &styled, Path::new("."), Some((&mut client, b)));
        }
        None => {
            let css_base = args
                .get(3)
                .map(|p| Path::new(p).parent().unwrap_or(Path::new(".")))
                .unwrap_or(Path::new("."));
            load_local_images(&urls, css_base, &mut imgs, &mut anims);
            load_local_images(&srcs, base_html, &mut imgs, &mut anims);
            load_faces(&sheet, &styled, css_base, None);
        }
    }
    if let Some(tree) = layout::layout_with(&styled, viewport, &imgs) {
        let canvas = render::paint_with(&tree, viewport, &imgs);
        fs::write(out, canvas.to_png()).unwrap_or_else(fail);
        println!("saved {}", out);
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
    let mut anims = img::AnimStore::new();
    let mut urls: Vec<String> = Vec::new();
    gather_images(&merged, &mut urls);
    let base_css = Path::new(&args[2]).parent().unwrap_or(Path::new("."));
    load_local_images(&urls, base_css, &mut imgs, &mut anims);
    let mut srcs: Vec<String> = Vec::new();
    gather_dom_images(&dom, &mut srcs);
    let base_html = Path::new(&args[1]).parent().unwrap_or(Path::new("."));
    load_local_images(&srcs, base_html, &mut imgs, &mut anims);
    load_faces(&sheet_from, &merged, base_css, None);
    load_faces(&sheet_to, &merged, base_css, None);
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
    let mut client = net::Client::new();
    let (base, mut dom, css_text) = remote_page(&mut client, url);
    html::print_tree(&dom, 0);
    let sheet = css::parse(&css_text);
    let mut scripts: Vec<Script> = Vec::new();
    gather_scripts(&dom, &mut scripts);
    run_scripts(
        &scripts,
        &mut dom,
        Path::new("."),
        Some((&mut client, &base)),
    );
    let styled = build(&dom, &sheet);
    println!();
    print_styled(&styled, 0);
    let mut imgs = img::Store::new();
    let mut anims = img::AnimStore::new();
    let mut urls: Vec<String> = Vec::new();
    gather_images(&styled, &mut urls);
    let mut srcs: Vec<String> = Vec::new();
    gather_dom_images(&dom, &mut srcs);
    for s in srcs {
        if !urls.iter().any(|u| u == &s) {
            urls.push(s);
        }
    }
    load_remote_images(&mut client, &urls, &base, &mut imgs, &mut anims);
    load_faces(&sheet, &styled, Path::new("."), Some((&mut client, &base)));
    if let Some(tree) = layout::layout_with(&styled, viewport, &imgs) {
        println!();
        layout::print_tree(&tree, 0);
        let canvas = render::paint_with(&tree, viewport, &imgs);
        fs::write(out, canvas.to_png()).unwrap_or_else(fail);
        println!("saved {}", out);
    }
}

fn remote_page(client: &mut net::Client, url: &str) -> (Url, Node, String) {
    let base = net::parse_url(url).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit(1);
    });
    let resp = client.get(url).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit(1);
    });
    if resp.status != 200 {
        eprintln!("HTTP {}", resp.status);
        exit(1);
    }
    let dom = html::parse(&String::from_utf8_lossy(&resp.body));
    let mut css_text = String::new();
    collect_links(client, &dom, &base, &mut css_text);
    style::collect_css(&dom, &mut css_text);
    (base, dom, css_text)
}

fn anim_cmd(args: &[String]) {
    let src = args.get(2).unwrap_or_else(|| usage());
    let prefix = args.get(3).map(|s| s.as_str()).unwrap_or("frame");
    let bytes = if src.contains("://") {
        match net::get(src) {
            Ok(resp) if resp.status == 200 => resp.body,
            Ok(resp) => {
                eprintln!("HTTP {}", resp.status);
                exit(1);
            }
            Err(e) => {
                eprintln!("{}", e);
                exit(1);
            }
        }
    } else {
        fs::read(src).unwrap_or_else(fail)
    };
    let anim = img::decode_anim(&bytes).unwrap_or_else(|| {
        eprintln!("{}: unsupported image", src);
        exit(1);
    });
    for (i, f) in anim.frames.iter().enumerate() {
        let name = format!("{}.{}.png", prefix, i);
        fs::write(&name, render::Canvas::from_image(&f.image).to_png()).unwrap_or_else(fail);
        println!("{} {}ms", name, f.delay_ms);
    }
}

fn animate_cmd(args: &[String]) {
    let src = args.get(2).unwrap_or_else(|| usage());
    let remote = src.contains("://");
    let (vi, pi) = if remote { (3, 4) } else { (4, 5) };
    let viewport = args
        .get(vi)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(800.0);
    let prefix = args.get(pi).map(|s| s.as_str()).unwrap_or("frame");
    let mut client = net::Client::new();
    let (mut dom, css_text, base) = if remote {
        let (b, d, c) = remote_page(&mut client, src);
        (d, c, Some(b))
    } else {
        let html_text = fs::read_to_string(src).unwrap_or_else(fail);
        let dom = html::parse(&html_text);
        let mut css_text = String::new();
        if let Some(p) = args.get(3).filter(|p| !p.is_empty()) {
            css_text = fs::read_to_string(p).unwrap_or_else(fail);
        }
        style::collect_css(&dom, &mut css_text);
        (dom, css_text, None)
    };
    let sheet = css::parse(&css_text);
    let mut scripts: Vec<Script> = Vec::new();
    gather_scripts(&dom, &mut scripts);
    let base_html = Path::new(src).parent().unwrap_or(Path::new("."));
    match &base {
        Some(b) => run_scripts(
            &scripts,
            &mut dom,
            Path::new("."),
            Some((&mut client, b)),
        ),
        None => run_scripts(&scripts, &mut dom, base_html, None),
    };
    let styled = build(&dom, &sheet);
    let mut imgs = img::Store::new();
    let mut anims = img::AnimStore::new();
    let mut urls: Vec<String> = Vec::new();
    gather_images(&styled, &mut urls);
    let mut srcs: Vec<String> = Vec::new();
    gather_dom_images(&dom, &mut srcs);
    match &base {
        Some(b) => {
            load_remote_images(&mut client, &urls, b, &mut imgs, &mut anims);
            load_remote_images(&mut client, &srcs, b, &mut imgs, &mut anims);
            load_faces(&sheet, &styled, Path::new("."), Some((&mut client, b)));
        }
        None => {
            let base_css = args
                .get(3)
                .filter(|p| !p.is_empty())
                .map(|p| Path::new(p).parent().unwrap_or(Path::new(".")));
            load_local_images(
                &urls,
                base_css.unwrap_or(Path::new(".")),
                &mut imgs,
                &mut anims,
            );
            load_local_images(&srcs, base_html, &mut imgs, &mut anims);
            load_faces(&sheet, &styled, base_css.unwrap_or(Path::new(".")), None);
        }
    }
    let tree = match layout::layout_with(&styled, viewport, &imgs) {
        Some(t) => t,
        None => {
            eprintln!("layout failed");
            exit(1);
        }
    };
    let total = anims.duration();
    let start = std::time::Instant::now();
    let mut now = 0u32;
    let mut i = 0usize;
    loop {
        let canvas = render::paint_at(&tree, viewport, &imgs, &anims, now);
        let name = format!("{}.{}.png", prefix, i);
        fs::write(&name, canvas.to_png()).unwrap_or_else(fail);
        println!("{} {}ms", name, now);
        let d = anims.next_tick(now);
        if d == 0 || now + d >= total {
            break;
        }
        i += 1;
        now += d;
        let target = start + std::time::Duration::from_millis(now as u64);
        if let Some(w) = target.checked_duration_since(std::time::Instant::now()) {
            std::thread::sleep(w);
        }
    }
}

struct Script {
    src: Option<String>,
    text: String,
}

fn gather_scripts(node: &Node, out: &mut Vec<Script>) {
    if let NodeKind::Element { name, attrs } = &node.kind {
        if name == "script" {
            let mut text = String::new();
            collect_text(node, &mut text);
            let src = attrs
                .iter()
                .find(|(k, _)| k == "src")
                .map(|(_, v)| v.clone());
            if src.is_some() || !text.trim().is_empty() {
                out.push(Script { src, text });
            }
            return;
        }
    }
    for c in &node.children {
        gather_scripts(c, out);
    }
}

fn collect_text(node: &Node, out: &mut String) {
    if let NodeKind::Text(s) = &node.kind {
        out.push_str(s);
    }
    for c in &node.children {
        collect_text(c, out);
    }
}

fn run_scripts(
    scripts: &[Script],
    dom: &mut Node,
    base: &Path,
    remote: Option<(&mut net::Client, &Url)>,
) -> js::Runtime {
    let (mut client, url_base) = match remote {
        Some((c, b)) => (Some(c), Some(b)),
        None => (None, None),
    };
    let mut rt = js::Runtime::new();
    rt.bind_dom(dom);
    for s in scripts {
        let code = match (&s.src, url_base) {
            (Some(u), Some(b)) => match net::resolve(b, u) {
                Ok(full) => match client.as_deref_mut().unwrap().get_url(&full) {
                    Ok(r) if r.status == 200 => String::from_utf8_lossy(&r.body).into_owned(),
                    Ok(r) => {
                        eprintln!("{}: HTTP {}", u, r.status);
                        continue;
                    }
                    Err(e) => {
                        eprintln!("{}: {}", u, e);
                        continue;
                    }
                },
                Err(e) => {
                    eprintln!("{}: {}", u, e);
                    continue;
                }
            },
            (Some(u), None) => match fs::read(base.join(u)) {
                Ok(b) => String::from_utf8_lossy(&b).into_owned(),
                Err(e) => {
                    eprintln!("{}: {}", u, e);
                    continue;
                }
            },
            (None, _) => s.text.clone(),
        };
        if let Err(e) = rt.eval(&code) {
            eprintln!("{}", e);
        }
    }
    rt
}

fn load_faces(
    sheet: &css::Stylesheet,
    styled: &StyledNode,
    base: &Path,
    remote: Option<(&mut net::Client, &Url)>,
) {
    let mut used: Vec<String> = Vec::new();
    collect_families(styled, &mut used);
    let (mut client, url_base) = match remote {
        Some((c, b)) => (Some(c), Some(b)),
        None => (None, None),
    };
    for f in &sheet.faces {
        if !used.iter().any(|u| u == &f.family) {
            continue;
        }
        let bytes = match url_base {
            Some(b) => match net::resolve(b, &f.src) {
                Ok(u) => match client.as_deref_mut().unwrap().get_url(&u) {
                    Ok(r) if r.status == 200 => Ok(r.body),
                    Ok(r) => Err(format!("HTTP {}", r.status)),
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            },
            None => fs::read(base.join(&f.src)).map_err(|e| e.to_string()),
        };
        match bytes {
            Ok(b) => {
                if !font::register(&f.family, &b) {
                    eprintln!("{}: unsupported font", f.src);
                }
            }
            Err(e) => eprintln!("{}: {}", f.src, e),
        }
    }
}

fn collect_families(node: &StyledNode, out: &mut Vec<String>) {
    let f = style::family(&node.computed);
    if !f.is_empty() && !out.iter().any(|x| x == &f) {
        out.push(f);
    }
    for c in &node.children {
        collect_families(c, out);
    }
}

fn load_font() {
    if let Ok(p) = env::var("BREAKY_FONT") {
        if let Ok(b) = fs::read(&p) {
            if font::set(&b) {
                return;
            }
            eprintln!("{}: unsupported font", p);
        }
    }
    for p in font_paths() {
        if let Ok(b) = fs::read(&p) {
            if font::set(&b) {
                return;
            }
        }
    }
}

fn font_paths() -> Vec<&'static str> {
    vec![
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/local/share/fonts/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ]
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

fn load_local_images(
    urls: &[String],
    base: &Path,
    imgs: &mut img::Store,
    anims: &mut img::AnimStore,
) {
    for u in urls {
        match fs::read(base.join(u)) {
            Ok(bytes) => match img::decode_anim(&bytes) {
                Some(a) => {
                    if let Some(f) = a.frames.first() {
                        imgs.insert(u.clone(), f.image.clone());
                    }
                    if a.frames.len() > 1 {
                        anims.insert(u.clone(), a);
                    }
                }
                None => eprintln!("{}: unsupported image", u),
            },
            Err(e) => eprintln!("{}: {}", u, e),
        }
    }
}

fn load_remote_images(
    client: &mut net::Client,
    urls: &[String],
    base: &Url,
    imgs: &mut img::Store,
    anims: &mut img::AnimStore,
) {
    for u in urls {
        match net::resolve(base, u) {
            Ok(full) => match client.get_url(&full) {
                Ok(resp) if resp.status == 200 => match img::decode_anim(&resp.body) {
                    Some(a) => {
                        if let Some(f) = a.frames.first() {
                            imgs.insert(u.clone(), f.image.clone());
                        }
                        if a.frames.len() > 1 {
                            anims.insert(u.clone(), a);
                        }
                    }
                    None => eprintln!("{}: unsupported image", u),
                },
                Ok(resp) => eprintln!("HTTP {} for {}", resp.status, full),
                Err(e) => eprintln!("{}: {}", full, e),
            },
            Err(e) => eprintln!("{}: {}", u, e),
        }
    }
}

fn collect_links(client: &mut net::Client, node: &Node, base: &Url, out: &mut String) {
    if let NodeKind::Element { name, attrs } = &node.kind {
        if name == "link" {
            let rel = attrs.iter().find(|(k, _)| k == "rel").map(|(_, v)| v);
            let href = attrs.iter().find(|(k, _)| k == "href").map(|(_, v)| v);
            let is_sheet = rel
                .map(|r| {
                    r.split_whitespace()
                        .any(|p| p.eq_ignore_ascii_case("stylesheet"))
                })
                .unwrap_or(false);
            if is_sheet {
                if let Some(h) = href {
                    match net::resolve(base, h) {
                        Ok(u) => match client.get_url(&u) {
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
        collect_links(client, c, base, out);
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
    eprintln!("       breaky anim <file|url> [prefix]");
    eprintln!("       breaky animate <file> [style.css] [width] [prefix]");
    eprintln!("       breaky animate <url> [width] [prefix]");
    eprintln!("       breaky click <file> [style.css] <selector> [width] [out.png]");
    eprintln!("       breaky click <url> <selector> [width] [out.png]");
    exit(2);
}
