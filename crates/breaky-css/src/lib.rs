use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum SimpleSelector {
    Universal,
    Tag(String),
    Id(String),
    Class(String),
    Pseudo(String),
}

#[derive(Debug, PartialEq)]
pub struct Selector {
    pub compounds: Vec<Vec<SimpleSelector>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Keyword(String),
    Length(f32, Unit),
    Color(Color),
    Url(String),
    Gradient(Gradient),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    pub dir: Dir,
    pub stops: Vec<(Color, Option<Stop>)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dir {
    Angle(f32),
    Corner(i8, i8),
    Radial(Shape, Size, Stop, Stop),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Circle,
    Ellipse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    ClosestSide,
    FarthestSide,
    ClosestCorner,
    FarthestCorner,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stop {
    Pct(f32),
    Px(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    Px,
    Em,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: Vec<Value>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Keyword(k) => write!(f, "{}", k),
            Value::Length(n, u) => write!(f, "{}{}", n, u),
            Value::Color(c) => write!(f, "{}", c),
            Value::Url(u) => write!(f, "url({})", u),
            Value::Gradient(g) => write!(f, "{}", g),
        }
    }
}

impl fmt::Display for Gradient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.dir {
            Dir::Angle(a) => write!(f, "linear-gradient({}deg", a.round()),
            Dir::Corner(x, y) => write!(
                f,
                "linear-gradient(to {}{}",
                if y < 0 { "top" } else if y > 0 { "bottom" } else { "" },
                if x < 0 { " left" } else if x > 0 { " right" } else { "" }
            ),
            Dir::Radial(shape, size, cx, cy) => {
                write!(
                    f,
                    "radial-gradient({}",
                    match shape {
                        Shape::Circle => "circle",
                        Shape::Ellipse => "ellipse",
                    }
                )?;
                if size != Size::FarthestCorner {
                    write!(
                        f,
                        " {}",
                        match size {
                            Size::ClosestSide => "closest-side",
                            Size::FarthestSide => "farthest-side",
                            Size::ClosestCorner => "closest-corner",
                            Size::FarthestCorner => "farthest-corner",
                        }
                    )?;
                }
                match (cx, cy) {
                    (Stop::Pct(x), Stop::Pct(y)) => write!(f, " at {}% {}%", x, y)?,
                    (Stop::Px(x), Stop::Px(y)) => write!(f, " at {}px {}px", x, y)?,
                    _ => write!(f, " at center")?,
                }
                Ok(())
            }
        }?;
        for (c, p) in &self.stops {
            write!(f, ", {}", c)?;
            match p {
                Some(Stop::Pct(v)) => write!(f, " {}%", v)?,
                Some(Stop::Px(v)) => write!(f, " {}px", v)?,
                None => {}
            }
        }
        write!(f, ")")
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unit::Px => write!(f, "px"),
            Unit::Em => write!(f, "em"),
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            write!(f, "#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

impl Selector {
    pub fn specificity(&self) -> (usize, usize, usize) {
        let mut s = (0, 0, 0);
        for compound in &self.compounds {
            for p in compound {
                match p {
                    SimpleSelector::Id(_) => s.0 += 1,
                    SimpleSelector::Class(_) => s.1 += 1,
                    SimpleSelector::Tag(_) => s.2 += 1,
                    SimpleSelector::Pseudo(_) => s.2 += 1,
                    SimpleSelector::Universal => {}
                }
            }
        }
        s
    }
}

pub fn parse(src: &str) -> Stylesheet {
    let chars: Vec<char> = src.chars().collect();
    let mut rules = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        skip_ws_comments(&chars, &mut i);
        if i >= chars.len() {
            break;
        }
        if chars[i] == '@' {
            i = skip_at(&chars, i);
            continue;
        }
        let (r, next) = rule(&chars, i);
        if let Some(r) = r {
            rules.push(r);
        }
        i = next;
    }
    Stylesheet { rules }
}

pub fn declarations(src: &str) -> Vec<Declaration> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (d, next) = declaration(&chars, i);
        if let Some(d) = d {
            out.push(d);
        }
        i = next + 1;
    }
    out
}

fn rule(chars: &[char], start: usize) -> (Option<Rule>, usize) {
    let mut i = start;
    let mut selectors = Vec::new();
    loop {
        match selector(chars, &mut i) {
            Some(s) => selectors.push(s),
            None => break,
        }
        skip_ws_comments(chars, &mut i);
        if i < chars.len() && chars[i] == ',' {
            i += 1;
            continue;
        }
        break;
    }
    if selectors.is_empty() || i >= chars.len() || chars[i] != '{' {
        return (None, recover(chars, i));
    }
    i += 1;
    let mut declarations = Vec::new();
    loop {
        skip_ws_comments(chars, &mut i);
        if i >= chars.len() {
            break;
        }
        match chars[i] {
            '}' => {
                i += 1;
                break;
            }
            ';' => i += 1,
            '{' => i = skip_balanced(chars, i),
            '@' => i = skip_at(chars, i),
            _ => {
                let (d, next) = declaration(chars, i);
                if let Some(d) = d {
                    declarations.push(d);
                }
                i = next;
            }
        }
    }
    (Some(Rule { selectors, declarations }), i.min(chars.len()))
}

fn selector(chars: &[char], i: &mut usize) -> Option<Selector> {
    let mut compounds = Vec::new();
    loop {
        skip_ws_comments(chars, i);
        let c = compound(chars, i);
        if c.is_empty() {
            break;
        }
        compounds.push(c);
        skip_ws_comments(chars, i);
        if *i < chars.len() && matches!(chars[*i], '>' | '+' | '~') {
            *i += 1;
            skip_ws_comments(chars, i);
            let c = compound(chars, i);
            if c.is_empty() {
                return None;
            }
            compounds.push(c);
        }
        if *i >= chars.len() || matches!(chars[*i], ',' | '{') {
            break;
        }
    }
    if compounds.is_empty() {
        None
    } else {
        Some(Selector { compounds })
    }
}

fn compound(chars: &[char], i: &mut usize) -> Vec<SimpleSelector> {
    let mut out = Vec::new();
    while *i < chars.len() {
        match chars[*i] {
            '.' | '#' => {
                let mark = chars[*i];
                *i += 1;
                let n = ident(chars, i);
                if !n.is_empty() {
                    out.push(if mark == '.' {
                        SimpleSelector::Class(n)
                    } else {
                        SimpleSelector::Id(n)
                    });
                }
            }
            ':' => {
                *i += 1;
                if *i < chars.len() && chars[*i] == ':' {
                    *i += 1;
                }
                let n = ident(chars, i);
                if n.is_empty() {
                    break;
                }
                out.push(SimpleSelector::Pseudo(n.to_lowercase()));
            }
            '*' => {
                *i += 1;
                out.push(SimpleSelector::Universal);
            }
            c if c.is_whitespace() || matches!(c, '>' | '+' | '~' | ',' | '{' | '}') => break,
            _ => {
                let n = ident(chars, i);
                if n.is_empty() {
                    break;
                }
                out.push(SimpleSelector::Tag(n.to_lowercase()));
            }
        }
    }
    out
}

fn ident(chars: &[char], i: &mut usize) -> String {
    let mut s = String::new();
    while *i < chars.len() && (chars[*i].is_alphanumeric() || chars[*i] == '-' || chars[*i] == '_')
    {
        s.push(chars[*i]);
        *i += 1;
    }
    s
}

fn declaration(chars: &[char], start: usize) -> (Option<Declaration>, usize) {
    let mut i = start;
    let mut name = String::new();
    while i < chars.len() && !matches!(chars[i], ':' | ';' | '}' | '{') {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            i = skip_comment(chars, i);
        } else {
            name.push(chars[i]);
            i += 1;
        }
    }
    if i >= chars.len() || chars[i] != ':' {
        return (None, i);
    }
    i += 1;
    let mut value = String::new();
    while i < chars.len() && !matches!(chars[i], ';' | '}') {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            i = skip_comment(chars, i);
        } else if chars[i] == '"' || chars[i] == '\'' {
            let (s, next) = string(chars, i);
            value.push_str(&s);
            i = next;
        } else {
            value.push(chars[i]);
            i += 1;
        }
    }
    let name = name.trim().to_lowercase();
    let value = if name == "content" {
        let v = value.trim().to_string();
        if v.is_empty() {
            Vec::new()
        } else {
            vec![Value::Keyword(v)]
        }
    } else {
        parse_value(&value)
    };
    if name.is_empty() || value.is_empty() {
        return (None, i);
    }
    (Some(Declaration { name, value }), i)
}

pub fn parse_value(s: &str) -> Vec<Value> {
    split_top(s).iter().map(|t| token(t)).collect()
}

fn token(t: &str) -> Value {
    if let Some(v) = length(t) {
        return v;
    }
    if let Some(c) = color(t) {
        return Value::Color(c);
    }
    if let Some(u) = url(t) {
        return Value::Url(u);
    }
    if let Some(g) = gradient(t) {
        return Value::Gradient(g);
    }
    Value::Keyword(t.to_lowercase())
}

fn gradient(t: &str) -> Option<Gradient> {
    let low = t.to_lowercase();
    if let Some(inner) = low.strip_prefix("radial-gradient(") {
        return radial(inner.strip_suffix(')')?);
    }
    let inner = low.strip_prefix("linear-gradient(")?.strip_suffix(')')?;
    let parts = split_commas(inner);
    if parts.is_empty() {
        return None;
    }
    let (dir, rest) = match side_dir(parts[0].trim()) {
        Some(d) => (d, &parts[1..]),
        None => (Dir::Angle(180.0), &parts[..]),
    };
    if rest.is_empty() {
        return None;
    }
    let stops = stops_of(rest)?;
    Some(Gradient { dir, stops })
}

fn radial(s: &str) -> Option<Gradient> {
    let parts = split_commas(s);
    if parts.is_empty() {
        return None;
    }
    let mut shape = Shape::Ellipse;
    let mut size = Size::FarthestCorner;
    let (mut cx, mut cy) = (Stop::Pct(50.0), Stop::Pct(50.0));
    let toks: Vec<&str> = parts[0].trim().split_whitespace().collect();
    let spec = toks.iter().any(|t| {
        matches!(
            *t,
            "circle"
                | "ellipse"
                | "at"
                | "closest-side"
                | "farthest-side"
                | "closest-corner"
                | "farthest-corner"
        )
    });
    let rest = if spec {
        let mut i = 0usize;
        while i < toks.len() {
            match toks[i] {
                "circle" => {
                    shape = Shape::Circle;
                    i += 1;
                }
                "ellipse" => {
                    shape = Shape::Ellipse;
                    i += 1;
                }
                "closest-side" => {
                    size = Size::ClosestSide;
                    i += 1;
                }
                "farthest-side" => {
                    size = Size::FarthestSide;
                    i += 1;
                }
                "closest-corner" => {
                    size = Size::ClosestCorner;
                    i += 1;
                }
                "farthest-corner" => {
                    size = Size::FarthestCorner;
                    i += 1;
                }
                "at" => {
                    i += 1;
                    let x = toks.get(i).copied()?;
                    cx = grad_pos(x)?;
                    i += 1;
                    if i < toks.len() {
                        if let Some(p) = grad_pos(toks[i]) {
                            cy = p;
                            i += 1;
                        }
                    }
                }
                _ => return None,
            }
        }
        &parts[1..]
    } else {
        &parts[..]
    };
    if rest.is_empty() {
        return None;
    }
    let stops = stops_of(rest)?;
    Some(Gradient {
        dir: Dir::Radial(shape, size, cx, cy),
        stops,
    })
}

fn grad_pos(t: &str) -> Option<Stop> {
    match t {
        "left" | "top" => Some(Stop::Pct(0.0)),
        "center" => Some(Stop::Pct(50.0)),
        "right" | "bottom" => Some(Stop::Pct(100.0)),
        _ => stop_pos(t),
    }
}

fn stops_of(rest: &[String]) -> Option<Vec<(Color, Option<Stop>)>> {
    let mut stops = Vec::new();
    for p in rest {
        let toks = split_top(p);
        if toks.is_empty() {
            continue;
        }
        let c = color(&toks[0])?;
        let pos = toks.get(1).and_then(|t| stop_pos(t));
        stops.push((c, pos));
    }
    if stops.len() < 2 {
        return None;
    }
    Some(stops)
}

fn side_dir(s: &str) -> Option<Dir> {
    if let Some(d) = s.strip_suffix("deg") {
        return d.trim().parse::<f32>().ok().map(Dir::Angle);
    }
    let toks: Vec<&str> = s.split_whitespace().collect();
    if toks.first() != Some(&"to") {
        return None;
    }
    let mut sx = 0i8;
    let mut sy = 0i8;
    for t in &toks[1..] {
        match *t {
            "top" => sy = -1,
            "bottom" => sy = 1,
            "left" => sx = -1,
            "right" => sx = 1,
            _ => return None,
        }
    }
    match (sx, sy) {
        (0, 0) => None,
        (0, -1) => Some(Dir::Angle(0.0)),
        (0, 1) => Some(Dir::Angle(180.0)),
        (-1, 0) => Some(Dir::Angle(270.0)),
        (1, 0) => Some(Dir::Angle(90.0)),
        (x, y) => Some(Dir::Corner(x, y)),
    }
}

fn stop_pos(t: &str) -> Option<Stop> {
    if let Some(p) = t.strip_suffix('%') {
        return p.trim().parse::<f32>().ok().map(Stop::Pct);
    }
    if let Some(n) = t.strip_suffix("px") {
        return n.trim().parse::<f32>().ok().map(Stop::Px);
    }
    None
}

fn split_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match quote {
            Some(q) => {
                cur.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                    cur.push(ch);
                }
                '(' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' => {
                    depth -= 1;
                    cur.push(ch);
                }
                ',' if depth == 0 => {
                    out.push(std::mem::take(&mut cur));
                }
                _ => cur.push(ch),
            },
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

fn url(t: &str) -> Option<String> {
    let inner = t.strip_prefix("url(")?.strip_suffix(')')?;
    let u = inner.trim();
    let v = match u.chars().next() {
        Some(q @ ('"' | '\'')) => u.strip_prefix(q)?.strip_suffix(q)?,
        _ => u,
    };
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

fn length(t: &str) -> Option<Value> {
    let (num, unit) = if let Some(n) = t.strip_suffix("px") {
        (n, Unit::Px)
    } else if let Some(n) = t.strip_suffix("em") {
        (n, Unit::Em)
    } else if t == "0" {
        (t, Unit::Px)
    } else {
        return None;
    };
    let n: f32 = num.parse().ok()?;
    Some(Value::Length(n, unit))
}

fn color(t: &str) -> Option<Color> {
    hex_color(t)
        .or_else(|| func_color(t))
        .or_else(|| named_color(t))
}

fn hex_color(t: &str) -> Option<Color> {
    let h = t.strip_prefix('#')?;
    let long: String = match h.len() {
        3 | 4 => h.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => h.to_string(),
        _ => return None,
    };
    if !long.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&long[i..i + 2], 16).ok();
    Some(Color {
        r: b(0)?,
        g: b(2)?,
        b: b(4)?,
        a: if long.len() == 8 { b(6)? } else { 255 },
    })
}

fn func_color(t: &str) -> Option<Color> {
    let low = t.to_lowercase();
    let (inner, has_alpha) = if let Some(s) = low.strip_prefix("rgba(") {
        (s.strip_suffix(')')?, true)
    } else if let Some(s) = low.strip_prefix("rgb(") {
        (s.strip_suffix(')')?, false)
    } else {
        return None;
    };
    let parts: Vec<&str> = inner.split(',').collect();
    match (parts.len(), has_alpha) {
        (3, false) => Some(Color {
            r: ch8(parts[0])?,
            g: ch8(parts[1])?,
            b: ch8(parts[2])?,
            a: 255,
        }),
        (4, true) => Some(Color {
            r: ch8(parts[0])?,
            g: ch8(parts[1])?,
            b: ch8(parts[2])?,
            a: al8(parts[3])?,
        }),
        _ => None,
    }
}

fn ch8(s: &str) -> Option<u8> {
    s.trim().parse().ok()
}

fn al8(s: &str) -> Option<u8> {
    let f: f32 = s.trim().parse().ok()?;
    Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn named_color(t: &str) -> Option<Color> {
    const fn c(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }
    const NAMED: [(&str, Color); 20] = [
        ("black", c(0, 0, 0)),
        ("silver", c(192, 192, 192)),
        ("gray", c(128, 128, 128)),
        ("white", c(255, 255, 255)),
        ("maroon", c(128, 0, 0)),
        ("red", c(255, 0, 0)),
        ("purple", c(128, 0, 128)),
        ("fuchsia", c(255, 0, 255)),
        ("green", c(0, 128, 0)),
        ("lime", c(0, 255, 0)),
        ("olive", c(128, 128, 0)),
        ("yellow", c(255, 255, 0)),
        ("navy", c(0, 0, 128)),
        ("blue", c(0, 0, 255)),
        ("teal", c(0, 128, 128)),
        ("aqua", c(0, 255, 255)),
        ("cyan", c(0, 255, 255)),
        ("magenta", c(255, 0, 255)),
        ("orange", c(255, 165, 0)),
        ("transparent", Color { r: 0, g: 0, b: 0, a: 0 }),
    ];
    let low = t.to_lowercase();
    NAMED.iter().find(|(n, _)| *n == low).map(|(_, v)| *v)
}

fn split_top(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match quote {
            Some(q) => {
                cur.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                    cur.push(ch);
                }
                '(' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' => {
                    depth -= 1;
                    cur.push(ch);
                }
                ',' if depth == 0 => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    out.push(','.to_string());
                }
                c if c.is_whitespace() && depth == 0 => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                c => cur.push(c),
            },
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn string(chars: &[char], start: usize) -> (String, usize) {
    let q = chars[start];
    let mut s = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            s.push(chars[i + 1]);
            i += 2;
        } else if chars[i] == q {
            return (s, i + 1);
        } else {
            s.push(chars[i]);
            i += 1;
        }
    }
    (s, i)
}

fn skip_at(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i] != ';' && chars[i] != '{' {
        i += 1;
    }
    if i < chars.len() && chars[i] == '{' {
        skip_balanced(chars, i)
    } else {
        (i + 1).min(chars.len())
    }
}

fn skip_balanced(chars: &[char], start: usize) -> usize {
    let mut depth = 1;
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    chars.len()
}

fn skip_comment(chars: &[char], start: usize) -> usize {
    match find_seq(chars, start + 2, &['*', '/']) {
        Some(e) => e + 2,
        None => chars.len(),
    }
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
}

fn skip_ws_comments(chars: &[char], i: &mut usize) {
    loop {
        skip_ws(chars, i);
        if *i + 1 < chars.len() && chars[*i] == '/' && chars[*i + 1] == '*' {
            *i = skip_comment(chars, *i);
        } else {
            return;
        }
    }
}

fn recover(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && chars[i] != '}' {
        i += 1;
    }
    (i + 1).min(chars.len())
}

fn find_seq(chars: &[char], from: usize, pat: &[char]) -> Option<usize> {
    if pat.is_empty() || chars.len() < pat.len() {
        return None;
    }
    (from..=chars.len() - pat.len()).find(|&j| chars[j..j + pat.len()] == *pat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_declarations() {
        let d = declarations("font-size: 24px; color:#f00; margin : 0 auto");
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].name, "font-size");
        assert_eq!(d[0].value, vec![Value::Length(24.0, Unit::Px)]);
        assert_eq!(d[1].value, vec![Value::Color(Color { r: 255, g: 0, b: 0, a: 255 })]);
        assert_eq!(
            d[2].value,
            vec![Value::Length(0.0, Unit::Px), Value::Keyword("auto".into())]
        );
    }

    #[test]
    fn basic_rule() {
        let sheet = parse("h1 { color: red; }");
        assert_eq!(sheet.rules.len(), 1);
        let r = &sheet.rules[0];
        assert_eq!(
            r.selectors,
            vec![Selector {
                compounds: vec![vec![SimpleSelector::Tag("h1".into())]]
            }]
        );
        assert_eq!(
            r.declarations,
            vec![Declaration {
                name: "color".into(),
                value: vec![Value::Color(Color { r: 255, g: 0, b: 0, a: 255 })]
            }]
        );
    }

    #[test]
    fn selector_group() {
        let sheet = parse("div.box, #main p.a { margin: 0 auto }");
        let r = &sheet.rules[0];
        assert_eq!(r.selectors.len(), 2);
        assert_eq!(
            r.selectors[0].compounds,
            vec![vec![
                SimpleSelector::Tag("div".into()),
                SimpleSelector::Class("box".into())
            ]]
        );
        assert_eq!(
            r.selectors[1].compounds,
            vec![
                vec![SimpleSelector::Id("main".into())],
                vec![
                    SimpleSelector::Tag("p".into()),
                    SimpleSelector::Class("a".into())
                ]
            ]
        );
        assert_eq!(
            r.declarations[0].value,
            vec![Value::Length(0.0, Unit::Px), Value::Keyword("auto".into())]
        );
    }

    #[test]
    fn specificity() {
        let sheet = parse("div.box#main p { color: red }");
        assert_eq!(sheet.rules[0].selectors[0].specificity(), (1, 1, 2));
        let sheet = parse("* { margin: 0 }");
        assert_eq!(sheet.rules[0].selectors[0].specificity(), (0, 0, 0));
    }

    #[test]
    fn comments() {
        let sheet = parse("/* hi */ p { /* c */ color: /* x */ blue }");
        let r = &sheet.rules[0];
        assert_eq!(
            r.declarations,
            vec![Declaration {
                name: "color".into(),
                value: vec![Value::Color(Color { r: 0, g: 0, b: 255, a: 255 })]
            }]
        );
    }

    #[test]
    fn at_rules() {
        let sheet = parse("@import url(x.css); @media screen { p { color: red } } div { width: 10px }");
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selectors[0].compounds[0][0], SimpleSelector::Tag("div".into()));
        assert_eq!(sheet.rules[0].declarations[0].name, "width");
        assert_eq!(
            sheet.rules[0].declarations[0].value,
            vec![Value::Length(10.0, Unit::Px)]
        );
    }

    #[test]
    fn strings_in_values() {
        let sheet = parse("p { content: \"a;b\"; font-family: 'x y' }");
        let r = &sheet.rules[0];
        assert_eq!(
            r.declarations[0].value,
            vec![Value::Keyword("a;b".into())]
        );
        assert_eq!(
            r.declarations[1].value,
            vec![Value::Keyword("x".into()), Value::Keyword("y".into())]
        );
    }

    #[test]
    fn malformed_recovery() {
        let sheet = parse("p { color: red } broken !!! { width: 1px } b { height: 2px }");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(
            sheet.rules[1].declarations[0].value,
            vec![Value::Length(2.0, Unit::Px)]
        );
    }

    #[test]
    fn value_types() {
        let v = parse_value("10px 2em 0 auto #fff #11223344 50%");
        assert_eq!(
            v,
            vec![
                Value::Length(10.0, Unit::Px),
                Value::Length(2.0, Unit::Em),
                Value::Length(0.0, Unit::Px),
                Value::Keyword("auto".into()),
                Value::Color(Color { r: 255, g: 255, b: 255, a: 255 }),
                Value::Color(Color { r: 17, g: 34, b: 51, a: 68 }),
                Value::Keyword("50%".into()),
            ]
        );
    }

    #[test]
    fn func_color_spaces() {
        let v = parse_value("rgb(10, 20, 30)");
        assert_eq!(
            v,
            vec![Value::Color(Color { r: 10, g: 20, b: 30, a: 255 })]
        );
    }

    #[test]
    fn named_colors() {
        let v = parse_value("red transparent");
        assert_eq!(
            v,
            vec![
                Value::Color(Color { r: 255, g: 0, b: 0, a: 255 }),
                Value::Color(Color { r: 0, g: 0, b: 0, a: 0 }),
            ]
        );
    }

    #[test]
    fn display() {
        assert_eq!(Value::Length(2.0, Unit::Em).to_string(), "2em");
        assert_eq!(
            Value::Color(Color { r: 255, g: 0, b: 0, a: 255 }).to_string(),
            "#ff0000"
        );
        assert_eq!(
            Value::Color(Color { r: 0, g: 0, b: 0, a: 128 }).to_string(),
            "#00000080"
        );
    }

    #[test]
    fn url_values() {
        let v = parse_value("url(bullet.png)");
        assert_eq!(v, vec![Value::Url("bullet.png".into())]);
        let v = parse_value("url( 'a b.PNG' )");
        assert_eq!(v, vec![Value::Url("a b.PNG".into())]);
        assert_eq!(parse_value("url( )"), vec![Value::Keyword("url( )".into())]);
    }

    #[test]
    fn url_inside_declaration() {
        let sheet = parse("li { list-style-image: url(\"dot.png\") }");
        assert_eq!(
            sheet.rules[0].declarations[0].value,
            vec![Value::Url("dot.png".into())]
        );
    }

    fn grad(v: Value) -> Gradient {
        match v {
            Value::Gradient(g) => g,
            _ => panic!("not a gradient"),
        }
    }

    #[test]
    fn gradient_default_direction() {
        let v = parse_value("linear-gradient(red, blue)");
        let g = grad(v.into_iter().next().unwrap());
        assert_eq!(g.dir, Dir::Angle(180.0));
        assert_eq!(
            g.stops,
            vec![
                (Color { r: 255, g: 0, b: 0, a: 255 }, None),
                (Color { r: 0, g: 0, b: 255, a: 255 }, None),
            ]
        );
    }

    #[test]
    fn gradient_angle_and_positions() {
        let v = parse_value("linear-gradient(45deg, #ff0000 0%, rgba(0, 0, 255, 0.5) 100%)");
        let g = grad(v.into_iter().next().unwrap());
        assert_eq!(g.dir, Dir::Angle(45.0));
        assert_eq!(g.stops[0], (Color { r: 255, g: 0, b: 0, a: 255 }, Some(Stop::Pct(0.0))));
        assert_eq!(g.stops[1], (Color { r: 0, g: 0, b: 255, a: 128 }, Some(Stop::Pct(100.0))));
    }

    #[test]
    fn gradient_side_and_corner() {
        let v = parse_value("linear-gradient(to right, red, blue)");
        assert_eq!(grad(v.into_iter().next().unwrap()).dir, Dir::Angle(90.0));
        let v = parse_value("linear-gradient(to left top, red, blue)");
        assert_eq!(grad(v.into_iter().next().unwrap()).dir, Dir::Corner(-1, -1));
        let v = parse_value("linear-gradient(to bottom right, red, blue)");
        assert_eq!(grad(v.into_iter().next().unwrap()).dir, Dir::Corner(1, 1));
    }

    #[test]
    fn gradient_px_stops() {
        let v = parse_value("linear-gradient(90deg, red 10px, blue 40px)");
        let g = grad(v.into_iter().next().unwrap());
        assert_eq!(g.stops[0].1, Some(Stop::Px(10.0)));
        assert_eq!(g.stops[1].1, Some(Stop::Px(40.0)));
    }

    #[test]
    fn gradient_display() {
        let v = parse_value("linear-gradient(to right, red, blue 50%)");
        assert_eq!(
            v[0].to_string(),
            "linear-gradient(90deg, #ff0000, #0000ff 50%)"
        );
    }

    #[test]
    fn pseudo_element_selectors() {
        let sheet = parse("p::before { content: \"a b\" } p:after { content: 'x' }");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(
            sheet.rules[0].selectors[0].compounds,
            vec![vec![
                SimpleSelector::Tag("p".into()),
                SimpleSelector::Pseudo("before".into())
            ]]
        );
        assert_eq!(
            sheet.rules[1].selectors[0].compounds[0][1],
            SimpleSelector::Pseudo("after".into())
        );
        assert_eq!(
            sheet.rules[0].declarations[0].value,
            vec![Value::Keyword("a b".into())]
        );
        assert_eq!(
            sheet.rules[1].declarations[0].value,
            vec![Value::Keyword("x".into())]
        );
    }

    #[test]
    fn pseudo_specificity_and_content_none() {
        let sheet = parse("p::before { color: red }");
        assert_eq!(sheet.rules[0].selectors[0].specificity(), (0, 0, 2));
        let d = declarations("q { content: none }");
        assert_eq!(
            d[0].value,
            vec![Value::Keyword("none".into())]
        );
    }

    #[test]
    fn gradient_rejects_single_stop() {
        let v = parse_value("linear-gradient(red)");
        assert_eq!(v, vec![Value::Keyword("linear-gradient(red)".into())]);
    }

    fn rad(v: Value) -> Gradient {
        match v {
            Value::Gradient(g) => g,
            _ => panic!("not a gradient"),
        }
    }

    #[test]
    fn radial_defaults() {
        let v = parse_value("radial-gradient(red, blue)");
        let g = rad(v.into_iter().next().unwrap());
        assert_eq!(
            g.dir,
            Dir::Radial(Shape::Ellipse, Size::FarthestCorner, Stop::Pct(50.0), Stop::Pct(50.0))
        );
        assert_eq!(g.stops.len(), 2);
    }

    #[test]
    fn radial_shape_size_position() {
        let v = parse_value("radial-gradient(circle closest-side at 30% 20%, red, blue)");
        let g = rad(v.into_iter().next().unwrap());
        assert_eq!(
            g.dir,
            Dir::Radial(Shape::Circle, Size::ClosestSide, Stop::Pct(30.0), Stop::Pct(20.0))
        );
        let v = parse_value("radial-gradient(circle at left top, red, blue)");
        let g = rad(v.into_iter().next().unwrap());
        assert_eq!(
            g.dir,
            Dir::Radial(Shape::Circle, Size::FarthestCorner, Stop::Pct(0.0), Stop::Pct(0.0))
        );
        let v = parse_value("radial-gradient(ellipse farthest-side at 10px 40px, red, blue)");
        let g = rad(v.into_iter().next().unwrap());
        assert_eq!(
            g.dir,
            Dir::Radial(Shape::Ellipse, Size::FarthestSide, Stop::Px(10.0), Stop::Px(40.0))
        );
    }

    #[test]
    fn radial_stop_positions() {
        let v = parse_value("radial-gradient(red 10%, blue 80%)");
        let g = rad(v.into_iter().next().unwrap());
        assert_eq!(g.stops[0].1, Some(Stop::Pct(10.0)));
        assert_eq!(g.stops[1].1, Some(Stop::Pct(80.0)));
    }

    #[test]
    fn radial_display() {
        let v = parse_value("radial-gradient(circle closest-side at 25% 75%, red, blue 50%)");
        assert_eq!(
            v[0].to_string(),
            "radial-gradient(circle closest-side at 25% 75%, #ff0000, #0000ff 50%)"
        );
        let v = parse_value("radial-gradient(red, blue)");
        assert_eq!(
            v[0].to_string(),
            "radial-gradient(ellipse at 50% 50%, #ff0000, #0000ff)"
        );
    }

    #[test]
    fn radial_rejects_garbage() {
        let v = parse_value("radial-gradient(red)");
        assert_eq!(v, vec![Value::Keyword("radial-gradient(red)".into())]);
        let v = parse_value("radial-gradient(nonsense at red, blue)");
        assert_eq!(
            v,
            vec![Value::Keyword("radial-gradient(nonsense at red, blue)".into())]
        );
    }
}
