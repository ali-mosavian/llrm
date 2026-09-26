//! LLVM's tokens, for the subset MIR reads.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// A local or global name: `%x`, `%"a b"`, or `%12`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Name {
    Named(String),
    Numbered(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Local(Name),
    Global(Name),
    /// `!name`: named metadata or an attachment's kind.
    MetadataName(String),
    /// `!12`.
    MetadataId(u32),
    /// `!` before `{` or a string.
    Exclaim,
    /// `#12`.
    AttributeGroup(u32),
    /// `name:`, `12:` or `"a b":`.
    Label(Name),
    Int { negative: bool, magnitude: u128 },
    /// A decimal floating literal.
    Float(f64),
    /// `0x` and 16 hex digits: a double's bits.
    HexFloat(u64),
    Str(Vec<u8>),
    /// `c"..."`.
    Bytes(Vec<u8>),
    Word(String),
    Punct(char),
    Dots,
    Eof,
}

pub fn lex(text: &str) -> Result<Vec<(Token, usize)>, ParseError> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let (mut at, mut line) = (0, 1);
    let fail = |line: usize, message: String| Err(ParseError { line, message });
    while at < bytes.len() {
        let c = bytes[at];
        match c {
            b'\n' => {
                line += 1;
                at += 1;
            }
            b' ' | b'\t' | b'\r' => at += 1,
            b';' => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
            }
            b'%' | b'@' => {
                let (name, next) = match name_at(bytes, at + 1) {
                    Ok(Some(found)) => found,
                    Ok(None) => return fail(line, format!("a name must follow `{}`", c as char)),
                    Err(message) => return fail(line, message),
                };
                at = next;
                out.push((if c == b'%' { Token::Local(name) } else { Token::Global(name) }, line));
            }
            b'!' => {
                let start = at + 1;
                let end = scan(bytes, start, |b| is_name(b) || b == b'\\');
                let word = &text[start..end];
                at = end;
                let token = if word.is_empty() {
                    Token::Exclaim
                } else if word.bytes().all(|b| b.is_ascii_digit()) {
                    Token::MetadataId(word.parse().map_err(|_| ParseError { line, message: format!("!{word} is too large") })?)
                } else {
                    Token::MetadataName(word.to_owned())
                };
                out.push((token, line));
            }
            b'#' => {
                let end = scan(bytes, at + 1, |b| b.is_ascii_digit());
                let Ok(group) = text[at + 1..end].parse() else { return fail(line, "`#` must be followed by a number".to_owned()) };
                out.push((Token::AttributeGroup(group), line));
                at = end;
            }
            b'"' => {
                let (value, next) = string_at(bytes, at).map_err(|message| ParseError { line, message })?;
                at = next;
                if bytes.get(at) == Some(&b':') {
                    at += 1;
                    out.push((Token::Label(Name::Named(utf8(value, line)?)), line));
                } else {
                    out.push((Token::Str(value), line));
                }
            }
            b'.' if text[at..].starts_with("...") => {
                out.push((Token::Dots, line));
                at += 3;
            }
            b'-' | b'0'..=b'9' => {
                let (token, next) = number_at(text, at).map_err(|message| ParseError { line, message })?;
                at = next;
                match token {
                    Token::Int { negative: false, magnitude } if bytes.get(at) == Some(&b':') => {
                        at += 1;
                        let number = u32::try_from(magnitude).map_err(|_| ParseError { line, message: "a label number is too large".to_owned() })?;
                        out.push((Token::Label(Name::Numbered(number)), line));
                    }
                    token => out.push((token, line)),
                }
            }
            b'=' | b',' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'<' | b'>' | b'*' | b':' => {
                out.push((Token::Punct(c as char), line));
                at += 1;
            }
            _ if c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c == b'.' => {
                let end = scan(bytes, at, is_name);
                let word = &text[at..end];
                at = end;
                if word == "c" && bytes.get(at) == Some(&b'"') {
                    let (value, next) = string_at(bytes, at).map_err(|message| ParseError { line, message })?;
                    at = next;
                    out.push((Token::Bytes(value), line));
                } else if bytes.get(at) == Some(&b':') {
                    at += 1;
                    out.push((Token::Label(Name::Named(word.to_owned())), line));
                } else {
                    out.push((Token::Word(word.to_owned()), line));
                }
            }
            _ => return fail(line, format!("unexpected character `{}`", c as char)),
        }
    }
    out.push((Token::Eof, line));
    Ok(out)
}

/// The characters an unquoted LLVM name may hold.
pub fn is_name(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'$' | b'.' | b'_')
}

fn scan(bytes: &[u8], mut at: usize, keep: impl Fn(u8) -> bool) -> usize {
    while at < bytes.len() && keep(bytes[at]) {
        at += 1;
    }
    at
}

fn utf8(bytes: Vec<u8>, line: usize) -> Result<String, ParseError> {
    String::from_utf8(bytes).map_err(|_| ParseError { line, message: "a name is not UTF-8".to_owned() })
}

/// The name after a sigil at `at`.
fn name_at(bytes: &[u8], at: usize) -> Result<Option<(Name, usize)>, String> {
    if bytes.get(at) == Some(&b'"') {
        let (value, next) = string_at(bytes, at)?;
        return Ok(Some((Name::Named(String::from_utf8(value).map_err(|_| "a name is not UTF-8".to_owned())?), next)));
    }
    let end = scan(bytes, at, is_name);
    if end == at {
        return Ok(None);
    }
    let word = std::str::from_utf8(&bytes[at..end]).expect("names are ASCII");
    if word.bytes().all(|b| b.is_ascii_digit()) {
        return word.parse().map(|number| Some((Name::Numbered(number), end))).map_err(|_| format!("{word} is too large"));
    }
    Ok(Some((Name::Named(word.to_owned()), end)))
}

/// A quoted string at `at`, its `\XX` and `\\` escapes decoded.
fn string_at(bytes: &[u8], at: usize) -> Result<(Vec<u8>, usize), String> {
    let mut out = Vec::new();
    let mut here = at + 1;
    loop {
        match bytes.get(here) {
            None | Some(b'\n') => return Err("a string is not closed".to_owned()),
            Some(b'"') => return Ok((out, here + 1)),
            Some(b'\\') if bytes.get(here + 1) == Some(&b'\\') => {
                out.push(b'\\');
                here += 2;
            }
            Some(b'\\') => {
                let hex = bytes.get(here + 1..here + 3).and_then(|two| std::str::from_utf8(two).ok());
                let value = hex.and_then(|two| u8::from_str_radix(two, 16).ok()).ok_or("a bad escape in a string")?;
                out.push(value);
                here += 3;
            }
            Some(&b) => {
                out.push(b);
                here += 1;
            }
        }
    }
}

fn number_at(text: &str, at: usize) -> Result<(Token, usize), String> {
    let bytes = text.as_bytes();
    if text[at..].starts_with("0x") {
        let end = scan(bytes, at + 2, |b| b.is_ascii_hexdigit());
        let digits = &text[at + 2..end];
        if digits.len() != 16 {
            return Err(format!("0x{digits}: MIR's hex floating literals are a double's 16 digits"));
        }
        return Ok((Token::HexFloat(u64::from_str_radix(digits, 16).expect("hex digits")), end));
    }
    let negative = bytes[at] == b'-';
    let start = at + usize::from(negative);
    let mut end = scan(bytes, start, |b| b.is_ascii_digit());
    if end == start {
        return Err("`-` must begin a number".to_owned());
    }
    if bytes.get(end) != Some(&b'.') {
        let magnitude = text[start..end].parse().map_err(|_| format!("{} is too large", &text[start..end]))?;
        return Ok((Token::Int { negative, magnitude }, end));
    }
    end = scan(bytes, end + 1, |b| b.is_ascii_digit());
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
        end = scan(bytes, end + 1 + sign, |b| b.is_ascii_digit());
    }
    let value: f64 = text[at..end].parse().map_err(|_| format!("{} is not a number", &text[at..end]))?;
    Ok((Token::Float(value), end))
}
