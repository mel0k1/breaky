use std::collections::HashMap;
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
        event_cmd(&args, "click");
        return;
    }
    if path == "mouse" {
        event_cmd(&args, "mouseover");
        return;
    }
    if path == "key" {
        event_cmd(&args, "keydown");
        return;
    }
    if path == "resize" {
        resize_cmd(&args);
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

fn event_cmd(args: &[String], typ: &str) {
    let src = args.get(2).unwrap_or_else(|| usage());
    let remote = src.contains("://");
    let si = if remote { 3 } else { 4 };
    let target_arg = args.get(si).unwrap_or_else(|| usage());
    let to_arg = if typ == "mouseover" || typ == "keydown" {
        args.get(si + 1).filter(|s| s.parse::<f32>().is_err())
    } else {
        None
    };
    let (vi, oi) = if to_arg.is_some() {
        (si + 2, si + 3)
    } else if remote {
        (4, 5)
    } else {
        (5, 6)
    };
    let viewport = args
        .get(vi)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(800.0);
    let out = args.get(oi).map(|s| s.as_str()).unwrap_or("event.png");
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
    let mut styled = build(&dom, &sheet);
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
    let point = if typ == "keydown" {
        None
    } else {
        parse_point(target_arg)
    };
    let mut rects: HashMap<Vec<usize>, (f32, f32, f32, f32)> = HashMap::new();
    if let Some(tree) = layout::layout_with(&styled, viewport, &imgs) {
        let mut map = HashMap::new();
        addr_paths(&dom, &mut map, &mut Vec::new());
        collect_rects(&tree, &map, &mut rects);
        rt.set_rects(&rects);
    }
    let target: Option<Vec<usize>> = if to_arg.is_some() {
        None
    } else if let Some((x, y)) = point {
        match layout::layout_with(&styled, viewport, &imgs) {
            Some(tree) => {
                let mut map = HashMap::new();
                addr_paths(&dom, &mut map, &mut Vec::new());
                match hit_test(&tree, x, y, &map) {
                    Some(p) => Some(p),
                    None => {
                        eprintln!("no element at {},{}", x, y);
                        None
                    }
                }
            }
            None => {
                eprintln!("empty layout");
                None
            }
        }
    } else if typ == "keydown" {
        match rt.active_path() {
            Some(p) => Some(p),
            None => Some(style::query_first(&dom, "body").unwrap_or_default()),
        }
    } else {
        match style::query_first(&dom, target_arg) {
            Some(p) => Some(p),
            None => {
                eprintln!("{}: no match", target_arg);
                None
            }
        }
    };
    if let Some(to) = &to_arg {
        if typ == "mouseover" {
            match resolve_target(&dom, &styled, viewport, &imgs, target_arg) {
                Ok(from) => match resolve_target(&dom, &styled, viewport, &imgs, to) {
                    Ok(dest) => {
                        if let Err(e) = rt.dispatch_move(&from.0, from.1, from.2) {
                            eprintln!("{}", e);
                        }
                        match rt.dispatch_move(&dest.0, dest.1, dest.2) {
                            Ok(true) => {}
                            Ok(false) => eprintln!("hover unchanged"),
                            Err(e) => eprintln!("{}", e),
                        }
                    }
                    Err(e) => eprintln!("{}", e),
                },
                Err(e) => eprintln!("{}", e),
            }
        } else {
            match resolve_target(&dom, &styled, viewport, &imgs, to) {
                Ok(t) => {
                    if let Err(e) = rt.focus(&t.0) {
                        eprintln!("{}", e);
                    }
                    match rt.dispatch_event("keydown", &t.0, t.1, t.2, target_arg) {
                        Ok(true) => {}
                        Ok(false) => eprintln!("{}: no keydown handler", to),
                        Err(e) => eprintln!("{}", e),
                    }
                }
                Err(e) => eprintln!("{}", e),
            }
        }
    } else if let Some(p) = &target {
        let (cx, cy) = if typ == "keydown" {
            (0.0, 0.0)
        } else {
            match point {
                Some(xy) => xy,
                None => match layout::layout_with(&styled, viewport, &imgs) {
                    Some(tree) => box_center(&tree, node_at(&dom, p)).unwrap_or((0.0, 0.0)),
                    None => (0.0, 0.0),
                },
            }
        };
        let key = if typ == "keydown" { target_arg.as_str() } else { "" };
        match rt.dispatch_event(typ, p, cx, cy, key) {
            Ok(true) => {}
            Ok(false) => eprintln!("{}: no {} handler", target_arg, typ),
            Err(e) => eprintln!("{}", e),
        }
    }
    styled = build(&dom, &sheet);
    urls.clear();
    gather_images(&styled, &mut urls);
    srcs.clear();
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

fn resize_cmd(args: &[String]) {
    let src = args.get(2).unwrap_or_else(|| usage());
    let remote = src.contains("://");
    let wi = if remote { 3 } else { 4 };
    let width = args
        .get(wi)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(800.0);
    let height = args
        .get(wi + 1)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(width);
    let oi = if args
        .get(wi + 1)
        .and_then(|s| s.parse::<f32>().ok())
        .is_some()
    {
        wi + 2
    } else {
        wi + 1
    };
    let out = args.get(oi).map(|s| s.as_str()).unwrap_or("resize.png");
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
    let mut styled = build(&dom, &sheet);
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
    if let Some(tree) = layout::layout_with(&styled, width, &imgs) {
        let mut map = HashMap::new();
        addr_paths(&dom, &mut map, &mut Vec::new());
        let mut rects: HashMap<Vec<usize>, (f32, f32, f32, f32)> = HashMap::new();
        collect_rects(&tree, &map, &mut rects);
        rt.set_rects(&rects);
    }
    rt.set_viewport(width, height);
    match rt.dispatch_event("resize", &[], 0.0, 0.0, "") {
        Ok(true) => {}
        Ok(false) => eprintln!("no resize handler"),
        Err(e) => eprintln!("{}", e),
    }
    styled = build(&dom, &sheet);
    urls.clear();
    gather_images(&styled, &mut urls);
    srcs.clear();
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
    if let Some(tree) = layout::layout_with(&styled, width, &imgs) {
        let canvas = render::paint_with(&tree, width, &imgs);
        fs::write(out, canvas.to_png()).unwrap_or_else(fail);
        println!("saved {}", out);
    }
}

fn parse_point(s: &str) -> Option<(f32, f32)> {
    let (a, b) = s.split_once(',')?;
    let x = a.trim().parse::<f32>().ok()?;
    let y = b.trim().parse::<f32>().ok()?;
    Some((x, y))
}

fn resolve_target(
    dom: &Node,
    styled: &StyledNode,
    viewport: f32,
    imgs: &img::Store,
    arg: &str,
) -> Result<(Vec<usize>, f32, f32), String> {
    if let Some((x, y)) = parse_point(arg) {
        let tree = match layout::layout_with(styled, viewport, imgs) {
            Some(t) => t,
            None => return Err("empty layout".to_string()),
        };
        let mut map = HashMap::new();
        addr_paths(dom, &mut map, &mut Vec::new());
        return match hit_test(&tree, x, y, &map) {
            Some(p) => Ok((p, x, y)),
            None => Err(format!("no element at {},{}", x, y)),
        };
    }
    let p = match style::query_first(dom, arg) {
        Some(p) => p,
        None => return Err(format!("{}: no match", arg)),
    };
    let tree = match layout::layout_with(styled, viewport, imgs) {
        Some(t) => t,
        None => return Err("empty layout".to_string()),
    };
    let (cx, cy) = box_center(&tree, node_at(dom, &p)).unwrap_or((0.0, 0.0));
    Ok((p, cx, cy))
}

fn addr_paths(dom: &Node, out: &mut HashMap<usize, Vec<usize>>, path: &mut Vec<usize>) {
    out.insert(dom as *const Node as usize, path.clone());
    for (i, c) in dom.children.iter().enumerate() {
        path.push(i);
        addr_paths(c, out, path);
        path.pop();
    }
}

fn node_at<'a>(root: &'a Node, path: &[usize]) -> &'a Node {
    let mut cur = root;
    for &i in path {
        cur = &cur.children[i];
    }
    cur
}

fn box_node<'a>(b: &'a layout::LayoutBox<'a>) -> Option<&'a Node> {
    match &b.kind {
        layout::BoxKind::Block(s)
        | layout::BoxKind::Inline(s)
        | layout::BoxKind::InlineBlock(s)
        | layout::BoxKind::Flex(s)
        | layout::BoxKind::Table(s)
        | layout::BoxKind::TableRow(s)
        | layout::BoxKind::TableCell(s) => Some(s.node),
        layout::BoxKind::Anonymous => None,
    }
}

fn hit_test(
    b: &layout::LayoutBox,
    x: f32,
    y: f32,
    map: &HashMap<usize, Vec<usize>>,
) -> Option<Vec<usize>> {
    for c in b.children.iter().chain(b.abs.iter()) {
        if let Some(p) = hit_test(c, x, y, map) {
            return Some(p);
        }
    }
    let node = box_node(b)?;
    node.name()?;
    let r = b.dims.border_box();
    if x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height {
        map.get(&(node as *const Node as usize)).cloned()
    } else {
        None
    }
}

fn box_center(b: &layout::LayoutBox, node: &Node) -> Option<(f32, f32)> {
    if let Some(n) = box_node(b) {
        if std::ptr::eq(n, node) {
            let r = b.dims.border_box();
            return Some((r.x + r.width / 2.0, r.y + r.height / 2.0));
        }
    }
    for c in b.children.iter().chain(b.abs.iter()) {
        if let Some(p) = box_center(c, node) {
            return Some(p);
        }
    }
    None
}

fn collect_rects(
    b: &layout::LayoutBox,
    map: &HashMap<usize, Vec<usize>>,
    out: &mut HashMap<Vec<usize>, (f32, f32, f32, f32)>,
) {
    for c in b.children.iter().chain(b.abs.iter()) {
        collect_rects(c, map, out);
    }
    let node = match box_node(b) {
        Some(n) => n,
        None => return,
    };
    let path = match map.get(&(node as *const Node as usize)) {
        Some(p) => p.clone(),
        None => return,
    };
    let r = b.dims.border_box();
    let e = out
        .entry(path)
        .or_insert((r.x, r.y, r.x + r.width, r.y + r.height));
    e.0 = e.0.min(r.x);
    e.1 = e.1.min(r.y);
    e.2 = e.2.max(r.x + r.width);
    e.3 = e.3.max(r.y + r.height);
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
    eprintln!("       breaky click <file> [style.css] <selector|X,Y> [width] [out.png]");
    eprintln!("       breaky click <url> <selector|X,Y> [width] [out.png]");
    eprintln!("       breaky mouse <file> [style.css] <selector|X,Y> [width] [out.png]");
    eprintln!("       breaky mouse <file> [style.css] <from> <to> [width] [out.png]");
    eprintln!("       breaky mouse <url> <selector|X,Y> [width] [out.png]");
    eprintln!("       breaky mouse <url> <from> <to> [width] [out.png]");
    eprintln!("       breaky key <file> [style.css] <key> [<selector|X,Y>] [width] [out.png]");
    eprintln!("       breaky resize <file> [style.css] <width> [height] [out.png]");
    eprintln!("       breaky resize <url> <width> [height] [out.png]");
    exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Node {
        html::parse(
            "<html><head><style>#outer{width:220px;height:110px;background:#3355aa;padding:10px}#inner{width:60px;height:40px;background:#cc4444}</style></head><body><div id=\"outer\"><div id=\"inner\"></div></div></body></html>",
        )
    }

    #[test]
    fn hit_test_finds_deepest_element() {
        let dom = page();
        let mut css_text = String::new();
        style::collect_css(&dom, &mut css_text);
        let sheet = css::parse(&css_text);
        let styled = build(&dom, &sheet);
        let tree = layout::layout_with(&styled, 800.0, &img::Store::new()).unwrap();
        let mut map = HashMap::new();
        addr_paths(&dom, &mut map, &mut Vec::new());
        let inner = style::query_first(&dom, "#inner").unwrap();
        let outer = style::query_first(&dom, "#outer").unwrap();
        let body = style::query_first(&dom, "body").unwrap();
        assert_eq!(hit_test(&tree, 15.0, 15.0, &map), Some(inner));
        assert_eq!(hit_test(&tree, 235.0, 90.0, &map).as_deref(), Some(&outer[..]));
        assert_eq!(hit_test(&tree, 250.0, 120.0, &map).as_deref(), Some(&body[..]));
    }

    #[test]
    fn hit_test_misses_empty_space() {
        let dom = page();
        let mut css_text = String::new();
        style::collect_css(&dom, &mut css_text);
        let sheet = css::parse(&css_text);
        let styled = build(&dom, &sheet);
        let tree = layout::layout_with(&styled, 800.0, &img::Store::new()).unwrap();
        let mut map = HashMap::new();
        addr_paths(&dom, &mut map, &mut Vec::new());
        assert_eq!(hit_test(&tree, 700.0, 600.0, &map), None);
    }

    #[test]
    fn box_center_targets_element_rect() {
        let dom = page();
        let mut css_text = String::new();
        style::collect_css(&dom, &mut css_text);
        let sheet = css::parse(&css_text);
        let styled = build(&dom, &sheet);
        let tree = layout::layout_with(&styled, 800.0, &img::Store::new()).unwrap();
        let inner_path = style::query_first(&dom, "#inner").unwrap();
        let node = node_at(&dom, &inner_path);
        let (cx, cy) = box_center(&tree, node).unwrap();
        let mut map = HashMap::new();
        addr_paths(&dom, &mut map, &mut Vec::new());
        assert_eq!(hit_test(&tree, cx, cy, &map), Some(inner_path));
    }

    #[test]
    fn parse_point_reads_coords() {
        assert_eq!(parse_point("120,80"), Some((120.0, 80.0)));
        assert_eq!(parse_point("12.5 , 7"), Some((12.5, 7.0)));
        assert_eq!(parse_point("#btn"), None);
        assert_eq!(parse_point("12,"), None);
    }
}
