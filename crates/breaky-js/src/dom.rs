use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

use html::{inner_html, parse, Node, NodeKind};
use style::query_first;

use super::{
    breaky_free_value, breaky_is_exception, breaky_to_cstring, cstr, to_string,
    JSContext, JSCFunction, JSValue, JSValueUnion, EVAL_GLOBAL,
};
use super::{
    JS_Eval, JS_FreeCString, JS_GetContextOpaque, JS_GetException, JS_GetGlobalObject,
    JS_NewCFunction2, JS_NewStringLen, JS_SetContextOpaque, JS_SetPropertyStr,
};

const BOOTSTRAP: &str = include_str!("bootstrap.js");

const TAG_NULL: i64 = 2;
const TAG_UNDEFINED: i64 = 3;
const CFUNC_GENERIC: c_int = 0;

pub(super) struct DomState {
    root: *mut Node,
    parking: Vec<Option<Node>>,
}

const FUNCTIONS: &[(&str, JSCFunction)] = &[
    ("__doc_by_id", doc_by_id),
    ("__doc_body", doc_body),
    ("__doc_create", doc_create),
    ("__doc_query", doc_query),
    ("__el_tag", el_tag),
    ("__el_attr", el_attr),
    ("__el_set_attr", el_set_attr),
    ("__el_append", el_append),
    ("__el_html", el_html),
    ("__el_set_html", el_set_html),
    ("__el_text", el_text),
    ("__el_set_text", el_set_text),
    ("__style_get", style_get),
    ("__style_set", style_set),
];

pub(super) fn install(rt: &mut super::Runtime, root: &mut Node) {
    let st = Box::into_raw(Box::new(DomState {
        root: root as *mut Node,
        parking: Vec::new(),
    }));
    rt.dom = st;
    unsafe {
        JS_SetContextOpaque(rt.ctx, st as *mut c_void);
        for (name, f) in FUNCTIONS {
            register(rt.ctx, name, *f);
        }
        let src = cstr(BOOTSTRAP);
        let name = cstr("dom.js");
        let v = JS_Eval(
            rt.ctx,
            src.as_ptr(),
            src.as_bytes().len(),
            name.as_ptr(),
            EVAL_GLOBAL,
        );
        if breaky_is_exception(v) != 0 {
            let e = JS_GetException(rt.ctx);
            let msg = to_string(rt.ctx, e);
            breaky_free_value(rt.ctx, e);
            eprintln!("{}", msg);
        } else {
            breaky_free_value(rt.ctx, v);
        }
    }
}

unsafe fn register(ctx: *mut JSContext, name: &str, f: JSCFunction) {
    let cname = cstr(name);
    let global = JS_GetGlobalObject(ctx);
    let func = JS_NewCFunction2(ctx, f, cname.as_ptr(), 1, CFUNC_GENERIC, 0);
    JS_SetPropertyStr(ctx, global, cname.as_ptr(), func);
    breaky_free_value(ctx, global);
}

unsafe fn opaque(ctx: *mut JSContext) -> &'static mut DomState {
    let p = JS_GetContextOpaque(ctx) as *mut DomState;
    &mut *p
}

unsafe fn arg(ctx: *mut JSContext, argc: c_int, argv: *const JSValue, i: usize) -> String {
    if i as c_int >= argc {
        return String::new();
    }
    let s = breaky_to_cstring(ctx, *argv.add(i));
    if s.is_null() {
        return String::new();
    }
    let out = CStr::from_ptr(s).to_string_lossy().into_owned();
    JS_FreeCString(ctx, s);
    out
}

unsafe fn js_string(ctx: *mut JSContext, s: &str) -> JSValue {
    JS_NewStringLen(ctx, s.as_ptr() as *const c_char, s.len())
}

fn js_null() -> JSValue {
    JSValue {
        u: JSValueUnion { as_int32: 0 },
        tag: TAG_NULL,
    }
}

fn js_undefined() -> JSValue {
    JSValue {
        u: JSValueUnion { as_int32: 0 },
        tag: TAG_UNDEFINED,
    }
}

fn encode_path(p: &[usize]) -> String {
    p.iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(".")
}

enum Slot {
    Tree(Vec<usize>),
    Parked { slot: usize, inner: Vec<usize> },
}

fn decode_path(s: &str) -> Option<Slot> {
    if let Some(rest) = s.strip_prefix('!') {
        let mut it = rest.split('.');
        let slot = it.next()?.parse::<usize>().ok()?;
        let inner = it
            .map(|x| x.parse::<usize>().ok())
            .collect::<Option<Vec<_>>>()?;
        return Some(Slot::Parked { slot, inner });
    }
    if s.is_empty() {
        return None;
    }
    s.split('.')
        .map(|x| x.parse::<usize>().ok())
        .collect::<Option<Vec<_>>>()
        .map(Slot::Tree)
}

fn slot_key(s: &Slot) -> String {
    match s {
        Slot::Tree(p) => encode_path(p),
        Slot::Parked { slot, inner } => {
            if inner.is_empty() {
                format!("!{}", slot)
            } else {
                format!("!{}.{}", slot, encode_path(inner))
            }
        }
    }
}

fn resolve_mut<'a>(root: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let mut cur = root;
    for &i in path {
        cur = cur.children.get_mut(i)?;
    }
    Some(cur)
}

fn find_first(
    node: &Node,
    pred: &dyn Fn(&Node) -> bool,
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    if pred(node) {
        return Some(path.clone());
    }
    for (i, c) in node.children.iter().enumerate() {
        path.push(i);
        if let Some(p) = find_first(c, pred, path) {
            return Some(p);
        }
        path.pop();
    }
    None
}

fn has_id(node: &Node, id: &str) -> bool {
    matches!(&node.kind, NodeKind::Element { attrs, .. } if attrs.iter().any(|(k, v)| k == "id" && v == id))
}

fn with_any<R>(st: &mut DomState, path: &str, f: impl FnOnce(&mut Node) -> R) -> Option<R> {
    match decode_path(path)? {
        Slot::Tree(p) => resolve_mut(unsafe { &mut *st.root }, &p).map(f),
        Slot::Parked { slot, inner } => {
            let n = st.parking.get_mut(slot)?.as_mut()?;
            resolve_mut(n, &inner).map(f)
        }
    }
}

fn owner_mut<'a>(st: &'a mut DomState, s: &Slot) -> Option<&'a mut Node> {
    match s {
        Slot::Tree(p) => resolve_mut(unsafe { &mut *st.root }, p),
        Slot::Parked { slot, inner } => {
            let n = st.parking.get_mut(*slot)?.as_mut()?;
            resolve_mut(n, inner)
        }
    }
}

fn take_child(st: &mut DomState, cs: &Slot) -> Option<Node> {
    match cs {
        Slot::Parked { slot, inner } if inner.is_empty() => {
            st.parking.get_mut(*slot).and_then(|s| s.take())
        }
        Slot::Parked { slot, inner } => {
            let d = inner.len().checked_sub(1)?;
            let p = owner_mut(
                st,
                &Slot::Parked {
                    slot: *slot,
                    inner: inner[..d].to_vec(),
                },
            )?;
            if !matches!(p.kind, NodeKind::Element { .. }) || inner[d] >= p.children.len() {
                return None;
            }
            Some(p.children.remove(inner[d]))
        }
        Slot::Tree(cpath) => {
            let d = cpath.len().checked_sub(1)?;
            let p = owner_mut(st, &Slot::Tree(cpath[..d].to_vec()))?;
            if !matches!(p.kind, NodeKind::Element { .. }) || cpath[d] >= p.children.len() {
                return None;
            }
            Some(p.children.remove(cpath[d]))
        }
    }
}

fn append(st: &mut DomState, ps: Slot, cs: Slot) -> Option<(String, Option<String>)> {
    let mut parent = ps;
    let mut moved_parent: Option<String> = None;
    if let (Slot::Tree(ref ppath), Slot::Tree(cpath)) = (&parent, &cs) {
        if ppath.len() >= cpath.len() && ppath[..cpath.len()] == cpath[..] {
            return None;
        }
        let d = cpath.len() - 1;
        if ppath.len() > d && ppath[..d] == cpath[..d] && ppath[d] > cpath[d] {
            let mut fixed = ppath.clone();
            fixed[d] -= 1;
            moved_parent = Some(encode_path(&fixed));
            parent = Slot::Tree(fixed);
        }
    }
    let node = take_child(st, &cs)?;
    let idx = owner_mut(st, &parent)?.children.len();
    owner_mut(st, &parent)?.children.push(node);
    Some((format!("{}.{}", slot_key(&parent), idx), moved_parent))
}

fn text_content(node: &Node, out: &mut String) {
    if let NodeKind::Text(t) = &node.kind {
        out.push_str(t);
    }
    for c in &node.children {
        text_content(c, out);
    }
}

fn upsert_attr(attrs: &mut Vec<(String, String)>, key: &str, val: String) {
    if let Some(slot) = attrs.iter_mut().find(|(k, _)| k == key) {
        slot.1 = val;
    } else {
        attrs.push((key.to_string(), val));
    }
}

fn parse_style(attrs: &[(String, String)]) -> Vec<(String, String)> {
    let raw = attrs
        .iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    raw.split(';')
        .filter_map(|part| {
            let (k, v) = part.split_once(':')?;
            let k = k.trim().to_lowercase();
            if k.is_empty() {
                return None;
            }
            Some((k, v.trim().to_string()))
        })
        .collect()
}

fn set_style_prop(node: &mut Node, prop: &str, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut node.kind else {
        return;
    };
    let prop = prop.trim().to_lowercase();
    if prop.is_empty() {
        return;
    }
    let mut m = parse_style(attrs);
    if value.trim().is_empty() {
        m.retain(|(k, _)| *k != prop);
    } else if let Some(slot) = m.iter_mut().find(|(k, _)| *k == prop) {
        slot.1 = value.trim().to_string();
    } else {
        m.push((prop, value.trim().to_string()));
    }
    let ser = m
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect::<Vec<_>>()
        .join("; ");
    if ser.is_empty() {
        attrs.retain(|(k, _)| k != "style");
    } else {
        upsert_attr(attrs, "style", ser);
    }
}

unsafe extern "C" fn doc_by_id(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let id = arg(ctx, argc, argv, 0);
    let root = &*st.root;
    match find_first(root, &|n| has_id(n, &id), &mut Vec::new()) {
        Some(p) => js_string(ctx, &encode_path(&p)),
        None => js_null(),
    }
}

unsafe extern "C" fn doc_body(
    ctx: *mut JSContext,
    _this: JSValue,
    _argc: c_int,
    _argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let root = &*st.root;
    match find_first(root, &|n| n.name() == Some("body"), &mut Vec::new()) {
        Some(p) => js_string(ctx, &encode_path(&p)),
        None => js_null(),
    }
}

unsafe extern "C" fn doc_create(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let tag = arg(ctx, argc, argv, 0).to_lowercase();
    let valid = tag
        .chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false);
    if !valid {
        return js_null();
    }
    let slot = match st.parking.iter().position(|s| s.is_none()) {
        Some(i) => i,
        None => {
            st.parking.push(None);
            st.parking.len() - 1
        }
    };
    st.parking[slot] = Some(Node::new(NodeKind::Element {
        name: tag,
        attrs: Vec::new(),
    }));
    js_string(ctx, &format!("!{}", slot))
}
unsafe extern "C" fn doc_query(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let sel = arg(ctx, argc, argv, 0);
    let root = unsafe { &*st.root };
    match query_first(root, &sel) {
        Some(p) => js_string(ctx, &encode_path(&p)),
        None => js_null(),
    }
}

unsafe extern "C" fn el_append(
    ctx: *mut JSContext,
    this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let pp = arg(ctx, argc, argv, 0);
    let cp = arg(ctx, argc, argv, 1);
    let ps = match decode_path(&pp) {
        Some(s) => s,
        None => return js_null(),
    };
    let cs = match decode_path(&cp) {
        Some(s) => s,
        None => return js_null(),
    };
    let Some((child_path, parent_fix)) = append(st, ps, cs) else {
        return js_null();
    };
    if argc >= 3 {
        set_path_prop(ctx, *argv.add(2), &child_path);
    }
    if let Some(fixed) = parent_fix {
        set_path_prop(ctx, this, &fixed);
    }
    js_string(ctx, &child_path)
}

unsafe fn set_path_prop(ctx: *mut JSContext, obj: JSValue, path: &str) {
    if obj.tag == TAG_NULL || obj.tag == TAG_UNDEFINED {
        return;
    }
    let key = cstr("__path");
    let val = js_string(ctx, path);
    JS_SetPropertyStr(ctx, obj, key.as_ptr(), val);
}

unsafe extern "C" fn el_tag(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    match with_any(st, &p, |n| n.name().map(|s| s.to_string())) {
        Some(Some(name)) => js_string(ctx, &name),
        _ => js_null(),
    }
}

unsafe extern "C" fn el_attr(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    let name = arg(ctx, argc, argv, 1).to_lowercase();
    let found = with_any(st, &p, |node| match &node.kind {
        NodeKind::Element { attrs, .. } => {
            attrs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone())
        }
        _ => None,
    })
    .flatten();
    match found {
        Some(v) => js_string(ctx, &v),
        None => js_null(),
    }
}

unsafe extern "C" fn el_set_attr(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    let name = arg(ctx, argc, argv, 1).to_lowercase();
    let val = arg(ctx, argc, argv, 2);
    if !name.is_empty() {
        with_any(st, &p, |node| {
            if let NodeKind::Element { attrs, .. } = &mut node.kind {
                upsert_attr(attrs, &name, val);
            }
        });
    }
    js_undefined()
}

unsafe extern "C" fn el_html(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    match with_any(st, &p, |node| inner_html(node)) {
        Some(s) => js_string(ctx, &s),
        None => js_string(ctx, ""),
    }
}

unsafe extern "C" fn el_set_html(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    let frag = arg(ctx, argc, argv, 1);
    with_any(st, &p, |node| {
        if matches!(node.kind, NodeKind::Element { .. }) {
            node.children = parse(&frag).children;
        }
    });
    js_undefined()
}

unsafe extern "C" fn el_text(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    let s = with_any(st, &p, |node| {
        let mut out = String::new();
        text_content(node, &mut out);
        out
    })
    .unwrap_or_default();
    js_string(ctx, &s)
}

unsafe extern "C" fn el_set_text(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    let text = arg(ctx, argc, argv, 1);
    with_any(st, &p, |node| {
        if matches!(node.kind, NodeKind::Element { .. }) {
            node.children = vec![Node::new(NodeKind::Text(text))];
        }
    });
    js_undefined()
}

unsafe extern "C" fn style_get(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    let name = arg(ctx, argc, argv, 1).to_lowercase();
    let v = with_any(st, &p, |node| {
        parse_style(attr_slice(node))
            .into_iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v)
    })
    .flatten();
    match v {
        Some(v) => js_string(ctx, &v),
        None => js_string(ctx, ""),
    }
}

unsafe extern "C" fn style_set(
    ctx: *mut JSContext,
    _this: JSValue,
    argc: c_int,
    argv: *const JSValue,
) -> JSValue {
    let st = opaque(ctx);
    let p = arg(ctx, argc, argv, 0);
    let name = arg(ctx, argc, argv, 1);
    let val = arg(ctx, argc, argv, 2);
    with_any(st, &p, |node| set_style_prop(node, &name, &val));
    js_undefined()
}

fn attr_slice(node: &Node) -> &[(String, String)] {
    match &node.kind {
        NodeKind::Element { attrs, .. } => attrs,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Runtime;

    fn by_id<'a>(root: &'a Node, id: &str) -> &'a Node {
        fn walk<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
            if has_id(n, id) {
                return Some(n);
            }
            n.children.iter().find_map(|c| walk(c, id))
        }
        walk(root, id).unwrap()
    }

    fn tree() -> Node {
        parse("<html><body><p id=\"a\">hi</p><p id=\"b\">yo</p></body></html>")
    }

    fn eval_on(dom: &mut Node, code: &str) -> String {
        let mut rt = Runtime::new();
        rt.bind_dom(dom);
        rt.eval(code).unwrap()
    }

    fn by_name<'a>(root: &'a Node, name: &str) -> &'a Node {
        fn walk<'a>(n: &'a Node, name: &str) -> Option<&'a Node> {
            if n.name() == Some(name) {
                return Some(n);
            }
            n.children.iter().find_map(|c| walk(c, name))
        }
        walk(root, name).unwrap()
    }

    fn style_attr(dom: &Node, id: &str) -> Option<String> {
        match &by_id(dom, id).kind {
            NodeKind::Element { attrs, .. } => attrs
                .iter()
                .find(|(k, _)| k == "style")
                .map(|(_, v)| v.clone()),
            _ => None,
        }
    }

    #[test]
    fn get_element_by_id() {
        let mut dom = tree();
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').tagName"),
            "P"
        );
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').getAttribute('id')"),
            "a"
        );
    }

    #[test]
    fn missing_element_and_attribute_are_null() {
        let mut dom = tree();
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('zz') === null"),
            "true"
        );
        assert_eq!(
            eval_on(
                &mut dom,
                "document.getElementById('a').getAttribute('nope') === null"
            ),
            "true"
        );
    }

    #[test]
    fn inner_html_read() {
        let mut dom = tree();
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').innerHTML"),
            "hi"
        );
    }

    #[test]
    fn inner_html_set() {
        let mut dom = tree();
        eval_on(&mut dom, "document.getElementById('a').innerHTML = '<b>x</b>y'");
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').innerHTML"),
            "<b>x</b>y"
        );
        let p = by_id(&dom, "a");
        assert_eq!(p.children.len(), 2);
        assert_eq!(p.children[0].name(), Some("b"));
        assert_eq!(
            p.children[0].children[0],
            Node::new(NodeKind::Text("x".into()))
        );
        assert_eq!(p.children[1], Node::new(NodeKind::Text("y".into())));
    }

    #[test]
    fn inner_html_multi_root_fragment() {
        let mut dom = tree();
        eval_on(&mut dom, "document.getElementById('a').innerHTML = 'a<br>b'");
        let p = by_id(&dom, "a");
        assert_eq!(p.children.len(), 3);
        assert_eq!(p.children[1].name(), Some("br"));
    }

    #[test]
    fn text_content_roundtrip() {
        let mut dom = tree();
        assert_eq!(eval_on(&mut dom, "document.body.textContent"), "hiyo");
        let mut dom = tree();
        eval_on(&mut dom, "document.getElementById('a').textContent = 'z & <w>'");
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').textContent"),
            "z & <w>"
        );
        let p = by_id(&dom, "a");
        assert_eq!(
            p.children,
            vec![Node::new(NodeKind::Text("z & <w>".into()))]
        );
    }

    #[test]
    fn attribute_roundtrip() {
        let mut dom = tree();
        eval_on(
            &mut dom,
            "document.getElementById('a').setAttribute('data-k', 'v')",
        );
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').getAttribute('data-k')"),
            "v"
        );
        match &by_id(&dom, "a").kind {
            NodeKind::Element { attrs, .. } => {
                assert!(attrs.contains(&("data-k".into(), "v".into())));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn style_set_and_read() {
        let mut dom = tree();
        eval_on(&mut dom, "document.getElementById('a').style.color = 'red'");
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').style.color"),
            "red"
        );
        assert_eq!(style_attr(&dom, "a").as_deref(), Some("color: red"));
    }

    #[test]
    fn style_multiple_props_and_override() {
        let mut dom = tree();
        eval_on(
            &mut dom,
            "var s = document.getElementById('a').style; s.color = 'red'; s['font-size'] = '12px'",
        );
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').style.color"),
            "red"
        );
        assert_eq!(
            style_attr(&dom, "a").as_deref(),
            Some("color: red; font-size: 12px")
        );
        eval_on(&mut dom, "document.getElementById('a').style.color = 'blue'");
        assert_eq!(
            style_attr(&dom, "a").as_deref(),
            Some("color: blue; font-size: 12px")
        );
    }

    #[test]
    fn style_unset_reads_empty() {
        let mut dom = tree();
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').style.color"),
            ""
        );
    }

    #[test]
    fn style_empty_value_clears_prop() {
        let mut dom = tree();
        eval_on(&mut dom, "var s = document.getElementById('a').style; s.color = 'red'; s.color = ''");
        assert_eq!(
            eval_on(&mut dom, "document.getElementById('a').style.color"),
            ""
        );
        assert_eq!(style_attr(&dom, "a"), None);
    }

    #[test]
    fn document_body_binding() {
        let mut dom = tree();
        assert_eq!(eval_on(&mut dom, "document.body.tagName"), "BODY");
        eval_on(&mut dom, "document.body.style.margin = '4px'");
        assert_eq!(
            eval_on(&mut dom, "document.body.style.margin"),
            "4px"
        );
        match &by_name(&dom, "body").kind {
            NodeKind::Element { attrs, .. } => {
                assert!(attrs.contains(&("style".into(), "margin: 4px".into())));
            }
            _ => panic!(),
        }
        assert_eq!(
            by_id(&dom, "a"),
            &{
                let mut n = Node::new(NodeKind::Element {
                    name: "p".into(),
                    attrs: vec![("id".into(), "a".into())],
                });
                n.children.push(Node::new(NodeKind::Text("hi".into())));
                n
            }
        );
    }

    #[test]
    fn element_reference_persists_between_scripts() {
        let mut dom = tree();
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        rt.eval("var el = document.getElementById('a')").unwrap();
        rt.eval("el.textContent = 'persist'").unwrap();
        let p = by_id(&dom, "a");
        assert_eq!(p.children, vec![Node::new(NodeKind::Text("persist".into()))]);
    }

    #[test]
    fn mutation_visible_to_next_read() {
        let mut dom = tree();
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        rt.eval("document.getElementById('a').innerHTML = '<i>one</i>'")
            .unwrap();
        assert_eq!(
            rt.eval("document.getElementById('a').innerHTML").unwrap(),
            "<i>one</i>"
        );
    }

    #[test]
    fn create_append_read_back() {
        let mut dom = tree();
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        rt.eval(
            "var d = document.createElement('div'); d.setAttribute('class', 'new'); d.textContent = 'made in js'; document.body.appendChild(d)",
        )
        .unwrap();
        let body = by_name(&dom, "body");
        assert_eq!(body.children.len(), 3);
        match &body.children[2].kind {
            NodeKind::Element { name, attrs } => {
                assert_eq!(name, "div");
                assert!(attrs.contains(&("class".into(), "new".into())));
            }
            _ => panic!(),
        }
        assert_eq!(
            body.children[2].children,
            vec![Node::new(NodeKind::Text("made in js".into()))]
        );
        assert_eq!(
            rt.eval("document.querySelector('div.new').textContent")
                .unwrap(),
            "made in js"
        );
    }

    #[test]
    fn detached_attrs_persist_until_insert() {
        let mut dom = tree();
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        rt.eval(
            "var d = document.createElement('p'); d.style.color = 'red'; d.setAttribute('data-x', '1'); document.body.appendChild(d)",
        )
        .unwrap();
        let body = by_name(&dom, "body");
        match &body.children[2].kind {
            NodeKind::Element { attrs, .. } => {
                assert!(attrs.contains(&("style".into(), "color: red".into())));
                assert!(attrs.contains(&("data-x".into(), "1".into())));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn append_moves_existing_node() {
        let mut dom = parse(
            "<html><body><div id=\"a\"><p id=\"x\">1</p><p id=\"y\">2</p></div></body></html>",
        );
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        rt.eval("document.getElementById('a').appendChild(document.getElementById('x'))")
            .unwrap();
        let div = by_id(&dom, "a");
        assert_eq!(div.children.len(), 2);
        assert_eq!(div.children[0].name(), Some("p"));
        match &div.children[0].kind {
            NodeKind::Element { attrs, .. } => {
                assert!(attrs.iter().any(|(k, v)| k == "id" && v == "y"));
            }
            _ => panic!(),
        }
        assert_eq!(
            div.children[1].name(),
            Some("p"),
            "moved node must be last"
        );
        assert_eq!(
            rt.eval("document.querySelector('#a p').textContent").unwrap(),
            "2"
        );
    }

    #[test]
    fn append_rejects_cycles() {
        let mut dom = parse(
            "<html><body><div id=\"a\"><p id=\"x\">1</p></div></body></html>",
        );
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        assert_eq!(
            rt.eval("document.getElementById('a').appendChild(document.getElementById('a')) === null")
                .unwrap(),
            "true"
        );
        assert_eq!(
            rt.eval("document.getElementById('x').appendChild(document.getElementById('a')) === null")
                .unwrap(),
            "true"
        );
        let div = by_id(&dom, "a");
        assert_eq!(div.children.len(), 1);
    }

    #[test]
    fn append_returns_valid_wrapper() {
        let mut dom = tree();
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        rt.eval("var w = document.body.appendChild(document.createElement('b')); w.textContent = 'after'")
            .unwrap();
        let body = by_name(&dom, "body");
        assert_eq!(body.children[2].name(), Some("b"));
        assert_eq!(
            body.children[2].children,
            vec![Node::new(NodeKind::Text("after".into()))]
        );
    }

    #[test]
    fn detached_parent_chain() {
        let mut dom = tree();
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        rt.eval(
            "var p = document.createElement('p'); var b = document.createElement('b'); p.appendChild(b); b.textContent = 'deep'; document.body.appendChild(p)",
        )
        .unwrap();
        let body = by_name(&dom, "body");
        assert_eq!(body.children[2].name(), Some("p"));
        assert_eq!(body.children[2].children[0].name(), Some("b"));
        assert_eq!(
            body.children[2].children[0].children,
            vec![Node::new(NodeKind::Text("deep".into()))]
        );
    }

    #[test]
    fn query_selector_bindings() {
        let mut dom = parse(
            "<html><body><ul id=\"list\"><li class=\"on\">a</li><li>b</li></ul></body></html>",
        );
        let mut rt = Runtime::new();
        rt.bind_dom(&mut dom);
        assert_eq!(
            rt.eval("document.querySelector('.on').textContent").unwrap(),
            "a"
        );
        assert_eq!(
            rt.eval("document.querySelector('#list').tagName").unwrap(),
            "UL"
        );
        assert_eq!(
            rt.eval("document.querySelector('ul li').tagName").unwrap(),
            "LI"
        );
        assert_eq!(
            rt.eval("document.querySelector('ol li') === null").unwrap(),
            "true"
        );
        assert_eq!(
            rt.eval("document.querySelector('###') === null").unwrap(),
            "true"
        );
    }
}
