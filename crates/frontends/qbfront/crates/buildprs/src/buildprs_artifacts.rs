//! Inspect text-shaped `buildprs` golden artifacts before wiring generated tables.
//!
//! The original DOS tool emits MASM-style `.inc` / `.asm` files (`prstab.inc`,
//! `prsirw.inc`, `prsstate.asm`, `prsrwt.asm`, …). Those files are not checked
//! into this tree yet, but downstream Rust modules need a pragmatic way to parse
//! and validate their public symbol contracts once they exist.

use std::collections::BTreeMap;

/// Node-directive constants from generated `prstab.inc`.
pub const PRSTAB_REQUIRED_EQUATES: &[&str] = &[
    "ND_ACCEPT",
    "ND_REJECT",
    "ND_MARK",
    "ND_EMIT",
    "ND_BRANCH",
    "ENCODE1BYTE",
    "NUMNTINT",
    "NUMNTEXT",
];

/// Reserved-word flag equates (`RWF_*`) that parser consumers reference.
pub const PRSTAB_REQUIRED_RWF_EQUATES: &[&str] = &[
    "RWF_OPERATOR",
    "RWF_NO_DIRECT",
    "RWF_NSTMTS",
    "RWF_FUNC",
    "RWF_STR",
];

/// Reserved-word token ids that lexer consumers treat as special cases.
pub const PRSIRW_REQUIRED_EQUATES: &[&str] = &["IRW_NewLine", "IRW_PRINT", "IRW_DATA", "IRW_REM"];

/// Labels expected in `prsstate.asm`.
pub const PRSSTATE_REQUIRED_LABELS: &[&str] = &["tState", "tIntNtDisp", "tExtNtHelp"];

/// Labels expected in `prsrwt.asm`.
pub const PRSRWT_REQUIRED_LABELS: &[&str] = &["tRw", "mpIRWtoIOP", "mpIRWtoChar"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseEquatesError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseEquatesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseEquatesError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDataError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseDataError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactValidationError {
    MissingEquate {
        artifact: &'static str,
        symbol: String,
    },
    MissingLabel {
        artifact: &'static str,
        label: String,
    },
    InvalidNdDirectives(BTreeMap<String, i64>),
    NonDenseIrwIds {
        missing: Vec<i64>,
    },
    ParseEquates {
        artifact: &'static str,
        error: ParseEquatesError,
    },
}

impl std::fmt::Display for ArtifactValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEquate { artifact, symbol } => {
                write!(f, "{artifact} is missing required equate {symbol}")
            }
            Self::MissingLabel { artifact, label } => {
                write!(f, "{artifact} is missing required label {label}")
            }
            Self::InvalidNdDirectives(values) => {
                write!(f, "ND_* directives have unexpected values: {values:?}")
            }
            Self::NonDenseIrwIds { missing } => {
                write!(
                    f,
                    "IRW_* ids are not dense 0..N-1; missing values: {missing:?}"
                )
            }
            Self::ParseEquates { artifact, error } => {
                write!(f, "failed to parse equates in {artifact}: {error}")
            }
        }
    }
}

impl std::error::Error for ArtifactValidationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    Byte,
    Word,
    Dword,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelDeclaration {
    pub kind: LabelKind,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSet {
    pub prstab: String,
    pub prsirw: String,
    pub prsstate: String,
    pub prsrwt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedArtifacts {
    pub prstab_equates: BTreeMap<String, i64>,
    pub prsirw_equates: BTreeMap<String, i64>,
    pub prsstate_labels: BTreeMap<String, LabelDeclaration>,
    pub prsrwt_labels: BTreeMap<String, LabelDeclaration>,
}

impl ArtifactSet {
    #[must_use]
    pub fn new(
        prstab: impl Into<String>,
        prsirw: impl Into<String>,
        prsstate: impl Into<String>,
        prsrwt: impl Into<String>,
    ) -> Self {
        Self {
            prstab: prstab.into(),
            prsirw: prsirw.into(),
            prsstate: prsstate.into(),
            prsrwt: prsrwt.into(),
        }
    }

    pub fn validate(self) -> Result<ValidatedArtifacts, ArtifactValidationError> {
        let prstab_equates =
            parse_equates(&self.prstab).map_err(|error| ArtifactValidationError::ParseEquates {
                artifact: "prstab.inc",
                error,
            })?;
        let prsirw_equates = parse_irw_equates(&self.prsirw).map_err(|error| {
            ArtifactValidationError::ParseEquates {
                artifact: "prsirw.inc",
                error,
            }
        })?;

        for symbol in PRSTAB_REQUIRED_EQUATES {
            if !prstab_equates.contains_key(*symbol) {
                return Err(ArtifactValidationError::MissingEquate {
                    artifact: "prstab.inc",
                    symbol: (*symbol).to_string(),
                });
            }
        }
        for symbol in PRSTAB_REQUIRED_RWF_EQUATES {
            if !prstab_equates.contains_key(*symbol) {
                return Err(ArtifactValidationError::MissingEquate {
                    artifact: "prstab.inc",
                    symbol: (*symbol).to_string(),
                });
            }
        }
        validate_nd_directives(&prstab_equates)?;

        for symbol in PRSIRW_REQUIRED_EQUATES {
            if !prsirw_equates.contains_key(*symbol) {
                return Err(ArtifactValidationError::MissingEquate {
                    artifact: "prsirw.inc",
                    symbol: (*symbol).to_string(),
                });
            }
        }
        validate_dense_irw_ids(&prsirw_equates)?;

        let prsstate_labels = find_label_declarations(&self.prsstate);
        for label in PRSSTATE_REQUIRED_LABELS {
            if !prsstate_labels.contains_key(*label) {
                return Err(ArtifactValidationError::MissingLabel {
                    artifact: "prsstate.asm",
                    label: (*label).to_string(),
                });
            }
        }

        let prsrwt_labels = find_label_declarations(&self.prsrwt);
        for label in PRSRWT_REQUIRED_LABELS {
            if !prsrwt_labels.contains_key(*label) {
                return Err(ArtifactValidationError::MissingLabel {
                    artifact: "prsrwt.asm",
                    label: (*label).to_string(),
                });
            }
        }

        Ok(ValidatedArtifacts {
            prstab_equates,
            prsirw_equates,
            prsstate_labels,
            prsrwt_labels,
        })
    }
}

/// Parse `NAME EQU <expr>` definitions from MASM-style include/asm text.
pub fn parse_equates(text: &str) -> Result<BTreeMap<String, i64>, ParseEquatesError> {
    let mut raw: BTreeMap<String, String> = BTreeMap::new();

    for (line_idx, line) in text.lines().enumerate() {
        let line_no = line_idx + 1;
        let trimmed = strip_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((name, expr)) = split_equate_line(trimmed) {
            if raw.insert(name.clone(), expr).is_some() {
                return Err(ParseEquatesError {
                    line: line_no,
                    message: format!("duplicate equate {name}"),
                });
            }
        }
    }

    resolve_equates(raw)
}

/// Parse `IRW_*` equates from `prsirw.inc` text.
pub fn parse_irw_equates(text: &str) -> Result<BTreeMap<String, i64>, ParseEquatesError> {
    let all = parse_equates(text)?;
    Ok(all
        .into_iter()
        .filter(|(name, _)| name.starts_with("IRW_"))
        .collect())
}

/// Return `label byte|word|dword` declarations in source order.
pub fn find_label_declarations(text: &str) -> BTreeMap<String, LabelDeclaration> {
    let mut labels = BTreeMap::new();

    for (line_idx, line) in text.lines().enumerate() {
        let line_no = line_idx + 1;
        let trimmed = strip_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut fields = trimmed.split_whitespace();
        let Some(name) = fields.next() else {
            continue;
        };
        let Some(label_keyword) = fields.next() else {
            continue;
        };
        if !label_keyword.eq_ignore_ascii_case("label") {
            continue;
        }
        let Some(kind_keyword) = fields.next() else {
            continue;
        };

        let Some(kind) = label_kind_from_keyword(kind_keyword) else {
            continue;
        };

        if !name.is_empty() {
            labels.entry(name.to_string()).or_insert(LabelDeclaration {
                kind,
                line: line_no,
            });
        }
    }

    labels
}

fn label_kind_from_keyword(keyword: &str) -> Option<LabelKind> {
    if keyword.eq_ignore_ascii_case("byte") {
        Some(LabelKind::Byte)
    } else if keyword.eq_ignore_ascii_case("word") {
        Some(LabelKind::Word)
    } else if keyword.eq_ignore_ascii_case("dword") {
        Some(LabelKind::Dword)
    } else {
        None
    }
}

/// Parse `db` initializer bytes following a label block.
pub fn parse_db_bytes(
    text: &str,
    label: &str,
    equates: &BTreeMap<String, i64>,
) -> Result<Vec<u8>, ParseDataError> {
    let values = parse_initializer_values(text, label, "db", equates)?;
    values
        .into_iter()
        .map(|value| i64_to_u8(value).map_err(|message| ParseDataError { line: 0, message }))
        .collect()
}

/// Parse `dw offset <symbol>` words following a label block.
pub fn parse_dw_offsets(text: &str, label: &str) -> Result<Vec<String>, ParseDataError> {
    let mut values = Vec::new();
    let mut in_block = false;
    let mut line_no = 0;

    for line in text.lines() {
        line_no += 1;
        let trimmed = strip_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if !in_block {
            if label_declares(trimmed, label) {
                in_block = true;
            }
            continue;
        }

        if is_label_line(trimmed) {
            break;
        }

        if let Some(rest) = lower.strip_prefix("db") {
            return Err(ParseDataError {
                line: line_no,
                message: format!("expected dw data under {label}, found db{rest}"),
            });
        }

        let Some(rest) = trimmed
            .get(..2)
            .filter(|prefix| prefix.eq_ignore_ascii_case("dw"))
            .and_then(|_| trimmed.get(2..))
        else {
            continue;
        };

        for item in split_data_items(rest) {
            values.push(parse_offset_operand(item, line_no)?);
        }
    }

    if !in_block {
        return Err(ParseDataError {
            line: 0,
            message: format!("label {label} not found"),
        });
    }

    Ok(values)
}

pub fn validate_nd_directives(
    equates: &BTreeMap<String, i64>,
) -> Result<(), ArtifactValidationError> {
    let expected = [
        ("ND_ACCEPT", 0),
        ("ND_REJECT", 1),
        ("ND_MARK", 2),
        ("ND_EMIT", 3),
        ("ND_BRANCH", 4),
    ];
    let mut mismatches = BTreeMap::new();

    for (name, value) in expected {
        match equates.get(name) {
            Some(actual) if *actual == value => {}
            Some(actual) => {
                mismatches.insert(name.to_string(), *actual);
            }
            None => {
                return Err(ArtifactValidationError::MissingEquate {
                    artifact: "prstab.inc",
                    symbol: name.to_string(),
                });
            }
        }
    }

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(ArtifactValidationError::InvalidNdDirectives(mismatches))
    }
}

pub fn validate_dense_irw_ids(irw: &BTreeMap<String, i64>) -> Result<(), ArtifactValidationError> {
    if irw.is_empty() {
        return Err(ArtifactValidationError::NonDenseIrwIds { missing: vec![0] });
    }

    let mut ids: Vec<i64> = irw.values().copied().collect();
    ids.sort_unstable();
    ids.dedup();

    if ids.first().copied() != Some(0) {
        return Err(ArtifactValidationError::NonDenseIrwIds { missing: vec![0] });
    }

    let max = *ids.last().expect("ids is non-empty");
    let missing = (0..=max).filter(|id| !ids.contains(id)).collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(ArtifactValidationError::NonDenseIrwIds { missing })
    }
}

pub fn symbols_with_prefix<'a>(
    equates: &'a BTreeMap<String, i64>,
    prefix: &str,
) -> BTreeMap<&'a str, i64> {
    equates
        .iter()
        .filter_map(|(name, value)| name.starts_with(prefix).then_some((name.as_str(), *value)))
        .collect()
}

fn strip_comment(line: &str) -> &str {
    line.split(';').next().unwrap_or(line)
}

fn split_equate_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let name = parts.next()?.to_string();
    let directive = parts.next()?;
    if !directive.eq_ignore_ascii_case("equ") {
        return None;
    }
    let expr = parts.collect::<Vec<_>>().join(" ");
    if expr.is_empty() {
        return None;
    }
    Some((name, expr))
}

fn resolve_equates(
    raw: BTreeMap<String, String>,
) -> Result<BTreeMap<String, i64>, ParseEquatesError> {
    let mut resolved: BTreeMap<String, i64> = BTreeMap::new();
    let mut pending: Vec<(String, String)> = raw.into_iter().collect();

    while !pending.is_empty() {
        let mut progress = false;
        let mut next_pending = Vec::new();

        for (name, expr) in pending {
            match eval_expression(&expr, &resolved) {
                Ok(value) => {
                    resolved.insert(name, value);
                    progress = true;
                }
                Err(EvalError::Unresolved(_)) => next_pending.push((name, expr)),
                Err(EvalError::Invalid(message)) => {
                    return Err(ParseEquatesError {
                        line: 0,
                        message: format!("{name} EQU {expr}: {message}"),
                    });
                }
            }
        }

        if !progress {
            let (name, expr) = next_pending
                .first()
                .cloned()
                .unwrap_or_else(|| ("?".to_string(), String::new()));
            return Err(ParseEquatesError {
                line: 0,
                message: format!("unresolved equate chain starting at {name} EQU {expr}"),
            });
        }

        pending = next_pending;
    }

    Ok(resolved)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EvalError {
    Unresolved(String),
    Invalid(String),
}

fn eval_expression(expr: &str, env: &BTreeMap<String, i64>) -> Result<i64, EvalError> {
    let tokens = tokenize_expression(expr)?;
    let mut parser = ExprParser {
        tokens: &tokens,
        pos: 0,
        env,
    };
    let value = parser.parse_expr()?;
    if parser.pos != tokens.len() {
        return Err(EvalError::Invalid(
            "trailing tokens in expression".to_string(),
        ));
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Number(i64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Offset,
}

fn tokenize_expression(expr: &str) -> Result<Vec<Token>, EvalError> {
    let mut tokens = Vec::new();
    let bytes = expr.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        let ch = bytes[idx];
        if ch.is_ascii_whitespace() {
            idx += 1;
            continue;
        }

        match ch {
            b'+' => {
                tokens.push(Token::Plus);
                idx += 1;
            }
            b'-' => {
                tokens.push(Token::Minus);
                idx += 1;
            }
            b'*' => {
                tokens.push(Token::Star);
                idx += 1;
            }
            b'/' => {
                tokens.push(Token::Slash);
                idx += 1;
            }
            b'(' => {
                tokens.push(Token::LParen);
                idx += 1;
            }
            b')' => {
                tokens.push(Token::RParen);
                idx += 1;
            }
            b'\'' => {
                idx += 1;
                if idx >= bytes.len() {
                    return Err(EvalError::Invalid(
                        "unterminated character literal".to_string(),
                    ));
                }
                let value = bytes[idx] as i64;
                idx += 1;
                if idx < bytes.len() && bytes[idx] == b'\'' {
                    idx += 1;
                }
                tokens.push(Token::Number(value));
            }
            b'0'..=b'9' => {
                let start = idx;
                idx += 1;
                while idx < bytes.len() && bytes[idx].is_ascii_alphanumeric() {
                    idx += 1;
                }
                let literal = std::str::from_utf8(&bytes[start..idx]).map_err(|_| {
                    EvalError::Invalid("invalid utf-8 in numeric literal".to_string())
                })?;
                tokens.push(Token::Number(parse_numeric_literal(literal)?));
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'@' => {
                let start = idx;
                idx += 1;
                while idx < bytes.len() {
                    let next = bytes[idx];
                    if next.is_ascii_alphanumeric() || next == b'_' || next == b'@' {
                        idx += 1;
                    } else {
                        break;
                    }
                }
                let ident = std::str::from_utf8(&bytes[start..idx])
                    .map_err(|_| EvalError::Invalid("invalid utf-8 in identifier".to_string()))?
                    .to_string();
                if ident.eq_ignore_ascii_case("offset") {
                    tokens.push(Token::Offset);
                } else {
                    tokens.push(Token::Ident(ident));
                }
            }
            _ => {
                return Err(EvalError::Invalid(format!(
                    "unexpected character {:?} in expression",
                    ch as char
                )));
            }
        }
    }

    Ok(tokens)
}

fn parse_numeric_literal(literal: &str) -> Result<i64, EvalError> {
    let lower = literal.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix('h') {
        let digits = if stem.is_empty() { "0" } else { stem };
        i64::from_str_radix(digits, 16)
            .map_err(|_| EvalError::Invalid(format!("invalid hex literal {literal}")))
    } else if let Some(stem) = lower.strip_suffix('b') {
        i64::from_str_radix(stem, 2)
            .map_err(|_| EvalError::Invalid(format!("invalid binary literal {literal}")))
    } else if let Some(stem) = lower.strip_suffix('d') {
        stem.parse::<i64>()
            .map_err(|_| EvalError::Invalid(format!("invalid decimal literal {literal}")))
    } else {
        literal
            .parse::<i64>()
            .map_err(|_| EvalError::Invalid(format!("invalid numeric literal {literal}")))
    }
}

struct ExprParser<'a> {
    tokens: &'a [Token],
    pos: usize,
    env: &'a BTreeMap<String, i64>,
}

impl<'a> ExprParser<'a> {
    fn parse_expr(&mut self) -> Result<i64, EvalError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<i64, EvalError> {
        let mut value = self.parse_multiplicative()?;
        while let Some(op) = self.current_op() {
            if !matches!(op, Token::Plus | Token::Minus) {
                break;
            }
            self.pos += 1;
            let rhs = self.parse_multiplicative()?;
            value = match op {
                Token::Plus => value + rhs,
                Token::Minus => value - rhs,
                _ => unreachable!(),
            };
        }
        Ok(value)
    }

    fn parse_multiplicative(&mut self) -> Result<i64, EvalError> {
        let mut value = self.parse_unary()?;
        while let Some(op) = self.current_op() {
            if !matches!(op, Token::Star | Token::Slash) {
                break;
            }
            self.pos += 1;
            let rhs = self.parse_unary()?;
            value = match op {
                Token::Star => value * rhs,
                Token::Slash => {
                    if rhs == 0 {
                        return Err(EvalError::Invalid("division by zero".to_string()));
                    }
                    value / rhs
                }
                _ => unreachable!(),
            };
        }
        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<i64, EvalError> {
        if matches!(self.current_op(), Some(Token::Minus)) {
            self.pos += 1;
            return Ok(-self.parse_unary()?);
        }
        if matches!(self.current_op(), Some(Token::Plus)) {
            self.pos += 1;
            return self.parse_unary();
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i64, EvalError> {
        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| EvalError::Invalid("empty expression".to_string()))?;
        self.pos += 1;

        match token {
            Token::Number(value) => Ok(value),
            Token::LParen => {
                let value = self.parse_expr()?;
                match self.tokens.get(self.pos) {
                    Some(Token::RParen) => self.pos += 1,
                    _ => {
                        return Err(EvalError::Invalid("missing ')'".to_string()));
                    }
                }
                Ok(value)
            }
            Token::Offset => self.parse_offset_operand(),
            Token::Ident(name) => env_lookup(self.env, &name),
            Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::RParen => Err(
                EvalError::Invalid("unexpected token in expression".to_string()),
            ),
        }
    }

    fn parse_offset_operand(&mut self) -> Result<i64, EvalError> {
        let Token::Ident(name) = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| EvalError::Invalid("expected symbol after OFFSET".to_string()))?
        else {
            return Err(EvalError::Invalid(
                "expected symbol after OFFSET".to_string(),
            ));
        };
        self.pos += 1;
        env_lookup(self.env, &name)
    }

    fn current_op(&self) -> Option<Token> {
        self.tokens.get(self.pos).cloned()
    }
}

fn env_lookup(env: &BTreeMap<String, i64>, name: &str) -> Result<i64, EvalError> {
    if let Some(value) = env.get(name) {
        return Ok(*value);
    }

    for (key, value) in env {
        if key.eq_ignore_ascii_case(name) {
            return Ok(*value);
        }
    }

    Err(EvalError::Unresolved(name.to_string()))
}

fn label_declares(line: &str, label: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(name) = parts.next() else {
        return false;
    };
    if name != label {
        return false;
    }
    let rest = line[name.len()..].to_ascii_lowercase();
    rest.contains("label byte") || rest.contains("label word") || rest.contains("label dword")
}

fn is_label_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains(" label byte") || lower.contains(" label word") || lower.contains(" label dword")
}

fn parse_initializer_values(
    text: &str,
    label: &str,
    directive: &str,
    equates: &BTreeMap<String, i64>,
) -> Result<Vec<i64>, ParseDataError> {
    let mut values = Vec::new();
    let mut in_block = false;
    let mut line_no = 0;

    for line in text.lines() {
        line_no += 1;
        let trimmed = strip_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if !in_block {
            if label_declares(trimmed, label) {
                in_block = true;
            }
            continue;
        }

        if is_label_line(trimmed) {
            break;
        }

        let Some(rest) = trimmed
            .get(..directive.len())
            .filter(|prefix| prefix.eq_ignore_ascii_case(directive))
            .and_then(|_| trimmed.get(directive.len()..))
        else {
            continue;
        };

        for item in split_data_items(rest) {
            values.push(parse_data_operand(item, equates, line_no)?);
        }
    }

    if !in_block {
        return Err(ParseDataError {
            line: 0,
            message: format!("label {label} not found"),
        });
    }

    Ok(values)
}

fn split_data_items(rest: &str) -> Vec<&str> {
    rest.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_data_operand(
    item: &str,
    equates: &BTreeMap<String, i64>,
    line_no: usize,
) -> Result<i64, ParseDataError> {
    if item.starts_with('\'') {
        let ch = item.chars().nth(1).ok_or_else(|| ParseDataError {
            line: line_no,
            message: format!("invalid character literal {item}"),
        })?;
        return Ok(ch as i64);
    }

    if let Ok(value) = parse_numeric_literal(item) {
        return Ok(value);
    }

    eval_expression(item, equates).map_err(|error| ParseDataError {
        line: line_no,
        message: format!("{item}: {error:?}"),
    })
}

fn parse_offset_operand(item: &str, line_no: usize) -> Result<String, ParseDataError> {
    let mut parts = item.split_whitespace();
    let first = parts
        .next()
        .ok_or_else(|| ParseDataError {
            line: line_no,
            message: "empty dw operand".to_string(),
        })?
        .to_string();

    if first.eq_ignore_ascii_case("offset") {
        let symbol = parts.next().ok_or_else(|| ParseDataError {
            line: line_no,
            message: format!("missing symbol after OFFSET in {item}"),
        })?;
        return Ok(symbol.to_string());
    }

    if first.eq_ignore_ascii_case("dw") || first.eq_ignore_ascii_case("db") {
        return parse_offset_operand(parts.collect::<Vec<_>>().join(" ").as_str(), line_no);
    }

    Ok(first)
}

fn i64_to_u8(value: i64) -> Result<u8, String> {
    if (0..=255).contains(&value) {
        Ok(value as u8)
    } else {
        Err(format!("byte value {value} is out of range"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRSTAB_FIXTURE: &str = r";
; Generated by buildprs
ND_ACCEPT EQU 0
ND_REJECT EQU 1
ND_MARK EQU 2
ND_EMIT EQU 3
ND_BRANCH EQU 4
ENCODE1BYTE EQU 0F0h
NUMNTINT EQU 52
NUMNTEXT EQU 41
RWF_OPERATOR EQU 1
RWF_NO_DIRECT EQU 2
RWF_NSTMTS EQU 8
RWF_FUNC EQU 4
RWF_STR EQU 10h
STI_AsClause EQU NUMNTINT + 3
";

    const PRSIRW_FIXTURE: &str = r"
IRW_NewLine EQU 0
IRW_PRINT EQU 1
IRW_ADD EQU 2
IRW_LParen EQU 3
";

    const PRSSTATE_FIXTURE: &str = r"
StNtStatement equ 0
StExp equ 16
tState label byte
    db ND_ACCEPT
    db ND_REJECT, ND_MARK
tIntNtDisp label word
    dw offset StNtStatement
    dw offset StExp
tExtNtHelp label word
    dw MSG_ExpStatement
";

    const PRSRWT_FIXTURE: &str = r"
tRw label byte
    db 0, 1, 2
mpIRWtoIOP label byte
    db 0, 80h, IOP_Add
mpIRWtoChar label byte
    db '!', '+', 0
";

    #[test]
    fn parse_equates_reads_hex_decimal_and_symbolic_expressions() {
        let equates = parse_equates(PRSTAB_FIXTURE).expect("prstab fixture should parse");

        assert_eq!(equates["ND_ACCEPT"], 0);
        assert_eq!(equates["ENCODE1BYTE"], 240);
        assert_eq!(equates["RWF_STR"], 16);
        assert_eq!(equates["STI_AsClause"], 55);
    }

    #[test]
    fn parse_irw_equates_filters_dense_token_ids() {
        let irw = parse_irw_equates(PRSIRW_FIXTURE).expect("prsirw fixture should parse");

        assert_eq!(irw.len(), 4);
        assert_eq!(irw["IRW_PRINT"], 1);
        validate_dense_irw_ids(&irw).expect("fixture ids should be dense");
    }

    #[test]
    fn find_label_declarations_collects_required_state_labels() {
        let labels = find_label_declarations(PRSSTATE_FIXTURE);

        assert_eq!(labels["tState"].kind, LabelKind::Byte);
        assert_eq!(labels["tIntNtDisp"].kind, LabelKind::Word);
        assert_eq!(labels["tExtNtHelp"].kind, LabelKind::Word);
    }

    #[test]
    fn find_label_declarations_accepts_masm_tab_separated_labels() {
        let labels = find_label_declarations("tState\tLABEL\tBYTE\nmpIRWtoIOP\tLABEL\tBYTE\n");

        assert_eq!(labels["tState"].kind, LabelKind::Byte);
        assert_eq!(labels["mpIRWtoIOP"].kind, LabelKind::Byte);
    }

    #[test]
    fn parse_db_and_dw_extract_initializer_payloads() {
        let equates = parse_equates(PRSTAB_FIXTURE).expect("prstab equates");
        let bytes = parse_db_bytes(PRSSTATE_FIXTURE, "tState", &equates).expect("tState bytes");
        assert_eq!(bytes, vec![0, 1, 2]);

        let offsets = parse_dw_offsets(PRSSTATE_FIXTURE, "tIntNtDisp").expect("tIntNtDisp offsets");
        assert_eq!(
            offsets,
            vec!["StNtStatement".to_string(), "StExp".to_string()]
        );

        let rw_bytes = parse_db_bytes(PRSRWT_FIXTURE, "mpIRWtoChar", &BTreeMap::new())
            .expect("mpIRWtoChar bytes");
        assert_eq!(rw_bytes, vec![b'!', b'+', 0]);
    }

    #[test]
    fn artifact_set_validation_accepts_minimal_contract_fixture() {
        let mut prsirw = PRSIRW_FIXTURE.to_string();
        prsirw.push_str("IRW_DATA EQU 4\nIRW_REM EQU 5\n");

        let validated = ArtifactSet::new(PRSTAB_FIXTURE, prsirw, PRSSTATE_FIXTURE, PRSRWT_FIXTURE)
            .validate()
            .expect("minimal artifact set should validate");

        assert_eq!(validated.prstab_equates["NUMNTEXT"], 41);
        assert!(validated.prsstate_labels.contains_key("tState"));
        assert!(validated.prsrwt_labels.contains_key("mpIRWtoIOP"));
    }

    #[test]
    fn artifact_set_validation_reports_missing_required_symbols() {
        let error = ArtifactSet::new("ND_ACCEPT EQU 0", "", "", "")
            .validate()
            .expect_err("incomplete artifacts should fail");

        assert!(matches!(
            error,
            ArtifactValidationError::MissingEquate { .. }
                | ArtifactValidationError::ParseEquates { .. }
        ));
    }

    #[test]
    fn symbols_with_prefix_returns_sorted_rwf_entries() {
        let equates = parse_equates(PRSTAB_FIXTURE).expect("prstab equates");
        let rwf = symbols_with_prefix(&equates, "RWF_");

        assert_eq!(rwf.len(), 5);
        assert_eq!(rwf["RWF_NSTMTS"], 8);
    }
}
