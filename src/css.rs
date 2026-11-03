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

#[derive(Debug, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: String,
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
    let value = value.trim().to_string();
    if name.is_empty() || value.is_empty() {
        return (None, i);
    }
    (Some(Declaration { name, value }), i)
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
                value: "red".into()
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
        assert_eq!(r.declarations[0].value, "0 auto");
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
                value: "blue".into()
            }]
        );
    }

    #[test]
    fn at_rules() {
        let sheet = parse("@import url(x.css); @media screen { p { color: red } } div { width: 10px }");
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selectors[0].compounds[0][0], SimpleSelector::Tag("div".into()));
        assert_eq!(sheet.rules[0].declarations[0].name, "width");
    }

    #[test]
    fn strings_in_values() {
        let sheet = parse("p { content: \"a;b\"; font-family: 'x y' }");
        let r = &sheet.rules[0];
        assert_eq!(r.declarations[0].value, "a;b");
        assert_eq!(r.declarations[1].value, "x y");
    }

    #[test]
    fn malformed_recovery() {
        let sheet = parse("p { color: red } broken !!! { width: 1px } b { height: 2px }");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[1].declarations[0].value, "2px");
    }
}
