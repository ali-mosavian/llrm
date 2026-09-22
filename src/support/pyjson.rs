//! Python's `json.loads` and `json.dumps` over Python's JSON objects.
//!
//! Direct port of CPython 3.13 `json.decoder` and the pure-Python scanner,
//! so a malformed document is refused with Python's exact message, and of
//! `json.encoder`'s `ensure_ascii` spelling.

use crate::support::hash::IndexMap;

use crate::support::pyrepr;

/// A value `json.loads` produces: `None | bool | int | float | str | list |
/// dict`.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Json>),
    Dict(IndexMap<String, Json>),
}

impl Json {
    /// `repr()` of the Python object.
    pub fn repr(&self) -> String {
        match self {
            Json::None => "None".to_owned(),
            Json::Bool(true) => "True".to_owned(),
            Json::Bool(false) => "False".to_owned(),
            Json::Int(one) => one.to_string(),
            Json::Float(one) => pyrepr::float(*one),
            Json::Str(one) => pyrepr::string(one),
            Json::List(items) => {
                let items: Vec<String> = items.iter().map(Json::repr).collect();
                format!("[{}]", items.join(", "))
            }
            Json::Dict(items) => {
                let items: Vec<String> =
                    items.iter().map(|(key, value)| format!("{}: {}", pyrepr::string(key), value.repr())).collect();
                format!("{{{}}}", items.join(", "))
            }
        }
    }
}

/// `json.JSONDecodeError.__str__`: `msg: line L column C (char P)`.
fn error(message: &str, doc: &[char], pos: usize) -> String {
    let lineno = doc[..pos].iter().filter(|one| **one == '\n').count() + 1;
    let colno = match doc[..pos].iter().rposition(|one| *one == '\n') {
        Some(newline) => pos - newline,
        None => pos + 1,
    };
    format!("{message}: line {lineno} column {colno} (char {pos})")
}

const WHITESPACE: [char; 4] = [' ', '\t', '\n', '\r'];

fn whitespace_end(s: &[char], mut end: usize) -> usize {
    while end < s.len() && WHITESPACE.contains(&s[end]) {
        end += 1;
    }
    end
}

/// `json.loads(text)`.
pub fn loads(text: &str) -> Result<Json, String> {
    let s: Vec<char> = text.chars().collect();
    if s.first() == Some(&'\u{feff}') {
        return Err(error("Unexpected UTF-8 BOM (decode using utf-8-sig)", &s, 0));
    }
    let start = whitespace_end(&s, 0);
    let (obj, end) = match scan_once(&s, start)? {
        Scanned::Value(obj, end) => (obj, end),
        Scanned::Stop(at) => return Err(error("Expecting value", &s, at)),
    };
    let end = whitespace_end(&s, end);
    if end != s.len() {
        return Err(error("Extra data", &s, end));
    }
    Ok(obj)
}

/// A scanned value, or Python's `StopIteration(idx)`.
enum Scanned {
    Value(Json, usize),
    Stop(usize),
}

fn starts(s: &[char], idx: usize, word: &str) -> bool {
    let word: Vec<char> = word.chars().collect();
    s.len() >= idx + word.len() && s[idx..idx + word.len()] == word[..]
}

fn scan_once(s: &[char], idx: usize) -> Result<Scanned, String> {
    let Some(&nextchar) = s.get(idx) else {
        return Ok(Scanned::Stop(idx));
    };
    if nextchar == '"' {
        let (string, end) = scanstring(s, idx + 1)?;
        return Ok(Scanned::Value(Json::Str(string), end));
    } else if nextchar == '{' {
        let (object, end) = parse_object(s, idx + 1)?;
        return Ok(Scanned::Value(object, end));
    } else if nextchar == '[' {
        let (array, end) = parse_array(s, idx + 1)?;
        return Ok(Scanned::Value(array, end));
    } else if nextchar == 'n' && starts(s, idx, "null") {
        return Ok(Scanned::Value(Json::None, idx + 4));
    } else if nextchar == 't' && starts(s, idx, "true") {
        return Ok(Scanned::Value(Json::Bool(true), idx + 4));
    } else if nextchar == 'f' && starts(s, idx, "false") {
        return Ok(Scanned::Value(Json::Bool(false), idx + 5));
    }
    if let Some((integer, frac, exp, end)) = match_number(s, idx) {
        let value = if frac.is_some() || exp.is_some() {
            let text = format!("{integer}{}{}", frac.unwrap_or_default(), exp.unwrap_or_default());
            Json::Float(text.parse::<f64>().expect("a JSON number is a float literal"))
        } else {
            match integer.parse::<i64>() {
                Ok(value) => Json::Int(value),
                Err(_) => return Err(format!("integer {integer} exceeds 64 bits")),
            }
        };
        return Ok(Scanned::Value(value, end));
    } else if nextchar == 'N' && starts(s, idx, "NaN") {
        return Ok(Scanned::Value(Json::Float(f64::NAN), idx + 3));
    } else if nextchar == 'I' && starts(s, idx, "Infinity") {
        return Ok(Scanned::Value(Json::Float(f64::INFINITY), idx + 8));
    } else if nextchar == '-' && starts(s, idx, "-Infinity") {
        return Ok(Scanned::Value(Json::Float(f64::NEG_INFINITY), idx + 9));
    }
    Ok(Scanned::Stop(idx))
}

/// `NUMBER_RE = r'(-?(?:0|[1-9]\d*))(\.\d+)?([eE][-+]?\d+)?'`.
fn match_number(s: &[char], idx: usize) -> Option<(String, Option<String>, Option<String>, usize)> {
    let digit = |at: usize| s.get(at).is_some_and(char::is_ascii_digit);
    let mut end = idx;
    if s.get(end) == Some(&'-') {
        end += 1;
    }
    if s.get(end) == Some(&'0') {
        end += 1;
    } else if s.get(end).is_some_and(|one| ('1'..='9').contains(one)) {
        end += 1;
        while digit(end) {
            end += 1;
        }
    } else {
        return None;
    }
    let integer: String = s[idx..end].iter().collect();
    let mut frac = None;
    if s.get(end) == Some(&'.') && digit(end + 1) {
        let start = end;
        end += 1;
        while digit(end) {
            end += 1;
        }
        frac = Some(s[start..end].iter().collect());
    }
    let mut exp = None;
    if s.get(end).is_some_and(|one| *one == 'e' || *one == 'E') {
        let start = end;
        let mut at = end + 1;
        if s.get(at).is_some_and(|one| *one == '-' || *one == '+') {
            at += 1;
        }
        if digit(at) {
            while digit(at) {
                at += 1;
            }
            end = at;
            exp = Some(s[start..end].iter().collect());
        }
    }
    Some((integer, frac, exp, end))
}

fn _decode_uxxxx(s: &[char], pos: usize) -> Result<u32, String> {
    let esc: String = s.get(pos + 1..pos + 5).map(|one| one.iter().collect()).unwrap_or_default();
    if esc.chars().count() == 4 && !matches!(esc.chars().nth(1), Some('x' | 'X')) {
        if let Ok(value) = u32::from_str_radix(&esc, 16) {
            return Ok(value);
        }
    }
    Err(error("Invalid \\uXXXX escape", s, pos))
}

/// `py_scanstring(s, end, strict=True)`.
fn scanstring(s: &[char], mut end: usize) -> Result<(String, usize), String> {
    let mut chunks = String::new();
    let begin = end - 1;
    loop {
        // STRINGCHUNK: `(.*?)(["\\\x00-\x1f])`
        let Some(stop) = (end..s.len()).find(|at| matches!(s[*at], '"' | '\\' | '\x00'..='\x1f')) else {
            return Err(error("Unterminated string starting at", s, begin));
        };
        chunks.extend(&s[end..stop]);
        let terminator = s[stop];
        end = stop + 1;
        if terminator == '"' {
            break;
        } else if terminator != '\\' {
            let message = format!("Invalid control character {} at", pyrepr::string(&terminator.to_string()));
            return Err(error(&message, s, end));
        }
        let Some(&esc) = s.get(end) else {
            return Err(error("Unterminated string starting at", s, begin));
        };
        let character = if esc != 'u' {
            let character = match esc {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\x08',
                'f' => '\x0c',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => {
                    let message = format!("Invalid \\escape: {}", pyrepr::string(&esc.to_string()));
                    return Err(error(&message, s, end));
                }
            };
            end += 1;
            character
        } else {
            let mut uni = _decode_uxxxx(s, end)?;
            end += 5;
            if (0xd800..=0xdbff).contains(&uni) && starts(s, end, "\\u") {
                let uni2 = _decode_uxxxx(s, end + 1)?;
                if (0xdc00..=0xdfff).contains(&uni2) {
                    uni = 0x10000 + (((uni - 0xd800) << 10) | (uni2 - 0xdc00));
                    end += 6;
                }
            }
            // A lone surrogate is a Python `str` but no Rust `char`.
            char::from_u32(uni).unwrap_or('\u{fffd}')
        };
        chunks.push(character);
    }
    Ok((chunks, end))
}

/// `JSONObject((s, end), strict=True, ...)`.
fn parse_object(s: &[char], mut end: usize) -> Result<(Json, usize), String> {
    let mut pairs: IndexMap<String, Json> = IndexMap::default();
    let mut nextchar = s.get(end).copied();
    if nextchar != Some('"') {
        if nextchar.is_some_and(|one| WHITESPACE.contains(&one)) {
            end = whitespace_end(s, end);
            nextchar = s.get(end).copied();
        }
        if nextchar == Some('}') {
            return Ok((Json::Dict(pairs), end + 1));
        } else if nextchar != Some('"') {
            return Err(error("Expecting property name enclosed in double quotes", s, end));
        }
    }
    end += 1;
    loop {
        let key;
        (key, end) = scanstring(s, end)?;
        if s.get(end) != Some(&':') {
            end = whitespace_end(s, end);
            if s.get(end) != Some(&':') {
                return Err(error("Expecting ':' delimiter", s, end));
            }
        }
        end += 1;
        if s.get(end).is_some_and(|one| WHITESPACE.contains(one)) {
            end += 1;
            if s.get(end).is_some_and(|one| WHITESPACE.contains(one)) {
                end = whitespace_end(s, end + 1);
            }
        }
        let value;
        (value, end) = match scan_once(s, end)? {
            Scanned::Value(value, end) => (value, end),
            Scanned::Stop(at) => return Err(error("Expecting value", s, at)),
        };
        pairs.insert(key, value);
        let mut nextchar = s.get(end).copied();
        if nextchar.is_some_and(|one| WHITESPACE.contains(&one)) {
            end = whitespace_end(s, end + 1);
            nextchar = s.get(end).copied();
        }
        end += 1;
        if nextchar == Some('}') {
            break;
        } else if nextchar != Some(',') {
            return Err(error("Expecting ',' delimiter", s, end - 1));
        }
        let comma_idx = end - 1;
        end = whitespace_end(s, end);
        let nextchar = s.get(end).copied();
        end += 1;
        if nextchar != Some('"') {
            if nextchar == Some('}') {
                return Err(error("Illegal trailing comma before end of object", s, comma_idx));
            }
            return Err(error("Expecting property name enclosed in double quotes", s, end - 1));
        }
    }
    Ok((Json::Dict(pairs), end))
}

/// `JSONArray((s, end), scan_once)`.
fn parse_array(s: &[char], mut end: usize) -> Result<(Json, usize), String> {
    let mut values = Vec::new();
    let mut nextchar = s.get(end).copied();
    if nextchar.is_some_and(|one| WHITESPACE.contains(&one)) {
        end = whitespace_end(s, end + 1);
        nextchar = s.get(end).copied();
    }
    if nextchar == Some(']') {
        return Ok((Json::List(values), end + 1));
    }
    loop {
        let value;
        (value, end) = match scan_once(s, end)? {
            Scanned::Value(value, end) => (value, end),
            Scanned::Stop(at) => return Err(error("Expecting value", s, at)),
        };
        values.push(value);
        let mut nextchar = s.get(end).copied();
        if nextchar.is_some_and(|one| WHITESPACE.contains(&one)) {
            end = whitespace_end(s, end + 1);
            nextchar = s.get(end).copied();
        }
        end += 1;
        if nextchar == Some(']') {
            break;
        } else if nextchar != Some(',') {
            return Err(error("Expecting ',' delimiter", s, end - 1));
        }
        let comma_idx = end - 1;
        if s.get(end).is_some_and(|one| WHITESPACE.contains(one)) {
            end += 1;
            if s.get(end).is_some_and(|one| WHITESPACE.contains(one)) {
                end = whitespace_end(s, end + 1);
            }
        }
        if s.get(end) == Some(&']') {
            return Err(error("Illegal trailing comma before end of array", s, comma_idx));
        }
    }
    Ok((Json::List(values), end))
}

/// `py_encode_basestring_ascii`.
fn encode_string(value: &str, out: &mut String) {
    out.push('"');
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ' '..='~' => out.push(character),
            _ => {
                let mut units = [0_u16; 2];
                for unit in character.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
}

/// `json.dumps(value, indent=indent, separators=separators,
/// sort_keys=sort_keys)` with Python's `ensure_ascii` and `allow_nan`
/// defaults.  `separators=None` is `(', ', ': ')`, or `(',', ': ')` with an
/// indent.
pub fn dumps(value: &Json, indent: Option<usize>, separators: Option<(&str, &str)>, sort_keys: bool) -> String {
    let (item, key) = separators.unwrap_or(if indent.is_some() { (",", ": ") } else { (", ", ": ") });
    let mut out = String::new();
    encode(value, indent, item, key, sort_keys, 0, &mut out);
    out
}

fn encode(value: &Json, indent: Option<usize>, item: &str, key: &str, sort_keys: bool, level: usize, out: &mut String) {
    let newline = |level: usize| indent.map(|width| format!("\n{}", " ".repeat(width * level)));
    match value {
        Json::None => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(one) => out.push_str(&one.to_string()),
        Json::Float(one) if one.is_nan() => out.push_str("NaN"),
        Json::Float(one) if one.is_infinite() => out.push_str(if *one > 0.0 { "Infinity" } else { "-Infinity" }),
        Json::Float(one) => out.push_str(&pyrepr::float(*one)),
        Json::Str(one) => encode_string(one, out),
        Json::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            let inner = newline(level + 1);
            let separator = format!("{item}{}", inner.clone().unwrap_or_default());
            out.push_str(&inner.unwrap_or_default());
            for (index, one) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(&separator);
                }
                encode(one, indent, item, key, sort_keys, level + 1, out);
            }
            out.push_str(&newline(level).unwrap_or_default());
            out.push(']');
        }
        Json::Dict(items) => {
            if items.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            let inner = newline(level + 1);
            let separator = format!("{item}{}", inner.clone().unwrap_or_default());
            out.push_str(&inner.unwrap_or_default());
            let mut pairs: Vec<(&String, &Json)> = items.iter().collect();
            if sort_keys {
                pairs.sort_by(|one, other| one.0.cmp(other.0));
            }
            for (index, (name, one)) in pairs.into_iter().enumerate() {
                if index > 0 {
                    out.push_str(&separator);
                }
                encode_string(name, out);
                out.push_str(key);
                encode(one, indent, item, key, sort_keys, level + 1, out);
            }
            out.push_str(&newline(level).unwrap_or_default());
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_refuses_with_pythons_messages() {
        // Expected text from CPython 3.13 json.loads.
        assert_eq!(loads("").unwrap_err(), "Expecting value: line 1 column 1 (char 0)");
        assert_eq!(loads("{\"a\":1,}").unwrap_err(), "Illegal trailing comma before end of object: line 1 column 7 (char 6)");
        assert_eq!(loads("[1 2]").unwrap_err(), "Expecting ',' delimiter: line 1 column 4 (char 3)");
        assert_eq!(loads("{}\n x").unwrap_err(), "Extra data: line 2 column 2 (char 4)");
    }

    #[test]
    fn dumps_spells_floats_and_non_ascii_as_python() {
        let value = loads("{\"b\":[1.5,1e20,\"\u{e9}\"],\"a\":-0}").unwrap();
        assert_eq!(dumps(&value, None, Some((",", ":")), true), "{\"a\":0,\"b\":[1.5,1e+20,\"\\u00e9\"]}");
        assert_eq!(dumps(&value, Some(2), None, true), "{\n  \"a\": 0,\n  \"b\": [\n    1.5,\n    1e+20,\n    \"\\u00e9\"\n  ]\n}");
    }
}
