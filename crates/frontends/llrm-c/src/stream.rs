//! The stream toolchain/owshim/cgshim.c
//! writes, one code-generator call per line.
//!
//! ```text
//! n7 CGBinary O_PLUS n5 n6 TY_INTEGER     a call and the handle it returned
//! - CGDone n7                             a call that returned nothing
//! SYM y1 name="pal_now" attr=0x42 seg=11  a record the shim adds
//! ```

use std::borrow::Cow;

/// One line of the stream, borrowing the text it was read from: a value is
/// copied only where it holds an escape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record<'a> {
    pub line: usize,
    pub result: Option<&'a str>,
    pub call: &'a str,
    pub args: Vec<Cow<'a, str>>,
    pub fields: Vec<(&'a str, Cow<'a, str>)>,
}

impl Record<'_> {
    /// The field `key` names, if the line has one.
    pub fn field(
        &self,
        key: &str,
    ) -> Option<&str> {
        self.fields.iter().find(|(name, _)| *name == key).map(|(_, value)| value.as_ref())
    }
}

/// `HANDLE.fullmatch`: `[a-z]\d+`.
fn is_handle(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() > 1 && bytes[0].is_ascii_lowercase() && bytes[1..].iter().all(u8::is_ascii_digit)
}

pub fn parse(text: &str) -> Vec<Record<'_>> {
    // A record per line at most: the newlines, and the last line if
    // unterminated.
    let mut records = Vec::with_capacity(text.bytes().filter(|&one| one == b'\n').count() + 1);
    for (index, line) in lines(text).enumerate() {
        let mut tokens = Tokens { line, at: 0 };
        let Some(mut first) = tokens.next() else { continue };
        let mut result = None;
        if first == "-" || is_handle(first) {
            result = if first == "-" { None } else { Some(first) };
            // Python indexed `tokens[1:]` and `tokens[0]` after `remove(0)`.
            let Some(next) = tokens.next() else { panic!("IndexError: list index out of range") };
            first = next;
        }
        let mut args = Vec::new();
        let mut fields: Vec<(&str, Cow<str>)> = Vec::new();
        for one in tokens {
            match one.split_once('=') {
                Some((key, value)) if is_identifier(key) => {
                    // A repeated key keeps its first place and its last value.
                    let value = value_of(value);
                    match fields.iter_mut().find(|(name, _)| *name == key) {
                        Some(found) => found.1 = value,
                        None => fields.push((key, value)),
                    }
                }
                _ => args.push(value_of(one)),
            }
        }
        records.push(Record { line: index + 1, result, call: first, args, fields });
    }
    records
}

/// `splitlines(text)`, one line at a time. The shim escapes everything but
/// printable ASCII, so a stream with no line break but `\n` splits on that
/// alone, by the searches `str` makes with SIMD.
fn lines(text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    let plain =
        text.is_ascii() && !['\r', '\x0b', '\x0c', '\x1c', '\x1d', '\x1e'].iter().any(|one| text.contains(*one));
    if plain { Box::new(text.split_terminator('\n')) } else { Box::new(splitlines(text).into_iter()) }
}

/// `str.splitlines`.
pub(crate) fn splitlines(text: &str) -> Vec<&str> {
    // The shim escapes everything but printable ASCII and writes `\n` only.
    if text.bytes().all(|b| b == b'\n' || (b >= 0x20 && b < 0x7f) || b == b'\t') {
        let mut lines: Vec<&str> = text.split('\n').collect();
        if lines.last() == Some(&"") {
            lines.pop();
        }
        return lines;
    }
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, character)) = chars.next() {
        let breaks = matches!(
            character,
            '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
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
    matches!(chars.next(), Some(c) if c == '_' || c.is_alphabetic()) && chars.all(|c| c == '_' || c.is_alphanumeric())
}

/// Space-separated tokens; a quoted string holds no quote, the shim escapes it.
struct Tokens<'a> {
    line: &'a str,
    at: usize,
}

impl<'a> Iterator for Tokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.line.as_bytes();
        while self.at < bytes.len() && bytes[self.at] == b' ' {
            self.at += 1;
        }
        if self.at >= bytes.len() {
            return None;
        }
        let start = self.at;
        let mut end = start;
        while end < bytes.len() && bytes[end] != b' ' {
            if bytes[end] == b'"' {
                end = end + 1 + self.line[end + 1..].find('"').expect("the shim closes every quote");
            }
            end += 1;
        }
        self.at = end;
        Some(&self.line[start..end])
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

    /// The records grew by doubling (12 growths, 0.8 M Ir on d_faces): 100
    /// lines left room for 128.
    #[test]
    fn test_the_records_are_sized_by_the_lines() {
        let text = "- CGDone n7\n".repeat(100);
        let records = parse(&text);
        assert_eq!((records.len(), records.capacity()), (100, 101));
    }

    #[test]
    fn unescapes_hex_bytes() {
        assert_eq!(value_of("\"a\\x41\\x7fz\""), "aA\u{7f}z");
    }

    /// Every token was copied into a String (a 10.8 M-instruction parse of
    /// d_faces' stream): only a value holding an escape is.
    #[test]
    fn copies_only_what_it_unescapes() {
        let records = parse("n7 CGBinary O_PLUS n5 name=\"a\\x41\" kind=plain\n");
        assert!(matches!(records[0].args[0], std::borrow::Cow::Borrowed(_)));
        assert!(matches!(records[0].fields[0].1, std::borrow::Cow::Owned(_)));
        assert!(matches!(records[0].fields[1].1, std::borrow::Cow::Borrowed(_)));
        assert_eq!(records[0].field("name"), Some("aA"));
    }

    #[test]
    fn a_repeated_key_keeps_its_place_and_its_last_value() {
        let records = parse("- X a=1 b=2 a=3\n");
        assert_eq!(
            records[0].fields.iter().map(|(k, v)| (*k, v.as_ref())).collect::<Vec<_>>(),
            [("a", "3"), ("b", "2")]
        );
    }

    /// `str.splitlines` breaks on more than `\n`; the fast path must not.
    #[test]
    fn lines_break_as_pythons_do() {
        assert_eq!(splitlines("a\nb\n"), ["a", "b"]);
        assert_eq!(splitlines("a\r\nb\u{2028}c"), ["a", "b", "c"]);
        assert_eq!(splitlines("a\n\nb"), ["a", "", "b"]);
        assert_eq!(splitlines(""), Vec::<&str>::new());
    }

    fn tokens(line: &str) -> Vec<&str> {
        Tokens { line, at: 0 }.collect()
    }

    /// A quoted value holds spaces; a token ends at the first space outside
    /// quotes, and leading, repeated and trailing spaces make none.
    #[test]
    fn a_quoted_value_keeps_its_spaces() {
        assert_eq!(tokens("a \"b c\" d=\"e f\"g h"), ["a", "\"b c\"", "d=\"e f\"g", "h"]);
        assert_eq!(tokens("  x   y  "), ["x", "y"]);
        assert_eq!(tokens("n=\"p q\" r=\"s t\""), ["n=\"p q\"", "r=\"s t\""]);
        assert_eq!(tokens("\"a\"\"b c\" z"), ["\"a\"\"b c\"", "z"]);
        assert!(tokens("   ").is_empty());
    }

    /// The lazy lines are `str.splitlines`'s on a stream and on text with the
    /// other line breaks.
    #[test]
    fn lazy_lines_equal_splitlines() {
        for text in ["a\nb\n", "a\n\nb", "", "\n", "a", "a\r\nb", "a\u{85}b\n", "x\x0cy\n", "é\nz\n"] {
            assert_eq!(lines(text).collect::<Vec<_>>(), splitlines(text), "{text:?}");
        }
    }
}
