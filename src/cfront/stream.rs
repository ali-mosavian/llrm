//! Port of `qbopt/cfront/stream.py`: the stream owshim/cgshim.c writes, one
//! code-generator call per line.
//!
//! ```text
//! n7 CGBinary O_PLUS n5 n6 TY_INTEGER     a call and the handle it returned
//! - CGDone n7                             a call that returned nothing
//! SYM y1 name="pal_now" attr=0x42 seg=11  a record the shim adds
//! ```

use indexmap::IndexMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub line: usize,
    pub result: Option<String>,
    pub call: String,
    pub args: Vec<String>,
    pub fields: IndexMap<String, String>,
}

/// `HANDLE.fullmatch`: `[a-z]\d+`.
fn is_handle(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(chars.next(), Some('a'..='z')) && token.len() > 1 && chars.all(|c| c.is_ascii_digit())
}

pub fn parse(text: &str) -> Vec<Record> {
    let mut records = Vec::new();
    for (index, line) in splitlines(text).into_iter().enumerate() {
        let mut tokens = split(line);
        if tokens.is_empty() {
            continue;
        }
        let mut result = None;
        if tokens[0] == "-" || is_handle(&tokens[0]) {
            result = if tokens[0] == "-" {
                None
            } else {
                Some(tokens[0].clone())
            };
            tokens.remove(0);
        }
        let mut args = Vec::new();
        let mut fields = IndexMap::new();
        for one in &tokens[1..] {
            match one.split_once('=') {
                Some((key, value)) if is_identifier(key) => {
                    fields.insert(key.to_owned(), value_of(value));
                }
                _ => args.push(value_of(one)),
            }
        }
        records.push(Record {
            line: index + 1,
            result,
            call: tokens[0].clone(),
            args,
            fields,
        });
    }
    records
}

/// `str.splitlines`.
pub(crate) fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, character)) = chars.next() {
        let breaks = matches!(
            character,
            '\n' | '\r'
                | '\u{0b}'
                | '\u{0c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !breaks {
            continue;
        }
        lines.push(&text[start..at]);
        start = at + character.len_utf8();
        if character == '\r' && matches!(chars.peek(), Some((_, '\n'))) {
            chars.next();
            start += 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// `str.isidentifier` for the ASCII keys the shim writes.
fn is_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '_' || c.is_alphanumeric())
}

/// Space-separated tokens; a quoted string holds no quote, the shim escapes it.
fn split(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let (mut tokens, mut start) = (Vec::new(), 0);
    while start < bytes.len() {
        if bytes[start] == b' ' {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < bytes.len() && bytes[end] != b' ' {
            if bytes[end] == b'"' {
                end = end
                    + 1
                    + line[end + 1..]
                        .find('"')
                        .expect("the shim closes every quote");
            }
            end += 1;
        }
        tokens.push(line[start..end].to_owned());
        start = end;
    }
    tokens
}

fn value_of(token: &str) -> String {
    if token.len() >= 2 && token.starts_with('"') && token.ends_with('"') {
        return unescape(&token[1..token.len() - 1]);
    }
    token.to_owned()
}

/// `re.sub(r"\\x([0-9a-f]{2})", chr(int(..., 16)), ...)`.
fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("\\x") {
        let hex = rest
            .get(at + 2..at + 4)
            .filter(|hex| hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        match hex {
            Some(hex) => {
                out.push_str(&rest[..at]);
                out.push(char::from(
                    u8::from_str_radix(hex, 16).expect("two hex digits"),
                ));
                rest = &rest[at + 4..];
            }
            None => {
                out.push_str(&rest[..at + 2]);
                rest = &rest[at + 2..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_results_arguments_and_fields() {
        let records = parse(
            "n7 CGBinary O_PLUS n5 n6 TY_INTEGER\n- CGDone n7\nSYM y1 name=\"pal now\" attr=0x42\n",
        );
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].result.as_deref(), Some("n7"));
        assert_eq!(records[0].args, ["O_PLUS", "n5", "n6", "TY_INTEGER"]);
        assert_eq!(records[1].result, None);
        assert_eq!(records[2].fields["name"], "pal now");
        assert_eq!(records[2].line, 3);
    }

    #[test]
    fn unescapes_hex_bytes() {
        assert_eq!(value_of("\"a\\x41\\x7fz\""), "aA\u{7f}z");
    }
}
