//! Python's builtin `repr`, for stage dumps that must match Python's text.
//!
//! A dump line such as `f"symbol {one}"` prints a dataclass through `repr`;
//! reproducing it byte for byte is what lets `tools/port_diff.py` compare
//! stages.

use std::fmt::Write;

use indexmap::IndexMap;

/// `repr(value)`.
pub trait Repr {
    fn repr(&self) -> String;
}

impl Repr for i64 {
    fn repr(&self) -> String {
        self.to_string()
    }
}

impl Repr for i32 {
    fn repr(&self) -> String {
        self.to_string()
    }
}

impl Repr for u32 {
    fn repr(&self) -> String {
        self.to_string()
    }
}

impl Repr for usize {
    fn repr(&self) -> String {
        self.to_string()
    }
}

impl Repr for bool {
    fn repr(&self) -> String {
        if *self { "True" } else { "False" }.to_owned()
    }
}

impl Repr for str {
    fn repr(&self) -> String {
        string(self)
    }
}

impl Repr for String {
    fn repr(&self) -> String {
        string(self)
    }
}

impl<T: Repr + ?Sized> Repr for &T {
    fn repr(&self) -> String {
        (**self).repr()
    }
}

impl<T: Repr> Repr for Option<T> {
    fn repr(&self) -> String {
        match self {
            Some(value) => value.repr(),
            None => "None".to_owned(),
        }
    }
}

/// A Python list is a `Vec`; a Python tuple is `Tuple`.
impl<T: Repr> Repr for Vec<T> {
    fn repr(&self) -> String {
        list(self)
    }
}

impl<A: Repr, B: Repr> Repr for (A, B) {
    fn repr(&self) -> String {
        format!("({}, {})", self.0.repr(), self.1.repr())
    }
}

impl<A: Repr, B: Repr, C: Repr> Repr for (A, B, C) {
    fn repr(&self) -> String {
        format!("({}, {}, {})", self.0.repr(), self.1.repr(), self.2.repr())
    }
}

impl<A: Repr, B: Repr, C: Repr, D: Repr> Repr for (A, B, C, D) {
    fn repr(&self) -> String {
        format!(
            "({}, {}, {}, {})",
            self.0.repr(),
            self.1.repr(),
            self.2.repr(),
            self.3.repr()
        )
    }
}

/// A Python tuple of any length.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Tuple<T>(pub Vec<T>);

impl<T: Repr> Repr for Tuple<T> {
    fn repr(&self) -> String {
        tuple(&self.0)
    }
}

impl<K: Repr, V: Repr> Repr for IndexMap<K, V> {
    fn repr(&self) -> String {
        let items: Vec<String> = self
            .iter()
            .map(|(key, value)| format!("{}: {}", key.repr(), value.repr()))
            .collect();
        format!("{{{}}}", items.join(", "))
    }
}

/// `repr(list)`.
pub fn list<T: Repr>(items: &[T]) -> String {
    let items: Vec<String> = items.iter().map(Repr::repr).collect();
    format!("[{}]", items.join(", "))
}

/// `repr(tuple)`.
pub fn tuple<T: Repr>(items: &[T]) -> String {
    match items {
        [] => "()".to_owned(),
        [one] => format!("({},)", one.repr()),
        _ => {
            let items: Vec<String> = items.iter().map(Repr::repr).collect();
            format!("({})", items.join(", "))
        }
    }
}

/// `repr(frozenset)`, elements in the order given.
pub fn frozenset<T: Repr>(items: &[T]) -> String {
    if items.is_empty() {
        return "frozenset()".to_owned();
    }
    let items: Vec<String> = items.iter().map(Repr::repr).collect();
    format!("frozenset({{{}}})", items.join(", "))
}

/// The generated `__repr__` of a dataclass: `Name(field=repr, ...)`.
pub fn dataclass(name: &str, fields: &[(&str, String)]) -> String {
    let fields: Vec<String> = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    format!("{name}({})", fields.join(", "))
}

/// `repr` of a `StrEnum` member: `<Class.MEMBER: 'value'>`.
pub fn str_enum(class: &str, member: &str, value: &str) -> String {
    format!("<{class}.{member}: {}>", string(value))
}

/// `repr(str)`.
pub fn string(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character == quote => {
                out.push('\\');
                out.push(character);
            }
            character if !printable(character) => {
                let scalar = character as u32;
                if scalar <= 0xff {
                    let _ = write!(out, "\\x{scalar:02x}");
                } else if scalar <= 0xffff {
                    let _ = write!(out, "\\u{scalar:04x}");
                } else {
                    let _ = write!(out, "\\U{scalar:08x}");
                }
            }
            character => out.push(character),
        }
    }
    out.push(quote);
    out
}

/// `repr(bytes)`.
pub fn bytes(value: &[u8]) -> String {
    let quote = if value.contains(&b'\'') && !value.contains(&b'"') {
        b'"'
    } else {
        b'\''
    };
    let mut out = String::from("b");
    out.push(quote as char);
    for &byte in value {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            byte if byte == quote => {
                out.push('\\');
                out.push(byte as char);
            }
            0x20..=0x7e => out.push(byte as char),
            byte => {
                let _ = write!(out, "\\x{byte:02x}");
            }
        }
    }
    out.push(quote as char);
    out
}

/// `str.isprintable` for one character.
fn printable(character: char) -> bool {
    // Rust's `escape_debug` leaves exactly the Unicode-printable characters
    // alone, which is Python's rule apart from the quotes it handles itself.
    matches!(character, '\'' | '"') || character.escape_debug().to_string() == character.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_quotes_like_python() {
        assert_eq!(string("a"), "'a'");
        assert_eq!(string("it's"), "\"it's\"");
        assert_eq!(string("'\""), "'\\'\"'");
        assert_eq!(string("\x01\u{e9}"), "'\\x01\u{e9}'");
        assert_eq!(string("\u{85}"), "'\\x85'");
    }

    #[test]
    fn bytes_quote_like_python() {
        assert_eq!(bytes(b"a'\x00"), "b\"a'\\x00\"");
        assert_eq!(bytes(b"\x7f"), "b'\\x7f'");
    }

    #[test]
    fn tuples_keep_the_single_comma() {
        assert_eq!(tuple::<i64>(&[]), "()");
        assert_eq!(tuple(&[1i64]), "(1,)");
        assert_eq!(tuple(&[1i64, 2]), "(1, 2)");
        assert_eq!(list(&[(1i64, "a".to_owned())]), "[(1, 'a')]");
        assert_eq!(frozenset::<i64>(&[]), "frozenset()");
    }
}
