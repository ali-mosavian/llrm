//! Logical lines: the physical lines of a source with BC's `_` continuations
//! joined.

use std::borrow::Cow;

use crate::Dialect;

/// Where BC takes a continuation to begin on a physical line: an `_` outside a
/// string, a comment and DATA's text. What follows it on the line is ignored,
/// and the next line is joined on (`PRINT 1 _ anything` and `PRINT 1_` are the
/// same). In VBDOS an `_` after a letter or digit is part of a name, except
/// straight after a number.
fn continuation_at(
    physical: &str,
    dialect: Dialect,
) -> Option<usize> {
    let bytes = physical.as_bytes();
    let mut quoted = false;
    let mut data = false;
    let mut statement_start = true;
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte == b'"' {
            quoted = !quoted;
        } else if !quoted {
            match byte {
                b'\'' => return None,
                b':' => {
                    data = false;
                    statement_start = true;
                }
                b'_' if !data => {
                    let previous = at.checked_sub(1).map(|index| bytes[index]);
                    let in_name = dialect.underscore_in_names()
                        && previous.is_some_and(|p| p.is_ascii_alphabetic() || p == b'_' || p == b'.')
                        || (dialect.underscore_in_names() && previous.is_some_and(|p| p.is_ascii_digit()) && {
                            // a digit ends a name or a number: a number takes
                            // the continuation, a name does not
                            let word = &physical[..at];
                            let start = word.rfind(|c: char| !c.is_ascii_alphanumeric()).map_or(0, |x| x + 1);
                            !word[start..].starts_with(|c: char| c.is_ascii_digit())
                        });
                    if !in_name {
                        return Some(at);
                    }
                }
                _ if byte.is_ascii_alphabetic() => {
                    let end = bytes[at..]
                        .iter()
                        .position(|b| !(b.is_ascii_alphanumeric() || *b == b'.' || *b == b'$'))
                        .map_or(bytes.len(), |n| at + n);
                    let word = &physical[at..end];
                    if word.eq_ignore_ascii_case("REM") && !bytes.get(end).is_some_and(|b| b.is_ascii_alphanumeric()) {
                        return None;
                    }
                    if statement_start && word.eq_ignore_ascii_case("DATA") {
                        data = true;
                    }
                    statement_start = false;
                    at = end;
                    continue;
                }
                b' ' | b'\t' => {}
                _ => statement_start = false,
            }
        }
        at += 1;
    }
    None
}

/// The same logical lines as the old lexer's, borrowed where nothing continues.
pub(crate) fn logical_lines(
    source: &str,
    dialect: Dialect,
) -> Vec<(usize, Cow<'_, str>)> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut begins = 1;
    for (index, physical) in source.lines().enumerate() {
        if current.is_empty() {
            begins = index + 1;
        }
        if let Some(at) = continuation_at(physical, dialect) {
            current.push_str(physical[..at].trim_end());
            current.push(' ');
        } else if current.is_empty() {
            out.push((begins, Cow::Borrowed(physical)));
        } else {
            current.push_str(physical);
            out.push((begins, Cow::Owned(std::mem::take(&mut current))));
        }
    }
    if !current.is_empty() {
        out.push((begins, Cow::Owned(current)));
    }
    out
}
