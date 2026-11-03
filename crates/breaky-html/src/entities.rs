const NAMED: [(&str, &str); 40] = [
    ("amp;", "&"),
    ("lt;", "<"),
    ("gt;", ">"),
    ("quot;", "\""),
    ("apos;", "'"),
    ("nbsp;", "\u{a0}"),
    ("copy;", "\u{a9}"),
    ("reg;", "\u{ae}"),
    ("trade;", "\u{2122}"),
    ("hellip;", "\u{2026}"),
    ("mdash;", "\u{2014}"),
    ("ndash;", "\u{2013}"),
    ("lsquo;", "\u{2018}"),
    ("rsquo;", "\u{2019}"),
    ("ldquo;", "\u{201c}"),
    ("rdquo;", "\u{201d}"),
    ("laquo;", "\u{ab}"),
    ("raquo;", "\u{bb}"),
    ("bull;", "\u{2022}"),
    ("middot;", "\u{b7}"),
    ("deg;", "\u{b0}"),
    ("plusmn;", "\u{b1}"),
    ("times;", "\u{d7}"),
    ("divide;", "\u{f7}"),
    ("frac12;", "\u{bd}"),
    ("frac14;", "\u{bc}"),
    ("frac34;", "\u{be}"),
    ("sup2;", "\u{b2}"),
    ("sup3;", "\u{b3}"),
    ("micro;", "\u{b5}"),
    ("para;", "\u{b6}"),
    ("sect;", "\u{a7}"),
    ("euro;", "\u{20ac}"),
    ("pound;", "\u{a3}"),
    ("yen;", "\u{a5}"),
    ("cent;", "\u{a2}"),
    ("larr;", "\u{2190}"),
    ("rarr;", "\u{2192}"),
    ("uarr;", "\u{2191}"),
    ("darr;", "\u{2193}"),
];

pub fn decode(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' {
            if let Some((rep, next)) = entity(&chars, i) {
                out.push_str(&rep);
                i = next;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn entity(chars: &[char], amp: usize) -> Option<(String, usize)> {
    numeric(chars, amp).or_else(|| named(chars, amp))
}

fn numeric(chars: &[char], amp: usize) -> Option<(String, usize)> {
    if chars.get(amp + 1) != Some(&'#') {
        return None;
    }
    let mut j = amp + 2;
    let hex = matches!(chars.get(j), Some('x') | Some('X'));
    if hex {
        j += 1;
    }
    let start = j;
    while j < chars.len() && (if hex { chars[j].is_ascii_hexdigit() } else { chars[j].is_ascii_digit() }) {
        j += 1;
    }
    if j == start {
        return None;
    }
    let text: String = chars[start..j].iter().collect();
    let code = u32::from_str_radix(&text, if hex { 16 } else { 10 }).ok()?;
    let ch = char::from_u32(code)?;
    let end = if chars.get(j) == Some(&';') { j + 1 } else { j };
    Some((ch.to_string(), end))
}

fn named(chars: &[char], amp: usize) -> Option<(String, usize)> {
    let mut best: Option<(&str, &str)> = None;
    for (name, rep) in NAMED {
        let pat: Vec<char> = name.chars().collect();
        if chars[amp + 1..].starts_with(&pat) && best.map_or(true, |(n, _)| n.len() < name.len()) {
            best = Some((name, rep));
        }
    }
    best.map(|(name, rep)| (rep.to_string(), amp + 1 + name.chars().count()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_names() {
        assert_eq!(decode("a &amp; b"), "a & b");
        assert_eq!(decode("&lt;p&gt;"), "<p>");
        assert_eq!(decode("&quot;x&quot;"), "\"x\"");
        assert_eq!(decode("&nbsp;"), "\u{a0}");
    }

    #[test]
    fn numeric_forms() {
        assert_eq!(decode("&#65;&#66;"), "AB");
        assert_eq!(decode("&#x42;&#X43;"), "BC");
        assert_eq!(decode("&#65"), "A");
        assert_eq!(decode("&#x1F600;"), "\u{1F600}");
    }

    #[test]
    fn unknown_kept_literal() {
        assert_eq!(decode("a &zz; b"), "a &zz; b");
        assert_eq!(decode("&amp"), "&amp");
        assert_eq!(decode("&#;"), "&#;");
        assert_eq!(decode("&#x;"), "&#x;");
        assert_eq!(decode("100$ & 2"), "100$ & 2");
    }

    #[test]
    fn double_decode_once() {
        assert_eq!(decode("&amp;amp;"), "&amp;");
    }

    #[test]
    fn longest_match_wins() {
        assert_eq!(decode("&times;"), "\u{d7}");
    }
}
