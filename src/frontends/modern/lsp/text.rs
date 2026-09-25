//! A module's text as LSP addresses it: spans to ranges and back, and the
//! names and comments around a declaration. A span's columns count bytes
//! from 1; a position's characters count UTF-16 units from 0.

use super::protocol::{Position, Range};
use crate::frontends::modern::syntax::Span;

/// Line `number`, counted from 1.
pub fn line(text: &str, number: usize) -> &str {
    text.lines().nth(number.saturating_sub(1)).unwrap_or("")
}

pub fn range(text: &str, span: Span) -> Range {
    Range {
        start: position(text, span.line, span.column),
        end: position(text, span.line, span.end_column.max(span.column)),
    }
}

/// The whole of line `number`.
pub fn line_range(text: &str, number: usize) -> Range {
    range(text, Span::new(number, 1, line(text, number).len() + 1))
}

fn position(text: &str, number: usize, column: usize) -> Position {
    let line = line(text, number);
    let mut byte = column.saturating_sub(1).min(line.len());
    while !line.is_char_boundary(byte) {
        byte -= 1;
    }
    Position {
        line: number.saturating_sub(1) as u32,
        character: line[..byte].encode_utf16().count() as u32,
    }
}

/// `position` as a line and byte column, both from 1.
pub fn at(text: &str, position: Position) -> (usize, usize) {
    let number = position.line as usize + 1;
    let mut units = 0;
    let byte = line(text, number)
        .char_indices()
        .find(|(_, character)| {
            units += character.len_utf16();
            units > position.character as usize
        })
        .map_or(line(text, number).len(), |(byte, _)| byte);
    (number, byte + 1)
}

/// Whether the cursor at `line` and `column` is on `span`, or just after it.
pub fn contains(span: Span, line: usize, column: usize) -> bool {
    span.line == line && span.column <= column && column <= span.end_column
}

/// Whether `span` spells `name`.
pub fn spells(text: &str, span: Span, name: &str) -> bool {
    line(text, span.line).get(span.column.saturating_sub(1)..span.end_column.saturating_sub(1)) == Some(name)
}

/// `name` where it first stands as a whole word on `span`'s line from
/// `span` on, as the name after a declaration's keyword; `span` if nowhere.
pub fn named(text: &str, span: Span, name: &str) -> Span {
    let line = line(text, span.line);
    let from = span.column.saturating_sub(1).min(line.len());
    let bytes = line.as_bytes();
    let whole = |at: usize| {
        let end = at + name.len();
        !(at > 0 && word(bytes[at - 1])) && !(end < bytes.len() && word(bytes[end]))
    };
    line[from..]
        .match_indices(name)
        .map(|(at, _)| from + at)
        .find(|&at| whole(at))
        .map_or(span, |at| Span { column: at + 1, end_column: at + 1 + name.len(), ..span })
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The dotted path ending in the name at `column` of `line`: `geo.Point`
/// with the cursor on `Point`. It starts with `.` when the name is a member
/// of something other than a name.
pub fn path_at(text: &str, number: usize, column: usize) -> Option<String> {
    let bytes = line(text, number).as_bytes();
    let mut at = column.saturating_sub(1).min(bytes.len());
    if at == bytes.len() || !word(bytes[at]) {
        at = at.checked_sub(1).filter(|&before| word(bytes[before]))?;
    }
    let end = (at..bytes.len()).find(|&one| !word(bytes[one])).unwrap_or(bytes.len());
    let start = (0..at).rev().find(|&one| !word(bytes[one]) && bytes[one] != b'.').map_or(0, |one| one + 1);
    Some(String::from_utf8_lossy(&bytes[start..end]).into_owned())
}

/// The text before `position` on its line.
pub fn before(text: &str, position: Position) -> &str {
    let (number, column) = at(text, position);
    &line(text, number)[..column - 1]
}

/// Line `number` as a declaration's signature, without the `:` opening its body.
pub fn signature(text: &str, number: usize) -> &str {
    let line = line(text, number).trim();
    line.strip_suffix(':').unwrap_or(line).trim_end()
}

/// The `#` comment lines directly above line `number`, attributes aside.
pub fn comments_above(text: &str, number: usize) -> Vec<&str> {
    let mut found = Vec::new();
    for above in (1..number).rev() {
        let one = line(text, above).trim();
        if let Some(comment) = one.strip_prefix('#') {
            found.push(comment.strip_prefix(' ').unwrap_or(comment));
        } else if !one.starts_with('@') {
            break;
        }
    }
    found.reverse();
    found
}
