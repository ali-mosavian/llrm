//! Parser for QBasic `qbasbnf.prs` grammar files consumed by `buildprs.exe`.
//!
//! This module performs structural parsing: section splitting, token declarations,
//! and typed grammar bodies for statements, functions, and nonterminals.

use std::fs;
use std::path::Path;

/// Top-level sections preserved from a `.prs` grammar file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarFile {
    pub tokens: Vec<TokenDef>,
    pub statements: RawSection,
    pub functions: RawSection,
    pub nonterminals: Vec<NonTerminalDef>,
}

/// A terminal token declaration from the `TOKENS:` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDef {
    pub name: String,
    pub spelling: String,
    pub attributes: Vec<String>,
}

/// A parsed grammar section with preserved raw text and typed rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSection {
    pub text: String,
    pub rules: Vec<GrammarRule>,
}

/// One statement or function production anchored by its leading `tk*` token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarRule {
    pub anchor: String,
    pub production: GrammarProduction,
}

/// One semicolon-terminated production, optionally followed by a `<Cg...>` hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarProduction {
    pub expr: GrammarExpr,
    pub cg_hint: Option<String>,
}

/// Parsed body of a `NonTerminals:` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarBody {
    pub raw: String,
    pub productions: Vec<GrammarProduction>,
    pub has_index: bool,
}

/// A grammar expression node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarExpr {
    Empty,
    Sequence(Vec<GrammarExpr>),
    Alternative(Vec<GrammarExpr>),
    Group(Box<GrammarExpr>),
    Optional(Box<GrammarExpr>),
    Repeat(Box<GrammarExpr>),
    TokenRef(String),
    NonTerminalRef(String),
    Emit(EmitDirective),
    Mark(MarkDirective),
    External { msg_hint: Option<String> },
}

/// `EMIT(...)` directive with one or more opcode / operand arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitDirective {
    pub args: Vec<EmitArg>,
}

/// One argument inside `EMIT(...)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitArg {
    Ident(String),
    Number(u32),
}

/// `MARK(n)` stack-slot directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkDirective {
    pub slot: u8,
}

/// A nonterminal from the `NonTerminals:` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonTerminalDef {
    pub name: String,
    pub body: GrammarBody,
    pub external: bool,
    pub msg_hint: Option<String>,
    pub has_index: bool,
}

/// Errors raised while parsing a `.prs` grammar file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarParseError {
    MissingSection(&'static str),
    DuplicateSection(&'static str),
    UnexpectedContent {
        section: &'static str,
        detail: String,
    },
    BodyParse {
        context: String,
        detail: String,
    },
    Io(String),
}

impl std::fmt::Display for GrammarParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSection(name) => write!(f, "missing required section `{name}:`"),
            Self::DuplicateSection(name) => write!(f, "duplicate section `{name}:`"),
            Self::UnexpectedContent { section, detail } => {
                write!(f, "unexpected content in `{section}:` section: {detail}")
            }
            Self::BodyParse { context, detail } => {
                write!(f, "grammar body parse error in `{context}`: {detail}")
            }
            Self::Io(message) => write!(f, "I/O error: {message}"),
        }
    }
}

impl std::error::Error for GrammarParseError {}

const SECTION_TOKENS: &str = "TOKENS";
const SECTION_STATEMENTS: &str = "Statements";
const SECTION_FUNCTIONS: &str = "Functions";
const SECTION_NONTERMINALS: &str = "NonTerminals";

const SECTION_ORDER: [(&str, &str); 4] = [
    (SECTION_TOKENS, "TOKENS"),
    (SECTION_STATEMENTS, "Statements"),
    (SECTION_FUNCTIONS, "Functions"),
    (SECTION_NONTERMINALS, "NonTerminals"),
];

/// Parse grammar source text into a [`GrammarFile`].
pub fn parse_grammar(source: &str) -> Result<GrammarFile, GrammarParseError> {
    let stripped = strip_comments(source);
    let sections = split_sections(&stripped)?;

    let tokens = parse_tokens_section(
        sections
            .get(SECTION_TOKENS)
            .ok_or(GrammarParseError::MissingSection(SECTION_TOKENS))?,
    )?;

    let statements = parse_rule_section(
        sections
            .get(SECTION_STATEMENTS)
            .ok_or(GrammarParseError::MissingSection(SECTION_STATEMENTS))?,
        SECTION_STATEMENTS,
    )?;

    let functions = parse_rule_section(
        sections
            .get(SECTION_FUNCTIONS)
            .ok_or(GrammarParseError::MissingSection(SECTION_FUNCTIONS))?,
        SECTION_FUNCTIONS,
    )?;

    let nonterminals = parse_nonterminals_section(
        sections
            .get(SECTION_NONTERMINALS)
            .ok_or(GrammarParseError::MissingSection(SECTION_NONTERMINALS))?,
    )?;

    Ok(GrammarFile {
        tokens,
        statements,
        functions,
        nonterminals,
    })
}

/// Read and parse a `.prs` grammar file from disk.
pub fn parse_grammar_file(path: impl AsRef<Path>) -> Result<GrammarFile, GrammarParseError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| GrammarParseError::Io(err.to_string()))?;
    parse_grammar(&source)
}

/// Parse only the `TOKENS:` section, without validating statement/function bodies.
///
/// Token-artifact generation uses this entry point so it stays independent of
/// the grammar-body AST packet.
pub fn parse_token_decls(source: &str) -> Result<Vec<TokenDef>, GrammarParseError> {
    let stripped = strip_comments(source);
    let sections = split_sections_present(&stripped)?;
    parse_tokens_section(
        sections
            .get(SECTION_TOKENS)
            .ok_or(GrammarParseError::MissingSection(SECTION_TOKENS))?,
    )
}

/// Read and parse token declarations from a `.prs` grammar file.
pub fn parse_token_decls_file(path: impl AsRef<Path>) -> Result<Vec<TokenDef>, GrammarParseError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| GrammarParseError::Io(err.to_string()))?;
    parse_token_decls(&source)
}

fn split_sections(
    source: &str,
) -> Result<std::collections::BTreeMap<&'static str, String>, GrammarParseError> {
    let mut sections = std::collections::BTreeMap::new();
    let mut current: Option<&'static str> = None;
    let mut body = String::new();

    for line in source.lines() {
        if let Some(name) = section_header_name(line) {
            if let Some(section) = current {
                sections.insert(section, body.trim_end().to_string());
                body.clear();
            }
            if sections.contains_key(name) {
                return Err(GrammarParseError::DuplicateSection(name));
            }
            current = Some(name);
            continue;
        }

        if current.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }

    if let Some(section) = current {
        sections.insert(section, body.trim_end().to_string());
    }

    for (required, _) in SECTION_ORDER {
        if !sections.contains_key(required) {
            return Err(GrammarParseError::MissingSection(required));
        }
    }

    Ok(sections)
}

fn split_sections_present(
    source: &str,
) -> Result<std::collections::BTreeMap<&'static str, String>, GrammarParseError> {
    let mut sections = std::collections::BTreeMap::new();
    let mut current: Option<&'static str> = None;
    let mut body = String::new();

    for line in source.lines() {
        if let Some(name) = section_header_name(line) {
            if let Some(section) = current {
                sections.insert(section, body.trim_end().to_string());
                body.clear();
            }
            if sections.contains_key(name) {
                return Err(GrammarParseError::DuplicateSection(name));
            }
            current = Some(name);
            continue;
        }

        if current.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }

    if let Some(section) = current {
        sections.insert(section, body.trim_end().to_string());
    }

    Ok(sections)
}

fn section_header_name(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    for (name, header) in SECTION_ORDER {
        if trimmed == header || trimmed == format!("{header}:") {
            return Some(name);
        }
    }
    None
}

fn parse_tokens_section(section: &str) -> Result<Vec<TokenDef>, GrammarParseError> {
    let mut tokens = Vec::new();

    for (line_number, line) in section.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_token_line(trimmed) {
            Some(token) => tokens.push(token),
            None => {
                return Err(GrammarParseError::UnexpectedContent {
                    section: SECTION_TOKENS,
                    detail: format!("line {}: `{trimmed}`", line_number + 1),
                });
            }
        }
    }

    Ok(tokens)
}

fn parse_token_line(line: &str) -> Option<TokenDef> {
    let line = line.trim_end_matches([',', ';']);
    let open_paren = line.find('(')?;
    let name = line[..open_paren].trim();
    if !name.starts_with("tk") {
        return None;
    }

    let (spelling, attributes) = parse_spelling_and_attributes(&line[open_paren..])?;
    Some(TokenDef {
        name: name.to_string(),
        spelling,
        attributes,
    })
}

fn parse_spelling_and_attributes(input: &str) -> Option<(String, Vec<String>)> {
    let input = input.trim();
    if !input.starts_with('(') {
        return None;
    }

    let mut chars = input.chars().peekable();
    chars.next(); // '('

    while chars.peek().is_some_and(|ch| ch.is_whitespace()) {
        chars.next();
    }

    if chars.next()? != '"' {
        return None;
    }

    let mut raw = String::new();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            break;
        }
        if ch == '\\' {
            let esc = chars.next()?;
            raw.push('\\');
            raw.push(esc);
            continue;
        }
        raw.push(ch);
    }

    let spelling = unescape_prs_string(&raw);
    let mut attributes = Vec::new();

    loop {
        while chars.peek().is_some_and(|ch| ch.is_whitespace()) {
            chars.next();
        }

        match chars.peek().copied() {
            Some(',') => {
                chars.next();
                while chars.peek().is_some_and(|ch| ch.is_whitespace()) {
                    chars.next();
                }
                let attr = parse_identifier(&mut chars)?;
                attributes.push(attr);
            }
            Some(')') => {
                chars.next();
                break;
            }
            _ => return None,
        }
    }

    Some((spelling, attributes))
}

fn parse_identifier(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    let mut ident = String::new();
    let first = chars.next()?;
    if !is_ident_start(first) {
        return None;
    }
    ident.push(first);

    while let Some(ch) = chars.peek().copied() {
        if !is_ident_continue(ch) {
            break;
        }
        ident.push(ch);
        chars.next();
    }

    Some(ident)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn unescape_prs_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let esc = chars.next().unwrap_or('\\');
        match esc {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '0'..='7' => {
                let mut octal = String::new();
                octal.push(esc);
                for _ in 0..2 {
                    if chars
                        .peek()
                        .is_some_and(|next| next.is_ascii_digit() && *next <= '7')
                    {
                        octal.push(chars.next().unwrap());
                    }
                }
                if let Ok(value) = u32::from_str_radix(&octal, 8) {
                    if let Some(decoded) = char::from_u32(value) {
                        out.push(decoded);
                    }
                }
            }
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }

    out
}

fn parse_rule_section(
    section: &str,
    section_name: &'static str,
) -> Result<RawSection, GrammarParseError> {
    let text = section.trim().to_string();
    let mut rules = Vec::new();

    for block in split_anchor_rule_blocks(&text) {
        let (anchor, body) = block;
        let production = parse_production_block(&body, &format!("{section_name}::{anchor}"))?;
        rules.push(GrammarRule { anchor, production });
    }

    Ok(RawSection { text, rules })
}

fn split_anchor_rule_blocks(section: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut start = 0;

    while let Some(end) = find_top_level_semicolon(&section[start..]) {
        let mut block_end = start + end + 1;
        let mut after = block_end;
        while let Some(ch) = section[after..].chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            after += ch.len_utf8();
        }
        if section[after..].starts_with('<') {
            if let Some(close) = section[after..].find('>') {
                block_end = after + close + 1;
            }
        }

        let block = section[start..block_end].trim();
        if let Some(anchor) = anchor_token(block) {
            let body = block[anchor.len()..].trim_start().to_string();
            blocks.push((anchor.to_string(), body));
        }
        start = block_end;
    }

    blocks
}

fn find_top_level_semicolon(input: &str) -> Option<usize> {
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut brace = 0i32;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => paren += 1,
            ')' => paren -= 1,
            '[' => bracket += 1,
            ']' => bracket -= 1,
            '{' => brace += 1,
            '}' => brace -= 1,
            ';' if paren == 0 && bracket == 0 && brace == 0 => return Some(idx),
            _ => {}
        }
    }

    None
}

fn anchor_token(line: &str) -> Option<&str> {
    let token = line.split_whitespace().next()?;
    if token.starts_with("tk") && token.chars().all(is_ident_continue) {
        Some(token)
    } else {
        None
    }
}

fn parse_nonterminals_section(section: &str) -> Result<Vec<NonTerminalDef>, GrammarParseError> {
    let mut nonterminals = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_body = String::new();

    for line in section.lines() {
        if let Some(name) = nonterminal_name_line(line) {
            if let Some(prev_name) = current_name.take() {
                nonterminals.push(finalize_nonterminal(prev_name, &current_body)?);
                current_body.clear();
            }
            current_name = Some(name);
            continue;
        }

        if current_name.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    if let Some(name) = current_name {
        nonterminals.push(finalize_nonterminal(name, &current_body)?);
    }

    Ok(nonterminals)
}

fn nonterminal_name_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let colon = trimmed.find(':')?;
    if colon == 0 {
        return None;
    }

    let name = &trimmed[..colon];
    if !name
        .chars()
        .all(|ch| is_ident_start(ch) || (ch.is_ascii_alphanumeric() && ch != ':'))
    {
        return None;
    }
    if !name.chars().next().is_some_and(is_ident_start) {
        return None;
    }
    if !trimmed[colon + 1..].trim().is_empty() {
        return None;
    }

    Some(name.to_string())
}

fn finalize_nonterminal(name: String, body: &str) -> Result<NonTerminalDef, GrammarParseError> {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return Err(GrammarParseError::UnexpectedContent {
            section: SECTION_NONTERMINALS,
            detail: format!("nonterminal `{name}` has an empty body"),
        });
    }

    let context = format!("NonTerminals::{name}");
    let parsed = parse_nonterminal_body(trimmed_body, &context)?;

    let external = is_external_expr(&parsed.productions);
    let msg_hint = external_msg_hint(&parsed.productions);
    let has_index = parsed.has_index;

    Ok(NonTerminalDef {
        name,
        body: parsed,
        external,
        msg_hint,
        has_index,
    })
}

fn parse_nonterminal_body(raw: &str, context: &str) -> Result<GrammarBody, GrammarParseError> {
    let mut productions = Vec::new();
    let mut has_index = false;
    let mut remainder = raw.trim();

    while !remainder.is_empty() {
        if let Some(rest) = remainder.strip_prefix("<INDEX>") {
            has_index = true;
            remainder = rest.trim();
            continue;
        }

        if remainder.starts_with('<') {
            return Err(GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: format!("unexpected metadata `{remainder}` before production"),
            });
        }

        let (production, rest) = parse_production_with_trailing_metadata(remainder, context)?;
        productions.push(production);
        remainder = rest.trim();
    }

    if productions.is_empty() {
        return Err(GrammarParseError::BodyParse {
            context: context.to_string(),
            detail: "no productions found".to_string(),
        });
    }

    Ok(GrammarBody {
        raw: raw.to_string(),
        productions,
        has_index,
    })
}

fn parse_production_block(
    body: &str,
    context: &str,
) -> Result<GrammarProduction, GrammarParseError> {
    let (production, remainder) = parse_production_with_trailing_metadata(body.trim(), context)?;
    if !remainder.trim().is_empty() {
        return Err(GrammarParseError::BodyParse {
            context: context.to_string(),
            detail: format!("unexpected trailing content `{remainder}`"),
        });
    }
    Ok(production)
}

fn parse_production_with_trailing_metadata<'a>(
    input: &'a str,
    context: &str,
) -> Result<(GrammarProduction, &'a str), GrammarParseError> {
    let mut parser = BodyParser::new(input);
    let expr = parser.parse_production_expr(context)?;
    parser.skip_whitespace_and_comments();
    parser.expect_char(';', context)?;
    parser.skip_whitespace_and_comments();

    let mut cg_hint = None;
    if parser.peek_angle_metadata().is_some() {
        let meta = parser.peek_angle_metadata().expect("metadata peeked");
        if meta == "INDEX" {
            return Ok((
                GrammarProduction {
                    expr,
                    cg_hint: None,
                },
                parser.rest(),
            ));
        }
        let meta = parser.consume_angle_metadata(context)?;
        cg_hint = Some(meta);
        parser.skip_whitespace_and_comments();
    }

    Ok((GrammarProduction { expr, cg_hint }, parser.rest()))
}

fn is_external_expr(productions: &[GrammarProduction]) -> bool {
    productions
        .first()
        .is_some_and(|production| matches!(production.expr, GrammarExpr::External { msg_hint: _ }))
}

fn external_msg_hint(productions: &[GrammarProduction]) -> Option<String> {
    match productions.first()?.expr {
        GrammarExpr::External { msg_hint: ref hint } => hint.clone(),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyTokenKind {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Pipe,
    Semicolon,
    Comma,
    TokenRef,
    Ident,
    Number,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BodyToken {
    kind: BodyTokenKind,
    text: String,
}

struct BodyLexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> BodyLexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.input[self.pos..].chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.peek_char().is_some_and(|ch| ch.is_whitespace()) {
                self.bump();
            }

            if self.rest().starts_with("/*") {
                self.bump();
                self.bump();
                while let Some(ch) = self.bump() {
                    if ch == '*' && self.peek_char() == Some('/') {
                        self.bump();
                        break;
                    }
                }
                continue;
            }

            if self.rest().starts_with("//") {
                while self.bump().is_some_and(|ch| ch != '\n') {}
                continue;
            }

            break;
        }
    }

    fn next_token(&mut self) -> Option<BodyToken> {
        self.skip_whitespace_and_comments();
        let ch = self.peek_char()?;

        let (kind, text) = match ch {
            '(' => {
                self.bump();
                (BodyTokenKind::LParen, "(".to_string())
            }
            ')' => {
                self.bump();
                (BodyTokenKind::RParen, ")".to_string())
            }
            '[' => {
                self.bump();
                (BodyTokenKind::LBracket, "[".to_string())
            }
            ']' => {
                self.bump();
                (BodyTokenKind::RBracket, "]".to_string())
            }
            '{' => {
                self.bump();
                (BodyTokenKind::LBrace, "{".to_string())
            }
            '}' => {
                self.bump();
                (BodyTokenKind::RBrace, "}".to_string())
            }
            '|' => {
                self.bump();
                (BodyTokenKind::Pipe, "|".to_string())
            }
            ';' => {
                self.bump();
                (BodyTokenKind::Semicolon, ";".to_string())
            }
            ',' => {
                self.bump();
                (BodyTokenKind::Comma, ",".to_string())
            }
            _ if ch.is_ascii_digit() => {
                let start = self.pos;
                while self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
                    self.bump();
                }
                let text = self.input[start..self.pos].to_string();
                (BodyTokenKind::Number, text)
            }
            _ if is_ident_start(ch) => {
                let start = self.pos;
                while self.peek_char().is_some_and(|c| is_ident_continue(c)) {
                    self.bump();
                }
                let text = self.input[start..self.pos].to_string();
                let kind = if text.starts_with("tk") {
                    BodyTokenKind::TokenRef
                } else {
                    BodyTokenKind::Ident
                };
                (kind, text)
            }
            _ => return None,
        };

        Some(BodyToken { kind, text })
    }
}

struct BodyParser<'a> {
    lexer: BodyLexer<'a>,
    peeked: Option<BodyToken>,
}

impl<'a> BodyParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lexer: BodyLexer::new(input),
            peeked: None,
        }
    }

    fn rest(&self) -> &'a str {
        self.lexer.rest()
    }

    fn skip_whitespace_and_comments(&mut self) {
        if self.peeked.is_none() {
            self.lexer.skip_whitespace_and_comments();
        }
    }

    fn peek(&mut self) -> Option<&BodyToken> {
        if self.peeked.is_none() {
            self.peeked = self.lexer.next_token();
        }
        self.peeked.as_ref()
    }

    fn bump(&mut self) -> Option<BodyToken> {
        if let Some(token) = self.peeked.take() {
            return Some(token);
        }
        self.lexer.next_token()
    }

    fn expect_char(&mut self, expected: char, context: &str) -> Result<(), GrammarParseError> {
        self.skip_whitespace_and_comments();
        match self.bump() {
            Some(token) if token.kind == BodyTokenKind::Semicolon && expected == ';' => Ok(()),
            Some(token) if token.text.chars().next() == Some(expected) => Ok(()),
            Some(token) => Err(GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: format!("expected `{expected}`, found `{}`", token.text),
            }),
            None => Err(GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: format!("expected `{expected}`, found end of input"),
            }),
        }
    }

    fn peek_angle_metadata(&mut self) -> Option<String> {
        self.skip_whitespace_and_comments();
        if self.lexer.rest().starts_with('<') {
            let rest = self.lexer.rest();
            let end = rest.find('>')?;
            Some(rest[1..end].trim().to_string())
        } else {
            None
        }
    }

    fn consume_angle_metadata(&mut self, context: &str) -> Result<String, GrammarParseError> {
        self.skip_whitespace_and_comments();
        if self.lexer.peek_char() != Some('<') {
            return Err(GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: "expected `<...>` metadata".to_string(),
            });
        }
        self.lexer.bump();
        let mut meta = String::new();
        while let Some(ch) = self.lexer.bump() {
            if ch == '>' {
                return Ok(meta.trim().to_string());
            }
            meta.push(ch);
        }
        Err(GrammarParseError::BodyParse {
            context: context.to_string(),
            detail: "unterminated `<...>` metadata".to_string(),
        })
    }

    fn at_end_of_expr(&mut self) -> bool {
        self.skip_whitespace_and_comments();
        matches!(
            self.peek(),
            None | Some(BodyToken {
                kind: BodyTokenKind::Pipe
                    | BodyTokenKind::Semicolon
                    | BodyTokenKind::RParen
                    | BodyTokenKind::RBracket
                    | BodyTokenKind::RBrace,
                ..
            })
        )
    }

    fn parse_production_expr(&mut self, context: &str) -> Result<GrammarExpr, GrammarParseError> {
        let mut alts = vec![self.parse_sequence(context)?];
        while self
            .peek()
            .is_some_and(|token| token.kind == BodyTokenKind::Pipe)
        {
            self.bump();
            alts.push(self.parse_sequence(context)?);
        }
        Ok(fold_alternatives(alts))
    }

    fn parse_sequence(&mut self, context: &str) -> Result<GrammarExpr, GrammarParseError> {
        if self.at_end_of_expr() {
            return Ok(GrammarExpr::Empty);
        }

        let mut items = Vec::new();
        while !self.at_end_of_expr() {
            items.push(self.parse_term(context)?);
        }
        Ok(fold_sequence(items))
    }

    fn parse_term(&mut self, context: &str) -> Result<GrammarExpr, GrammarParseError> {
        let token = self.bump().ok_or_else(|| GrammarParseError::BodyParse {
            context: context.to_string(),
            detail: "unexpected end of input".to_string(),
        })?;

        match token.kind {
            BodyTokenKind::LParen => {
                let expr = self.parse_production_expr(context)?;
                self.expect_delimiter(')', BodyTokenKind::RParen, context)?;
                Ok(GrammarExpr::Group(Box::new(expr)))
            }
            BodyTokenKind::LBracket => {
                let expr = self.parse_production_expr(context)?;
                self.expect_delimiter(']', BodyTokenKind::RBracket, context)?;
                Ok(GrammarExpr::Optional(Box::new(expr)))
            }
            BodyTokenKind::LBrace => {
                let expr = self.parse_production_expr(context)?;
                self.expect_delimiter('}', BodyTokenKind::RBrace, context)?;
                Ok(GrammarExpr::Repeat(Box::new(expr)))
            }
            BodyTokenKind::TokenRef => Ok(GrammarExpr::TokenRef(token.text)),
            BodyTokenKind::Ident => match token.text.as_str() {
                "EMIT" => Ok(GrammarExpr::Emit(self.parse_emit(context)?)),
                "MARK" => Ok(GrammarExpr::Mark(self.parse_mark(context)?)),
                "EXTERNAL" => {
                    let msg_hint = self.parse_external_msg_hint(context)?;
                    Ok(GrammarExpr::External { msg_hint })
                }
                _ if token.text.starts_with("MSG_") => Err(GrammarParseError::BodyParse {
                    context: context.to_string(),
                    detail: format!("unexpected `{0}` outside EXTERNAL", token.text),
                }),
                _ => Ok(GrammarExpr::NonTerminalRef(token.text)),
            },
            other => Err(GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: format!("unexpected token `{other:?}`"),
            }),
        }
    }

    fn expect_delimiter(
        &mut self,
        ch: char,
        kind: BodyTokenKind,
        context: &str,
    ) -> Result<(), GrammarParseError> {
        self.skip_whitespace_and_comments();
        match self.bump() {
            Some(token) if token.kind == kind => Ok(()),
            Some(token) => Err(GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: format!("expected `{ch}`, found `{}`", token.text),
            }),
            None => Err(GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: format!("expected `{ch}`, found end of input"),
            }),
        }
    }

    fn parse_external_msg_hint(
        &mut self,
        _context: &str,
    ) -> Result<Option<String>, GrammarParseError> {
        self.skip_whitespace_and_comments();
        match self.peek() {
            Some(BodyToken {
                kind: BodyTokenKind::Ident,
                text,
            }) if text.starts_with("MSG_") => {
                let hint = self.bump().expect("peeked ident").text;
                Ok(Some(hint))
            }
            _ => Ok(None),
        }
    }

    fn parse_emit(&mut self, context: &str) -> Result<EmitDirective, GrammarParseError> {
        self.expect_delimiter('(', BodyTokenKind::LParen, context)?;
        let mut args = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            let token = self.bump().ok_or_else(|| GrammarParseError::BodyParse {
                context: context.to_string(),
                detail: "EMIT(...) missing argument".to_string(),
            })?;

            match token.kind {
                BodyTokenKind::Ident => args.push(EmitArg::Ident(token.text)),
                BodyTokenKind::Number => {
                    let value =
                        token
                            .text
                            .parse::<u32>()
                            .map_err(|_| GrammarParseError::BodyParse {
                                context: context.to_string(),
                                detail: format!("invalid EMIT numeric argument `{}`", token.text),
                            })?;
                    args.push(EmitArg::Number(value));
                }
                BodyTokenKind::RParen => {
                    if args.is_empty() {
                        return Err(GrammarParseError::BodyParse {
                            context: context.to_string(),
                            detail: "EMIT(...) requires at least one argument".to_string(),
                        });
                    }
                    break;
                }
                _ => {
                    return Err(GrammarParseError::BodyParse {
                        context: context.to_string(),
                        detail: format!("invalid EMIT argument `{}`", token.text),
                    });
                }
            }

            self.skip_whitespace_and_comments();
            match self.peek() {
                Some(BodyToken {
                    kind: BodyTokenKind::Comma,
                    ..
                }) => {
                    self.bump();
                }
                Some(BodyToken {
                    kind: BodyTokenKind::RParen,
                    ..
                }) => {
                    self.bump();
                    break;
                }
                Some(token) => {
                    return Err(GrammarParseError::BodyParse {
                        context: context.to_string(),
                        detail: format!("expected `,` or `)` in EMIT(...), found `{}`", token.text),
                    });
                }
                None => {
                    return Err(GrammarParseError::BodyParse {
                        context: context.to_string(),
                        detail: "unterminated EMIT(...)".to_string(),
                    });
                }
            }
        }

        Ok(EmitDirective { args })
    }

    fn parse_mark(&mut self, context: &str) -> Result<MarkDirective, GrammarParseError> {
        self.expect_delimiter('(', BodyTokenKind::LParen, context)?;
        self.skip_whitespace_and_comments();
        let token = self.bump().ok_or_else(|| GrammarParseError::BodyParse {
            context: context.to_string(),
            detail: "MARK(...) missing slot".to_string(),
        })?;

        let slot = match token.kind {
            BodyTokenKind::Number => {
                token
                    .text
                    .parse::<u8>()
                    .map_err(|_| GrammarParseError::BodyParse {
                        context: context.to_string(),
                        detail: format!("invalid MARK slot `{}`", token.text),
                    })?
            }
            _ => {
                return Err(GrammarParseError::BodyParse {
                    context: context.to_string(),
                    detail: format!("MARK(...) expects numeric slot, found `{}`", token.text),
                });
            }
        };

        self.expect_delimiter(')', BodyTokenKind::RParen, context)?;
        Ok(MarkDirective { slot })
    }
}

fn fold_sequence(items: Vec<GrammarExpr>) -> GrammarExpr {
    if items.is_empty() {
        GrammarExpr::Empty
    } else if items.len() == 1 {
        items.into_iter().next().expect("one item")
    } else {
        GrammarExpr::Sequence(items)
    }
}

fn fold_alternatives(items: Vec<GrammarExpr>) -> GrammarExpr {
    if items.len() == 1 {
        items.into_iter().next().expect("one item")
    } else {
        GrammarExpr::Alternative(items)
    }
}

#[cfg(test)]
fn is_alternative_expr(expr: &GrammarExpr) -> bool {
    matches!(expr, GrammarExpr::Alternative(_))
        || matches!(expr, GrammarExpr::Group(inner) if matches!(inner.as_ref(), GrammarExpr::Alternative(_)))
}

#[cfg(test)]
fn alternative_branches(expr: &GrammarExpr) -> &[GrammarExpr] {
    match expr {
        GrammarExpr::Alternative(alts) => alts,
        GrammarExpr::Group(inner) => match inner.as_ref() {
            GrammarExpr::Alternative(alts) => alts,
            _ => panic!("expected grouped alternative"),
        },
        _ => panic!("expected alternative expression"),
    }
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(next) = chars.next() {
                if next == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            while chars.next().is_some_and(|next| next != '\n') {}
            out.push('\n');
            continue;
        }

        out.push(ch);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"TOKENS:
   tkBEEP ("BEEP"),
   tkAND ("AND", operator),

Statements:
   tkBEEP EMIT(opStBeep);
   tkLET EMIT(opStLet) Assignment;

Functions:
   tkABS fn1arg EMIT(opFnAbs);

NonTerminals:

Assignment:
   EXTERNAL MSG_ExpAssignment;
AsClausePrim:
   ((tkINTEGER EMIT(opAsTypeExp) EMIT(ET_I2)) |
    (tkSTRING EMIT(opAsTypeExp) EMIT(ET_SD)));
    <INDEX>
caseItem:
   (Exp EMIT(opStCase));
"#;

    #[test]
    fn parse_inline_snippet_splits_sections_and_tokens() {
        let grammar = parse_grammar(SAMPLE).expect("sample grammar should parse");

        assert_eq!(grammar.tokens.len(), 2);
        assert_eq!(grammar.tokens[0].name, "tkBEEP");
        assert_eq!(grammar.tokens[0].spelling, "BEEP");
        assert!(grammar.tokens[0].attributes.is_empty());

        assert_eq!(grammar.tokens[1].name, "tkAND");
        assert_eq!(grammar.tokens[1].spelling, "AND");
        assert_eq!(grammar.tokens[1].attributes, vec!["operator".to_string()]);

        assert_eq!(grammar.statements.rules.len(), 2);
        assert_eq!(grammar.statements.rules[0].anchor, "tkBEEP");
        assert_eq!(grammar.functions.rules.len(), 1);
        assert_eq!(grammar.functions.rules[0].anchor, "tkABS");
    }

    #[test]
    fn parse_inline_snippet_structures_nonterminals() {
        let grammar = parse_grammar(SAMPLE).expect("sample grammar should parse");
        assert_eq!(grammar.nonterminals.len(), 3);

        let assignment = &grammar.nonterminals[0];
        assert_eq!(assignment.name, "Assignment");
        assert!(assignment.external);
        assert_eq!(assignment.msg_hint.as_deref(), Some("MSG_ExpAssignment"));
        assert!(!assignment.has_index);
        assert!(matches!(
            assignment.body.productions[0].expr,
            GrammarExpr::External {
                msg_hint: Some(ref hint)
            } if hint == "MSG_ExpAssignment"
        ));

        let as_clause = &grammar.nonterminals[1];
        assert_eq!(as_clause.name, "AsClausePrim");
        assert!(!as_clause.external);
        assert!(as_clause.has_index);
        assert!(is_alternative_expr(&as_clause.body.productions[0].expr));

        let case_item = &grammar.nonterminals[2];
        assert_eq!(case_item.name, "caseItem");
        assert!(!case_item.external);
        assert!(!case_item.has_index);
    }

    #[test]
    fn parse_grouped_alternatives_and_sequence() {
        let grammar = parse_grammar(SAMPLE).expect("sample grammar should parse");
        let as_clause = &grammar.nonterminals[1];

        let alts = alternative_branches(&as_clause.body.productions[0].expr);
        assert_eq!(alts.len(), 2);

        for alt in alts {
            let GrammarExpr::Group(inner) = alt else {
                panic!("expected grouped alternative");
            };
            let GrammarExpr::Sequence(items) = inner.as_ref() else {
                panic!("expected sequence inside group");
            };
            assert_eq!(items.len(), 3);
            assert!(matches!(items[0], GrammarExpr::TokenRef(_)));
            assert!(matches!(items[1], GrammarExpr::Emit(_)));
            assert!(matches!(items[2], GrammarExpr::Emit(_)));
        }
    }

    #[test]
    fn parse_statement_directives_and_optional_repeat() {
        let source = r#"TOKENS:
   tkCALL ("CALL"),

Statements:
   tkCALL MARK(1) IdSubRef [tkLParen IdCallArg {tkComma IdCallArg} tkRParen];
     <CgCall(opStCall)>

Functions:

NonTerminals:
Exp:
   EXTERNAL MSG_ExpExp;
"#;

        let grammar = parse_grammar(source).expect("directive snippet should parse");
        let rule = &grammar.statements.rules[0];
        assert_eq!(rule.anchor, "tkCALL");
        assert_eq!(rule.production.cg_hint.as_deref(), Some("CgCall(opStCall)"));

        let GrammarExpr::Sequence(items) = &rule.production.expr else {
            panic!("expected statement sequence");
        };
        assert!(matches!(
            items[0],
            GrammarExpr::Mark(MarkDirective { slot: 1 })
        ));
        assert!(matches!(items[1], GrammarExpr::NonTerminalRef(_)));

        let GrammarExpr::Optional(inner) = &items[2] else {
            panic!("expected optional group");
        };
        let GrammarExpr::Sequence(opt_items) = inner.as_ref() else {
            panic!("expected optional sequence");
        };
        let GrammarExpr::Repeat(repeat_inner) = &opt_items[2] else {
            panic!("expected repeat inside optional, got {:?}", opt_items);
        };
        let GrammarExpr::Sequence(repeat_items) = repeat_inner.as_ref() else {
            panic!("expected repeat sequence");
        };
        assert_eq!(repeat_items.len(), 2);
    }

    #[test]
    fn parse_empty_alternative_in_production() {
        let source = r#"TOKENS:
   tkDO ("DO"),

Statements:
   tkDO (tkWHILE Exp EMIT(opStDoWhile) EMITFFFF) |
        (tkUNTIL Exp EMIT(opStDoUntil) EMITFFFF) |
        (EMIT(opStDo));

Functions:

NonTerminals:
EMITFFFF:
   EMIT(UNDEFINED);
"#;

        let grammar = parse_grammar(source).expect("empty-alternative snippet should parse");
        let rule = &grammar.statements.rules[0];
        let GrammarExpr::Alternative(alts) = &rule.production.expr else {
            panic!("expected alternatives");
        };
        assert_eq!(alts.len(), 3);

        let GrammarExpr::Group(do_while) = &alts[0] else {
            panic!("expected first DO alternative group");
        };
        let GrammarExpr::Sequence(do_items) = do_while.as_ref() else {
            panic!("expected first DO alternative sequence");
        };
        assert!(matches!(do_items[0], GrammarExpr::TokenRef(ref t) if t == "tkWHILE"));

        let GrammarExpr::Group(inner) = &alts[2] else {
            panic!("expected grouped empty-ish alternative");
        };
        assert!(matches!(inner.as_ref(), GrammarExpr::Emit(_)));
    }

    #[test]
    fn strip_comments_preserves_newlines() {
        let stripped = strip_comments("TOKENS:\n   tkA (\"A\") /* comment */\n");
        assert!(stripped.contains("TOKENS:"));
        assert!(stripped.contains("tkA (\"A\")"));
        assert!(!stripped.contains("comment"));
    }

    #[test]
    fn unescape_octal_and_named_escapes() {
        assert_eq!(unescape_prs_string(r"\042"), "\"");
        assert_eq!(unescape_prs_string(r"\n"), "\n");
    }

    #[test]
    fn missing_section_is_reported() {
        let err = parse_grammar("TOKENS:\n   tkA (\"A\")\n").unwrap_err();
        assert!(matches!(
            err,
            GrammarParseError::MissingSection("Statements")
        ));
    }

    #[test]
    fn body_parse_error_is_explicit() {
        let source = r#"TOKENS:
   tkA ("A"),

Statements:

Functions:

NonTerminals:
Broken:
   EMIT();
"#;
        let err = parse_grammar(source).unwrap_err();
        assert!(matches!(err, GrammarParseError::BodyParse { .. }));
    }

    #[test]
    fn real_qbasbnf_file_counts_when_present() {
        let path = Path::new("../../grammar/qbasbnf.prs");

        let grammar = parse_grammar_file(path).expect("vendored qbasbnf.prs should parse");

        assert_eq!(grammar.tokens.len(), 246);
        assert_eq!(grammar.statements.rules.len(), 115);
        assert_eq!(grammar.functions.rules.len(), 84);
        assert!(grammar.statements.text.contains("tkPRINT"));
        assert!(grammar.functions.rules.iter().any(|r| r.anchor == "tkABS"));
        assert!(
            grammar.nonterminals.len() > 70,
            "expected dozens of nonterminals"
        );

        assert_eq!(grammar.tokens[0].name, "tkEtInteger");
        assert_eq!(grammar.tokens[0].spelling, "%");
        assert_eq!(grammar.nonterminals[0].name, "ACTIONidCommon");
        assert!(grammar.nonterminals[0].external);

        let assignment = grammar
            .nonterminals
            .iter()
            .find(|nt| nt.name == "Assignment")
            .expect("Assignment nonterminal");
        assert!(assignment.external);
        assert_eq!(assignment.msg_hint.as_deref(), Some("MSG_ExpAssignment"));

        let as_clause = grammar
            .nonterminals
            .iter()
            .find(|nt| nt.name == "AsClausePrim")
            .expect("AsClausePrim nonterminal");
        assert!(as_clause.has_index);
        assert!(!as_clause.external);
        assert!(is_alternative_expr(&as_clause.body.productions[0].expr));

        let exp = grammar
            .nonterminals
            .iter()
            .find(|nt| nt.name == "Exp")
            .expect("Exp nonterminal");
        assert!(exp.external);
        assert_eq!(exp.msg_hint.as_deref(), Some("MSG_ExpExp"));

        let indexed = grammar
            .nonterminals
            .iter()
            .filter(|nt| nt.has_index)
            .count();
        assert!(indexed >= 3, "expected several indexed nonterminals");
    }
}
