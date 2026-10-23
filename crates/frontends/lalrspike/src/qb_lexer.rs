//! A logos-generated QB lexer producing the token stream of `qbfront::generated_parser::lex`.
//!
//! logos dispatches on the first byte and recognises the reserved words with a
//! generated DFA (`qb_words.rs`); the callbacks keep the old lexer's rules for
//! numbers, strings and the context-dependent forms. QuickrBASIC's `f"…"`
//! strings are not handled here: `lex` hands those dialects to the old lexer.

use std::borrow::Cow;
use std::sync::OnceLock;

use logos::{FilterResult, Lexer, Logos};
use qbfront::generated_parser::tables;
use qbfront::generated_parser::{Token, TokenKind};
use qbfront::syntax::{Binary, Span};
use qbfront::{Dialect, LexError};

use crate::qb_kw::Kw;
use crate::qb_tokens::{terminal, Tok};
use crate::qb_words::Word;

pub struct Context {
    dialect: Dialect,
    /// Whether tokens carry their text; the parser needs only their kinds.
    payload: bool,
    line: usize,
    error: Option<LexError>,
}

type Lex<'a> = Lexer<'a, Raw>;

#[derive(Logos, Debug)]
#[logos(extras = Context)]
#[logos(skip r"[ \t]+")]
enum Raw {
    #[token("'", comment)]
    #[regex("[Rr][Ee][Mm][%&!#$]?", rem)]
    #[regex("[A-Za-z][A-Za-z0-9_]*", word)]
    #[regex("[0-9]", digit)]
    #[token(".", period)]
    #[token("&", based)]
    #[token("\"", string)]
    #[token("<=", |_| TokenKind::Comparison(Binary::LessEqual))]
    #[token("<>", |_| TokenKind::Comparison(Binary::NotEqual))]
    #[token(">=", |_| TokenKind::Comparison(Binary::GreaterEqual))]
    #[token("<", |_| punct(b'<'))]
    #[token(">", |_| punct(b'>'))]
    #[token("(", |_| punct(b'('))]
    #[token("[", |_| punct(b'('))]
    #[token(")", |_| punct(b')'))]
    #[token("]", |_| punct(b')'))]
    #[regex(r"[?+\-*/\\^=,#;:]", |lex| punct(lex.slice().as_bytes()[0]))]
    Kind(TokenKind),
}

/// The reserved id of each punctuation spelling, and of each `Word`.
struct Ids {
    punct: [u16; 128],
    words: Vec<u16>,
}

fn ids() -> &'static Ids {
    static IDS: OnceLock<Ids> = OnceLock::new();
    IDS.get_or_init(|| {
        let id = |spelling: &str| {
            tables::TOKENS.iter().find_map(|(_, candidate, id, _)| candidate.eq_ignore_ascii_case(spelling).then_some(*id))
        };
        let mut punct = [0; 128];
        for byte in "<>()?+-*/\\^=,#;:\n".bytes() {
            punct[usize::from(byte)] = id(&char::from(byte).to_string()).expect("punctuation is in qbasbnf.prs");
        }
        let words = crate::qb_words::NAMES
            .iter()
            .map(|name| {
                let spelling = tables::TOKENS.iter().find(|(candidate, ..)| candidate == name).expect("word is a token").1;
                id(spelling).expect("word")
            })
            .collect();
        Ids { punct, words }
    })
}

fn punct(byte: u8) -> TokenKind {
    TokenKind::Reserved(ids().punct[usize::from(byte)])
}

fn reserved(name: &str) -> Option<u16> {
    let mut lexer = Word::lexer(name);
    match lexer.next() {
        Some(Ok(word)) if lexer.span().end == name.len() => Some(ids().words[word as usize]),
        _ => None,
    }
}

fn fail<T>(lex: &mut Lex, end: usize, message: &str) -> Result<T, ()> {
    let line = lex.extras.line;
    lex.extras.error = Some(LexError { span: Span { line, start: lex.span().start, end }, message: message.into() });
    Err(())
}

fn directive(rest: &str) -> Option<TokenKind> {
    match rest.trim().to_ascii_uppercase().as_str() {
        "$DYNAMIC" => Some(TokenKind::ArrayDynamic),
        "$STATIC" => Some(TokenKind::ArrayStatic),
        _ => None,
    }
}

fn comment(lex: &mut Lex) -> FilterResult<TokenKind, ()> {
    let rest = lex.remainder();
    let found = directive(rest);
    lex.bump(rest.len());
    found.map_or(FilterResult::Skip, FilterResult::Emit)
}

fn rem(lex: &mut Lex) -> TokenKind {
    let rest = lex.remainder();
    let found = directive(rest);
    lex.bump(rest.len());
    found.unwrap_or_else(|| punct_word("REM"))
}

fn punct_word(spelling: &str) -> TokenKind {
    TokenKind::Reserved(reserved(spelling).expect("local spelling exists in qbasbnf.prs"))
}

fn is_type_suffix(byte: u8, dialect: Dialect) -> bool {
    matches!(byte, b'%' | b'&' | b'!' | b'#' | b'$') || (byte == b'@' && dialect.currency_suffix())
}

fn identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn word(lex: &mut Lex) -> TokenKind {
    let dialect = lex.extras.dialect;
    if lex.remainder().as_bytes().first().is_some_and(|&byte| is_type_suffix(byte, dialect)) {
        lex.bump(1);
    }
    let name = lex.slice();
    let bare = name.trim_end_matches(['%', '&', '!', '#', '$']);
    reserved(name)
        .or_else(|| reserved(bare))
        .map(TokenKind::Reserved)
        .or_else(|| extension_keyword(bare).map(TokenKind::ExtensionKeyword))
        .unwrap_or_else(|| TokenKind::Identifier(if lex.extras.payload { name.to_ascii_uppercase() } else { String::new() }))
}

/// The extension words are short, so an upper-cased copy fits the stack.
fn extension_keyword(bare: &str) -> Option<&'static str> {
    let mut buffer = [0_u8; 16];
    let upper = buffer.get_mut(..bare.len())?;
    upper.copy_from_slice(bare.as_bytes());
    upper.make_ascii_uppercase();
    tables::extension_keyword(std::str::from_utf8(upper).ok()?)
}

fn string(lex: &mut Lex) -> Result<TokenKind, ()> {
    let escape = lex.extras.dialect.quote_escape();
    let payload = lex.extras.payload;
    let rest = lex.remainder();
    let bytes = rest.as_bytes();
    let mut at = 0;
    let mut content = 0;
    let mut value = String::new();
    loop {
        match bytes[at..].iter().position(|&byte| byte == b'"') {
            None => return fail(lex, lex.span().end + bytes.len(), "unterminated string"),
            Some(offset) => at += offset,
        }
        if escape && bytes.get(at + 1) == Some(&b'"') {
            if payload {
                value.push_str(&rest[content..=at]);
            }
            at += 2;
            content = at;
            continue;
        }
        if payload {
            value.push_str(&rest[content..at]);
        }
        break;
    }
    lex.bump(at + 1);
    Ok(TokenKind::String(value))
}

fn period(lex: &mut Lex) -> Result<TokenKind, ()> {
    let start = lex.span().start;
    let line = lex.source().as_bytes();
    let dialect_free = lex.remainder().as_bytes().first().is_some_and(u8::is_ascii_digit)
        && (start == 0 || !identifier_char(line[start - 1]));
    if dialect_free {
        return number(lex);
    }
    Ok(TokenKind::Period)
}

fn digit(lex: &mut Lex) -> Result<TokenKind, ()> {
    let start = lex.span().start;
    if start > 0 && lex.source().as_bytes()[start - 1] == b'.' {
        let dialect = lex.extras.dialect;
        let rest = lex.remainder().as_bytes();
        let mut at = 0;
        while at < rest.len() && identifier_char(rest[at]) {
            at += 1;
        }
        if at < rest.len() && is_type_suffix(rest[at], dialect) {
            at += 1;
        }
        lex.bump(at);
        return Ok(TokenKind::Identifier(lex.slice().to_ascii_uppercase()));
    }
    number(lex)
}

/// A number from its first byte (a digit, or the `.` before one).
fn number(lex: &mut Lex) -> Result<TokenKind, ()> {
    let dialect = lex.extras.dialect;
    let start = lex.span().start;
    let line = lex.source();
    let bytes = line.as_bytes();
    let mut at = start + 1;
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        at += 1;
    }
    let mut real = bytes[start] == b'.';
    if at < bytes.len() && bytes[at] == b'.' {
        real = true;
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
    }
    let exponent_marker = bytes.get(at).copied().filter(|byte| matches!(byte, b'E' | b'e' | b'D' | b'd'));
    let mantissa_end = at;
    if exponent_marker.is_some() {
        real = true;
        at += 1;
        if at < bytes.len() && matches!(bytes[at], b'+' | b'-') {
            at += 1;
        }
        let exponent = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        if exponent == at {
            lex.bump(at - lex.span().end);
            return fail(lex, at, "missing exponent digits");
        }
    }
    let currency = dialect.currency_suffix();
    let explicit_suffix =
        if at < bytes.len() && (matches!(bytes[at], b'%' | b'&' | b'!' | b'#') || (currency && bytes[at] == b'@')) {
            let suffix = char::from(bytes[at]);
            real |= matches!(bytes[at], b'!' | b'#' | b'@');
            at += 1;
            Some(suffix)
        } else {
            None
        };
    lex.bump(at - lex.span().end);
    let text = &line[start..at];
    if real {
        let suffix = explicit_suffix.or_else(|| {
            if exponent_marker.is_some_and(|byte| matches!(byte, b'D' | b'd'))
                || (exponent_marker.is_none()
                    && bytes[start..mantissa_end].iter().filter(|byte| byte.is_ascii_digit()).count() > 15)
            {
                Some('#')
            } else {
                None
            }
        });
        let text = if lex.extras.payload {
            text.trim_end_matches(['!', '#', '@']).replace(['D', 'd'], "E")
        } else {
            String::new()
        };
        Ok(TokenKind::Real(text, suffix))
    } else {
        let Ok(number) = text.trim_end_matches(['%', '&']).parse::<i64>() else {
            return fail(lex, at, "integer literal is out of range");
        };
        if number > i64::from(i32::MAX) || (explicit_suffix == Some('%') && number > i64::from(i16::MAX)) {
            return fail(lex, at, "integer literal is out of range");
        }
        let suffix = explicit_suffix.or_else(|| (number > i64::from(i16::MAX)).then_some('&'));
        Ok(TokenKind::Integer(number, suffix))
    }
}

fn based(lex: &mut Lex) -> Result<TokenKind, ()> {
    let line = lex.source();
    let bytes = line.as_bytes();
    let mut at = lex.span().end;
    let radix = match bytes.get(at).copied() {
        Some(b'H' | b'h') => {
            at += 1;
            16
        }
        Some(b'O' | b'o') => {
            at += 1;
            8
        }
        // `&377` is the scanner's default-octal spelling.
        _ => 8,
    };
    let digits = at;
    while at < bytes.len() && if radix == 16 { bytes[at].is_ascii_hexdigit() } else { matches!(bytes[at], b'0'..=b'7') } {
        at += 1;
    }
    if digits == at {
        lex.bump(at - lex.span().end);
        return fail(lex, at, "missing based-integer digits");
    }
    let end_digits = at;
    let suffix = if bytes.get(at).is_some_and(|one| matches!(one, b'%' | b'&')) {
        let suffix = char::from(bytes[at]);
        at += 1;
        Some(suffix)
    } else {
        None
    };
    lex.bump(at - lex.span().end);
    let Ok(unsigned) = u64::from_str_radix(&line[digits..end_digits], radix) else {
        return fail(lex, at, "integer literal is out of range");
    };
    let bits = if suffix == Some('&') || (suffix.is_none() && unsigned > 0xffff) { 32 } else { 16 };
    if unsigned >= (1_u64 << bits) {
        return fail(lex, at, "based integer does not fit its type");
    }
    let value =
        if unsigned & (1_u64 << (bits - 1)) != 0 { (unsigned as i64) - (1_i64 << bits) } else { unsigned as i64 };
    Ok(TokenKind::Integer(value, if bits == 32 { Some('&') } else { suffix }))
}

/// The same logical lines as the old lexer's, borrowed where nothing continues.
fn logical_lines(source: &str, dialect: Dialect) -> Vec<(usize, Cow<'_, str>)> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut begins = 1;
    for (index, physical) in source.lines().enumerate() {
        if current.is_empty() {
            begins = index + 1;
        }
        let trimmed = physical.trim_end();
        let before = trimmed.strip_suffix('_').unwrap_or("");
        let word = before.len() - before.trim_end_matches(|c: char| c.is_ascii_alphanumeric()).len();
        let after_number = word > 0 && before[before.len() - word..].starts_with(|c: char| c.is_ascii_digit());
        let continued = trimmed.ends_with('_')
            && (before.ends_with(char::is_whitespace) || (dialect.tight_continuation() && after_number));
        if continued {
            current.push_str(trimmed[..trimmed.len() - 1].trim_end());
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

pub fn lex(source: &str, dialect: Dialect) -> Result<Vec<Token>, LexError> {
    if dialect.format_strings() {
        return qbfront::generated_parser::lex(source, dialect);
    }
    let newline = TokenKind::Reserved(ids().punct[usize::from(b'\n')]);
    let mut out = Vec::with_capacity(source.len() / 4);
    for (line_index, line) in logical_lines(source, dialect) {
        let mut lexer = Raw::lexer_with_extras(&line, Context { dialect, payload: true, line: line_index, error: None });
        while let Some(item) = lexer.next() {
            let span = lexer.span();
            match item {
                Ok(Raw::Kind(kind)) => {
                    out.push(Token { kind, span: Span { line: line_index, start: span.start, end: span.end } })
                }
                Err(()) => {
                    if let Some(error) = lexer.extras.error.take() {
                        return Err(error);
                    }
                    let byte = line.as_bytes()[span.start];
                    let message = if byte == b'_' {
                        "identifier cannot begin with underscore".to_owned()
                    } else {
                        format!("unknown character {:?}", char::from(byte))
                    };
                    let span = Span { line: line_index, start: span.start, end: span.start + 1 };
                    return Err(LexError { span, message });
                }
            }
        }
        out.push(Token { kind: newline.clone(), span: Span { line: line_index, start: line.len(), end: line.len() } });
    }
    if source.is_empty() {
        out.push(Token { kind: newline, span: Span { line: 1, start: 0, end: 0 } });
    }
    Ok(out)
}

/// A source's logical lines, to stream tokens from.
pub struct Lines<'a> {
    lines: Vec<(usize, Cow<'a, str>)>,
    dialect: Dialect,
}

impl<'a> Lines<'a> {
    /// `None` where this lexer does not read the dialect (QuickrBASIC's `f"…"`).
    pub fn new(source: &'a str, dialect: Dialect) -> Option<Self> {
        (!dialect.format_strings()).then(|| Self { lines: logical_lines(source, dialect), dialect })
    }

    /// The tokens as the parser pulls them, kinds only, with each one's span pushed to `spans`.
    pub fn stream<'s>(&'s self, spans: &'s mut Vec<Span>) -> Stream<'s> {
        self.stream_with(Some(spans))
    }

    fn stream_with<'s>(&'s self, spans: Option<&'s mut Vec<Span>>) -> Stream<'s> {
        Stream {
            lines: &self.lines,
            dialect: self.dialect,
            next_line: 0,
            lexer: None,
            line_text: "",
            line_index: 0,
            ended: false,
            pending: None,
            spans,
            emitted: 0,
        }
    }

    /// The same tokens with no span kept.
    pub fn stream_bare<'s>(&'s self) -> Stream<'s> {
        let mut stream = self.stream_with(None);
        stream.spans = None;
        stream
    }
}

struct Piece {
    tok: Tok,
    fn_word: bool,
    span: Span,
}

pub struct Stream<'s> {
    lines: &'s [(usize, Cow<'s, str>)],
    dialect: Dialect,
    next_line: usize,
    lexer: Option<Lexer<'s, Raw>>,
    line_text: &'s str,
    line_index: usize,
    ended: bool,
    pending: Option<Piece>,
    spans: Option<&'s mut Vec<Span>>,
    emitted: usize,
}

impl<'s> Stream<'s> {
    fn piece(&mut self) -> Option<Result<Piece, LexError>> {
        loop {
            let Some(lexer) = self.lexer.as_mut() else {
                let lines: &'s [(usize, Cow<'s, str>)] = self.lines;
                let Some((index, text)) = lines.get(self.next_line) else {
                    if self.ended {
                        return None;
                    }
                    self.ended = true;
                    if self.lines.is_empty() {
                        let span = Span { line: 1, start: 0, end: 0 };
                        return Some(Ok(Piece { tok: Tok::Kw(Kw::NewLine), fn_word: false, span }));
                    }
                    return None;
                };
                self.next_line += 1;
                self.line_index = *index;
                self.line_text = text;
                self.lexer = Some(Raw::lexer_with_extras(
                    text,
                    Context { dialect: self.dialect, payload: false, line: *index, error: None },
                ));
                continue;
            };
            match lexer.next() {
                Some(Ok(Raw::Kind(kind))) => {
                    let range = lexer.span();
                    let span = Span { line: self.line_index, start: range.start, end: range.end };
                    let fn_word = matches!(kind, TokenKind::Identifier(_))
                        && self.line_text[range.clone()].eq_ignore_ascii_case("FN");
                    let tok = match &kind {
                        TokenKind::Reserved(id) if crate::qb_tokens::suffixed(*id, range.len()) => Tok::Ident,
                        kind => terminal(kind),
                    };
                    return Some(Ok(Piece { tok, fn_word, span }));
                }
                Some(Err(())) => {
                    let range = lexer.span();
                    let error = lexer.extras.error.take().unwrap_or_else(|| {
                        let byte = self.line_text.as_bytes()[range.start];
                        let message = if byte == b'_' {
                            "identifier cannot begin with underscore".to_owned()
                        } else {
                            format!("unknown character {:?}", char::from(byte))
                        };
                        LexError { span: Span { line: self.line_index, start: range.start, end: range.start + 1 }, message }
                    });
                    self.next_line = self.lines.len();
                    self.ended = true;
                    self.lexer = None;
                    return Some(Err(error));
                }
                None => {
                    self.lexer = None;
                    let end = self.line_text.len();
                    let span = Span { line: self.line_index, start: end, end };
                    return Some(Ok(Piece { tok: Tok::Kw(Kw::NewLine), fn_word: false, span }));
                }
            }
        }
    }
}

impl<'s> Iterator for Stream<'s> {
    type Item = crate::qb_tokens::Spanned;

    fn next(&mut self) -> Option<Self::Item> {
        let mut piece = match self.pending.take() {
            Some(piece) => piece,
            None => match self.piece()? {
                Ok(piece) => piece,
                Err(error) => return Some(Err(error)),
            },
        };
        if piece.fn_word {
            // `FN twice`: apart from the name it prefixes
            match self.piece() {
                Some(Ok(next)) => {
                    if next.tok == Tok::Ident {
                        piece.tok = Tok::FnWord;
                    }
                    self.pending = Some(next);
                }
                Some(Err(error)) => return Some(Err(error)),
                None => {}
            }
        }
        let at = self.emitted;
        self.emitted += 1;
        if let Some(spans) = self.spans.as_deref_mut() {
            spans.push(piece.span);
        }
        Some(Ok((at, piece.tok, at + 1)))
    }
}

/// Every token of a source, kinds and spans, before any parsing: for a parser that wants the vector.
pub fn prelex(source: &str, dialect: Dialect) -> Option<Result<(Vec<Tok>, Vec<Span>), LexError>> {
    let lines = Lines::new(source, dialect)?;
    let mut spans = Vec::with_capacity(source.len() / 4);
    let mut toks = Vec::with_capacity(source.len() / 4);
    for item in lines.stream(&mut spans) {
        match item {
            Ok((_, tok, _)) => toks.push(tok),
            Err(error) => return Some(Err(error)),
        }
    }
    Some(Ok((toks, spans)))
}
