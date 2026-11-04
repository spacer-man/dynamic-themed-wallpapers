//! Turning untrusted HTML snippets from photo metadata into plain text.
//!
//! The output is shown as text only (never as markup), so this only needs to be readable,
//! not a security boundary.

/// Strip tags, decode common entities, collapse whitespace.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    collapse_ws(&decode_entities(&out))
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[..rest.len().min(12)].find(';');
        let decoded = end.and_then(|e| decode_entity(&rest[1..e]).map(|c| (c, e)));
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)?
        }
    })
}

fn collapse_ws(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    // Tags replaced by spaces leave "word ," artefacts.
    joined
        .replace(" ,", ",")
        .replace(" .", ".")
        .replace("( ", "(")
        .replace(" )", ")")
}

/// Shorten to at most `max` characters on a word boundary, adding "…".
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > max / 2 => &cut[..i],
        _ => &cut,
    };
    format!("{}…", cut.trim_end_matches([',', ' ', ';', ':']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_commons_artist_markup() {
        let html = r#"<a href="//commons.wikimedia.org/wiki/User:A_B" title="User:A B">A B</a>"#;
        assert_eq!(html_to_text(html), "A B");
    }

    #[test]
    fn keeps_punctuation_tidy() {
        let html =
            r#"<a href="x">Bamboo Forest</a>, <a>Kyoto</a>. Done &amp; dusted &#8211; &#x41;"#;
        assert_eq!(
            html_to_text(html),
            "Bamboo Forest, Kyoto. Done & dusted – A"
        );
    }

    #[test]
    fn unknown_entities_are_left_alone() {
        assert_eq!(html_to_text("a &bogus; b & c"), "a &bogus; b & c");
    }

    #[test]
    fn truncation() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(
            truncate("Torii path with lantern at Fushimi Inari", 20),
            "Torii path with…"
        );
        assert_eq!(truncate("日本語のテキストです", 5), "日本語の…");
    }
}
