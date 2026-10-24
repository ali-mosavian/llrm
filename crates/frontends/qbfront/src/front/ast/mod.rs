//! The QB grammar of `qb.lalrpop` as a recursive-descent parser that builds
//! qbfront's typed syntax (`syntax::Module`) directly, from the tokens of the
//! logos lexer.
//!
//! The grammar's ambiguities stay where the existing parser leaves them for the
//! semantic pass: `name(args)` is an `Apply` whether it names an array or a
//! function; a statement `foo (a), b` is a call whose first argument is `(a)`.
//! What this parser decides is what the syntax decides: blocks, precedence,
//! which statement a line is.

use std::collections::BTreeSet;

use crate::front::classes::*;
use crate::front::kw::Kw;
use crate::front::tokens::{terminal, Tok};
use crate::front::token::{Token, TokenKind};
use crate::syntax::{
    Binary, Bound, CaseItem, Declaration, ExitTarget, Expr, FileAccess, FileLock, FileMode, Literal, Module, Parameter,
    PrintItem, PrintKind, PrintSeparator, Procedure, ProcedureKind, ResumeTarget, Span, Statement, TypeName, Unary,
};
use crate::Dialect;

mod block;
mod cursor;
mod declaration;
mod event;
mod expression;
mod graphics;
mod io;
mod keyword;
mod module;
mod names;

/// The index of the token the parse stopped at.
type R<T> = Result<T, usize>;

/// STEP-relative coordinate pairs wrap their x in this call, as the
/// table-driven parser does.
pub const STEP_MARK: &str = crate::front::quickr::STEP_MARK;
/// ON n GOTO|GOSUB's selector.
pub const ON_SELECTOR: &str = crate::front::quickr::ON_SELECTOR;

/// Which primary an expression may begin with: the grammar's `P`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum First {
    Full,
    Joined,
    NoErr,
}

pub fn parse(
    source: &str,
    dialect: Dialect,
) -> Result<Module, crate::ParseError> {
    let fail = |span, message: &str| crate::ParseError::coded(span, message);
    let tokens = crate::front::scan::scan(source, dialect).map_err(crate::ParseError::from)?;
    let (tokens, volatile_at) = without_volatile(next_lists(tokens));
    // QuickrBASIC's statements are rewritten into ones the parser has: `x += 1`
    // is a call of `$AUG+`
    let (tokens, private_at) = if dialect.format_strings() {
        use crate::front::quickr;
        quickr::without_private(quickr::for_in(quickr::augmented(quickr::tuple_assignment(
            quickr::returned(tokens, dialect),
            dialect,
        ))))
    } else {
        (tokens, BTreeSet::new())
    };
    let mut kinds: Vec<Tok> = tokens.iter().map(|token| terminal(&token.kind)).collect();
    // `FN twice`: FN written apart from the name it prefixes
    for at in 0..tokens.len().saturating_sub(1) {
        if matches!(&tokens[at].kind, TokenKind::Identifier(word) if word == "FN") && kinds[at + 1] == Tok::Ident {
            kinds[at] = Tok::FnWord;
        }
    }
    let mut ast = Ast::over(&tokens, &kinds, dialect);
    ast.volatile_at = volatile_at;
    ast.private_at = private_at;
    ast.module().map_err(|at| {
        let span = tokens.get(at).or(tokens.last()).map_or(Span { line: 1, start: 0, end: 0 }, |token| token.span);
        let mut error = fail(span, ast.reason.unwrap_or("Syntax error"));
        // what the parser wanted where it stopped, when it knew
        error.expected =
            ast.expected.filter(|(index, _)| *index == at).map(|(_, wanted)| crate::front::explain::describe(wanted));
        error
    })
}

/// `NEXT i, j` closes two loops: it is read as `NEXT i : NEXT j`.
fn next_lists(tokens: Vec<Token>) -> Vec<Token> {
    let reserved = |token: &Token, word: Kw| token.kind == TokenKind::Reserved(word);
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        out.push(token.clone());
        at += 1;
        if reserved(token, Kw::NEXT) {
            // NEXT ident , ident , …
            while let (
                Some(name @ Token { kind: TokenKind::Identifier(_), .. }),
                Some(comma),
                Some(Token { kind: TokenKind::Identifier(_), .. }),
            ) = (tokens.get(at), tokens.get(at + 1), tokens.get(at + 2))
            {
                if !reserved(comma, Kw::Comma) {
                    break;
                }
                out.push(name.clone());
                out.push(Token { kind: TokenKind::Reserved(Kw::Colon), span: comma.span });
                out.push(Token { kind: token.kind.clone(), span: token.span });
                at += 2;
            }
        }
    }
    out
}

/// `ID AS VOLATILE TYPE`: the grammar parses the declaration without the word,
/// so VOLATILE leaves the stream and its declaration's identifier is recorded
/// by line and start. `BYREF` before a parameter, the default said aloud, goes
/// too.
fn without_volatile(tokens: Vec<Token>) -> (Vec<Token>, BTreeSet<(usize, usize)>) {
    let reserved = |token: Option<&Token>, word: Kw| token.is_some_and(|token| token.kind == TokenKind::Reserved(word));
    let mut kept: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut marked = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        let word = |name: &str| matches!(&token.kind, TokenKind::Identifier(word) if word == name);
        if word("BYREF")
            && (reserved(kept.last(), Kw::LParen) || reserved(kept.last(), Kw::Comma))
            && matches!(
                tokens.get(index + 1).map(|next| &next.kind),
                Some(TokenKind::Identifier(_))
            )
        {
            continue;
        }
        let types = |next: Option<&Token>| {
            next.is_some_and(|next| {
                matches!(next.kind, TokenKind::Identifier(_))
                    || matches!(
                        next.kind,
                        TokenKind::Reserved(word) if ![Kw::NewLine, Kw::Colon, Kw::Comma, Kw::RParen].contains(&word)
                    )
            })
        };
        if word("VOLATILE") && reserved(kept.last(), Kw::AS) && types(tokens.get(index + 1)) {
            let mut at = kept.len() - 1;
            if reserved(at.checked_sub(1).and_then(|one| kept.get(one)), Kw::RParen) {
                let mut depth = 0;
                while at > 0 {
                    at -= 1;
                    if reserved(kept.get(at), Kw::RParen) {
                        depth += 1;
                    } else if reserved(kept.get(at), Kw::LParen) {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
            }
            if let Some(Token { kind: TokenKind::Identifier(_), span }) =
                at.checked_sub(1).and_then(|one| kept.get(one))
            {
                marked.insert((span.line, span.start));
                continue;
            }
        }
        kept.push(token.clone());
    }
    (kept, marked)
}

struct Ast<'a> {
    tokens: &'a [Token],
    kinds: &'a [Tok],
    i: usize,
    /// A primary already read, for `foo (a) * b`: the next `prim` returns it.
    injected: Option<Expr>,
    volatile_at: BTreeSet<(usize, usize)>,
    /// The SUB and FUNCTION tokens that PRIVATE came before (QuickrBASIC).
    private_at: BTreeSet<usize>,
    dialect: Dialect,
    /// `'$DYNAMIC` is in force.
    dynamic_arrays: bool,
    /// A line number labelled the statement after it: a `:` stands between
    /// them.
    colon_pending: bool,
    /// The statements read so far, of the module or of the open procedure.
    out: Vec<Statement>,
    procedures: Vec<Procedure>,
    open_procedure: Option<usize>,
    /// The token index and the token `expect` last looked for and did not find.
    expected: Option<(usize, Kw)>,
    /// BC's name for why the parse stopped, where it is not a plain syntax
    /// error.
    reason: Option<&'static str>,
    /// A QuickrBASIC slice or format string was read: the program needs the
    /// prelude.
    prelude: bool,
}

impl<'a> Ast<'a> {
    /// A parser over `tokens`, whose terminals are `kinds`.
    fn over(
        tokens: &'a [Token],
        kinds: &'a [Tok],
        dialect: Dialect,
    ) -> Self {
        Self {
            tokens,
            kinds,
            i: 0,
            injected: None,
            volatile_at: BTreeSet::new(),
            private_at: BTreeSet::new(),
            dialect,
            dynamic_arrays: false,
            colon_pending: false,
            out: Vec::new(),
            procedures: Vec::new(),
            open_procedure: None,
            expected: None,
            reason: None,
            prelude: false,
        }
    }
}

fn span_of(
    a: Span,
    b: Span,
) -> Span {
    Span { line: a.line, start: a.start, end: b.end }
}

/// `A.B.C` as one name, where the expression is nothing but names and fields.
fn dotted_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Field { base, name, .. } => Some(format!("{}.{name}", dotted_name(base)?)),
        Expr::Name(name, _) => Some(name.clone()),
        _ => None,
    }
}

/// The type a real literal's suffix gives it.
fn without_type_character(name: &str) -> bool {
    !name.ends_with(['%', '&', '!', '#', '$', '@'])
}

fn real_type(suffix: Option<char>) -> TypeName {
    match suffix {
        Some('#') => TypeName::Double,
        Some('@') => TypeName::Currency,
        _ => TypeName::Single,
    }
}

fn name_of_suffix(name: &str) -> Option<TypeName> {
    match name.as_bytes().last() {
        Some(b'%') => Some(TypeName::Integer),
        Some(b'&') => Some(TypeName::Long),
        Some(b'!') => Some(TypeName::Single),
        Some(b'#') => Some(TypeName::Double),
        Some(b'$') => Some(TypeName::String),
        Some(b'@') => Some(TypeName::Currency),
        _ => None,
    }
}

/// The words that stay reserved where a name stands; every other word is a name
/// there.
fn contextual_reserved_name(word: Kw) -> Option<String> {
    let spelling = word.spelling().to_ascii_uppercase();
    if !spelling.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        || matches!(
            spelling.as_str(),
            "ALIAS"
                | "AND"
                | "AS"
                | "BYVAL"
                | "CALL"
                | "CASE"
                | "CDECL"
                | "CONST"
                | "DECLARE"
                | "DIM"
                | "DO"
                | "ELSE"
                | "ELSEIF"
                | "END"
                | "EQV"
                | "EXIT"
                | "FOR"
                | "FUNCTION"
                | "GOTO"
                | "IF"
                | "IMP"
                | "LET"
                | "LOOP"
                | "MOD"
                | "NEXT"
                | "NOT"
                | "OFF"
                | "ON"
                | "OPTION"
                | "OR"
                | "REM"
                | "SEG"
                | "SELECT"
                | "SHARED"
                | "STATIC"
                | "STEP"
                | "STOP"
                | "SUB"
                | "THEN"
                | "TO"
                | "TYPE"
                | "UNTIL"
                | "USING"
                | "WEND"
                | "WHILE"
                | "XOR"
        )
    {
        None
    } else {
        Some(spelling)
    }
}

fn statement_end(statement: &Statement) -> usize {
    match statement {
        Statement::Dim(items) | Statement::Static(items) | Statement::Shared(items) | Statement::Redim(items) => {
            items.last().map_or(0, |item| item.span.end)
        }
        Statement::Erase(items) => items.last().map_or(0, |item| item.span().end),
        other => other.span().end,
    }
}
