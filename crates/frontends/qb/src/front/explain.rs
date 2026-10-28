//! Words for what went wrong, for the person who wrote the source.
//!
//! The parser and the checker know an error by BC's name and where it is. This
//! module turns that into a sentence about the program ("`x%` is used as an
//! array before any DIM ..."), from `messages.txt` and the source text at the
//! error; nothing here knows how an error was found.

use std::sync::OnceLock;

use llrm_support::hash::HashMap;

use crate::front::kw::Kw;
use crate::syntax::Span;
use crate::{Dialect, ParseError};

/// BC's name for an error, to what is wrong and what to do about it.
fn table() -> &'static HashMap<&'static str, (&'static str, &'static str)> {
    static CELL: OnceLock<HashMap<&'static str, (&'static str, &'static str)>> = OnceLock::new();
    CELL.get_or_init(|| {
        include_str!("messages.txt")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(|line| {
                let mut parts = line.splitn(3, '|');
                Some((parts.next()?, (parts.next()?, parts.next().unwrap_or(""))))
            })
            .collect()
    })
}

/// How a statement is written, by its first word.
fn forms() -> &'static HashMap<&'static str, &'static str> {
    static CELL: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        include_str!("syntax_forms.txt")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(|line| line.split_once('|'))
            .collect()
    })
}

/// A token the parser wanted, as a person reads it.
pub fn describe(wanted: Kw) -> String {
    match wanted {
        Kw::LParen => "`(`".into(),
        Kw::RParen => "`)`".into(),
        Kw::Comma => "`,`".into(),
        Kw::Colon => "`:`".into(),
        Kw::SColon => "`;`".into(),
        Kw::EQ => "`=`".into(),
        Kw::Minus => "`-`".into(),
        Kw::Add => "`+`".into(),
        Kw::Mult => "`*`".into(),
        Kw::Lbs => "`#`".into(),
        Kw::NewLine => "the end of the line".into(),
        other => format!("`{other:?}`"),
    }
}

/// The dialect as the manuals call it.
pub fn dialect_name(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::QBasic11 => "QBasic 1.1",
        Dialect::QuickBasic45 => "QuickBASIC 4.5",
        Dialect::Pds71 => "PDS 7.1",
        Dialect::VbDos => "VBDOS",
        Dialect::Quickr => "QuickrBASIC",
    }
}

/// The `--dialect` word for it.
pub fn dialect_flag(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::QBasic11 => "qbasic11",
        Dialect::QuickBasic45 => "qb45",
        Dialect::Pds71 => "pds71",
        Dialect::VbDos => "vbdos",
        Dialect::Quickr => "quickr",
    }
}

/// The text a span covers, and the statement it is in.
struct Where {
    text: String,
    statement: String,
    at_end: bool,
    /// The source on this line before the error, without trailing blanks.
    before: String,
    /// The error is at the first word of its statement.
    starts: bool,
    /// The letters, digits and dots right before the error, with no blank
    /// between: the start of a malformed number or name.
    run: String,
    /// The name the statement is about: the first word that is no statement
    /// keyword (`p` in `SUB p (a%)`).
    name: String,
}

/// Words that begin a statement and are followed by the name it is about.
const LEADING: &[&str] = &[
    "SUB", "FUNCTION", "TYPE", "DECLARE", "CALL", "STATIC", "SHARED", "DIM", "REDIM", "COMMON", "CONST", "DEF", "END",
    "EXIT", "ERASE", "SELECT", "CASE", "GOTO", "GOSUB", "RETURN",
];

fn locate(
    source: &str,
    span: Span,
) -> Where {
    let line = span.line.checked_sub(1).and_then(|index| source.lines().nth(index)).unwrap_or("");
    let chars: Vec<char> = line.chars().collect();
    let text: String = chars.iter().skip(span.start).take(span.end.saturating_sub(span.start)).collect();
    // the statement is what follows the last `:` before the error, outside a
    // string
    let mut start = 0;
    let mut quoted = false;
    for (at, c) in chars.iter().enumerate().take(span.start.min(chars.len())) {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => start = at + 1,
            _ => {}
        }
    }
    let head: String = chars[start.min(chars.len())..span.start.min(chars.len())].iter().collect();
    let word: String = head
        .trim_start()
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let starts = word.is_empty();
    let rest: String = chars[span.start.min(chars.len())..].iter().collect();
    let rest = rest.trim();
    let first_word = |s: &str| -> String { s.chars().take_while(|c| c.is_ascii_alphabetic()).collect() };
    let statement = match (word.len() < 2, first_word(rest)) {
        (true, first) if first.len() >= 2 && starts => first.to_ascii_uppercase(),
        (true, _) => String::new(),
        _ => word.to_ascii_uppercase(),
    };
    // a variable at the start of `x = ...` is no statement keyword
    let statement = if forms().contains_key(statement.as_str())
        || crate::front::reserved::is_reserved_name(crate::Dialect::VbDos, &statement)
    {
        statement
    } else {
        String::new()
    };
    let run: String = {
        let mut end = span.start.min(chars.len());
        let begin = loop {
            if end == 0 || !(chars[end - 1].is_ascii_alphanumeric() || matches!(chars[end - 1], '.' | '&')) {
                break end;
            }
            end -= 1;
        };
        chars[begin..span.start.min(chars.len())].iter().collect()
    };
    // the name: skip the statement keywords at the start of the rest
    let mut name = String::new();
    let mut words = rest.split_whitespace();
    while let Some(candidate) = words.next() {
        let bare = candidate.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        if LEADING.contains(&bare.to_ascii_uppercase().as_str()) {
            continue;
        }
        name = candidate
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '$' | '%' | '&' | '!' | '#' | '@'))
            .collect();
        break;
    }
    // a lone keyword stands for its whole statement
    let mut shown: String = text.trim().to_owned();
    if shown.chars().count() > 60 {
        shown = format!("{}...", shown.chars().take(57).collect::<String>());
    }
    if LEADING.contains(&shown.to_ascii_uppercase().as_str()) && !rest.is_empty() {
        let end = rest.find(':').unwrap_or(rest.len());
        shown = rest[..end].trim().chars().take(60).collect();
    }
    let before: String = chars[..span.start.min(chars.len())].iter().collect();
    Where {
        starts,
        run,
        name,
        text: shown,
        statement,
        at_end: span.start >= chars.len() || text.trim().is_empty(),
        before: before.trim_end().to_owned(),
    }
}

/// The span of the word `subject` at or after the start of `span`: the span
/// names the statement, the word is in it.
fn narrowed(
    source: &str,
    span: Span,
    subject: &str,
) -> Span {
    let line = span.line.checked_sub(1).and_then(|index| source.lines().nth(index)).unwrap_or("");
    let chars: Vec<char> = line.chars().collect();
    let wanted: Vec<char> = subject.to_ascii_uppercase().chars().collect();
    let word = |c: &char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '$' | '%' | '&' | '!' | '#');
    for at in span.start..chars.len().saturating_sub(wanted.len() - 1) {
        let found = chars[at..at + wanted.len()].iter().map(char::to_ascii_uppercase).eq(wanted.iter().copied());
        let bounded = (at == 0 || !word(&chars[at - 1])) && chars.get(at + wanted.len()).is_none_or(|c| !word(c));
        if found && bounded {
            return Span { line: span.line, start: at, end: at + wanted.len() };
        }
    }
    span
}

/// The keys to try before the plain code, most specific first: the text of
/// the error (`Identifier expected /TAB`), its kind, or the end of the line.
fn variants(
    code: &str,
    place: &Where,
) -> Vec<String> {
    let mut keys = vec![format!("{code} /{}", place.text.to_ascii_uppercase())];
    // a word of the text the table has a wording for (`/BYVAL`, `/CURRENCY`)
    keys.extend(
        place
            .text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| format!("{code} /{}", word.to_ascii_uppercase())),
    );
    if place.text.replace(' ', "").contains("()") {
        keys.push(format!("{code} /()"));
    }
    let text = place.text.as_str();
    if text.ends_with(['%', '&']) && text.contains(['.', 'E', 'D', 'e', 'd']) && !text.starts_with("&") {
        keys.push(format!("{code} /decimal"));
    } else if text.ends_with(['%', '&']) {
        keys.push(format!("{code} /range"));
    }
    if place.at_end {
        keys.push(format!("{code} /end"));
    }
    keys
}

fn fill(
    template: &str,
    place: &Where,
    other: &str,
) -> String {
    let statement = if place.statement.is_empty() { "this".to_owned() } else { place.statement.clone() };
    let name = if place.name.is_empty() { place.text.clone() } else { place.name.clone() };
    template
        .replace("{text}", &place.text)
        .replace("{name}", &name)
        .replace("{statement}", &statement)
        .replace("{other}", other)
}

/// A syntax error in words: what was found, in which statement, and what was
/// wanted instead.
fn syntax(
    error: &ParseError,
    place: &Where,
) -> (String, String) {
    // an error at the statement's own first word names that word
    let first = place.starts && place.text.chars().all(|c| c.is_ascii_alphabetic()) && !place.text.is_empty();
    let own = if first { place.text.to_ascii_uppercase() } else { place.statement.clone() };
    let statement = if own.is_empty() { "statement".to_owned() } else { format!("{own} statement") };
    let found = if place.at_end { "the end of the line".to_owned() } else { format!("`{}`", place.text) };
    // a number cut short by a stray letter or dot: `1E`, `1.2.3`
    if place.run.starts_with(|c: char| c.is_ascii_digit() || c == '.' || c == '&')
        && place.text.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '.')
    {
        let number = format!("{}{}", place.run, place.text);
        return (
            format!("`{number}` is not a valid number"),
            "check the digits, the exponent (`1E5`) and the type character".to_owned(),
        );
    }
    // `SUB p% ()`: a SUB returns nothing, so its name takes no type character
    if place.text == "(" && place.statement == "SUB" {
        let name = place.before.split_whitespace().last().unwrap_or("");
        if name.ends_with(['%', '&', '!', '#', '$', '@']) {
            return (
                format!("a SUB name cannot carry a type character, as `{name}` does"),
                "write the name without it; only a FUNCTION has a type".to_owned(),
            );
        }
    }
    // a label or line number too large to be one
    if place.text.len() >= 5
        && place.text.chars().all(|c| c.is_ascii_digit())
        && (place.starts
            || matches!(
                place.statement.as_str(),
                "GOTO" | "GOSUB" | "THEN" | "ELSE" | "RESUME" | "RESTORE" | "ON"
            ))
    {
        return (
            format!("`{}` is too large for a line number", place.text),
            "use a smaller number, or a label with a name".to_owned(),
        );
    }
    // `FNa (1)`, `FN a(1)`, `FNa(1)` where `DEF FNa` was written without
    // parameters
    if place.text == "(" {
        let words: Vec<&str> = place.before.split_whitespace().collect();
        let name = match words.as_slice() {
            [.., fn_word, name] if fn_word.eq_ignore_ascii_case("FN") => Some(format!("FN{name}")),
            [.., last] if last.len() > 2 && last[..2].eq_ignore_ascii_case("FN") => {
                let end = last
                    .rfind(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '$' | '%' | '&' | '!' | '#'))
                    .map_or(0, |at| at + 1);
                Some(last[end..].to_owned())
            }
            _ => None,
        };
        if let Some(name) = name.filter(|name| name.len() > 2) {
            return (
                format!("`{name}` takes no arguments: it was defined without a parameter list"),
                format!("remove the parentheses, or give `DEF {name}` parameters"),
            );
        }
    }
    // two values side by side: `"a" "b"`, `"a""b"`
    if place.text.starts_with('"') && place.before.ends_with('"') {
        return (
            format!("`{}` follows another string with nothing between them", place.text),
            "join strings with `+`; BASIC has no quote escape, write a quote as CHR$(34)".to_owned(),
        );
    }
    // `;` joins items only in PRINT-style lists
    if place.text == ";" && place.statement.is_empty() {
        return (
            "`;` cannot follow a value here".to_owned(),
            "`;` separates the items of PRINT, LPRINT and INPUT; join strings with `+`".to_owned(),
        );
    }
    // a line number, twice or malformed: `10 10 x = 1`, `-1 x = 1`, `1E3 x = 1`
    if place.starts && (place.text.starts_with('-') || place.text.starts_with(|c: char| c.is_ascii_digit())) {
        return (
            format!("`{}` is not a valid line number", place.text),
            "a line number is a plain whole number, and a line has at most one".to_owned(),
        );
    }
    if place.text.starts_with(|c: char| c.is_ascii_digit())
        && place.before.chars().all(|c| c.is_ascii_digit() || c == ' ')
        && !place.before.is_empty()
    {
        return (
            "a line can have only one line number".to_owned(),
            "remove the second number, or put the statement on its own line".to_owned(),
        );
    }
    let summary = match &error.expected {
        Some(wanted) => format!("expected {wanted} in the {statement}, but found {found}"),
        None if place.at_end => match place.before.chars().last() {
            Some(c @ ('+' | '-' | '*' | '/' | '\\' | '^' | '=' | '<' | '>' | ',' | ';')) => {
                format!("the {statement} ends after `{c}`, which needs a value after it")
            }
            Some('(') => format!("the {statement} ends after `(`, which needs an argument and a closing `)`"),
            _ => format!("the {statement} is incomplete: the line ends where more is needed"),
        },
        None if first => format!("{found} cannot be used here: it does not start a valid statement at this point"),
        None => format!("{found} does not fit the {statement} here"),
    };
    let help = match forms().get(own.as_str()) {
        Some(form) => format!("{own} is written: {form}"),
        None => {
            "check the statement against the manual for this dialect; the mark shows where BC stopped understanding it"
                .to_owned()
        }
    };
    (summary, help)
}

/// `error` worded for the person who wrote `source`; its BC name stays in
/// `code` and in the message.
pub fn explain(
    mut error: ParseError,
    source: &str,
) -> ParseError {
    if let Some(subject) = error.subject.clone() {
        error.span = narrowed(source, error.span, &subject);
    }
    let place = locate(source, error.span);
    // a variant of the wording for an error at the end of the line (`NEXT`
    // with no variable)
    let entry = variants(&error.code, &place)
        .iter()
        .find_map(|key| table().get(key.as_str()))
        .or_else(|| table().get(error.code.as_str()));
    let other = error
        .related
        .map(|span| {
            let place = locate(source, span);
            if place.name.is_empty() { place.text } else { place.name }
        })
        .unwrap_or_default();
    let (summary, help) = match entry {
        Some((summary, help)) => (fill(summary, &place, &other), fill(help, &place, &other)),
        None if error.code == "Syntax error" => syntax(&error, &place),
        None => (format!("{} (at `{}`)", error.code, place.text), String::new()),
    };
    // the checker's own sentence, where it knew more than the code says; a
    // syntax error then needs no form of the statement as well
    let (summary, help) = match error.headline.take() {
        Some(headline) if table().get(error.code.as_str()).is_none() => {
            (fill(&headline, &place, &other), String::new())
        }
        Some(headline) => (fill(&headline, &place, &other), if error.detail.is_some() { String::new() } else { help }),
        None => (summary, help),
    };
    error.message = format!("{summary} [BC: {}]", error.code);
    let help = match error.detail.take().map(|detail| fill(&detail, &place, &other)) {
        Some(detail) if help.is_empty() => detail,
        Some(detail) => format!("{detail}\n{help}"),
        None => help,
    };
    if !help.is_empty() {
        error.help = Some(help);
    }
    error
}

/// One more line of help.
pub fn add_help(
    error: &mut ParseError,
    more: String,
) {
    error.help = Some(match error.help.take() {
        Some(help) => format!("{help}\n{more}"),
        None => more,
    });
}
