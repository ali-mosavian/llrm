//! Spike: LALR(1) parsers for Nib and the QB family.

pub mod ast;
pub mod nib_tokens;

#[allow(clippy::all, unused)]
pub mod nib {
    lalrpop_util::lalrpop_mod!(pub grammar, "/nib.rs");
}

use llrm_nib::lexer::{Token, lex};

/// A syntax error of the LALR parser, as the index of the token it stopped at.
pub fn parse_nib(tokens: &[Token]) -> Result<ast::Module, String> {
    nib::grammar::ModuleParser::new()
        .parse(nib_tokens::tokens(tokens))
        .map_err(|error| {
            let at = |location: usize| tokens.get(location).map(|t| (t.span.line, t.span.column, &t.kind));
            match error {
                lalrpop_util::ParseError::UnrecognizedToken { token: (start, ..), expected } => {
                    format!("unexpected {:?}, expected one of {}", at(start), expected.len())
                }
                lalrpop_util::ParseError::UnrecognizedEof { location, .. } => format!("eof at {:?}", at(location)),
                other => format!("{other:?}"),
            }
        })
}

pub fn parse_nib_source(source: &str) -> Result<ast::Module, String> {
    let tokens = lex(source).map_err(|d| format!("lex: {d:?}"))?;
    parse_nib(&tokens)
}

pub mod qb_ast;
pub mod qb_classes;
pub mod qb_kw;
pub mod qb_rd;
pub mod qb_lexer;
pub mod qb_words;
pub mod qb_tokens;
pub mod qb_tree;

#[allow(clippy::all, unused)]
pub mod qb {
    lalrpop_util::lalrpop_mod!(pub grammar, "/qb.rs");
}

type QbError = lalrpop_util::ParseError<usize, qb_tokens::Tok, qb_tokens::Failure>;

/// The QB token stream parsed to a generic tree, or where it stopped.
pub fn parse_qb(tokens: &[qbfront::generated_parser::Token]) -> Result<qb_tree::Tree, String> {
    qb::grammar::ProgramParser::new().parse(qb_tokens::tokens(tokens)).map_err(|error| qb_error(tokens, error))
}

/// A QB source parsed to a generic tree with the tokens pulled from the lexer as the parser
/// needs them, kinds only. `spans` has each token's span, indexed as the tree's `At` nodes.
pub fn parse_qb_stream(
    source: &str,
    dialect: qbfront::Dialect,
    spans: &mut Vec<qbfront::syntax::Span>,
) -> Result<qb_tree::Tree, String> {
    let lines = qb_lexer::Lines::new(source, dialect).ok_or("dialect is lexed by the old lexer")?;
    let result = qb::grammar::ProgramParser::new().parse(lines.stream(spans));
    result.map_err(|error| match error {
        lalrpop_util::ParseError::UnrecognizedToken { token: (at, token, _), expected } => {
            let span = spans.get(at).map_or_else(String::new, |span| format!("{}:{} ", span.line, span.start));
            format!("{span}unexpected {token:?}, expected {}", expected.join(" "))
        }
        lalrpop_util::ParseError::UnrecognizedEof { expected, .. } => format!("eof, expected {}", expected.join(" ")),
        lalrpop_util::ParseError::User { error } => format!("lex: {}", error.message),
        other => format!("{other:?}"),
    })
}

/// The recursive-descent parser fed a line at a time: the lexer fills one reused buffer up to a
/// newline, the parser reads that line, the buffer is cleared. No token vector, no spans.
pub fn parse_qb_rd_by_line(source: &str, dialect: qbfront::Dialect) -> Result<qb_tree::Tree, String> {
    let lines = qb_lexer::Lines::new(source, dialect).ok_or("dialect is lexed by the old lexer")?;
    let mut buffer: Vec<qb_tokens::Tok> = Vec::with_capacity(256);
    let (mut base, mut trees) = (0, Vec::new());
    for item in lines.stream_bare() {
        let (_, tok, _) = item.map_err(|error| format!("lex: {}", error.message))?;
        let end = tok == qb_tokens::Tok::Kw(qb_kw::Kw::NewLine);
        buffer.push(tok);
        if end {
            trees.push(qb_rd::parse_line(&buffer, base).map_err(|at| format!("token {}", base + at))?);
            base += buffer.len();
            buffer.clear();
        }
    }
    Ok(qb_tree::Tree::List(trees))
}

/// A parse of kinds already in a vector, for comparing parsers on tokens no source spells.
pub fn parse_qb_kinds(kinds: &[qb_tokens::Tok]) -> Result<qb_tree::Tree, usize> {
    let tokens = kinds.iter().enumerate().map(|(at, tok)| Ok((at, tok.clone(), at + 1)));
    qb::grammar::ProgramParser::new().parse(tokens).map_err(|error| match error {
        lalrpop_util::ParseError::UnrecognizedToken { token: (at, ..), .. } => at,
        lalrpop_util::ParseError::UnrecognizedEof { .. } => kinds.len(),
        _ => 0,
    })
}

fn qb_error(tokens: &[qbfront::generated_parser::Token], error: QbError) -> String {
    match error {
        lalrpop_util::ParseError::UnrecognizedToken { token: (at, ..), expected } => {
            let token = &tokens[at];
            format!("{}:{} unexpected {:?}, expected {}", token.span.line, token.span.start, token.kind, expected.join(" "))
        }
        lalrpop_util::ParseError::UnrecognizedEof { expected, .. } => format!("eof, expected {}", expected.join(" ")),
        other => format!("{other:?}"),
    }
}

/// A DOS source file as text: no byte order mark, and no Ctrl-Z, which ends a
/// DOS text file and which QB ignores.
pub fn dos_source(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text: Vec<u8> = bytes.iter().copied().filter(|&byte| byte != 0x1A).collect();
    String::from_utf8_lossy(&text).into_owned()
}
