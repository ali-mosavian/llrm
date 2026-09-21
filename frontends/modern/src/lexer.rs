use crate::error::Diagnostic;
use crate::syntax::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Integer(i64),
    Fn,
    Let,
    Var,
    Return,
    If,
    Else,
    While,
    Break,
    Continue,
    True,
    False,
    Not,
    I16,
    I32,
    Bool,
    Void,
    LeftParen,
    RightParen,
    Comma,
    Colon,
    Arrow,
    Equal,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

fn token(kind: TokenKind, line: usize, start: usize, end: usize) -> Token {
    Token {
        kind,
        span: Span::new(line, start + 1, end + 1),
    }
}

fn keyword(word: &str) -> Option<TokenKind> {
    Some(match word {
        "fn" => TokenKind::Fn,
        "let" => TokenKind::Let,
        "var" => TokenKind::Var,
        "return" => TokenKind::Return,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "while" => TokenKind::While,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "not" => TokenKind::Not,
        "i16" => TokenKind::I16,
        "i32" => TokenKind::I32,
        "bool" => TokenKind::Bool,
        "void" => TokenKind::Void,
        _ => return None,
    })
}

/// Convert source text into tokens, making layout explicit as INDENT/DEDENT.
pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut indents = vec![0usize];
    let mut nesting = 0usize;
    let mut last_line = 1usize;

    for (zero_line, raw_line) in source.split_inclusive('\n').enumerate() {
        let line_number = zero_line + 1;
        last_line = line_number;
        let line = raw_line
            .strip_suffix('\n')
            .unwrap_or(raw_line)
            .strip_suffix('\r')
            .unwrap_or_else(|| raw_line.strip_suffix('\n').unwrap_or(raw_line));
        let bytes = line.as_bytes();
        let mut index = 0usize;

        if nesting == 0 {
            while index < bytes.len() && bytes[index] == b' ' {
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b'\t' {
                return Err(Diagnostic::new(
                    Span::new(line_number, index + 1, index + 2),
                    "tabs are not allowed in indentation",
                ));
            }
            let blank = index == bytes.len() || bytes[index] == b'#';
            if blank {
                continue;
            }
            match index.cmp(indents.last().expect("indentation stack")) {
                std::cmp::Ordering::Greater => {
                    indents.push(index);
                    tokens.push(token(TokenKind::Indent, line_number, 0, index));
                }
                std::cmp::Ordering::Less => {
                    while index < *indents.last().expect("indentation stack") {
                        indents.pop();
                        tokens.push(token(TokenKind::Dedent, line_number, 0, index));
                    }
                    if index != *indents.last().expect("indentation stack") {
                        return Err(Diagnostic::new(
                            Span::new(line_number, 1, index + 1),
                            "indentation does not match an enclosing block",
                        ));
                    }
                }
                std::cmp::Ordering::Equal => {}
            }
        }

        while index < bytes.len() {
            let start = index;
            match bytes[index] {
                b' ' | b'\t' => {
                    index += 1;
                }
                b'#' => break,
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                    index += 1;
                    while index < bytes.len()
                        && matches!(bytes[index], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
                    {
                        index += 1;
                    }
                    let word = &line[start..index];
                    let kind = keyword(word).unwrap_or_else(|| TokenKind::Identifier(word.into()));
                    tokens.push(token(kind, line_number, start, index));
                }
                b'0'..=b'9' => {
                    index += 1;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                    let spelling = &line[start..index];
                    let value = spelling.parse::<i64>().map_err(|_| {
                        Diagnostic::new(
                            Span::new(line_number, start + 1, index + 1),
                            "integer literal is too large",
                        )
                    })?;
                    tokens.push(token(TokenKind::Integer(value), line_number, start, index));
                }
                b'(' => {
                    nesting += 1;
                    index += 1;
                    tokens.push(token(TokenKind::LeftParen, line_number, start, index));
                }
                b')' => {
                    if nesting == 0 {
                        return Err(Diagnostic::new(
                            Span::new(line_number, start + 1, start + 2),
                            "unmatched ')'",
                        ));
                    }
                    nesting -= 1;
                    index += 1;
                    tokens.push(token(TokenKind::RightParen, line_number, start, index));
                }
                b',' => {
                    index += 1;
                    tokens.push(token(TokenKind::Comma, line_number, start, index));
                }
                b':' => {
                    index += 1;
                    tokens.push(token(TokenKind::Colon, line_number, start, index));
                }
                b'+' => {
                    index += 1;
                    tokens.push(token(TokenKind::Plus, line_number, start, index));
                }
                b'-' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'>' {
                        index += 1;
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                b'*' => {
                    index += 1;
                    tokens.push(token(TokenKind::Star, line_number, start, index));
                }
                b'/' => {
                    index += 1;
                    tokens.push(token(TokenKind::Slash, line_number, start, index));
                }
                b'%' => {
                    index += 1;
                    tokens.push(token(TokenKind::Percent, line_number, start, index));
                }
                b'=' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'=' {
                        index += 1;
                        TokenKind::EqualEqual
                    } else {
                        TokenKind::Equal
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                b'!' if index + 1 < bytes.len() && bytes[index + 1] == b'=' => {
                    index += 2;
                    tokens.push(token(TokenKind::NotEqual, line_number, start, index));
                }
                b'<' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'=' {
                        index += 1;
                        TokenKind::LessEqual
                    } else {
                        TokenKind::Less
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                b'>' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'=' {
                        index += 1;
                        TokenKind::GreaterEqual
                    } else {
                        TokenKind::Greater
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                byte if byte >= 0x80 => {
                    return Err(Diagnostic::new(
                        Span::new(line_number, start + 1, start + 2),
                        "identifiers and operators must be ASCII",
                    ));
                }
                other => {
                    return Err(Diagnostic::new(
                        Span::new(line_number, start + 1, start + 2),
                        format!("unexpected character {:?}", other as char),
                    ));
                }
            }
        }

        if nesting == 0 {
            tokens.push(token(
                TokenKind::Newline,
                line_number,
                bytes.len(),
                bytes.len(),
            ));
        }
    }

    if nesting != 0 {
        return Err(Diagnostic::new(
            Span::new(last_line, 1, 1),
            "unclosed parenthesized expression",
        ));
    }
    while indents.len() > 1 {
        indents.pop();
        tokens.push(token(TokenKind::Dedent, last_line + 1, 0, 0));
    }
    tokens.push(token(TokenKind::Eof, last_line + 1, 0, 0));
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indentation_is_explicit_and_blank_lines_are_invisible() {
        let tokens = lex("if true:\n    work()\n\n    # note\nnext()\n").unwrap();
        let kinds: Vec<_> = tokens.into_iter().map(|one| one.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::If,
                TokenKind::True,
                TokenKind::Colon,
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("work".into()),
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Identifier("next".into()),
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn a_non_enclosing_dedent_is_rejected() {
        let error = lex("if true:\n    work()\n  next()\n").unwrap_err();
        assert_eq!(error.span.line, 3);
        assert!(error.message.contains("does not match"));
    }
}
