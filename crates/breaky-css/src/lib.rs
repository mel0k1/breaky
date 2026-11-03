use std::fmt;

#[derive(Debug, PartialEq)]
pub enum SimpleSelector {
    Universal,
    Tag(String),
    Id(String),
    Class(String),
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
        }
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
    let value = parse_value(&value);
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
    Value::Keyword(t.to_lowercase())
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
}
