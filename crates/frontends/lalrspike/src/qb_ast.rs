//! The QB grammar of `qb.lalrpop` as a recursive-descent parser that builds qbfront's
//! typed syntax (`syntax::Module`) directly, from the tokens of the logos lexer.
//!
//! The grammar's ambiguities stay where the existing parser leaves them for the semantic
//! pass: `name(args)` is an `Apply` whether it names an array or a function; a statement
//! `foo (a), b` is a call whose first argument is `(a)`. What this parser decides is what
//! the syntax decides: blocks, precedence, which statement a line is.

use std::collections::BTreeSet;

use qbfront::generated_parser::tables;
use qbfront::generated_parser::{Token, TokenKind};
use qbfront::syntax::{
    Binary, Bound, CaseItem, Declaration, Expr, ExitTarget, FileAccess, FileLock, FileMode, Literal, Module, Parameter, PrintItem,
    PrintKind, PrintSeparator, Procedure, ProcedureKind, ResumeTarget, Span, Statement, TypeName, Unary,
};
use qbfront::Dialect;

use crate::qb_classes::*;
use crate::qb_kw::Kw;
use crate::qb_tokens::{terminal, Tok};

/// The index of the token the parse stopped at.
type R<T> = Result<T, usize>;

/// STEP-relative coordinate pairs wrap their x in this call, as the table-driven parser does.
pub const STEP_MARK: &str = qbfront::generated_parser::STEP_MARK;
/// ON n GOTO|GOSUB's selector.
pub const ON_SELECTOR: &str = qbfront::generated_parser::ON_SELECTOR;

/// Which primary an expression may begin with: the grammar's `P`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum First {
    Full,
    NoParen,
    Joined,
    NoErr,
}

pub fn parse(source: &str, dialect: Dialect) -> Result<Module, qbfront::ParseError> {
    let fail = |span, message: &str| qbfront::ParseError { span, message: message.into() };
    if dialect.format_strings() {
        return Err(fail(Span { line: 1, start: 0, end: 0 }, "QuickrBASIC extensions are not in this parser"));
    }
    let tokens = crate::qb_scan::scan(source, dialect).map_err(qbfront::ParseError::from)?;
    let (tokens, volatile_at) = without_volatile(next_lists(tokens));
    let mut kinds: Vec<Tok> = tokens.iter().map(|token| terminal(&token.kind)).collect();
    // `FN twice`: FN written apart from the name it prefixes
    for at in 0..tokens.len().saturating_sub(1) {
        if matches!(&tokens[at].kind, TokenKind::Identifier(word) if word == "FN") && kinds[at + 1] == Tok::Ident {
            kinds[at] = Tok::FnWord;
        }
    }
    let mut ast = Ast {
        tokens: &tokens,
        kinds: &kinds,
        i: 0,
        injected: None,
        volatile_at,
        dialect,
        dynamic_arrays: false,
        colon_pending: false,
        out: Vec::new(),
        procedures: Vec::new(),
        open_procedure: None,
    };
    ast.module().map_err(|at| {
        let span = tokens.get(at).or(tokens.last()).map_or(Span { line: 1, start: 0, end: 0 }, |token| token.span);
        fail(span, "syntax error")
    })
}

/// `NEXT i, j` closes two loops: it is read as `NEXT i : NEXT j`.
fn next_lists(tokens: Vec<Token>) -> Vec<Token> {
    let reserved = |token: &Token, name: &str| matches!(&token.kind, TokenKind::Reserved(id) if tables::token_id(name) == Some(*id));
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        out.push(token.clone());
        at += 1;
        if reserved(token, "tkNEXT") {
            // NEXT ident , ident , …
            while let (Some(name @ Token { kind: TokenKind::Identifier(_), .. }), Some(comma), Some(Token { kind: TokenKind::Identifier(_), .. })) =
                (tokens.get(at), tokens.get(at + 1), tokens.get(at + 2))
            {
                if !reserved(comma, "tkComma") {
                    break;
                }
                out.push(name.clone());
                out.push(Token { kind: TokenKind::Reserved(tables::token_id("tkColon").unwrap()), span: comma.span });
                out.push(Token { kind: token.kind.clone(), span: token.span });
                at += 2;
            }
        }
    }
    out
}

/// `ID AS VOLATILE TYPE`: the grammar parses the declaration without the word, so VOLATILE
/// leaves the stream and its declaration's identifier is recorded by line and start. `BYREF`
/// before a parameter, the default said aloud, goes too.
fn without_volatile(tokens: Vec<Token>) -> (Vec<Token>, BTreeSet<(usize, usize)>) {
    let reserved = |token: Option<&Token>, name: &str| {
        token.is_some_and(|token| matches!(&token.kind, TokenKind::Reserved(id) if tables::token_id(name) == Some(*id)))
    };
    let mut kept: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut marked = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        let word = |name: &str| matches!(&token.kind, TokenKind::Identifier(word) if word == name);
        if word("BYREF")
            && (reserved(kept.last(), "tkLParen") || reserved(kept.last(), "tkComma"))
            && matches!(tokens.get(index + 1).map(|next| &next.kind), Some(TokenKind::Identifier(_)))
        {
            continue;
        }
        let types = |next: Option<&Token>| {
            next.is_some_and(|next| {
                matches!(next.kind, TokenKind::Identifier(_))
                    || matches!(next.kind, TokenKind::Reserved(id) if ["tkNewLine", "tkColon", "tkComma", "tkRParen"]
                        .iter()
                        .all(|name| tables::token_id(name) != Some(id)))
            })
        };
        if word("VOLATILE") && reserved(kept.last(), "tkAS") && types(tokens.get(index + 1)) {
            let mut at = kept.len() - 1;
            if reserved(at.checked_sub(1).and_then(|one| kept.get(one)), "tkRParen") {
                let mut depth = 0;
                while at > 0 {
                    at -= 1;
                    if reserved(kept.get(at), "tkRParen") {
                        depth += 1;
                    } else if reserved(kept.get(at), "tkLParen") {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
            }
            if let Some(Token { kind: TokenKind::Identifier(_), span }) = at.checked_sub(1).and_then(|one| kept.get(one))
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
    dialect: Dialect,
    /// `'$DYNAMIC` is in force.
    dynamic_arrays: bool,
    /// A line number labelled the statement after it: a `:` stands between them.
    colon_pending: bool,
    /// The statements read so far, of the module or of the open procedure.
    out: Vec<Statement>,
    procedures: Vec<Procedure>,
    open_procedure: Option<usize>,
}

fn span_of(a: Span, b: Span) -> Span {
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

impl Ast<'_> {
    // ---- tokens ------------------------------------------------------------------------

    fn kw_n(&self, n: usize) -> Option<Kw> {
        match self.kinds.get(self.i + n) {
            Some(Tok::Kw(k)) => Some(*k),
            _ => None,
        }
    }

    fn kw(&self) -> Option<Kw> {
        self.kw_n(0)
    }

    fn is(&self, k: Kw) -> bool {
        self.kw() == Some(k)
    }

    fn is_n(&self, n: usize, k: Kw) -> bool {
        self.kw_n(n) == Some(k)
    }

    fn tok_n(&self, n: usize) -> Option<&Tok> {
        self.kinds.get(self.i + n)
    }

    fn period_n(&self, n: usize) -> bool {
        matches!(self.tok_n(n), Some(Tok::Period))
    }

    fn eat(&mut self, k: Kw) -> bool {
        let found = self.is(k);
        self.i += usize::from(found);
        found
    }

    fn expect(&mut self, k: Kw) -> R<()> {
        if self.eat(k) { Ok(()) } else { Err(self.i) }
    }

    fn fail<T>(&self) -> R<T> {
        Err(self.i)
    }

    fn span(&self, at: usize) -> Span {
        self.tokens[at].span
    }

    /// The span of the token last read.
    fn previous(&self) -> Span {
        self.tokens[self.i - 1].span
    }

    fn previous_end(&self) -> usize {
        self.tokens.get(self.i.wrapping_sub(1)).map_or(0, |token| token.span.end)
    }

    /// Take the reserved word at the cursor: its token index.
    fn take(&mut self) -> usize {
        self.i += 1;
        self.i - 1
    }

    fn identifier(&mut self) -> R<(String, Span)> {
        match &self.tokens.get(self.i).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => {
                let out = (name.clone(), self.span(self.i));
                self.i += 1;
                Ok(out)
            }
            _ => self.fail(),
        }
    }

    /// The name a word spells where a name stands: an identifier, or a reserved word that is
    /// no structural keyword.
    fn name_at(&self, at: usize) -> Option<String> {
        match &self.tokens.get(at)?.kind {
            TokenKind::Identifier(name) => Some(name.clone()),
            TokenKind::Reserved(id) => contextual_reserved_name(*id),
            _ => None,
        }
    }

    /// The name of a function word or a contextual one.
    fn word_name(&self, at: usize) -> Option<String> {
        match &self.tokens.get(at)?.kind {
            TokenKind::Reserved(id) => {
                if tables::T_FUNC_DISPATCH.iter().any(|(function, _)| function == id) {
                    tables::token_spelling(*id).map(str::to_ascii_uppercase)
                } else {
                    contextual_reserved_name(*id)
                }
            }
            _ => self.name_at(at),
        }
    }

    fn lab_ln(&mut self) -> R<(String, Span)> {
        match &self.tokens.get(self.i).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => {
                let out = (name.clone(), self.span(self.i));
                self.i += 1;
                Ok(out)
            }
            Some(TokenKind::Integer(value, None)) if (0..=65_529).contains(value) => {
                let out = (value.to_string(), self.span(self.i));
                self.i += 1;
                Ok(out)
            }
            _ => self.fail(),
        }
    }

    // ---- expressions --------------------------------------------------------------------

    fn exp(&mut self) -> R<Expr> {
        self.imp(First::Full)
    }

    fn binary(op: Binary, left: Expr, right: Expr) -> Expr {
        let span = span_of(left.span(), right.span());
        Expr::Binary { op, left: Box::new(left), right: Box::new(right), span }
    }

    fn imp(&mut self, f: First) -> R<Expr> {
        let mut l = self.eqv(f)?;
        while self.eat(Kw::IMP) {
            let r = self.eqv(First::Full)?;
            l = Self::binary(Binary::Imp, l, r);
        }
        Ok(l)
    }

    fn eqv(&mut self, f: First) -> R<Expr> {
        let mut l = self.xor(f)?;
        while self.eat(Kw::EQV) {
            let r = self.xor(First::Full)?;
            l = Self::binary(Binary::Eqv, l, r);
        }
        Ok(l)
    }

    fn xor(&mut self, f: First) -> R<Expr> {
        let mut l = self.or(f)?;
        while self.eat(Kw::XOR) {
            let r = self.or(First::Full)?;
            l = Self::binary(Binary::Xor, l, r);
        }
        Ok(l)
    }

    fn or(&mut self, f: First) -> R<Expr> {
        let mut l = self.and(f)?;
        while self.eat(Kw::OR) {
            let r = self.and(First::Full)?;
            l = Self::binary(Binary::Or, l, r);
        }
        Ok(l)
    }

    fn and(&mut self, f: First) -> R<Expr> {
        let mut l = self.not(f)?;
        while self.eat(Kw::AND) {
            let r = self.not(First::Full)?;
            l = Self::binary(Binary::And, l, r);
        }
        Ok(l)
    }

    fn not(&mut self, f: First) -> R<Expr> {
        if f != First::Joined && self.injected.is_none() && self.is(Kw::NOT) {
            let at = self.take();
            let operand = self.not(First::Full)?;
            let span = span_of(self.span(at), operand.span());
            return Ok(Expr::Unary { op: Unary::Not, operand: Box::new(operand), span });
        }
        self.rel(f)
    }

    fn rel_op(&self) -> Option<Binary> {
        match self.tok_n(0)? {
            Tok::Le => Some(Binary::LessEqual),
            Tok::Ne => Some(Binary::NotEqual),
            Tok::Ge => Some(Binary::GreaterEqual),
            Tok::Kw(Kw::EQ) => Some(Binary::Eq),
            Tok::Kw(Kw::LT) => Some(Binary::Less),
            Tok::Kw(Kw::GT) => Some(Binary::Greater),
            _ => None,
        }
    }

    fn rel(&mut self, f: First) -> R<Expr> {
        let mut l = self.add(f)?;
        while let Some(op) = self.rel_op() {
            self.i += 1;
            let r = self.add(First::Full)?;
            l = Self::binary(op, l, r);
        }
        Ok(l)
    }

    fn add(&mut self, f: First) -> R<Expr> {
        let mut l = self.modulo(f)?;
        loop {
            let op = match self.kw() {
                Some(Kw::Add) => Binary::Add,
                Some(Kw::Minus) => Binary::Subtract,
                _ => return Ok(l),
            };
            self.i += 1;
            let r = self.modulo(First::Full)?;
            l = Self::binary(op, l, r);
        }
    }

    fn modulo(&mut self, f: First) -> R<Expr> {
        let mut l = self.idiv(f)?;
        while self.eat(Kw::MOD) {
            let r = self.idiv(First::Full)?;
            l = Self::binary(Binary::Modulo, l, r);
        }
        Ok(l)
    }

    fn idiv(&mut self, f: First) -> R<Expr> {
        let mut l = self.mul(f)?;
        while self.eat(Kw::Idiv) {
            let r = self.mul(First::Full)?;
            l = Self::binary(Binary::IntegerDivide, l, r);
        }
        Ok(l)
    }

    fn mul(&mut self, f: First) -> R<Expr> {
        let mut l = self.neg(f)?;
        loop {
            let op = match self.kw() {
                Some(Kw::Mult) => Binary::Multiply,
                Some(Kw::Div) => Binary::Divide,
                _ => return Ok(l),
            };
            self.i += 1;
            let r = self.neg(First::Full)?;
            l = Self::binary(op, l, r);
        }
    }

    fn sign(&self) -> Option<Unary> {
        match self.kw() {
            Some(Kw::Add) => Some(Unary::Positive),
            Some(Kw::Minus) => Some(Unary::Negative),
            _ => None,
        }
    }

    fn neg(&mut self, f: First) -> R<Expr> {
        if f != First::Joined && self.injected.is_none() {
            if let Some(op) = self.sign() {
                let at = self.take();
                let operand = self.neg(First::Full)?;
                let span = span_of(self.span(at), operand.span());
                return Ok(Expr::Unary { op, operand: Box::new(operand), span });
            }
        }
        self.pow(f)
    }

    fn pow(&mut self, f: First) -> R<Expr> {
        let mut l = self.prim(f)?;
        while self.eat(Kw::Pwr) {
            let r = self.pow_operand()?;
            l = Self::binary(Binary::Power, l, r);
        }
        Ok(l)
    }

    fn pow_operand(&mut self) -> R<Expr> {
        if let Some(op) = self.sign() {
            let at = self.take();
            let operand = self.pow_operand()?;
            let span = span_of(self.span(at), operand.span());
            return Ok(Expr::Unary { op, operand: Box::new(operand), span });
        }
        self.prim(First::Full)
    }

    fn prim(&mut self, f: First) -> R<Expr> {
        if let Some(expr) = self.injected.take() {
            return Ok(expr);
        }
        if self.is(Kw::LParen) {
            if !matches!(f, First::Full | First::NoErr) {
                return self.fail();
            }
            let open = self.take();
            let value = self.exp()?;
            self.expect(Kw::RParen)?;
            // Grouping only changes meaning around a reference.
            return Ok(match value {
                Expr::Name(..) | Expr::Apply { .. } | Expr::Index { .. } | Expr::Field { .. } => Expr::Unary {
                    op: Unary::Grouped,
                    span: Span { line: self.span(open).line, start: self.span(open).start, end: self.previous_end() },
                    operand: Box::new(value),
                },
                value => value,
            });
        }
        self.prim_base(f)
    }

    fn literal(&mut self) -> R<Expr> {
        let token = &self.tokens[self.i];
        let span = token.span;
        let expr = match &token.kind {
            TokenKind::Integer(value, suffix) => {
                Expr::Literal(Literal::Integer(*value, if *suffix == Some('&') { TypeName::Long } else { TypeName::Integer }), span)
            }
            TokenKind::Real(value, suffix) => Expr::Literal(Literal::Real(value.clone(), real_type(*suffix)), span),
            TokenKind::String(value) => Expr::Literal(Literal::String(value.clone()), span),
            _ => return self.fail(),
        };
        self.i += 1;
        Ok(expr)
    }

    fn prim_base(&mut self, f: First) -> R<Expr> {
        let k = match self.tok_n(0) {
            Some(Tok::Int | Tok::Real | Tok::Str) => return self.literal(),
            Some(Tok::Ident | Tok::FnWord) => return self.name_ref(),
            Some(Tok::Kw(k)) => *k,
            _ => return self.fail(),
        };
        let c = class(k);
        if c & OBJ_WORD != 0 && self.period_n(1) {
            return self.name_ref();
        }
        if k == Kw::MOD && self.period_n(1) {
            if !matches!(f, First::Full | First::NoParen) {
                return self.fail();
            }
            let at = self.take();
            let base = Expr::Name("MOD".into(), self.span(at));
            self.i += 1;
            let (name, field_span) = self.field_name()?;
            let span = span_of(base.span(), field_span);
            return Ok(Expr::Field { base: Box::new(base), name, span });
        }
        if k == Kw::RND {
            let at = self.take();
            let name = "RND".to_owned();
            if self.eat(Kw::LParen) {
                let argument = self.exp()?;
                self.expect(Kw::RParen)?;
                let span = Span { line: self.span(at).line, start: self.span(at).start, end: self.previous_end() };
                return Ok(Expr::Apply { name, arguments: vec![argument], span });
            }
            return Ok(Expr::Name(name, self.span(at)));
        }
        if self.is_n(1, Kw::LParen) {
            if let Some(expr) = self.function(k, c)? {
                return Ok(expr);
            }
        }
        if c & FN0 != 0 || c & (FN_WORD | CTX_WORD) != 0 {
            let at = self.take();
            let name = self.word_name(at).ok_or(at)?;
            return Ok(Expr::Name(name, self.span(at)));
        }
        if k == Kw::ERROR && f != First::NoErr {
            let at = self.take();
            return Ok(Expr::Name("ERROR".into(), self.span(at)));
        }
        self.fail()
    }

    /// A function word before a parenthesis: `None` where the word is no function of that shape.
    fn function(&mut self, k: Kw, c: u16) -> R<Option<Expr>> {
        let at = self.i;
        let shaped = c & (FN1 | FN2 | FN23 | FN_BOUND | FN_REF) != 0
            || matches!(k, Kw::POINT | Kw::ERROR | Kw::INPUT_ | Kw::IOCTL_);
        if !shaped {
            return Ok(None);
        }
        let mut name = self.word_name(at).ok_or(at)?;
        if name.eq_ignore_ascii_case("VARPTR$") {
            name = "$VARPTR".to_owned();
        }
        self.i += 2;
        let mut arguments = Vec::new();
        if c & (FN_BOUND | FN_REF) != 0 {
            arguments.push(self.name_ref()?);
            if c & FN_BOUND != 0 && self.eat(Kw::Comma) {
                arguments.push(self.exp()?);
            }
        } else if matches!(k, Kw::IOCTL_) {
            self.eat(Kw::Lbs);
            arguments.push(self.exp()?);
        } else {
            arguments.push(self.exp()?);
            while self.eat(Kw::Comma) {
                if k == Kw::INPUT_ {
                    self.eat(Kw::Lbs);
                }
                arguments.push(self.exp()?);
            }
        }
        self.expect(Kw::RParen)?;
        let span = Span { line: self.span(at).line, start: self.span(at).start, end: self.previous_end() };
        Ok(Some(Expr::Apply { name, arguments, span }))
    }

    fn field_name(&mut self) -> R<(String, Span)> {
        let at = self.i;
        match self.tok_n(0) {
            Some(Tok::Ident) | Some(Tok::Kw(_)) => {
                let name = self.name_at(at).ok_or(at)?;
                self.i += 1;
                Ok((name, self.span(at)))
            }
            _ => self.fail(),
        }
    }

    /// The head of a NameRef, and whether it is already an applied form (`name(...)`).
    fn name_base(&mut self) -> R<(Expr, bool)> {
        let at = self.i;
        let fn_word = matches!(self.tok_n(0), Some(Tok::FnWord));
        if fn_word || matches!(self.tok_n(0), Some(Tok::Ident)) {
            let (name, start) = if fn_word {
                // `FN twice`: the name is FNTWICE
                let fn_span = self.span(self.i);
                self.i += 1;
                let (name, ident) = self.identifier()?;
                (format!("FN{name}"), span_of(fn_span, ident))
            } else {
                self.identifier()?
            };
            if self.eat(Kw::LParen) {
                let arguments = self.arguments()?;
                let span = Span { line: start.line, start: start.start, end: self.previous_end() };
                return Ok((Expr::Apply { name, arguments, span }, true));
            }
            return Ok((Expr::Name(name, start), false));
        }
        match self.tok_n(0) {
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                if c & (OBJ_WORD | FN_WORD | CTX_WORD) != 0 {
                    let name = self.word_name(at).ok_or(at)?;
                    self.i += 1;
                    return Ok((Expr::Name(name, self.span(at)), false));
                }
                self.fail()
            }
            _ => self.fail(),
        }
    }

    /// After `(`: the arguments to the `)`.
    fn arguments(&mut self) -> R<Vec<Expr>> {
        let mut values = Vec::new();
        if self.eat(Kw::RParen) {
            return Ok(values);
        }
        loop {
            values.push(self.exp()?);
            if self.eat(Kw::Comma) {
                continue;
            }
            self.expect(Kw::RParen)?;
            return Ok(values);
        }
    }

    /// A NameRef in an expression: every `.field` and `(args)` belongs to it.
    fn name_ref(&mut self) -> R<Expr> {
        let (mut value, _) = self.name_base()?;
        loop {
            if matches!(self.tok_n(0), Some(Tok::Period)) {
                self.i += 1;
                let (name, field) = self.field_name()?;
                let span = span_of(value.span(), field);
                value = Expr::Field { base: Box::new(value), name, span };
            } else if self.is(Kw::LParen) {
                self.i += 1;
                let indices = self.arguments()?;
                let span = Span { line: value.span().line, start: value.span().start, end: self.previous_end() };
                value = match value {
                    Expr::Name(name, _) => Expr::Apply { name, arguments: indices, span },
                    base => Expr::Index { base: Box::new(base), indices, span },
                };
            } else {
                return Ok(value);
            }
        }
    }
}

/// The words that stay reserved where a name stands; every other word is a name there.
fn contextual_reserved_name(id: u16) -> Option<String> {
    let spelling = tables::token_spelling(id)?.to_ascii_uppercase();
    if !spelling.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        || matches!(
            spelling.as_str(),
            "ALIAS" | "AND" | "AS" | "BYVAL" | "CALL" | "CASE" | "CDECL" | "CONST" | "DECLARE" | "DIM" | "DO"
                | "ELSE" | "ELSEIF" | "END" | "EQV" | "EXIT" | "FOR" | "FUNCTION" | "GOTO" | "IF" | "IMP" | "LET"
                | "LOOP" | "MOD" | "NEXT" | "NOT" | "OFF" | "ON" | "OPTION" | "OR" | "REM" | "SEG" | "SELECT"
                | "SHARED" | "STATIC" | "STEP" | "STOP" | "SUB" | "THEN" | "TO" | "TYPE" | "UNTIL" | "USING" | "WEND"
                | "WHILE" | "XOR"
        )
    {
        None
    } else {
        Some(spelling)
    }
}

// ---- the module and its statements --------------------------------------------------------------

impl Ast<'_> {
    fn module(&mut self) -> R<Module> {
        while self.i < self.tokens.len() {
            while self.eat(Kw::NewLine) || self.eat(Kw::Colon) {}
            if self.i >= self.tokens.len() {
                break;
            }
            let before = self.out.len();
            let procedures_before = self.procedures.len();
            let open_before = self.open_procedure;
            self.statement()?;
            self.route(before, procedures_before, open_before);
            if !self.ends_statement() {
                return self.fail();
            }
            self.colon_pending = false;
        }
        if self.open_procedure.is_some() {
            return self.fail();
        }
        Ok(Module { statements: std::mem::take(&mut self.out), procedures: std::mem::take(&mut self.procedures), prelude: false })
    }

    /// A statement read while a procedure is open belongs to its body.
    fn route(&mut self, before: usize, procedures_before: usize, open_before: Option<usize>) {
        if let Some(index) = open_before {
            if self.open_procedure == Some(index) && self.procedures.len() == procedures_before && self.out.len() > before {
                let body = self.out.split_off(before);
                self.procedures[index].body.extend(body);
            }
        }
    }

    fn ends_statement(&self) -> bool {
        self.colon_pending || self.i >= self.tokens.len() || self.is(Kw::NewLine) || self.is(Kw::Colon)
    }

    /// `:` between two statements, or the one a line number stands for.
    fn separator(&mut self) -> bool {
        if self.colon_pending {
            self.colon_pending = false;
            true
        } else {
            self.eat(Kw::Colon)
        }
    }

    fn skip_separators(&mut self) {
        while self.eat(Kw::NewLine) || self.separator() {}
    }

    fn statement(&mut self) -> R<()> {
        let at = self.i;
        match &self.tokens[at].kind {
            TokenKind::Integer(value, None) => {
                if !(0..=65_529).contains(value) {
                    return self.fail();
                }
                self.out.push(Statement::Label(value.to_string(), self.span(at)));
                self.i += 1;
                if !(self.is(Kw::NewLine) || self.is(Kw::Colon)) {
                    self.colon_pending = true;
                }
                return Ok(());
            }
            TokenKind::ArrayDynamic | TokenKind::ArrayStatic => {
                self.dynamic_arrays = matches!(self.tokens[at].kind, TokenKind::ArrayDynamic);
                self.out.push(Statement::Comment(self.span(at)));
                self.i += 1;
                return Ok(());
            }
            _ => {}
        }
        match self.kinds[at].clone() {
            Tok::Ident | Tok::FnWord => self.name_statement(),
            Tok::Kw(k) => self.keyword_statement(k),
            Tok::Ext => self.extension_statement(),
            _ => self.fail(),
        }
    }

    /// `DEFBYTE` and the `DEFU*` statements: extension words before letter ranges.
    fn extension_statement(&mut self) -> R<()> {
        let TokenKind::ExtensionKeyword(word) = self.tokens[self.i].kind else { return self.fail() };
        let (width, signed) = match word {
            "DEFBYTE" => (1, true),
            "DEFUBYTE" => (1, false),
            "DEFUINT" => (2, false),
            "DEFULNG" => (4, false),
            _ => return self.fail(),
        };
        self.def_type_of(TypeName::Integral { width, signed })
    }

    fn line_start(&self) -> bool {
        self.i == 0 || self.is_prev_newline()
    }

    fn is_prev_newline(&self) -> bool {
        matches!(self.kinds.get(self.i.wrapping_sub(1)), Some(Tok::Kw(Kw::NewLine)))
    }

    /// A statement that begins with a name: a label, an assignment, or a call.
    fn name_statement(&mut self) -> R<()> {
        let start = self.i;
        let start_span = self.span(start);
        if matches!(self.kinds[start], Tok::Ident) && self.line_start() && self.is_n(1, Kw::Colon) {
            let name = self.identifier()?.0;
            self.out.push(Statement::Label(name, start_span));
            return Ok(());
        }
        if let Ok(target) = self.name_ref() {
            if self.eat(Kw::EQ) {
                let value = self.exp().map_err(|_| self.i)?;
                let span = span_of(start_span, value.span());
                self.out.push(Statement::Assign { target, value, span });
                return Ok(());
            }
            // `Graphics.Init` and `obj.Method a, b`: a call by a dotted name
            if let Some(name) = dotted_name(&target) {
                let arguments = self.call_arguments()?;
                self.out.push(Statement::Call { name, arguments, explicit: false, span: start_span });
                return Ok(());
            }
        }
        self.i = start;
        let (name, span) = match self.kinds[start] {
            Tok::FnWord => return self.fail(),
            _ => self.identifier()?,
        };
        let arguments = self.call_arguments()?;
        self.out.push(Statement::Call { name, arguments, explicit: false, span });
        Ok(())
    }

    fn call_arguments(&mut self) -> R<Vec<Expr>> {
        let mut arguments = Vec::new();
        while !(self.ends_statement() || self.is(Kw::ELSE)) {
            arguments.push(self.exp()?);
            if !self.eat(Kw::Comma) {
                break;
            }
        }
        Ok(arguments)
    }
}

// ---- keyword statements -------------------------------------------------------------------------

impl Ast<'_> {
    fn runtime(&mut self, name: &str, arguments: Vec<Expr>, span: Span) {
        self.out.push(Statement::Runtime { name: name.into(), arguments, span });
    }

    fn implicit_call(&mut self, name: &str, arguments: Vec<Expr>, span: Span) {
        self.out.push(Statement::Call { name: name.into(), arguments, explicit: false, span });
    }

    /// `a, , b,`: an expression where the source gave one, Omitted at the comma that closes an empty slot.
    fn argument_list(&mut self, trailing_ok: bool) -> R<Vec<Expr>> {
        let mut out = Vec::new();
        while !(self.ends_statement() || self.is(Kw::ELSE)) {
            if self.is(Kw::Comma) {
                let at = self.take();
                out.push(Expr::Omitted(self.span(at)));
                continue;
            }
            out.push(self.exp()?);
            if !self.eat(Kw::Comma) {
                break;
            }
            if self.ends_statement() || self.is(Kw::ELSE) {
                out.push(Expr::Omitted(self.previous()));
                break;
            }
        }
        if !trailing_ok && matches!(out.last(), Some(Expr::Omitted(_))) {
            return self.fail();
        }
        Ok(out)
    }

    fn optional_exp(&mut self) -> R<Vec<Expr>> {
        if self.starts_exp() { Ok(vec![self.exp()?]) } else { Ok(Vec::new()) }
    }

    fn starts_exp(&self) -> bool {
        match self.tok_n(0) {
            Some(Tok::Int | Tok::Real | Tok::Str | Tok::Ident | Tok::FnWord) => true,
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                c & (FN0 | FN_WORD | CTX_WORD) != 0
                    || (c & OBJ_WORD != 0 && self.period_n(1))
                    || (c & (FN1 | FN2 | FN23 | FN_BOUND | FN_REF) != 0 && self.is_n(1, Kw::LParen))
                    || matches!(k, Kw::ERROR | Kw::RND | Kw::LParen | Kw::Add | Kw::Minus | Kw::NOT)
                    || (*k == Kw::MOD && self.period_n(1))
            }
            _ => false,
        }
    }

    fn keyword_statement(&mut self, k: Kw) -> R<()> {
        let at = self.i;
        let span = self.span(at);
        let c = class(k);
        let assigned = c & (CTX_WORD | FN_WORD | FN0_VAR) != 0 || k == Kw::ERROR;
        if (assigned && self.is_n(1, Kw::EQ)) || (c & OBJ_WORD != 0 && self.period_n(1)) {
            return self.name_statement();
        }
        match k {
            Kw::REM => {
                self.i += 1;
                self.out.push(Statement::Comment(span));
            }
            Kw::IF => return self.if_statement(),
            Kw::FOR => return self.for_statement(),
            Kw::WHILE => return self.while_statement(),
            Kw::DO => return self.do_statement(),
            Kw::SELECT => return self.select_statement(),
            Kw::DATA => return self.data_statement(),
            Kw::SUB | Kw::FUNCTION => return self.procedure(false, None, false),
            Kw::STATIC if matches!(self.kw_n(1), Some(Kw::SUB | Kw::FUNCTION)) => {
                self.i += 1;
                return self.procedure(false, None, true);
            }
            Kw::DECLARE => return self.declare(),
            Kw::DEF => return self.def(),
            Kw::TYPE => return self.type_statement(),
            Kw::END => return self.end(),
            Kw::DIM | Kw::REDIM | Kw::STATIC | Kw::SHARED => return self.declaration_statement(k),
            Kw::COMMON => return self.common(),
            Kw::CONST => return self.const_statement(),
            Kw::DEFINT | Kw::DEFLNG | Kw::DEFSNG | Kw::DEFDBL | Kw::DEFSTR => return self.def_type(k),
            Kw::OPTION => return self.option(),
            Kw::GOTO => {
                self.i += 1;
                let (label, _) = self.lab_ln()?;
                self.out.push(Statement::Goto(label, span));
            }
            Kw::GOSUB => {
                self.i += 1;
                let (label, label_span) = self.lab_ln()?;
                self.implicit_call("GOSUB", vec![Expr::Name(label, label_span)], span);
            }
            Kw::RETURN => {
                self.i += 1;
                let arguments = if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) {
                    let (label, label_span) = self.lab_ln()?;
                    vec![Expr::Name(label, label_span)]
                } else {
                    Vec::new()
                };
                self.implicit_call("RETURN", arguments, span);
            }
            Kw::RESTORE => {
                self.i += 1;
                let arguments = if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) {
                    let (label, label_span) = self.lab_ln()?;
                    vec![Expr::Name(label, label_span)]
                } else {
                    Vec::new()
                };
                self.runtime("RESTORE", arguments, span);
            }
            Kw::RESUME => {
                self.i += 1;
                let target = if self.eat(Kw::NEXT) {
                    ResumeTarget::Next
                } else if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) {
                    ResumeTarget::Label(self.lab_ln()?.0)
                } else {
                    ResumeTarget::Current
                };
                self.out.push(Statement::Resume { target, span });
            }
            Kw::EXIT => {
                self.i += 1;
                let target = match self.kw() {
                    Some(Kw::SUB) => ExitTarget::Sub,
                    Some(Kw::FUNCTION) => ExitTarget::Function,
                    Some(Kw::DO) => ExitTarget::Do,
                    Some(Kw::FOR) => ExitTarget::For,
                    _ => return self.fail(),
                };
                self.i += 1;
                self.out.push(Statement::Exit(target, span));
            }
            Kw::ON => return self.on(),
            Kw::CALL => {
                self.i += 1;
                let (mut name, _) = self.identifier()?;
                while matches!(self.tok_n(0), Some(Tok::Period)) {
                    self.i += 1;
                    name.push('.');
                    name.push_str(&self.field_name()?.0);
                }
                let mut arguments = Vec::new();
                if self.eat(Kw::LParen) && !self.eat(Kw::RParen) {
                    loop {
                        self.eat(Kw::BYVAL);
                        self.eat(Kw::SEG);
                        arguments.push(self.exp()?);
                        if self.eat(Kw::Comma) {
                            continue;
                        }
                        self.expect(Kw::RParen)?;
                        break;
                    }
                }
                self.out.push(Statement::Call { name, arguments, explicit: true, span });
            }
            Kw::LET => {
                self.i += 1;
                let start = self.i;
                let target = self.name_ref()?;
                self.expect(Kw::EQ)?;
                let value = self.exp()?;
                let begin = self.span(start);
                self.out.push(Statement::Assign { target, span: span_of(begin, value.span()), value });
            }
            Kw::MID_ => {
                self.i += 1;
                self.expect(Kw::LParen)?;
                let mut arguments = vec![self.name_ref()?];
                self.expect(Kw::Comma)?;
                arguments.push(self.exp()?);
                if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                }
                self.expect(Kw::RParen)?;
                self.expect(Kw::EQ)?;
                let value = self.exp()?;
                // The table-driven parser ends the target at the last `)` read, the value's included.
                let right_parenthesis = self.kinds[..self.i]
                    .iter()
                    .rposition(|kind| *kind == Tok::Kw(Kw::RParen))
                    .map_or(value.span().end, |at| self.span(at).end);
                let target_span = Span { line: span.line, start: span.start, end: right_parenthesis };
                let assign_span = Span { line: span.line, start: span.start, end: value.span().end };
                self.out.push(Statement::Assign {
                    target: Expr::Apply { name: "MID$".into(), arguments, span: target_span },
                    span: assign_span,
                    value,
                });
            }
            Kw::LSET | Kw::RSET => {
                self.i += 1;
                let left = self.name_ref()?;
                self.expect(Kw::EQ)?;
                let right = self.exp()?;
                let comparison_span = span_of(left.span(), right.span());
                let name = if k == Kw::LSET { "LSET" } else { "RSET" };
                let comparison = Expr::Binary { op: Binary::Eq, left: Box::new(left), right: Box::new(right), span: comparison_span };
                self.implicit_call(name, vec![comparison], span);
            }
            Kw::SWAP => {
                self.i += 1;
                let a = self.name_ref()?;
                self.expect(Kw::Comma)?;
                let b = self.name_ref()?;
                self.runtime("SWAP", vec![a, b], span);
            }
            Kw::READ => {
                self.i += 1;
                let mut destinations = vec![self.name_ref()?];
                while self.eat(Kw::Comma) {
                    destinations.push(self.name_ref()?);
                }
                self.out.push(Statement::Read { destinations, span });
            }
            Kw::ERASE => {
                self.i += 1;
                let mut names = vec![self.name_ref()?];
                while self.eat(Kw::Comma) {
                    names.push(self.name_ref()?);
                }
                self.out.push(Statement::Erase(names));
            }
            Kw::PRINT | Kw::QMark | Kw::LPRINT | Kw::WRITE => return self.print(if k == Kw::QMark { Kw::PRINT } else { k }),
            Kw::INPUT => return self.input(),
            Kw::LINE => return self.line(),
            Kw::OPEN => return self.open(),
            Kw::CLOSE => {
                self.i += 1;
                let mut files = Vec::new();
                while !self.ends_statement() && !self.is(Kw::ELSE) {
                    self.eat(Kw::Lbs);
                    files.push(self.exp()?);
                    if !self.eat(Kw::Comma) {
                        break;
                    }
                }
                self.out.push(Statement::Close { files, span });
            }
            Kw::GET | Kw::PUT => return self.get_put(k),
            Kw::SEEK => {
                self.i += 1;
                self.eat(Kw::Lbs);
                let file = self.exp()?;
                self.expect(Kw::Comma)?;
                let position = self.exp()?;
                self.out.push(Statement::Seek { file, position, span });
            }
            Kw::DATE_ | Kw::TIME_ => {
                self.i += 1;
                self.expect(Kw::EQ)?;
                let value = self.exp()?;
                self.runtime(if k == Kw::DATE_ { "DATE$" } else { "TIME$" }, vec![value], span);
            }
            _ => return self.simple_statement(k),
        }
        Ok(())
    }
}

// ---- blocks ---------------------------------------------------------------------------------------

fn statement_end(statement: &Statement) -> usize {
    match statement {
        Statement::Dim(items) | Statement::Static(items) | Statement::Shared(items) | Statement::Redim(items) => {
            items.last().map_or(0, |item| item.span.end)
        }
        Statement::Erase(items) => items.last().map_or(0, |item| item.span().end),
        other => other.span().end,
    }
}

impl Ast<'_> {
    /// Statements up to a line `stop` recognises, which is left unread.
    fn collect_until(&mut self, stop: fn(&Self) -> bool) -> R<Vec<Statement>> {
        let mut body = Vec::new();
        loop {
            self.skip_separators();
            if self.i >= self.tokens.len() {
                return self.fail();
            }
            if stop(self) {
                return Ok(body);
            }
            let before = self.out.len();
            self.statement()?;
            body.extend(self.out.split_off(before));
            if !self.ends_statement() {
                return self.fail();
            }
        }
    }

    fn at_end_select(&self) -> bool {
        self.is(Kw::END) && self.is_n(1, Kw::SELECT)
    }

    fn if_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let condition = self.exp()?;
        if self.is(Kw::GOTO) {
            // IF c GOTO label [ELSE …]: the label is the whole THEN branch
            let goto = self.take();
            let (label, _) = self.lab_ln()?;
            let then_branch = vec![Statement::Goto(label, self.span(goto))];
            return self.finish_if_with(condition, self.span(keyword), then_branch);
        }
        self.expect(Kw::THEN)?;
        self.finish_if(condition, self.span(keyword))
    }

    fn finish_if_with(&mut self, condition: Expr, keyword_span: Span, then_branch: Vec<Statement>) -> R<()> {
        let mut else_branch = Vec::new();
        if self.eat(Kw::ELSE) {
            else_branch = self.single_line_branch()?;
        }
        let end = else_branch.last().or_else(|| then_branch.last()).map_or(condition.span().end, statement_end);
        let span = Span { line: condition.span().line, start: keyword_span.start, end };
        self.out.push(Statement::If { condition, then_branch, else_branch, span });
        Ok(())
    }

    fn finish_if(&mut self, condition: Expr, keyword_span: Span) -> R<()> {
        if self.eat(Kw::NewLine) {
            return self.block_if(condition, keyword_span);
        }
        let then_branch = self.single_line_branch()?;
        self.finish_if_with(condition, keyword_span, then_branch)
    }

    fn single_line_branch(&mut self) -> R<Vec<Statement>> {
        let mut branch = Vec::new();
        loop {
            let before = self.out.len();
            // A line number where a branch starts is a GOTO to it, not a label.
            let number = match &self.tokens.get(self.i).map(|token| &token.kind) {
                Some(TokenKind::Integer(value, None)) if branch.is_empty() && (0..=65_529).contains(value) => {
                    Some((value.to_string(), self.span(self.i)))
                }
                _ => None,
            };
            if let Some((label, span)) = number {
                self.i += 1;
                self.out.push(Statement::Goto(label, span));
            } else {
                if self.i >= self.tokens.len() {
                    return self.fail();
                }
                self.statement()?;
            }
            let mut parsed = self.out.split_off(before);
            if parsed.is_empty() {
                return self.fail();
            }
            branch.append(&mut parsed);
            if self.i >= self.tokens.len() || self.is(Kw::NewLine) || self.is(Kw::ELSE) {
                break;
            }
            if !self.separator() {
                return self.fail();
            }
            if self.i >= self.tokens.len() || self.is(Kw::NewLine) || self.is(Kw::ELSE) {
                break;
            }
        }
        Ok(branch)
    }

    fn block_if(&mut self, condition: Expr, span: Span) -> R<()> {
        let mut then_branch = Vec::new();
        let mut else_branch = Vec::new();
        let mut in_else = false;
        loop {
            self.skip_separators();
            if self.eat(Kw::ELSE) {
                if in_else {
                    return self.fail();
                }
                in_else = true;
                continue;
            }
            if self.is(Kw::ELSEIF) {
                if in_else {
                    return self.fail();
                }
                let else_if = self.take();
                let nested_condition = self.exp()?;
                self.expect(Kw::THEN)?;
                let before = self.out.len();
                self.finish_if(nested_condition, self.span(else_if))?;
                let mut nested = self.out.split_off(before);
                if nested.len() != 1 {
                    return self.fail();
                }
                else_branch.push(nested.remove(0));
                self.out.push(Statement::If { condition, then_branch, else_branch, span });
                return Ok(());
            }
            if self.is(Kw::END) && self.is_n(1, Kw::IF) {
                self.i += 2;
                break;
            }
            if self.i >= self.tokens.len() {
                return self.fail();
            }
            let before = self.out.len();
            self.statement()?;
            let mut parsed = self.out.split_off(before);
            if parsed.is_empty() {
                return self.fail();
            }
            if in_else {
                else_branch.append(&mut parsed);
            } else {
                then_branch.append(&mut parsed);
            }
            if !self.ends_statement() {
                return self.fail();
            }
        }
        self.out.push(Statement::If { condition, then_branch, else_branch, span });
        Ok(())
    }

    fn for_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let counter = self.name_ref()?;
        self.expect(Kw::EQ)?;
        let start = self.exp()?;
        self.expect(Kw::TO)?;
        let end = self.exp()?;
        let step = if self.eat(Kw::STEP) { Some(self.exp()?) } else { None };
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        let body = self.collect_until(|ast| ast.is(Kw::NEXT))?;
        self.i += 1;
        let mut next_variable = None;
        if let (Some(Tok::Ident), TokenKind::Identifier(name)) = (self.tok_n(0), &self.tokens[self.i].kind) {
            next_variable = Some((name.clone(), self.span(self.i)));
            self.i += 1;
        }
        self.out.push(Statement::For { counter, start, end, step, body, next_variable, span: self.span(keyword) });
        Ok(())
    }

    fn while_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let condition = self.exp()?;
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        let body = self.collect_until(|ast| ast.is(Kw::WEND))?;
        self.i += 1;
        self.out.push(Statement::While { condition, body, span: self.span(keyword) });
        Ok(())
    }

    fn do_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let pre = if self.eat(Kw::WHILE) {
            Some((true, self.exp()?))
        } else if self.eat(Kw::UNTIL) {
            Some((false, self.exp()?))
        } else {
            None
        };
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        let body = self.collect_until(|ast| ast.is(Kw::LOOP))?;
        self.i += 1;
        let post = if self.eat(Kw::WHILE) {
            Some((true, self.exp()?))
        } else if self.eat(Kw::UNTIL) {
            Some((false, self.exp()?))
        } else {
            None
        };
        if pre.is_some() && post.is_some() {
            return self.fail();
        }
        self.out.push(Statement::Do { pre, post, body, span: self.span(keyword) });
        Ok(())
    }

    fn select_statement(&mut self) -> R<()> {
        let keyword = self.take();
        self.expect(Kw::CASE)?;
        let selector = self.exp()?;
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        let mut arms = Vec::new();
        let mut otherwise = Vec::new();
        loop {
            self.skip_separators();
            if self.at_end_select() {
                self.i += 2;
                break;
            }
            self.expect(Kw::CASE)?;
            let is_else = self.eat(Kw::ELSE);
            let mut matches = Vec::new();
            if !is_else {
                loop {
                    let relation = if self.eat(Kw::IS) {
                        Some(self.rel_op().ok_or(self.i)?)
                    } else {
                        self.rel_op()
                    };
                    if let Some(relation) = relation {
                        self.i += 1;
                        matches.push(CaseItem::Relation(relation, self.exp()?));
                    } else {
                        let lower = self.exp()?;
                        if self.eat(Kw::TO) {
                            matches.push(CaseItem::Range(lower, self.exp()?));
                        } else {
                            matches.push(CaseItem::Value(lower));
                        }
                    }
                    if !self.eat(Kw::Comma) {
                        break;
                    }
                }
            }
            if !(self.eat(Kw::NewLine) || self.separator()) {
                return self.fail();
            }
            let body = self.collect_until(|ast| ast.is(Kw::CASE) || ast.at_end_select())?;
            if is_else {
                otherwise = body;
            } else {
                arms.push((matches, body));
            }
        }
        self.out.push(Statement::Select { selector, arms, otherwise, span: self.span(keyword) });
        Ok(())
    }

    /// One DATA item: a number where it is plainly one, else the text (a quoted item without its quotes).
    fn data_item(&self, item: &str, span: Span) -> Expr {
        if let Some(rest) = item.strip_prefix('"') {
            return Expr::Literal(Literal::String(rest.split('"').next().unwrap_or("").to_owned()), span);
        }
        let (sign, digits) = match item.as_bytes().first() {
            Some(b'-') => (Some(Unary::Negative), &item[1..]),
            Some(b'+') => (Some(Unary::Positive), &item[1..]),
            _ => (None, item),
        };
        if digits.as_bytes().first().is_some_and(|b| b.is_ascii_digit() || *b == b'.' || *b == b'&') {
            if let Ok(tokens) = crate::qb_scan::scan(digits, self.dialect) {
                if let [Token { kind, .. }, Token { kind: TokenKind::Reserved(_), .. }] = tokens.as_slice() {
                    let literal = match kind {
                        TokenKind::Integer(value, suffix) => Some(Expr::Literal(
                            Literal::Integer(*value, if *suffix == Some('&') { TypeName::Long } else { TypeName::Integer }),
                            span,
                        )),
                        TokenKind::Real(text, suffix) => Some(Expr::Literal(
                            Literal::Real(text.clone(), real_type(*suffix)),
                            span,
                        )),
                        _ => None,
                    };
                    if let Some(literal) = literal {
                        return match sign {
                            Some(op) => Expr::Unary { op, operand: Box::new(literal), span },
                            None => literal,
                        };
                    }
                }
            }
        }
        Expr::Literal(Literal::String(item.to_owned()), span)
    }

    /// DATA's text as written: the items between its commas, a quoted one with its quotes dropped.
    fn data_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let raw_at = self.i;
        let TokenKind::String(raw) = &self.tokens[raw_at].kind else { return self.fail() };
        self.i += 1;
        let origin = self.span(raw_at);
        let mut values = Vec::new();
        let bytes = raw.as_bytes();
        let mut from = 0;
        loop {
            let mut at = from;
            let mut quoted = false;
            while at < bytes.len() && (quoted || bytes[at] != b',') {
                if bytes[at] == b'"' {
                    quoted = !quoted;
                }
                at += 1;
            }
            let item = raw[from..at].trim();
            let span = Span { line: origin.line, start: origin.start + from, end: origin.start + at };
            values.push(self.data_item(item, span));
            if at >= bytes.len() {
                break;
            }
            from = at + 1;
        }
        self.out.push(Statement::Data { values, span: self.span(keyword) });
        Ok(())
    }

    // ---- declarations ---------------------------------------------------------------------------------

    /// `SIGNED`/`UNSIGNED` before `BYTE`, `INTEGER` or `LONG`; both words stay names elsewhere.
    fn signed_type(&mut self) -> Option<TypeName> {
        let TokenKind::Identifier(modifier) = &self.tokens.get(self.i)?.kind else { return None };
        let signed = match modifier.as_str() {
            "SIGNED" => true,
            "UNSIGNED" => false,
            _ => return None,
        };
        let width = match &self.tokens.get(self.i + 1)?.kind {
            TokenKind::Identifier(name) if name == "BYTE" => 1,
            _ if self.kinds.get(self.i + 1) == Some(&Tok::Kw(Kw::INTEGER)) => 2,
            _ if self.kinds.get(self.i + 1) == Some(&Tok::Kw(Kw::LONG)) => 4,
            _ => return None,
        };
        self.i += 2;
        Some(TypeName::Integral { width, signed })
    }

    fn declaration_type(&mut self) -> R<TypeName> {
        if let Some(integral) = self.signed_type() {
            return Ok(integral);
        }
        let at = self.i;
        let type_name = match &self.tokens[at].kind {
            TokenKind::Reserved(_) => match self.kw() {
                Some(Kw::INTEGER) => TypeName::Integer,
                Some(Kw::LONG) => TypeName::Long,
                Some(Kw::SINGLE) => TypeName::Single,
                Some(Kw::DOUBLE) => TypeName::Double,
                Some(Kw::STRING) => TypeName::String,
                Some(Kw::ANY) => TypeName::Named("ANY".into()),
                _ => TypeName::Named(self.name_at(at).ok_or(at)?),
            },
            TokenKind::Identifier(name) if name == "CURRENCY" && self.dialect.currency_suffix() => TypeName::Currency,
            TokenKind::Identifier(name) => TypeName::Named(name.clone()),
            _ => return self.fail(),
        };
        self.i += 1;
        Ok(type_name)
    }

    fn declaration(&mut self, shared: bool, require_array: bool) -> R<Declaration> {
        let at = self.i;
        let mut name = self.name_at(at).ok_or(at)?;
        if !matches!(self.tok_n(0), Some(Tok::Ident | Tok::Kw(_))) {
            return self.fail();
        }
        self.i += 1;
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            self.i += 1;
            name.push('.');
            name.push_str(&self.field_name()?.0);
        }
        let mut bounds = Vec::new();
        let mut array = false;
        if self.eat(Kw::LParen) {
            array = true;
            if !self.eat(Kw::RParen) {
                loop {
                    let first = self.exp()?;
                    let (lower, upper) = if self.eat(Kw::TO) { (Some(first), self.exp()?) } else { (None, first) };
                    bounds.push(Bound { lower, upper });
                    if self.eat(Kw::Comma) {
                        continue;
                    }
                    self.expect(Kw::RParen)?;
                    break;
                }
            }
        }
        if require_array && !array {
            return self.fail();
        }
        let mut fixed_length = None;
        let type_name = if self.eat(Kw::AS) {
            let type_name = self.declaration_type()?;
            if type_name == TypeName::String && self.eat(Kw::Mult) {
                fixed_length = Some(self.exp()?);
            }
            Some(type_name)
        } else {
            name_of_suffix(&name)
        };
        let token = self.span(at);
        Ok(Declaration {
            name,
            type_name,
            array,
            bounds,
            fixed_length,
            shared,
            dynamic: self.dynamic_arrays,
            volatile: self.volatile_at.contains(&(token.line, token.start)),
            span: Span { line: token.line, start: token.start, end: self.previous_end() },
        })
    }

    fn declaration_statement(&mut self, k: Kw) -> R<()> {
        self.i += 1;
        let shared = matches!(k, Kw::DIM | Kw::REDIM) && self.eat(Kw::SHARED);
        let mut declarations = vec![self.declaration(shared, k == Kw::REDIM)?];
        while self.eat(Kw::Comma) {
            declarations.push(self.declaration(shared, k == Kw::REDIM)?);
        }
        // SHARED inside a procedure names module variables: they are shared
        let declarations = if k == Kw::SHARED {
            declarations.into_iter().map(|declaration| Declaration { shared: true, ..declaration }).collect()
        } else {
            declarations
        };
        self.out.push(match k {
            Kw::DIM => Statement::Dim(declarations),
            Kw::REDIM => Statement::Redim(declarations),
            Kw::STATIC => Statement::Static(declarations),
            _ => Statement::Shared(declarations),
        });
        Ok(())
    }

    /// `COMMON [SHARED] [/block/] a, b AS INTEGER`: variables a CHAINed program would keep. With no
    /// CHAIN to carry them it declares them, shared where SHARED says so.
    fn common(&mut self) -> R<()> {
        self.i += 1;
        let keyword = self.i - 1;
        let shared = self.eat(Kw::SHARED);
        let mut block = None;
        if self.eat(Kw::Div) {
            block = Some(self.identifier()?.0);
            self.expect(Kw::Div)?;
        }
        let mut declarations = vec![self.declaration(shared, false)?];
        while self.eat(Kw::Comma) {
            declarations.push(self.declaration(shared, false)?);
        }
        let span = self.span(keyword);
        self.out.push(Statement::Common { shared, block, declarations, span });
        Ok(())
    }

    fn const_statement(&mut self) -> R<()> {
        self.i += 1;
        loop {
            let at = self.i;
            let mut name = if self.is(Kw::MOD) {
                self.i += 1;
                "MOD".to_owned()
            } else {
                let name = self.name_at(at).ok_or(at)?;
                self.i += 1;
                name
            };
            while matches!(self.tok_n(0), Some(Tok::Period)) {
                self.i += 1;
                let (field, _) = self.field_name()?;
                name.push('.');
                name.push_str(&field);
            }
            self.expect(Kw::EQ)?;
            let value = self.exp()?;
            let first = self.span(at);
            self.out.push(Statement::Const { name, span: Span { line: first.line, start: first.start, end: value.span().end }, value });
            if !self.eat(Kw::Comma) {
                return Ok(());
            }
        }
    }

    fn def_type(&mut self, k: Kw) -> R<()> {
        let type_name = match k {
            Kw::DEFINT => TypeName::Integer,
            Kw::DEFLNG => TypeName::Long,
            Kw::DEFSNG => TypeName::Single,
            Kw::DEFDBL => TypeName::Double,
            _ => TypeName::String,
        };
        self.def_type_of(type_name)
    }

    fn def_type_of(&mut self, type_name: TypeName) -> R<()> {
        let span = self.span(self.i);
        self.i += 1;
        let mut ranges = Vec::new();
        loop {
            let first = self.letter()?;
            let last = if self.eat(Kw::Minus) { self.letter()? } else { first };
            if first > last {
                return self.fail();
            }
            ranges.push((first, last));
            if !self.eat(Kw::Comma) {
                break;
            }
        }
        self.out.push(Statement::DefType { type_name, ranges, span });
        Ok(())
    }

    fn letter(&mut self) -> R<char> {
        let (name, _) = self.identifier()?;
        if name.len() != 1 || !name.as_bytes()[0].is_ascii_alphabetic() {
            self.i -= 1;
            return self.fail();
        }
        Ok(name.as_bytes()[0].to_ascii_uppercase() as char)
    }

    fn option(&mut self) -> R<()> {
        let first = self.take();
        if self.eat(Kw::BASE) {
            let base = match &self.tokens.get(self.i).map(|token| &token.kind) {
                Some(TokenKind::Integer(value @ (0 | 1), _)) => *value,
                _ => return self.fail(),
            };
            self.i += 1;
            self.out.push(Statement::OptionBase(base, self.span(first)));
            return Ok(());
        }
        if matches!(self.tok_n(0), Some(Tok::Ext)) {
            self.i += 1;
            self.out.push(Statement::OptionExplicit(self.span(first)));
            return Ok(());
        }
        self.fail()
    }

    fn type_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let at = self.i;
        let name = self.name_at(at).ok_or(at)?;
        self.i += 1;
        self.expect(Kw::NewLine)?;
        let mut fields = Vec::new();
        loop {
            self.skip_separators();
            if self.is(Kw::END) && self.is_n(1, Kw::TYPE) {
                self.i += 2;
                break;
            }
            let at = self.i;
            let field = self.name_at(at).ok_or(at)?;
            self.i += 1;
            let mut bounds = Vec::new();
            let array = self.eat(Kw::LParen);
            if array && !self.eat(Kw::RParen) {
                loop {
                    bounds.push(Bound { lower: None, upper: self.exp()? });
                    if self.eat(Kw::Comma) {
                        continue;
                    }
                    self.expect(Kw::RParen)?;
                    break;
                }
            }
            self.expect(Kw::AS)?;
            let type_name = self.declaration_type()?;
            let fixed_length = if type_name == TypeName::String && self.eat(Kw::Mult) { Some(self.exp()?) } else { None };
            fields.push(Declaration {
                name: field,
                type_name: Some(type_name),
                array,
                bounds,
                fixed_length,
                shared: false,
                dynamic: false,
                volatile: false,
                span: self.span(at),
            });
            if !self.is(Kw::NewLine) {
                return self.fail();
            }
        }
        self.out.push(Statement::TypeDecl { name, fields, span: self.span(keyword) });
        Ok(())
    }

    fn end(&mut self) -> R<()> {
        let keyword = self.take();
        match self.kw() {
            Some(k @ (Kw::SUB | Kw::FUNCTION | Kw::DEF)) => {
                self.i += 1;
                let kind = if k == Kw::SUB { ProcedureKind::Sub } else { ProcedureKind::Function };
                match self.open_procedure {
                    Some(index) if self.procedures[index].kind == kind => {
                        self.open_procedure = None;
                        Ok(())
                    }
                    _ => self.fail(),
                }
            }
            Some(Kw::IF | Kw::SELECT | Kw::TYPE) => self.fail(),
            _ => {
                self.runtime("END", Vec::new(), self.span(keyword));
                Ok(())
            }
        }
    }

    // ---- procedures -----------------------------------------------------------------------------------

    fn parameter(&mut self) -> R<Parameter> {
        let by_value = self.eat(Kw::BYVAL);
        let segmented = self.eat(Kw::SEG);
        let (name, token) = self.identifier()?;
        let mut array = false;
        if self.eat(Kw::LParen) {
            array = true;
            if !self.is(Kw::RParen) {
                self.arguments_loose()?;
            } else {
                self.i += 1;
            }
        }
        let type_name = if self.eat(Kw::AS) {
            let type_name = self.declaration_type()?;
            if type_name == TypeName::String && self.eat(Kw::Mult) {
                self.exp()?;
            }
            Some(type_name)
        } else {
            name_of_suffix(&name)
        };
        Ok(Parameter {
            declaration: Declaration {
                name,
                type_name,
                array,
                bounds: Vec::new(),
                fixed_length: None,
                shared: false,
                dynamic: false,
                volatile: self.volatile_at.contains(&(token.line, token.start)),
                span: Span { line: token.line, start: token.start, end: self.previous_end() },
            },
            by_value,
            segmented,
        })
    }

    /// Bounds in parentheses after a parameter name, to the `)`: read and dropped.
    fn arguments_loose(&mut self) -> R<()> {
        loop {
            self.exp()?;
            if self.eat(Kw::TO) {
                self.exp()?;
            }
            if self.eat(Kw::Comma) {
                continue;
            }
            return self.expect(Kw::RParen);
        }
    }

    fn parameters(&mut self) -> R<Vec<Parameter>> {
        let mut parameters = Vec::new();
        if self.eat(Kw::LParen) && !self.eat(Kw::RParen) {
            loop {
                parameters.push(self.parameter()?);
                if self.eat(Kw::Comma) {
                    continue;
                }
                self.expect(Kw::RParen)?;
                break;
            }
        }
        Ok(parameters)
    }

    /// `SUB name …` or `FUNCTION name …`, a definition or, after DECLARE, a declaration.
    fn procedure(&mut self, declaration: bool, _marker: Option<()>, is_static_prefix: bool) -> R<()> {
        let keyword = self.take();
        let kind = if self.kinds[keyword] == Tok::Kw(Kw::SUB) { ProcedureKind::Sub } else { ProcedureKind::Function };
        let span = self.span(keyword);
        let (mut name, _) = self.identifier()?;
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            self.i += 1;
            name.push('.');
            name.push_str(&self.field_name()?.0);
        }
        let mut result = name_of_suffix(&name);
        if kind == ProcedureKind::Sub && result.is_some() {
            return self.fail();
        }
        let mut cdecl = false;
        let mut alias = None;
        if declaration {
            cdecl = self.eat(Kw::CDECL);
            if self.eat(Kw::ALIAS) {
                alias = match &self.tokens.get(self.i).map(|token| &token.kind) {
                    Some(TokenKind::String(text)) => Some(text.clone()),
                    _ => return self.fail(),
                };
                self.i += 1;
            }
        }
        let parameters = self.parameters()?;
        if kind == ProcedureKind::Function && self.eat(Kw::AS) {
            result = Some(self.declaration_type()?);
        }
        let is_static = self.eat(Kw::STATIC) || is_static_prefix;
        self.procedures.push(Procedure {
            name,
            alias,
            cdecl,
            kind,
            parameters,
            result,
            body: Vec::new(),
            declaration,
            is_static,
            exported: true,
            private: false,
            module_scope: false,
            span,
        });
        if !declaration {
            if self.open_procedure.is_some() {
                return self.fail();
            }
            self.open_procedure = Some(self.procedures.len() - 1);
        }
        Ok(())
    }

    fn declare(&mut self) -> R<()> {
        self.i += 1;
        if !matches!(self.kw(), Some(Kw::SUB | Kw::FUNCTION)) {
            return self.fail();
        }
        self.procedure(true, None, false)
    }

    fn def(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        if self.eat(Kw::SEG) {
            let value = if self.eat(Kw::EQ) { Some(self.exp()?) } else { None };
            self.out.push(Statement::DefSeg { value, span });
            return Ok(());
        }
        let (name, _) = if matches!(self.tok_n(0), Some(Tok::FnWord)) {
            self.i += 1;
            let (name, at) = self.identifier()?;
            (format!("FN{name}"), at)
        } else {
            self.identifier()?
        };
        if !name.starts_with("FN") {
            return self.fail();
        }
        let mut result = name_of_suffix(&name);
        let parameters = self.parameters()?;
        if self.eat(Kw::AS) {
            result = Some(self.declaration_type()?);
        }
        let is_static = self.eat(Kw::STATIC);
        let definition = if self.eat(Kw::EQ) { Some(self.exp()?) } else { None };
        let inline = definition.is_some();
        let body = definition
            .map(|value| vec![Statement::Assign { target: Expr::Name(name.clone(), span), value, span }])
            .unwrap_or_default();
        self.procedures.push(Procedure {
            name,
            alias: None,
            cdecl: false,
            kind: ProcedureKind::Function,
            parameters,
            result,
            body,
            declaration: false,
            is_static,
            exported: !inline,
            private: false,
            module_scope: true,
            span,
        });
        if !inline {
            if self.open_procedure.is_some() {
                return self.fail();
            }
            self.open_procedure = Some(self.procedures.len() - 1);
        }
        Ok(())
    }
}

// ---- input and output, files, graphics, events -------------------------------------------------------

impl Ast<'_> {
    /// A word that can begin a PRINT item that follows another without a separator.
    fn starts_joined(&self) -> bool {
        match self.tok_n(0) {
            Some(Tok::Int | Tok::Real | Tok::Str | Tok::Ident | Tok::FnWord) => true,
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                c & (FN0 | FN_WORD | CTX_WORD) != 0
                    || (c & OBJ_WORD != 0 && self.period_n(1))
                    || (c & (FN1 | FN2 | FN23 | FN_BOUND | FN_REF) != 0 && self.is_n(1, Kw::LParen))
                    || matches!(k, Kw::ERROR | Kw::RND)
            }
            _ => false,
        }
    }

    fn print(&mut self, k: Kw) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let kind = match k {
            Kw::LPRINT => PrintKind::Lprint,
            Kw::WRITE => PrintKind::Write,
            _ => PrintKind::Print,
        };
        let file = if self.eat(Kw::Lbs) {
            let channel = self.exp()?;
            self.expect(Kw::Comma)?;
            Some(channel)
        } else {
            None
        };
        let mut using = None;
        let mut items: Vec<PrintItem> = Vec::new();
        while !(self.ends_statement() || self.is(Kw::ELSE)) {
            if k != Kw::WRITE && using.is_none() && self.eat(Kw::USING) {
                using = Some(self.exp()?);
                // `USING fmt;` must be followed by a value (a comma is "Semicolon missing")
                if !self.eat(Kw::SColon) || !self.starts_exp() {
                    return self.fail();
                }
                continue;
            }
            if matches!(self.kw(), Some(Kw::TAB | Kw::SPC)) && self.is_n(1, Kw::LParen) {
                let name = if self.is(Kw::TAB) { "TAB" } else { "SPC" };
                self.i += 2;
                let argument = self.exp()?;
                self.expect(Kw::RParen)?;
                let arg_span = argument.span();
                if !(self.eat(Kw::SColon) || self.eat(Kw::Comma)) {
                    // no separator: the next item follows directly
                }
                items.push(PrintItem {
                    value: Expr::Apply { name: name.into(), arguments: vec![argument], span: arg_span },
                    separator: PrintSeparator::Semicolon,
                });
                continue;
            }
            if self.is(Kw::Comma) || self.is(Kw::SColon) {
                let at = self.take();
                let separator = if self.kinds[at] == Tok::Kw(Kw::Comma) { PrintSeparator::Comma } else { PrintSeparator::Semicolon };
                items.push(PrintItem { value: Expr::Omitted(self.span(at)), separator });
                continue;
            }
            if !self.starts_exp() {
                return self.fail();
            }
            let mut value = self.exp()?;
            while self.starts_joined() {
                items.push(PrintItem { value, separator: PrintSeparator::Semicolon });
                value = self.imp(First::Joined)?;
            }
            let separator = if self.eat(Kw::Comma) {
                PrintSeparator::Comma
            } else if self.eat(Kw::SColon) {
                PrintSeparator::Semicolon
            } else {
                PrintSeparator::End
            };
            items.push(PrintItem { value, separator });
        }
        self.out.push(Statement::Print { kind, file, using, items, span });
        Ok(())
    }

    /// `[#file,] [prompt ;|,]`: the file, the prompt and whether its separator was a comma,
    /// and whether the statement began with `;`.
    fn input_prefix(&mut self) -> R<(Option<Expr>, Option<Expr>, bool, bool)> {
        let mut file = None;
        let mut keep_cursor = false;
        if self.eat(Kw::Lbs) {
            file = Some(self.exp()?);
            self.expect(Kw::Comma)?;
            return Ok((file, None, false, false));
        }
        if self.eat(Kw::SColon) {
            keep_cursor = true;
        }
        let mut prompt = None;
        let mut suppress = false;
        if matches!(self.tok_n(0), Some(Tok::Str)) {
            prompt = Some(self.literal()?);
            if self.eat(Kw::Comma) {
                suppress = true;
            } else {
                self.expect(Kw::SColon)?;
            }
        }
        Ok((file, prompt, suppress, keep_cursor))
    }

    fn input(&mut self) -> R<()> {
        let keyword = self.take();
        let (file, prompt, suppress_question_mark, keep_cursor) = self.input_prefix()?;
        let mut destinations = vec![self.name_ref()?];
        while self.eat(Kw::Comma) {
            destinations.push(self.name_ref()?);
        }
        self.out.push(Statement::Input {
            file,
            prompt,
            suppress_question_mark,
            keep_cursor,
            destinations,
            span: self.span(keyword),
        });
        Ok(())
    }

    fn coordinates(&mut self) -> R<(Expr, Expr, bool)> {
        let step = self.eat(Kw::STEP);
        self.expect(Kw::LParen)?;
        let x = self.exp()?;
        self.expect(Kw::Comma)?;
        let y = self.exp()?;
        self.expect(Kw::RParen)?;
        Ok((x, y, step))
    }

    /// The x of a STEP-relative pair is wrapped, as the table-driven parser does.
    fn stepped(x: Expr, step: bool) -> Expr {
        if step {
            let span = x.span();
            Expr::Apply { name: STEP_MARK.into(), arguments: vec![x], span }
        } else {
            x
        }
    }

    /// `, [exp]`s after the leading arguments, an empty slot being Omitted at its comma.
    fn rest(&mut self, trailing_ok: bool) -> R<Vec<Expr>> {
        let mut out = Vec::new();
        while self.is(Kw::Comma) {
            let comma = self.take();
            out.push(if self.starts_exp() { self.exp()? } else { Expr::Omitted(self.span(comma)) });
        }
        if !trailing_ok && matches!(out.last(), Some(Expr::Omitted(_))) {
            return self.fail();
        }
        Ok(out)
    }

    fn line(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        if self.eat(Kw::INPUT) {
            let (file, prompt, _, _) = self.input_prefix()?;
            let destination = self.name_ref()?;
            self.out.push(Statement::LineInput { file, prompt, destination, span });
            return Ok(());
        }
        let (mut arguments, omitted_first) = if self.is(Kw::Minus) {
            let zero = Expr::Literal(Literal::Integer(0, TypeName::Integer), span);
            (vec![Self::stepped(zero.clone(), true), zero], true)
        } else {
            let (x, y, step) = self.coordinates()?;
            (vec![Self::stepped(x, step), y], false)
        };
        let _ = omitted_first;
        self.expect(Kw::Minus)?;
        let (x, y, step) = self.coordinates()?;
        arguments.push(Self::stepped(x, step));
        arguments.push(y);
        let mut color = None;
        let mut mode = None;
        let mut style = None;
        // BC takes `,color,` and `,,` (nothing after the second comma); VBDOS does not (checker)
        let mut trailing = false;
        if self.eat(Kw::Comma) {
            if self.starts_exp() && !self.at_box_word() {
                color = Some(self.exp()?);
            }
            if self.eat(Kw::Comma) {
                if self.at_box_word() {
                    let at = self.i;
                    let (name, _) = self.identifier()?;
                    mode = Some(Expr::Name(name, self.span(at)));
                }
                if self.eat(Kw::Comma) {
                    if !self.starts_exp() {
                        return self.fail();
                    }
                    style = Some(self.exp()?);
                } else {
                    trailing = mode.is_none();
                }
            } else if color.is_none() {
                return self.fail();
            }
        }
        for slot in [color, mode, style] {
            arguments.push(slot.unwrap_or(Expr::Omitted(span)));
        }
        while arguments.len() > if trailing { 6 } else { 4 } && matches!(arguments.last(), Some(Expr::Omitted(_))) {
            arguments.pop();
        }
        self.runtime("LINE", arguments, span);
        Ok(())
    }

    fn at_box_word(&self) -> bool {
        match &self.tokens.get(self.i).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => name == "B" || name == "BF",
            _ => false,
        }
    }

    fn open(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let first = self.exp()?;
        if self.eat(Kw::Comma) {
            // OPEN mode$, [#]file, path$[, record length]
            self.eat(Kw::Lbs);
            let file = self.exp()?;
            self.expect(Kw::Comma)?;
            let path = self.exp()?;
            let record_length = if self.eat(Kw::Comma) { Some(self.exp()?) } else { None };
            self.out.push(Statement::OpenClassic { mode: first, file, path, record_length, span });
            return Ok(());
        }
        let mut mode_written = false;
        let mut mode = FileMode::Random;
        if self.eat(Kw::FOR) {
            mode_written = true;
            mode = match self.kw() {
                Some(Kw::APPEND) => FileMode::Append,
                Some(Kw::INPUT) => FileMode::Input,
                Some(Kw::OUTPUT) => FileMode::Output,
                Some(Kw::BINARY) => FileMode::Binary,
                Some(Kw::RANDOM) => FileMode::Random,
                _ => return self.fail(),
            };
            self.i += 1;
        }
        let access = if self.eat(Kw::ACCESS) {
            Some(if self.eat(Kw::READ) {
                if self.eat(Kw::WRITE) { FileAccess::ReadWrite } else { FileAccess::Read }
            } else {
                self.expect(Kw::WRITE)?;
                FileAccess::Write
            })
        } else {
            None
        };
        let lock = if self.eat(Kw::SHARED) {
            Some(FileLock::Shared)
        } else if self.eat(Kw::LOCK) {
            Some(if self.eat(Kw::READ) {
                if self.eat(Kw::WRITE) { FileLock::ReadWrite } else { FileLock::Read }
            } else {
                self.expect(Kw::WRITE)?;
                FileLock::Write
            })
        } else {
            None
        };
        self.expect(Kw::AS)?;
        self.eat(Kw::Lbs);
        let file = self.exp()?;
        let record_length = if self.is(Kw::LEN) && self.is_n(1, Kw::EQ) {
            self.i += 2;
            Some(self.exp()?)
        } else {
            None
        };
        self.out.push(Statement::Open { path: first, mode, file, mode_written, access, lock, record_length, span });
        Ok(())
    }

    /// The array of a graphics GET/PUT: `a%` or `a%(i)`, never `a%()`.
    fn transfer_array(&mut self) -> R<Expr> {
        let target = self.name_ref()?;
        if matches!(&target, Expr::Apply { arguments, .. } if arguments.is_empty()) {
            return self.fail();
        }
        Ok(target)
    }

    fn get_put(&mut self, k: Kw) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let graphic = self.is(Kw::STEP) || self.is(Kw::LParen) && {
            let save = self.i;
            let found = self.coordinates().is_ok();
            self.i = save;
            found
        };
        if graphic {
            let (x, y, step) = self.coordinates()?;
            let mut arguments = vec![Self::stepped(x, step), y];
            if k == Kw::GET {
                self.expect(Kw::Minus)?;
                let (x, y, step) = self.coordinates()?;
                arguments.push(Self::stepped(x, step));
                arguments.push(y);
                self.expect(Kw::Comma)?;
                arguments.push(self.transfer_array()?);
                self.runtime("GET", arguments, span);
            } else {
                self.expect(Kw::Comma)?;
                arguments.push(self.transfer_array()?);
                if self.eat(Kw::Comma) {
                    let at = self.i;
                    let mode = match self.kw() {
                        Some(Kw::AND) => "AND",
                        Some(Kw::OR) => "OR",
                        Some(Kw::PRESET) => "PRESET",
                        Some(Kw::PSET) => "PSET",
                        Some(Kw::XOR) => "XOR",
                        _ => return self.fail(),
                    };
                    self.i += 1;
                    arguments.push(Expr::Name(mode.into(), self.span(at)));
                }
                self.runtime("PUT", arguments, span);
            }
            return Ok(());
        }
        self.eat(Kw::Lbs);
        let file = self.exp()?;
        let mut position = None;
        let mut target = None;
        if self.eat(Kw::Comma) {
            if self.starts_exp() {
                position = Some(self.exp()?);
            } else if !self.is(Kw::Comma) {
                return self.fail();
            }
            if self.eat(Kw::Comma) {
                target = Some(self.exp()?);
            }
        }
        self.out.push(Statement::FileTransfer { write: k == Kw::PUT, file, position, target, span });
        Ok(())
    }

    fn event(&mut self, device: &str, number: Option<Expr>, verb: &str, label: Option<(String, Span)>, span: Span) {
        let arguments = vec![
            Expr::Name(device.into(), span),
            number.unwrap_or(Expr::Omitted(span)),
            Expr::Name(verb.into(), span),
            label.map_or(Expr::Omitted(span), |(name, at)| Expr::Name(name, at)),
        ];
        self.runtime("EVENT", arguments, span);
    }

    fn on(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let local = self.is(Kw::LOCAL);
        if (self.is(Kw::ERROR) && (self.is_n(1, Kw::GOTO) || self.is_n(1, Kw::RESUME)))
            || (local && self.is_n(1, Kw::ERROR))
        {
            self.i += 1 + usize::from(local);
            if self.eat(Kw::GOTO) {
                let label = if matches!(self.tokens.get(self.i).map(|token| &token.kind), Some(TokenKind::Integer(0, None))) {
                    self.i += 1;
                    "0".to_owned()
                } else {
                    self.lab_ln()?.0
                };
                self.out.push(Statement::OnError { label, local, span });
                return Ok(());
            }
            self.expect(Kw::RESUME)?;
            self.expect(Kw::NEXT)?;
            self.out.push(Statement::OnErrorResumeNext { local, span });
            return Ok(());
        }
        let named = match self.kw() {
            Some(Kw::KEY) => Some("KEY"),
            Some(Kw::COM) => Some("COM"),
            Some(Kw::SIGNAL) => Some("SIGNAL"),
            Some(Kw::TIMER) if self.is_n(1, Kw::LParen) => Some("TIMER"),
            Some(Kw::PEN) if self.is_n(1, Kw::GOSUB) => Some("PEN"),
            Some(Kw::UEVENT) if self.is_n(1, Kw::GOSUB) => Some("UEVENT"),
            _ => None,
        };
        // ON PEN and ON UEVENT take no argument; elsewhere they are not a selector either
        if named.is_none() && matches!(self.kw(), Some(Kw::PEN | Kw::UEVENT)) {
            return self.fail();
        }
        if let Some(device) = named {
            self.i += 1;
            let number = if self.eat(Kw::LParen) {
                let number = self.exp()?;
                self.expect(Kw::RParen)?;
                Some(number)
            } else {
                None
            };
            self.expect(Kw::GOSUB)?;
            let label = self.lab_ln()?;
            self.event(device, number, "GOSUB", Some(label), span);
            return Ok(());
        }
        let selector = self.imp(First::NoErr)?;
        let gosub = match self.kw() {
            Some(Kw::GOTO) => false,
            Some(Kw::GOSUB) => true,
            _ => return self.fail(),
        };
        self.i += 1;
        let mut labels = vec![self.lab_ln()?];
        while self.eat(Kw::Comma) {
            labels.push(self.lab_ln()?);
        }
        // ON PLAY(n) GOSUB and ON STRIG(n) GOSUB: a trap, whose "selector" is the device call
        if gosub {
            if let Expr::Apply { name, arguments, .. } = &selector {
                if matches!(name.as_str(), "PLAY" | "STRIG") && arguments.len() == 1 && labels.len() == 1 {
                    let device = if name == "PLAY" { "PLAY" } else { "STRIG" };
                    self.event(device, Some(arguments[0].clone()), "GOSUB", labels.pop(), span);
                    return Ok(());
                }
            }
        }
        let number = |value: i64| Expr::Literal(Literal::Integer(value, TypeName::Integer), span);
        let arms = labels
            .into_iter()
            .enumerate()
            .map(|(index, (label, label_span))| {
                let jump = if gosub {
                    Statement::Call { name: "GOSUB".into(), arguments: vec![Expr::Name(label, label_span)], explicit: false, span }
                } else {
                    Statement::Goto(label, span)
                };
                (vec![CaseItem::Value(number(index as i64 + 1))], vec![jump])
            })
            .collect();
        self.out.push(Statement::Select {
            selector: Expr::Apply { name: ON_SELECTOR.into(), arguments: vec![selector], span },
            arms,
            otherwise: Vec::new(),
            span,
        });
        Ok(())
    }

    fn switch_verb(&mut self) -> R<&'static str> {
        let verb = match self.kw() {
            Some(Kw::ON) => "ON",
            Some(Kw::OFF) => "OFF",
            Some(Kw::STOP) => "STOP",
            _ => return self.fail(),
        };
        self.i += 1;
        Ok(verb)
    }

    fn simple_statement(&mut self, k: Kw) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        match k {
            Kw::BEEP => self.runtime("BEEP", Vec::new(), span),
            // BC warns "Statement Ignored" and skips the rest of the line
            Kw::TRON | Kw::TROFF => {
                while self.i < self.tokens.len() && !self.is(Kw::NewLine) {
                    self.i += 1;
                }
                self.runtime(if k == Kw::TRON { "TRON" } else { "TROFF" }, Vec::new(), span);
            }
            Kw::SYSTEM => self.runtime("SYSTEM", Vec::new(), span),
            Kw::STOP => self.implicit_call("STOP", Vec::new(), span),
            Kw::RESET => self.implicit_call("RESET", Vec::new(), span),
            Kw::ENDIF => self.runtime("ENDIF", Vec::new(), span),
            Kw::CLS | Kw::SLEEP | Kw::FILES | Kw::RUN => {
                let arguments = self.optional_exp()?;
                let name = match k {
                    Kw::CLS => "CLS",
                    Kw::SLEEP => "SLEEP",
                    Kw::FILES => "FILES",
                    _ => "RUN",
                };
                self.runtime(name, arguments, span);
            }
            Kw::RANDOMIZE | Kw::SHELL => {
                let arguments = self.optional_exp()?;
                self.implicit_call(if k == Kw::RANDOMIZE { "RANDOMIZE" } else { "SHELL" }, arguments, span);
            }
            Kw::COLOR | Kw::LOCATE | Kw::SCREEN => {
                let arguments = self.argument_list(false)?;
                let name = match k {
                    Kw::COLOR => "COLOR",
                    Kw::LOCATE => "LOCATE",
                    _ => "SCREEN",
                };
                self.runtime(name, arguments, span);
            }
            Kw::CLEAR => {
                let arguments = self.argument_list(true)?;
                self.implicit_call("CLEAR", arguments, span);
            }
            Kw::ENVIRON | Kw::KILL | Kw::MKDIR | Kw::CHDIR | Kw::RMDIR => {
                let argument = self.exp()?;
                let name = match k {
                    Kw::ENVIRON => "ENVIRON",
                    Kw::KILL => "KILL",
                    Kw::MKDIR => "MKDIR",
                    Kw::CHDIR => "CHDIR",
                    _ => "RMDIR",
                };
                self.implicit_call(name, vec![argument], span);
            }
            Kw::BLOAD => {
                let mut arguments = vec![self.exp()?];
                if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                }
                self.implicit_call("BLOAD", arguments, span);
            }
            Kw::BSAVE => {
                let mut arguments = vec![self.exp()?];
                for _ in 0..2 {
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                }
                self.implicit_call("BSAVE", arguments, span);
            }
            Kw::CHAIN | Kw::DRAW | Kw::ERROR => {
                let argument = self.exp()?;
                let name = match k {
                    Kw::CHAIN => "CHAIN",
                    Kw::DRAW => "DRAW",
                    _ => "ERROR",
                };
                self.runtime(name, vec![argument], span);
            }
            Kw::PLAY => {
                if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::STOP)) {
                    let verb = self.switch_verb()?;
                    self.event("PLAY", None, verb, None, span);
                } else {
                    let argument = self.exp()?;
                    self.runtime("PLAY", vec![argument], span);
                }
            }
            Kw::SOUND | Kw::OUT | Kw::POKE | Kw::PCOPY | Kw::NAME => {
                let a = self.exp()?;
                if k == Kw::NAME {
                    self.expect(Kw::AS)?;
                } else {
                    self.expect(Kw::Comma)?;
                }
                let b = self.exp()?;
                let name = match k {
                    Kw::SOUND => "SOUND",
                    Kw::OUT => "OUT",
                    Kw::POKE => "POKE",
                    Kw::PCOPY => "PCOPY",
                    _ => "NAME",
                };
                self.runtime(name, vec![a, b], span);
            }
            Kw::WAIT => {
                let mut arguments = vec![self.exp()?];
                self.expect(Kw::Comma)?;
                arguments.push(self.exp()?);
                if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                }
                self.runtime("WAIT", arguments, span);
            }
            Kw::WIDTH => {
                let mut arguments = Vec::new();
                let mut name = "WIDTH";
                if self.eat(Kw::Lbs) {
                    name = "WIDTH_FILE";
                    arguments.push(self.exp()?);
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                } else if self.eat(Kw::LPRINT) {
                    name = "WIDTH_LPRINT";
                    arguments.push(self.exp()?);
                } else if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                } else {
                    arguments.push(self.exp()?);
                    if self.eat(Kw::Comma) {
                        arguments.push(self.exp()?);
                    }
                }
                self.runtime(name, arguments, span);
            }
            Kw::PALETTE => {
                let mut arguments = Vec::new();
                if self.eat(Kw::USING) {
                    arguments.push(self.transfer_array()?);
                    self.runtime("PALETTE_USING", arguments, span);
                    return Ok(());
                } else if self.starts_exp() {
                    arguments.push(self.exp()?);
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                }
                self.runtime("PALETTE", arguments, span);
            }
            Kw::PSET | Kw::PRESET | Kw::PAINT | Kw::CIRCLE => {
                let (x, y, step) = self.coordinates()?;
                let mut arguments = vec![Self::stepped(x, step), y];
                if k == Kw::CIRCLE {
                    let comma = self.expect(Kw::Comma);
                    comma?;
                    arguments.push(self.exp()?);
                }
                arguments.extend(self.rest(k == Kw::PAINT)?);
                let name = match k {
                    Kw::PSET => "PSET",
                    Kw::PRESET => "PRESET",
                    Kw::PAINT => "PAINT",
                    _ => "CIRCLE",
                };
                self.runtime(name, arguments, span);
            }
            Kw::VIEW => {
                if self.eat(Kw::PRINT) {
                    let mut arguments = Vec::new();
                    if self.starts_exp() {
                        arguments.push(self.exp()?);
                        self.expect(Kw::TO)?;
                        arguments.push(self.exp()?);
                    }
                    self.runtime("VIEW_PRINT", arguments, span);
                } else {
                    let screen = self.eat(Kw::SCREEN);
                    let mut arguments = Vec::new();
                    if self.is(Kw::LParen) {
                        let (x, y, _) = self.coordinates()?;
                        self.expect(Kw::Minus)?;
                        let (x2, y2, _) = self.coordinates()?;
                        arguments.extend([x, y, x2, y2]);
                        arguments.extend(self.rest(false)?);
                    }
                    self.runtime(if screen { "VIEW_SCREEN" } else { "VIEW" }, arguments, span);
                }
            }
            Kw::WINDOW => {
                self.eat(Kw::SCREEN);
                let mut arguments = Vec::new();
                if self.is(Kw::LParen) {
                    let (x, y, _) = self.coordinates()?;
                    self.expect(Kw::Minus)?;
                    let (x2, y2, _) = self.coordinates()?;
                    arguments.extend([x, y, x2, y2]);
                }
                self.runtime("WINDOW", arguments, span);
            }
            Kw::COM | Kw::SIGNAL | Kw::STRIG | Kw::PEN | Kw::TIMER | Kw::UEVENT => {
                let number = if matches!(k, Kw::COM | Kw::SIGNAL | Kw::STRIG) && self.is(Kw::LParen) {
                    self.i += 1;
                    let number = self.exp()?;
                    self.expect(Kw::RParen)?;
                    Some(number)
                } else {
                    None
                };
                let verb = self.switch_verb()?;
                let device = match k {
                    Kw::COM => "COM",
                    Kw::SIGNAL => "SIGNAL",
                    Kw::STRIG => "STRIG",
                    Kw::PEN => "PEN",
                    Kw::TIMER => "TIMER",
                    _ => "UEVENT",
                };
                self.event(device, number, verb, None, span);
            }
            Kw::KEY => {
                if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::LIST)) {
                    let at = self.take();
                    let word = if self.kinds[at] == Tok::Kw(Kw::ON) { "ON" } else if self.kinds[at] == Tok::Kw(Kw::OFF) { "OFF" } else { "LIST" };
                    self.runtime("KEY", vec![Expr::Name(word.into(), self.span(at))], span);
                } else {
                    let number = self.exp()?;
                    if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::STOP)) {
                        let verb = self.switch_verb()?;
                        self.event("KEY", Some(number), verb, None, span);
                    } else {
                        self.expect(Kw::Comma)?;
                        let text = self.exp()?;
                        self.runtime("KEY", vec![number, text], span);
                    }
                }
            }
            Kw::LOCK | Kw::UNLOCK => {
                // LOCK [#]file [, record | [start] TO end]: [file], [file, record] or [file, start|omitted, end]
                self.eat(Kw::Lbs);
                let mut arguments = vec![self.exp()?];
                if self.eat(Kw::Comma) {
                    if self.is(Kw::TO) {
                        arguments.push(Expr::Omitted(span));
                    } else {
                        arguments.push(self.exp()?);
                    }
                    if self.eat(Kw::TO) {
                        arguments.push(self.exp()?);
                    }
                }
                self.runtime(if k == Kw::LOCK { "LOCK" } else { "UNLOCK" }, arguments, span);
            }
            Kw::IOCTL => {
                self.eat(Kw::Lbs);
                let file = self.exp()?;
                self.expect(Kw::Comma)?;
                let text = self.exp()?;
                self.runtime("IOCTL", vec![file, text], span);
            }
            Kw::FIELD => {
                // FIELD [#]file, width AS variable [, width AS variable]...: [file, width, variable, ...]
                self.eat(Kw::Lbs);
                let mut arguments = vec![self.exp()?];
                loop {
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                    self.expect(Kw::AS)?;
                    arguments.push(self.name_ref()?);
                    if !self.is(Kw::Comma) {
                        break;
                    }
                }
                self.runtime("FIELD", arguments, span);
            }
            _ => return Err(keyword),
        }
        Ok(())
    }
}
