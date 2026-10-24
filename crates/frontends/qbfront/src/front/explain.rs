//! Words for what went wrong, for the person who wrote the source.
//!
//! The parser and the checker know an error by BC's name and where it is. This
//! module turns that into a sentence about the program ("`x%` is used as an
//! array before any DIM ..."), from `messages.txt` and the source text at the
//! error; nothing here knows how an error was found.

use std::collections::HashMap;
use std::sync::OnceLock;

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
}

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
    let statement = if word.is_empty() || word.len() < 2 { String::new() } else { word.to_ascii_uppercase() };
    let before: String = chars[..span.start.min(chars.len())].iter().collect();
    Where {
        text: text.trim().to_owned(),
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

fn fill(
    template: &str,
    place: &Where,
    other: &str,
) -> String {
    let statement = if place.statement.is_empty() { "this".to_owned() } else { place.statement.clone() };
    let name: String = place
        .text
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '$' | '%' | '&' | '!' | '#' | '@'))
        .collect();
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
    let first = place.statement.is_empty() && place.text.chars().all(|c| c.is_ascii_alphabetic()) && !place.text.is_empty();
    let own = if first { place.text.to_ascii_uppercase() } else { place.statement.clone() };
    let statement = if own.is_empty() { "statement".to_owned() } else { format!("{own} statement") };
    let found = if place.at_end { "the end of the line".to_owned() } else { format!("`{}`", place.text) };
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
        None => "check the statement against the manual for this dialect; the mark shows where BC stopped understanding it"
            .to_owned(),
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
    let variant = format!("{} /end", error.code);
    let entry = place.at_end.then(|| table().get(variant.as_str())).flatten().or_else(|| table().get(error.code.as_str()));
    let other = error.related.map(|span| locate(source, span).text).unwrap_or_default();
    let (summary, help) = match entry {
        Some((summary, help)) => (fill(summary, &place, &other), fill(help, &place, &other)),
        None if error.code == "Syntax error" => syntax(&error, &place),
        None => (format!("{} (at `{}`)", error.code, place.text), String::new()),
    };
    // the checker's own sentence, where it knew more than the code says; a
    // syntax error then needs no form of the statement as well
    let (summary, help) = match error.headline.take() {
        Some(headline) if table().get(error.code.as_str()).is_none() => (fill(&headline, &place, &other), String::new()),
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
