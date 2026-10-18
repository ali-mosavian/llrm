//! The stream toolchain/owshim/cgshim.c
//! writes, one code-generator call per line.
//!
//! ```text
//! n7 CGBinary O_PLUS n5 n6 TY_INTEGER     a call and the handle it returned
//! - CGDone n7                             a call that returned nothing
//! SYM y1 name="pal_now" attr=0x42 seg=11  a record the shim adds
//! ```

use std::borrow::Cow;

/// One line of the stream. Tokens borrow the text; only a quoted value holding an
/// escape is copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record<'a> {
    pub line: usize,
    pub result: Option<&'a str>,
    pub call: &'a str,
    pub args: Vec<Cow<'a, str>>,
    pub fields: Vec<(&'a str, Cow<'a, str>)>,
}

impl Record<'_> {
    /// The last value written for `key`, as a dictionary keeps it.
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.iter().rev().find(|(name, _)| *name == key).map(|(_, value)| &**value)
    }
}

/// `HANDLE.fullmatch`: `[a-z]\d+`.
fn is_handle(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() > 1 && bytes[0].is_ascii_lowercase() && bytes[1..].iter().all(u8::is_ascii_digit)
}

/// Like Clang's `Lexer`: one forward pass over the buffer, a token is a slice of it.
pub fn parse(text: &str) -> Vec<Record<'_>> {
    let mut records = Vec::with_capacity(text.len() / 32);
    for (index, line) in Lines(text).enumerate() {
        let mut tokens = Tokens(line);
        let Some(first) = tokens.next() else { continue };
        let (result, call) = if first == "-" || is_handle(first) {
            (if first == "-" { None } else { Some(first) }, tokens.next().expect("a call after its result"))
        } else {
            (None, first)
        };
        let (mut args, mut fields) = (Vec::new(), Vec::new());
        for one in tokens {
            match one.split_once('=') {
                Some((key, value)) if is_identifier(key) => fields.push((key, value_of(value))),
                _ => args.push(value_of(one)),
            }
        }
        records.push(Record { line: index + 1, result, call, args, fields });
    }
    records
}

/// `str.splitlines`.
struct Lines<'a>(&'a str);

impl<'a> Iterator for Lines<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let text = self.0;
        if text.is_empty() {
            return None;
        }
        let bytes = text.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            let width = match bytes[at] {
                b'\n' | 0x0b | 0x0c | 0x1c..=0x1e => 1,
                b'\r' => 1 + usize::from(bytes.get(at + 1) == Some(&b'\n')),
                0xc2 if bytes.get(at + 1) == Some(&0x85) => 2,
                0xe2 if bytes.get(at + 1) == Some(&0x80) && matches!(bytes.get(at + 2), Some(0xa8 | 0xa9)) => 3,
                _ => {
                    at += 1;
                    continue;
                }
            };
            self.0 = &text[at + width..];
            return Some(&text[..at]);
        }
        self.0 = "";
        Some(text)
    }
}

/// `str.isidentifier` for the ASCII keys the shim writes.
fn is_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_alphabetic()) && chars.all(|c| c == '_' || c.is_alphanumeric())
}

/// Space-separated tokens; a quoted string holds no quote, the shim escapes it.
struct Tokens<'a>(&'a str);

impl<'a> Iterator for Tokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let line = self.0;
        let bytes = line.as_bytes();
        let start = bytes.iter().position(|&b| b != b' ')?;
        let mut end = start;
        while end < bytes.len() && bytes[end] != b' ' {
            if bytes[end] == b'"' {
                end = end + 1 + line[end + 1..].find('"').expect("the shim closes every quote");
            }
            end += 1;
        }
        self.0 = &line[end..];
        Some(&line[start..end])
    }
}

fn value_of(token: &str) -> Cow<'_, str> {
    if token.len() >= 2 && token.starts_with('"') && token.ends_with('"') {
        return unescape(&token[1..token.len() - 1]);
    }
    Cow::Borrowed(token)
}

/// `re.sub(r"\\x([0-9a-f]{2})", chr(int(..., 16)), ...)`.
fn unescape(text: &str) -> Cow<'_, str> {
    if !text.contains("\\x") {
        return Cow::Borrowed(text);
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("\\x") {
        let hex = rest.get(at + 2..at + 4).filter(|hex| hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        match hex {
            Some(hex) => {
                out.push_str(&rest[..at]);
                out.push(char::from(u8::from_str_radix(hex, 16).expect("two hex digits")));
                rest = &rest[at + 4..];
            }
            None => {
                out.push_str(&rest[..at + 2]);
                rest = &rest[at + 2..];
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_results_arguments_and_fields() {
        let records = parse("n7 CGBinary O_PLUS n5 n6 TY_INTEGER\n- CGDone n7\nSYM y1 name=\"pal now\" attr=0x42\n");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].result, Some("n7"));
        assert_eq!(records[0].args, ["O_PLUS", "n5", "n6", "TY_INTEGER"]);
        assert_eq!(records[1].result, None);
        assert_eq!(records[2].field("name"), Some("pal now"));
        assert_eq!(records[2].line, 3);
    }

    #[test]
    fn unescapes_hex_bytes() {
        assert_eq!(value_of("\"a\\x41\\x7fz\""), "aA\u{7f}z");
    }

    #[test]
    fn lines_break_as_splitlines_does() {
        let records = parse("A\r\nB\n\nC\u{85}D\u{2028}E\rF\x0bG");
        let calls: Vec<_> = records.iter().map(|one| (one.line, one.call)).collect();
        assert_eq!(calls, [(1, "A"), (2, "B"), (4, "C"), (5, "D"), (6, "E"), (7, "F"), (8, "G")]);
    }

    #[test]
    fn the_last_of_a_repeated_field_wins() {
        assert_eq!(parse("X k=1 k=2")[0].field("k"), Some("2"));
    }

    /// Copying every token of the 6,231 lines of `d_faces` into a `String` cost 17.9 M instructions.
    #[test]
    fn a_token_without_an_escape_is_not_copied() {
        let records = parse("n7 CGBinary O_PLUS name=\"a b\" other=\"c\\x41\"");
        assert!(records[0].args.iter().all(|one| matches!(one, Cow::Borrowed(_))));
        assert!(matches!(records[0].fields[0].1, Cow::Borrowed("a b")));
        assert!(matches!(records[0].fields[1].1, Cow::Owned(_)));
    }
}
