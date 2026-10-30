//! QuickrBASIC's statements as the parser has them: each is rewritten, before
//! parsing, into statements and calls the parser reads without knowing the
//! dialect. `x += 1` becomes `x = $AUG+(1)`, `a, b = b, a` becomes `$TUPLE(a,
//! b) = $TUPLE(b, a)`, `RETURN v` in a FUNCTION becomes `name = v: EXIT
//! FUNCTION`. No source name can spell the `$` names, and the semantic pass
//! reads them back.

use std::collections::BTreeSet;

use crate::front::kw::Kw;
use crate::front::token::{FormatSegment, Token, TokenKind};
use crate::syntax::Span;
use crate::{Dialect, LexError};

/// The prefix of the function an augmented assignment calls: `x += e`
/// becomes `x = $AUG+(e)`. No source name can spell it.
pub const AUGMENTED: &str = "$AUG";

/// QuickrBASIC's augmented assignment, `target op= value`, rewritten into
/// an ordinary assignment the grammar parses anywhere a statement goes. An
/// operator written directly against `=` cannot occur in QuickBASIC, so the
/// rewrite changes no valid program.
pub(crate) fn augmented(tokens: Vec<Token>) -> Vec<Token> {
    let is = |token: &Token, kw: Kw| token.kind == TokenKind::Reserved(kw);
    let operators = [
        (Kw::Add, "tkAdd"),
        (Kw::Minus, "tkMinus"),
        (Kw::Mult, "tkMult"),
        (Kw::Div, "tkDiv"),
        (Kw::Idiv, "tkIdiv"),
        (Kw::Pwr, "tkPwr"),
        (Kw::MOD, "tkMOD"),
        (Kw::AND, "tkAND"),
        (Kw::OR, "tkOR"),
        (Kw::XOR, "tkXOR"),
    ];
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        let operator = operators.iter().find(|(kw, _)| is(token, *kw)).map(|(_, name)| name);
        let equals = tokens
            .get(at + 1)
            .filter(|next| is(next, Kw::EQ) && next.span.line == token.span.line && next.span.start == token.span.end);
        let (Some(operator), Some(equals)) = (operator, equals) else {
            out.push(token.clone());
            at += 1;
            continue;
        };
        let end = token_statement_end(&tokens, at + 2);
        let at_token = |kind: TokenKind| Token { kind, span: token.span };
        out.push(equals.clone());
        out.push(at_token(TokenKind::Identifier(format!("{AUGMENTED}{}", operator))));
        out.push(at_token(TokenKind::Reserved(Kw::LParen)));
        out.extend(tokens[at + 2..end].iter().cloned());
        out.push(at_token(TokenKind::Reserved(Kw::RParen)));
        at = end;
    }
    out
}

/// QuickrBASIC's `RETURN value` in a FUNCTION, rewritten into QB's own way
/// to return: `name = value: EXIT FUNCTION`. A bare RETURN still ends a
/// GOSUB, and QB's `RETURN label` has no place left in a FUNCTION.

/// QuickrBASIC's `RETURN value` in a FUNCTION, rewritten into QB's own way
/// to return: `name = value: EXIT FUNCTION`. A bare RETURN still ends a
/// GOSUB, and QB's `RETURN label` has no place left in a FUNCTION.
pub(crate) fn returned(
    tokens: Vec<Token>,
    dialect: Dialect,
) -> Vec<Token> {
    if !dialect.return_values() {
        return tokens;
    }
    let is = |token: Option<&Token>, kw: Kw| token.is_some_and(|token| token.kind == TokenKind::Reserved(kw));
    let mut function: Option<Token> = None;
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        let previous = at.checked_sub(1).and_then(|before| tokens.get(before));
        if is(Some(token), Kw::FUNCTION) {
            match tokens.get(at + 1) {
                _ if is(previous, Kw::END) => function = None,
                Some(name @ Token { kind: TokenKind::Identifier(_), .. })
                    if !is(previous, Kw::DECLARE) && !is(previous, Kw::EXIT) =>
                {
                    function = Some(name.clone())
                }
                _ => {}
            }
        }
        let value_end = || token_statement_end(&tokens, at + 1);
        match &function {
            Some(name) if is(Some(token), Kw::RETURN) && value_end() > at + 1 => {
                let end = value_end();
                let reserved = |kw: Kw| Token { kind: TokenKind::Reserved(kw), span: token.span };
                out.push(Token { span: token.span, ..name.clone() });
                out.push(reserved(Kw::EQ));
                let value = &tokens[at + 1..end];
                if depth_zero_comma(value).is_some() {
                    out.push(Token { kind: TokenKind::Identifier(TUPLE.into()), span: token.span });
                    out.push(reserved(Kw::LParen));
                    out.extend(value.iter().cloned());
                    out.push(reserved(Kw::RParen));
                } else {
                    out.extend(value.iter().cloned());
                }
                out.push(reserved(Kw::Colon));
                out.push(reserved(Kw::EXIT));
                out.push(reserved(Kw::FUNCTION));
                at = end;
            }
            _ => {
                out.push(token.clone());
                at += 1;
            }
        }
    }
    out
}

/// The function a tuple calls, on either side of `=`: `a, b = b, a`

/// The function a tuple calls, on either side of `=`: `a, b = b, a`
/// becomes `$TUPLE(a, b) = $TUPLE(b, a)`. No source name can spell it.
pub const TUPLE: &str = "$TUPLE";

/// ON n GOTO|GOSUB's selector: `n` rounded to INTEGER, ERROR 5 raised by the
/// ON statement itself where it is outside 0..255, so RESUME retries the ON.
pub const ON_SELECTOR: &str = "$ON";
/// An event trap statement as `EVENT(device, number, verb, label)`:

/// The name a coordinate pair's x carries when the pair is STEP-relative to the
/// last point.
pub const STEP_MARK: &str = "$STEP";

/// Where the first comma outside parentheses stands in `tokens`.
fn depth_zero_comma(tokens: &[Token]) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Reserved(Kw::LParen) => depth += 1,
            TokenKind::Reserved(Kw::RParen) => depth = depth.saturating_sub(1),
            TokenKind::Reserved(Kw::Comma) if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

/// QuickrBASIC's tuple assignment, `a, b = x, y`, rewritten into one the
/// grammar parses: `$TUPLE(a, b) = $TUPLE(x, y)`. A statement that starts
/// with a name and has a comma before its `=` is one. A slice target,
/// `s(1:3) = v`, which the grammar cannot parse either, becomes a one-target
/// list: `$TUPLE(s(1:3)) = v`.

/// QuickrBASIC's tuple assignment, `a, b = x, y`, rewritten into one the
/// grammar parses: `$TUPLE(a, b) = $TUPLE(x, y)`. A statement that starts
/// with a name and has a comma before its `=` is one. A slice target,
/// `s(1:3) = v`, which the grammar cannot parse either, becomes a one-target
/// list: `$TUPLE(s(1:3)) = v`.
pub(crate) fn tuple_assignment(
    tokens: Vec<Token>,
    dialect: Dialect,
) -> Vec<Token> {
    if !dialect.tuples() {
        return tokens;
    }
    let is = |token: Option<&Token>, kw: Kw| token.is_some_and(|token| token.kind == TokenKind::Reserved(kw));
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        let starts = out
            .last()
            .is_none_or(
                |last: &Token| [Kw::NewLine, Kw::Colon, Kw::THEN, Kw::ELSE].iter().any(|name| is(Some(last), *name)),
            );
        let end = if starts && matches!(token.kind, TokenKind::Identifier(_)) {
            token_statement_end(&tokens, at)
        } else {
            at
        };
        let statement = &tokens[at..end];
        let equals = statement.iter().position(|one| is(Some(one), Kw::EQ));
        let Some(equals) = equals else {
            out.push(token.clone());
            at += 1;
            continue;
        };
        let targets = depth_zero_comma(statement).is_some_and(|comma| comma < equals);
        // A statement's own colons end it, so any before `=` is a slice's.
        let slice = statement[..equals].iter().any(|one| is(Some(one), Kw::Colon));
        if !targets && !slice {
            out.push(token.clone());
            at += 1;
            continue;
        }
        let wrapped = |out: &mut Vec<Token>, part: &[Token]| {
            let span = part[0].span;
            out.push(Token { kind: TokenKind::Identifier(TUPLE.into()), span });
            out.push(Token { kind: TokenKind::Reserved(Kw::LParen), span });
            out.extend(part.iter().cloned());
            out.push(Token { kind: TokenKind::Reserved(Kw::RParen), span });
        };
        wrapped(&mut out, &statement[..equals]);
        out.push(statement[equals].clone());
        let value = &statement[equals + 1..];
        if value.is_empty() || depth_zero_comma(value).is_none() {
            out.extend(value.iter().cloned());
        } else {
            wrapped(&mut out, value);
        }
        at = end;
    }
    out
}

/// Where the statement running from `at` ends: `:`, a new line, or the ELSE
/// of a one-line IF, outside any parentheses. An ELSE answering an IF inside
/// the statement belongs to a conditional expression.

/// Where the statement running from `at` ends: `:`, a new line, or the ELSE
/// of a one-line IF, outside any parentheses. An ELSE answering an IF inside
/// the statement belongs to a conditional expression.
fn token_statement_end(
    tokens: &[Token],
    mut at: usize,
) -> usize {
    let is = |token: &Token, kw: Kw| token.kind == TokenKind::Reserved(kw);
    let mut depth = 0usize;
    let mut conditionals = 0usize;
    while let Some(next) = tokens.get(at) {
        if depth == 0 && is(next, Kw::ELSE) && conditionals > 0 {
            conditionals -= 1;
            at += 1;
            continue;
        }
        if depth == 0 && (is(next, Kw::Colon) || is(next, Kw::NewLine) || is(next, Kw::ELSE)) {
            break;
        }
        if is(next, Kw::IF) {
            conditionals += 1;
        } else if is(next, Kw::LParen) {
            depth += 1;
        } else if is(next, Kw::RParen) {
            depth = depth.saturating_sub(1);
        }
        at += 1;
    }
    at
}

/// The function a `FOR … IN` loop starts from, and the prefix of the name
/// its `AS` declares: `FOR x AS t IN e` becomes
/// `DIM $EACHx AS t: FOR x = $EACH(e) TO 0`. No source name can spell it.

/// The function a `FOR … IN` loop starts from, and the prefix of the name
/// its `AS` declares: `FOR x AS t IN e` becomes
/// `DIM $EACHx AS t: FOR x = $EACH(e) TO 0`. No source name can spell it.
pub const EACH: &str = "$EACH";

/// QuickrBASIC's `FOR x [AS type] IN iterable`, rewritten into a FOR the
/// grammar parses and pairs with its NEXT. A QB FOR has `=` after its
/// counter, never AS or IN.
pub(crate) fn for_in(tokens: Vec<Token>) -> Vec<Token> {
    let is = |token: Option<&Token>, kw: Kw| token.is_some_and(|token| token.kind == TokenKind::Reserved(kw));
    let word = |token: Option<&Token>, text: &str| {
        token.is_some_and(|token| matches!(&token.kind, TokenKind::Identifier(word) if word == text))
    };
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let variable = match tokens.get(at + 1).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) if is(tokens.get(at), Kw::FOR) => name.clone(),
            _ => {
                out.push(tokens[at].clone());
                at += 1;
                continue;
            }
        };
        let end = token_statement_end(&tokens, at);
        let Some(within) = (at + 2..end).find(|&index| word(tokens.get(index), "IN")) else {
            out.push(tokens[at].clone());
            at += 1;
            continue;
        };
        let span = tokens[at].span;
        let token = |kind: TokenKind| Token { kind, span };
        let reserved = |kw: Kw| token(TokenKind::Reserved(kw));
        if is(tokens.get(at + 2), Kw::AS) {
            out.push(reserved(Kw::DIM));
            out.push(token(TokenKind::Identifier(format!("{EACH}{variable}"))));
            out.extend(tokens[at + 2..within].iter().cloned());
            out.push(reserved(Kw::Colon));
        } else if within != at + 2 {
            out.push(tokens[at].clone());
            at += 1;
            continue;
        }
        out.push(tokens[at].clone());
        out.push(tokens[at + 1].clone());
        out.push(reserved(Kw::EQ));
        out.push(token(TokenKind::Identifier(EACH.into())));
        out.push(reserved(Kw::LParen));
        out.extend(tokens[within + 1..end].iter().cloned());
        out.push(reserved(Kw::RParen));
        out.push(reserved(Kw::TO));
        out.push(token(TokenKind::Integer(0, None)));
        at = end;
    }
    out
}

/// `PRIVATE SUB` and `PRIVATE FUNCTION`: the grammar parses an ordinary
/// header, so PRIVATE leaves the stream and where its SUB or FUNCTION now
/// stands marks the procedure that statement creates.
pub(crate) fn without_private(tokens: Vec<Token>) -> (Vec<Token>, BTreeSet<usize>) {
    let separator = |token: &Token| matches!(token.kind, TokenKind::Reserved(Kw::NewLine | Kw::Colon));
    let header = |token: Option<&Token>| {
        token.is_some_and(|token| matches!(token.kind, TokenKind::Reserved(Kw::SUB | Kw::FUNCTION)))
    };
    let mut kept = Vec::with_capacity(tokens.len());
    let mut marked = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        let starts = kept.last().is_none_or(separator);
        if starts
            && matches!(&token.kind, TokenKind::Identifier(word) if word == "PRIVATE")
            && header(tokens.get(index + 1))
        {
            marked.insert(kept.len());
            continue;
        }
        kept.push(token.clone());
    }
    (kept, marked)
}

/// The name of the conversion an f-string field makes. No source name can spell
/// it, so no program can call or shadow it.
pub const FORMAT_FIELD: &str = "$FSTRING";

/// `f"..."` from its `f`: the text and the fields, each field's expression
/// already scanned.
pub(crate) fn format_string(
    line: &str,
    at: &mut usize,
    line_index: usize,
    dialect: Dialect,
) -> Result<TokenKind, LexError> {
    let bytes = line.as_bytes();
    let span = |start, end| Span { line: line_index, start, end };
    let error = |span, message: &str| LexError { span, message: message.into() };
    let start = *at;
    *at += 2;
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut text_from = *at;
    let flush = |text: &mut String, segments: &mut Vec<FormatSegment>| {
        if !text.is_empty() {
            segments.push(FormatSegment::Text(std::mem::take(text)));
        }
    };
    loop {
        let Some(&byte) = bytes.get(*at) else {
            return Err(error(span(start, *at), "unterminated string"));
        };
        let doubled = bytes.get(*at + 1) == Some(&byte);
        match byte {
            b'"' => {
                text.push_str(&line[text_from..*at]);
                *at += 1;
                break;
            }
            b'{' | b'}' if doubled => {
                text.push_str(&line[text_from..=*at]);
                *at += 2;
                text_from = *at;
            }
            b'}' => return Err(error(span(*at, *at + 1), "single '}' in an f-string")),
            b'{' => {
                text.push_str(&line[text_from..*at]);
                flush(&mut text, &mut segments);
                let open = *at + 1;
                let close = line[open..].find(['}', '"']).map_or(bytes.len(), |one| one + open);
                if bytes.get(close) != Some(&b'}') {
                    return Err(error(span(*at, close), "unterminated f-string field; a field cannot hold a string"));
                }
                let field = &line[open..close];
                let (expression, spec) = match field.split_once(':') {
                    Some((expression, spec)) => (expression, Some(spec.to_owned())),
                    None => (field, None),
                };
                if expression.trim().is_empty() {
                    return Err(error(span(*at, close + 1), "empty f-string field"));
                }
                if spec.as_deref().is_some_and(|spec| spec.contains('{')) {
                    return Err(error(span(*at, close + 1), "nested f-string fields are not supported"));
                }
                let mut tokens = crate::front::scan::scan(expression, dialect)
                    .map_err(|inner| error(span(open + inner.span.start, open + inner.span.end), &inner.message))?;
                tokens.pop(); // the end-of-line token
                for token in &mut tokens {
                    token.span = span(open + token.span.start, open + token.span.end);
                }
                segments.push(FormatSegment::Field { tokens, spec, span: span(*at, close + 1) });
                *at = close + 1;
                text_from = *at;
            }
            _ => *at += 1,
        }
    }
    flush(&mut text, &mut segments);
    Ok(TokenKind::FormatString(segments))
}
