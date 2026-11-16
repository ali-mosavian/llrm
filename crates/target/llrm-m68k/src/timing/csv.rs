//! The two readers of a transcribed table that `build.rs` shares with the
//! runtime: a CSV line split into fields, and a label normalized to its key.

pub fn split_csv(line: &str) -> Vec<String> {
    if !line.contains('"') {
        return line.split(',').map(str::to_owned).collect();
    }
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                fields.last_mut().unwrap().push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(String::new()),
            c => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

/// A label without what typesetting varies: spaces, `*`, the dash kinds.
pub fn normalize(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    out.extend(
        label
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(*c, '*' | '\u{2020}'))
            .map(|c| if matches!(c, '\u{2013}' | '\u{2212}' | '\u{2014}') { '-' } else { c.to_ascii_lowercase() }),
    );
    out
}
