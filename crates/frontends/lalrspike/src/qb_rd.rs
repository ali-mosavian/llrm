//! The QB grammar of `qb.lalrpop` as a recursive-descent parser over the kinds of a
//! pre-lexed token vector. It builds the same generic tree, rule for rule, so the
//! two parsers can be compared on speed and on what they accept.
//!
//! Where the LR automaton decides with a token of lookahead after a shared prefix, this
//! parser reads the prefix and then looks: a name's parentheses and fields, `foo (a) * b, c`,
//! `GET (x,y)-(x,y), a` against `GET (n), x`.

use crate::qb_classes::*;
use crate::qb_kw::Kw;
use crate::qb_tokens::Tok;
use crate::qb_tree::Tree;

/// A tree, or the index of the token the parse stopped at.
type R = Result<Tree, usize>;

/// Which primary an expression may begin with: the grammar's `P`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum First {
    Full,
    /// not a parenthesis: `foo (a), b` leaves it to the name
    NoParen,
    /// a PRINT item that follows another: no sign, no NOT, no parenthesis, no `mod.x`
    Joined,
    /// `ON ERROR GOTO` is the trap, so no bare ERROR
    NoErr,
}

pub fn parse(tokens: &[Tok]) -> R {
    let mut parser = Parser { t: tokens, i: 0, injected: None };
    let mut lines = Vec::new();
    while parser.i < tokens.len() {
        lines.push(parser.line()?);
    }
    Ok(Tree::List(lines))
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
    /// A primary already read, for `foo (a) * b`: the next `prim` returns it.
    injected: Option<Tree>,
}

fn node(rule: &'static str, children: Vec<Tree>) -> Tree {
    Tree::Node(rule, children)
}

fn bin(l: Tree, o: Tree, r: Tree) -> Tree {
    node("Binary", vec![l, o, r])
}

impl Parser<'_> {
    // ---- tokens -------------------------------------------------------------------------

    fn kw_n(&self, n: usize) -> Option<Kw> {
        match self.t.get(self.i + n) {
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
        self.t.get(self.i + n)
    }

    fn period_n(&self, n: usize) -> bool {
        matches!(self.tok_n(n), Some(Tok::Period))
    }


    fn eat(&mut self, k: Kw) -> bool {
        let found = self.is(k);
        self.i += usize::from(found);
        found
    }

    fn expect(&mut self, k: Kw) -> Result<(), usize> {
        if self.eat(k) { Ok(()) } else { Err(self.i) }
    }

    fn fail<T>(&self) -> Result<T, usize> {
        Err(self.i)
    }

    /// The reserved word at the cursor, as a leaf.
    fn word(&mut self) -> R {
        match self.kw() {
            Some(k) => {
                self.i += 1;
                Ok(Tree::Kw(k))
            }
            None => self.fail(),
        }
    }

    fn leaf(&mut self, want: &Tok) -> R {
        if self.tok_n(0) == Some(want) {
            self.i += 1;
            Ok(Tree::At(self.i - 1))
        } else {
            self.fail()
        }
    }

    fn ident(&mut self) -> R {
        self.leaf(&Tok::Ident)
    }

    fn int(&mut self) -> R {
        self.leaf(&Tok::Int)
    }

    fn string(&mut self) -> R {
        self.leaf(&Tok::Str)
    }

    fn lab_ln(&mut self) -> R {
        match self.tok_n(0) {
            Some(Tok::Ident | Tok::Int) => {
                self.i += 1;
                Ok(Tree::At(self.i - 1))
            }
            _ => self.fail(),
        }
    }

    fn sep(&mut self, mut item: impl FnMut(&mut Self) -> R) -> R {
        let mut list = vec![item(self)?];
        while self.eat(Kw::Comma) {
            list.push(item(self)?);
        }
        Ok(Tree::List(list))
    }

    fn opt_kw(&mut self, k: Kw) -> Tree {
        if self.eat(k) { Tree::Kw(k) } else { Tree::Empty }
    }

    // ---- lines and statement lists -------------------------------------------------------

    fn line(&mut self) -> R {
        if self.eat(Kw::NewLine) {
            return Ok(Tree::Empty);
        }
        let start = self.line_start()?;
        self.expect(Kw::NewLine)?;
        Ok(start)
    }

    fn line_start(&mut self) -> R {
        match self.tok_n(0) {
            Some(Tok::Int) => {
                let n = self.int()?;
                let s = self.stmts()?;
                Ok(node("Label", vec![n, s]))
            }
            Some(Tok::Ident) if matches!(self.kw_n(1), Some(Kw::Colon)) => {
                let n = self.ident()?;
                self.i += 1;
                let s = self.stmts()?;
                Ok(node("Label", vec![n, s]))
            }
            Some(Tok::Ident) if matches!(self.kw_n(1), Some(Kw::NewLine)) => {
                let n = self.ident()?;
                Ok(node("Call", vec![n]))
            }
            _ if self.is(Kw::Colon) => {
                self.i += 1;
                self.stmts()
            }
            _ => self.stmt_item(false),
        }
    }

    fn stmts(&mut self) -> R {
        if self.is(Kw::NewLine) { Ok(Tree::Empty) } else { self.stmt_item(true) }
    }

    fn stmts_after_head(&mut self) -> R {
        if matches!(self.tok_n(0), Some(Tok::Int)) {
            let n = self.int()?;
            let t = self.tail()?;
            return Ok(node("ThenLine", vec![n, t]));
        }
        self.stmts()
    }

    fn tail(&mut self) -> R {
        match self.kw() {
            Some(Kw::NewLine) => Ok(Tree::Empty),
            Some(Kw::Colon) => {
                self.i += 1;
                self.stmts()
            }
            Some(Kw::ELSE) => {
                self.i += 1;
                let s = self.stmts_after_head()?;
                Ok(node("ELSE", vec![s]))
            }
            _ => self.fail(),
        }
    }

    fn tail_colon(&mut self) -> R {
        match self.kw() {
            Some(Kw::NewLine) => Ok(Tree::Empty),
            Some(Kw::Colon) => {
                self.i += 1;
                self.stmts()
            }
            _ => self.fail(),
        }
    }

    /// One statement and what follows it on the line; `bare` lets a lone name be a call.
    fn stmt_item(&mut self, bare: bool) -> R {
        match self.kw() {
            Some(Kw::IF) => {
                self.i += 1;
                let c = self.exp()?;
                if self.eat(Kw::THEN) {
                    let s = self.stmts_after_head()?;
                    Ok(Tree::List(vec![node("IF", vec![c]), s]))
                } else if self.eat(Kw::GOTO) {
                    let l = self.lab_ln()?;
                    let t = self.tail()?;
                    Ok(Tree::List(vec![node("IFGOTO", vec![c, l]), t]))
                } else {
                    self.fail()
                }
            }
            Some(Kw::ELSEIF) => {
                self.i += 1;
                let c = self.exp()?;
                self.expect(Kw::THEN)?;
                let s = self.stmts_after_head()?;
                Ok(Tree::List(vec![node("ELSEIF", vec![c]), s]))
            }
            Some(Kw::ELSE) => {
                self.i += 1;
                let s = self.stmts_after_head()?;
                Ok(node("ELSE", vec![s]))
            }
            Some(Kw::DATA) if !self.is_n(1, Kw::EQ) && !self.is_n(1, Kw::AS) => {
                let d = self.data()?;
                let t = self.tail_colon()?;
                Ok(Tree::List(vec![d, t]))
            }
            _ => {
                if bare
                    && matches!(self.tok_n(0), Some(Tok::Ident))
                    && matches!(self.kw_n(1), Some(Kw::NewLine | Kw::Colon | Kw::ELSE))
                {
                    let n = self.ident()?;
                    let t = self.tail()?;
                    return Ok(Tree::List(vec![node("Call", vec![n]), t]));
                }
                let s = self.simple()?;
                let t = self.tail()?;
                Ok(Tree::List(vec![s, t]))
            }
        }
    }

    /// DATA is text to the end of the statement: any tokens between the commas.
    fn data(&mut self) -> R {
        self.i += 1;
        let mut items = Vec::new();
        loop {
            if self.data_first() {
                self.i += 1;
                while self.data_rest() {
                    self.i += 1;
                }
            }
            items.push(Tree::Empty);
            if !self.eat(Kw::Comma) {
                break;
            }
        }
        Ok(node("DATA", vec![Tree::List(items)]))
    }

    fn data_first(&self) -> bool {
        match self.tok_n(0) {
            Some(Tok::Kw(k)) => class(*k) & DATA_FIRST != 0,
            Some(Tok::Period) | None => false,
            Some(_) => true,
        }
    }

    fn data_rest(&self) -> bool {
        match self.tok_n(0) {
            Some(Tok::Kw(k)) => class(*k) & DATA_FIRST != 0 || matches!(k, Kw::AS | Kw::EQ),
            None => false,
            Some(_) => true,
        }
    }

    // ---- statements ------------------------------------------------------------------------

    fn simple(&mut self) -> R {
        match self.tok_n(0) {
            Some(Tok::Ident) => self.ident_stmt(),
            Some(Tok::FnWord) => self.name_ref_stmt(),
            Some(Tok::Dynamic) => {
                self.i += 1;
                Ok(node("DYNAMIC", vec![]))
            }
            Some(Tok::Static) => {
                self.i += 1;
                Ok(node("STATIC", vec![]))
            }
            Some(Tok::Ext) => {
                self.i += 1;
                let r = self.letter_ranges()?;
                Ok(node("DEFEXT", vec![r]))
            }
            Some(Tok::Kw(k)) => self.kw_stmt(*k),
            _ => self.fail(),
        }
    }

    fn ident_stmt(&mut self) -> R {
        match self.kw_n(1) {
            Some(Kw::AS) => {
                let n = self.ident()?;
                let a = self.as_part()?;
                Ok(node("Member", vec![n, a]))
            }
            Some(Kw::SUB | Kw::FUNCTION) => {
                // PRIVATE (QuickrBASIC) is a name before SUB and FUNCTION, checked later
                self.i += 1;
                self.proc_stmt()
            }
            Some(Kw::LParen | Kw::EQ) => self.name_ref_stmt(),
            _ if self.period_n(1) => self.name_ref_stmt(),
            _ => {
                let n = self.ident()?;
                let a = self.exp_list_np()?;
                Ok(node("CallArgs", vec![n, a]))
            }
        }
    }

    /// A statement that begins with a name: an assignment, a call by its field, or
    /// `foo (a)` which is a call or an argument, as the symbol table says.
    fn name_ref_stmt(&mut self) -> R {
        let (mut r, mut applied) = self.name_base()?;
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            let after = self.t.get(self.i + 2);
            self.i += 1;
            let f = self.field_name()?;
            match after {
                Some(Tok::Kw(Kw::LParen)) => {
                    self.i += 1;
                    let a = self.arg_list()?;
                    self.expect(Kw::RParen)?;
                    r = node("FieldApply", vec![r, f, a]);
                    applied = true;
                }
                Some(Tok::Period | Tok::Kw(Kw::EQ)) => {
                    r = node("Field", vec![r, f]);
                    applied = false;
                }
                _ => {
                    if matches!(self.kw(), Some(Kw::NewLine | Kw::Colon | Kw::ELSE)) {
                        return Ok(node("CallField", vec![r, f]));
                    }
                    let a = self.exp_list_np()?;
                    return Ok(node("CallFieldArgs", vec![r, f, a]));
                }
            }
        }
        if self.eat(Kw::EQ) {
            let e = self.exp()?;
            return Ok(node("Assign", vec![r, e]));
        }
        if !applied {
            return self.fail();
        }
        match self.kw() {
            Some(Kw::AS) => {
                let a = self.as_part()?;
                Ok(node("Member", vec![r, a]))
            }
            Some(Kw::Comma) => {
                self.i += 1;
                let a = self.exp_list()?;
                Ok(node("CallAppliedList", vec![r, a]))
            }
            Some(Kw::Add | Kw::Minus | Kw::MOD | Kw::Idiv | Kw::Mult | Kw::Div | Kw::Pwr) => {
                self.injected = Some(r);
                let e = self.add(First::Full)?;
                if self.eat(Kw::Comma) {
                    let a = self.exp_list()?;
                    Ok(node("CallAppliedExpList", vec![e, a]))
                } else {
                    Ok(node("CallAppliedExp", vec![e]))
                }
            }
            _ => Ok(node("CallApplied", vec![r])),
        }
    }

    fn kw_stmt(&mut self, k: Kw) -> R {
        let c = class(k);
        if self.is_n(1, Kw::AS) && c & ANY_WORD != 0 {
            let w = self.word()?;
            let a = self.as_part()?;
            return Ok(node("Member", vec![w, a]));
        }
        if c & FN_WORD != 0 && self.is_n(1, Kw::LParen) {
            let w = self.word()?;
            self.i += 1;
            let d = self.arg_list()?;
            self.expect(Kw::RParen)?;
            let a = self.as_part()?;
            return Ok(node("Member", vec![w, d, a]));
        }
        if self.is_n(1, Kw::EQ) {
            if c & CTX_WORD != 0 {
                let w = self.word()?;
                self.i += 1;
                let e = self.exp()?;
                return Ok(node("Assign", vec![node("Name", vec![w]), e]));
            }
            if k == Kw::ERROR {
                self.i += 2;
                let e = self.exp()?;
                return Ok(node("Assign", vec![node("Name", vec![]), e]));
            }
            if c & FN0_VAR != 0 {
                let w = self.word()?;
                self.i += 1;
                let e = self.exp()?;
                return Ok(node("Assign", vec![w, e]));
            }
        }
        if c & OBJ_WORD != 0 && self.period_n(1) {
            return self.name_ref_stmt();
        }
        if c & FN_WORD != 0 {
            return self.name_ref_stmt();
        }
        match k {
            Kw::FOR => {
                self.i += 1;
                let v = self.name_ref_exp()?;
                self.expect(Kw::EQ)?;
                let a = self.exp()?;
                self.expect(Kw::TO)?;
                let b = self.exp()?;
                let s = if self.eat(Kw::STEP) { self.exp()? } else { Tree::Empty };
                Ok(node("FOR", vec![v, a, b, s]))
            }
            Kw::NEXT => {
                self.i += 1;
                let v = if self.starts_name_ref() { self.sep(Self::name_ref_exp)? } else { Tree::Empty };
                Ok(node("NEXT", vec![v]))
            }
            Kw::WHILE => {
                self.i += 1;
                let c = self.exp()?;
                Ok(node("WHILE", vec![c]))
            }
            Kw::WEND => self.bare("WEND"),
            Kw::DO | Kw::LOOP => {
                self.i += 1;
                let c = self.loop_cond()?;
                Ok(node(if k == Kw::DO { "DO" } else { "LOOP" }, vec![c]))
            }
            Kw::SELECT => {
                self.i += 1;
                self.expect(Kw::CASE)?;
                let e = self.exp()?;
                Ok(node("SELECT", vec![e]))
            }
            Kw::CASE => {
                self.i += 1;
                if self.eat(Kw::ELSE) {
                    return Ok(node("CASEELSE", vec![]));
                }
                let i = self.sep(Self::case_item)?;
                Ok(node("CASE", vec![i]))
            }
            Kw::GOTO | Kw::GOSUB => {
                self.i += 1;
                let l = self.lab_ln()?;
                Ok(node(if k == Kw::GOTO { "GOTO" } else { "GOSUB" }, vec![l]))
            }
            Kw::RETURN => {
                self.i += 1;
                let l = if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) { self.lab_ln()? } else { Tree::Empty };
                Ok(node("RETURN", vec![l]))
            }
            Kw::RESUME => {
                self.i += 1;
                let t = match self.tok_n(0) {
                    Some(Tok::Ident | Tok::Int) => self.lab_ln()?,
                    Some(Tok::Kw(Kw::NEXT)) => self.word()?,
                    _ => Tree::Empty,
                };
                Ok(node("RESUME", vec![t]))
            }
            Kw::EXIT => {
                self.i += 1;
                match self.kw() {
                    Some(Kw::DEF | Kw::FUNCTION | Kw::SUB | Kw::DO | Kw::FOR) => {
                        let w = self.word()?;
                        Ok(node("EXIT", vec![w]))
                    }
                    _ => self.fail(),
                }
            }
            Kw::END => {
                self.i += 1;
                match self.kw() {
                    Some(Kw::DEF | Kw::FUNCTION | Kw::IF | Kw::SELECT | Kw::SUB | Kw::TYPE) => {
                        let w = self.word()?;
                        Ok(node("END", vec![w]))
                    }
                    _ => Ok(node("END", vec![])),
                }
            }
            Kw::ENDIF => self.bare("ENDIF"),
            Kw::STOP => self.bare("STOP"),
            Kw::SYSTEM => self.bare("SYSTEM"),
            Kw::RUN => self.opt_exp_stmt("RUN"),
            Kw::ERROR => {
                self.i += 1;
                let e = self.exp()?;
                Ok(node("ERROR", vec![e]))
            }
            Kw::ON => self.on(),
            Kw::DIM | Kw::REDIM => {
                self.i += 1;
                let s = self.opt_kw(Kw::SHARED);
                let l = self.sep(Self::dim_name)?;
                Ok(node(if k == Kw::DIM { "DIM" } else { "REDIM" }, vec![s, l]))
            }
            Kw::SHARED => {
                self.i += 1;
                let l = self.sep(Self::var_name)?;
                Ok(node("SHARED", vec![l]))
            }
            Kw::STATIC => {
                self.i += 1;
                if matches!(self.kw(), Some(Kw::SUB | Kw::FUNCTION)) {
                    return self.proc_stmt();
                }
                let l = self.sep(Self::var_name)?;
                Ok(node("STATIC", vec![l]))
            }
            Kw::COMMON => {
                self.i += 1;
                let s = self.opt_kw(Kw::SHARED);
                let b = if self.eat(Kw::Div) {
                    let n = self.ident()?;
                    self.expect(Kw::Div)?;
                    n
                } else {
                    Tree::Empty
                };
                let l = self.sep(Self::var_name)?;
                Ok(node("COMMON", vec![s, b, l]))
            }
            Kw::CONST => {
                self.i += 1;
                let l = self.sep(Self::const_assign)?;
                Ok(node("CONST", vec![l]))
            }
            Kw::TYPE => {
                self.i += 1;
                let n = self.type_name()?;
                Ok(node("TYPE", vec![n]))
            }
            Kw::DEFINT | Kw::DEFLNG | Kw::DEFSNG | Kw::DEFDBL | Kw::DEFSTR => {
                self.i += 1;
                let r = self.letter_ranges()?;
                let rule = match k {
                    Kw::DEFINT => "DEFINT",
                    Kw::DEFLNG => "DEFLNG",
                    Kw::DEFSNG => "DEFSNG",
                    Kw::DEFDBL => "DEFDBL",
                    _ => "DEFSTR",
                };
                Ok(node(rule, vec![r]))
            }
            Kw::OPTION => {
                self.i += 1;
                if self.eat(Kw::BASE) {
                    let n = self.int()?;
                    return Ok(node("OPTIONBASE", vec![n]));
                }
                if matches!(self.tok_n(0), Some(Tok::Ext)) {
                    self.i += 1;
                    return Ok(node("OPTIONEXT", vec![]));
                }
                self.fail()
            }
            Kw::ERASE => {
                self.i += 1;
                let l = self.sep(Self::array_name)?;
                Ok(node("ERASE", vec![l]))
            }
            Kw::DECLARE => {
                self.i += 1;
                let kind = match self.kw() {
                    Some(Kw::FUNCTION | Kw::SUB) => self.word()?,
                    _ => return self.fail(),
                };
                let n = self.proc_name()?;
                let c = match self.kw() {
                    Some(Kw::CDECL) => {
                        self.i += 1;
                        if self.eat(Kw::ALIAS) { self.string()? } else { Tree::Empty }
                    }
                    Some(Kw::ALIAS) => {
                        self.i += 1;
                        self.string()?
                    }
                    _ => Tree::Empty,
                };
                let p = self.parms()?;
                let r = self.opt_return_as()?;
                Ok(node("DECLARE", vec![kind, n, c, p, r]))
            }
            Kw::FUNCTION | Kw::SUB => self.proc_stmt(),
            Kw::DEF => {
                self.i += 1;
                if self.eat(Kw::SEG) {
                    let e = if self.eat(Kw::EQ) { self.exp()? } else { Tree::Empty };
                    return Ok(node("DEFSEG", vec![e]));
                }
                if matches!(self.tok_n(0), Some(Tok::FnWord)) {
                    self.i += 1;
                }
                let n = self.ident()?;
                let p = self.parms()?;
                let e = if self.eat(Kw::EQ) { self.exp()? } else { Tree::Empty };
                Ok(node("DEFFN", vec![n, p, e]))
            }
            Kw::READ => {
                self.i += 1;
                let l = self.sep(Self::name_ref_exp)?;
                Ok(node("READ", vec![l]))
            }
            Kw::RESTORE => {
                self.i += 1;
                let l = if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) { self.lab_ln()? } else { Tree::Empty };
                Ok(node("RESTORE", vec![l]))
            }
            Kw::LET => {
                self.i += 1;
                let t = self.name_ref_exp()?;
                self.expect(Kw::EQ)?;
                let e = self.exp()?;
                Ok(node("Assign", vec![t, e]))
            }
            Kw::SWAP => {
                self.i += 1;
                let a = self.name_ref_exp()?;
                self.expect(Kw::Comma)?;
                let b = self.name_ref_exp()?;
                Ok(node("SWAP", vec![a, b]))
            }
            Kw::LSET | Kw::RSET => {
                self.i += 1;
                let t = self.name_ref_exp()?;
                self.expect(Kw::EQ)?;
                let e = self.exp()?;
                Ok(node(if k == Kw::LSET { "LSET" } else { "RSET" }, vec![t, e]))
            }
            Kw::MID_ => {
                self.i += 1;
                self.expect(Kw::LParen)?;
                let t = self.name_ref_exp()?;
                self.expect(Kw::Comma)?;
                let a = self.exp()?;
                let b = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
                self.expect(Kw::RParen)?;
                self.expect(Kw::EQ)?;
                let e = self.exp()?;
                Ok(node("MIDASSIGN", vec![t, a, b, e]))
            }
            Kw::DATE_ | Kw::TIME_ => {
                self.i += 1;
                self.expect(Kw::EQ)?;
                let e = self.exp()?;
                Ok(node(if k == Kw::DATE_ { "DATEASSIGN" } else { "TIMEASSIGN" }, vec![e]))
            }
            Kw::RANDOMIZE => self.opt_exp_stmt("RANDOMIZE"),
            Kw::PRINT => {
                self.i += 1;
                let f = self.channel_comma()?;
                let l = self.print_list()?;
                Ok(node("PRINT", vec![f, l]))
            }
            Kw::LPRINT => {
                self.i += 1;
                let l = self.print_list()?;
                Ok(node("LPRINT", vec![l]))
            }
            Kw::WRITE => {
                self.i += 1;
                let f = self.channel_comma()?;
                let l = if self.starts_exp() {
                    let mut items = vec![self.exp()?];
                    while self.is(Kw::Comma) || self.is(Kw::SColon) {
                        self.i += 1;
                        items.push(self.exp()?);
                    }
                    Tree::List(items)
                } else {
                    Tree::Empty
                };
                Ok(node("WRITE", vec![f, l]))
            }
            Kw::INPUT => {
                self.i += 1;
                let p = self.input_prefix()?;
                let l = self.sep(Self::name_ref_exp)?;
                Ok(node("INPUT", vec![p, l]))
            }
            Kw::LINE => {
                self.i += 1;
                if self.eat(Kw::INPUT) {
                    let p = self.input_prefix()?;
                    let v = self.name_ref_exp()?;
                    return Ok(node("LINEINPUT", vec![p, v]));
                }
                let c = if self.is(Kw::Minus) { Tree::Empty } else { self.coord_step()? };
                self.expect(Kw::Minus)?;
                let d = self.coord_step()?;
                let r = self.opt_rest()?;
                Ok(node("LINE", vec![c, d, r]))
            }
            Kw::OPEN => {
                self.i += 1;
                let f = self.exp()?;
                let t = self.open_tail()?;
                Ok(node("OPEN", vec![f, t]))
            }
            Kw::CLOSE => {
                self.i += 1;
                let l = if self.starts_filenum() { self.sep(Self::filenum)? } else { Tree::Empty };
                Ok(node("CLOSE", vec![l]))
            }
            Kw::FIELD => {
                self.i += 1;
                let f = self.filenum()?;
                let mut items = Vec::new();
                while self.eat(Kw::Comma) {
                    let w = self.exp()?;
                    self.expect(Kw::AS)?;
                    let v = self.name_ref_exp()?;
                    items.push(node("Field", vec![w, v]));
                }
                if items.is_empty() {
                    return self.fail();
                }
                Ok(node("FIELD", vec![f, Tree::List(items)]))
            }
            Kw::GET | Kw::PUT => self.get_put(k),
            Kw::SEEK | Kw::IOCTL => {
                self.i += 1;
                let f = self.filenum()?;
                self.expect(Kw::Comma)?;
                let e = self.exp()?;
                Ok(node(if k == Kw::SEEK { "SEEK" } else { "IOCTL" }, vec![f, e]))
            }
            Kw::LOCK | Kw::UNLOCK => {
                self.i += 1;
                let f = self.filenum()?;
                let r = if self.eat(Kw::Comma) {
                    if self.eat(Kw::TO) {
                        let b = self.exp()?;
                        node("Range", vec![b])
                    } else {
                        let a = self.exp()?;
                        let b = if self.eat(Kw::TO) { self.exp()? } else { Tree::Empty };
                        node("Range", vec![a, b])
                    }
                } else {
                    Tree::Empty
                };
                Ok(node(if k == Kw::LOCK { "LOCK" } else { "UNLOCK" }, vec![f, r]))
            }
            Kw::NAME => {
                self.i += 1;
                let a = self.exp()?;
                self.expect(Kw::AS)?;
                let b = self.exp()?;
                Ok(node("NAME", vec![a, b]))
            }
            Kw::KILL => self.one_exp("KILL"),
            Kw::MKDIR => self.one_exp("MKDIR"),
            Kw::CHDIR => self.one_exp("CHDIR"),
            Kw::RMDIR => self.one_exp("RMDIR"),
            Kw::CHAIN => self.one_exp("CHAIN"),
            Kw::ENVIRON => self.one_exp("ENVIRON"),
            Kw::DRAW => self.one_exp("DRAW"),
            Kw::FILES => self.opt_exp_stmt("FILES"),
            Kw::SHELL => self.opt_exp_stmt("SHELL"),
            Kw::CLS => self.opt_exp_stmt("CLS"),
            Kw::SLEEP => self.opt_exp_stmt("SLEEP"),
            Kw::RESET => self.bare("RESET"),
            Kw::BLOAD => {
                self.i += 1;
                let a = self.exp()?;
                let b = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
                Ok(node("BLOAD", vec![a, b]))
            }
            Kw::BSAVE => {
                self.i += 1;
                let a = self.exp()?;
                self.expect(Kw::Comma)?;
                let b = self.exp()?;
                self.expect(Kw::Comma)?;
                let c = self.exp()?;
                Ok(node("BSAVE", vec![a, b, c]))
            }
            Kw::WIDTH => {
                self.i += 1;
                if self.eat(Kw::Lbs) {
                    let a = self.exp()?;
                    self.expect(Kw::Comma)?;
                    let b = self.exp()?;
                    return Ok(node("WIDTHFILE", vec![a, b]));
                }
                if self.eat(Kw::LPRINT) {
                    let e = self.exp()?;
                    return Ok(node("WIDTHLPRINT", vec![e]));
                }
                if self.eat(Kw::Comma) {
                    let b = self.exp()?;
                    return Ok(node("WIDTH", vec![b]));
                }
                let a = self.exp()?;
                let b = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
                Ok(node("WIDTH", vec![a, b]))
            }
            Kw::PSET | Kw::PRESET | Kw::PAINT => {
                self.i += 1;
                let c = self.coord_step()?;
                let r = self.opt_rest()?;
                let rule = match k {
                    Kw::PSET => "PSET",
                    Kw::PRESET => "PRESET",
                    _ => "PAINT",
                };
                Ok(node(rule, vec![c, r]))
            }
            Kw::CIRCLE => {
                self.i += 1;
                let c = self.coord_step()?;
                self.expect(Kw::Comma)?;
                let e = self.exp()?;
                let r = self.opt_rest()?;
                Ok(node("CIRCLE", vec![c, e, r]))
            }
            Kw::VIEW => {
                self.i += 1;
                let w = self.view_rest()?;
                Ok(node("VIEW", vec![w]))
            }
            Kw::WINDOW => {
                self.i += 1;
                let w = if self.eat(Kw::SCREEN) {
                    let a = self.coord_pair()?;
                    self.expect(Kw::Minus)?;
                    let b = self.coord_pair()?;
                    node("Screen", vec![a, b])
                } else if self.is(Kw::LParen) {
                    let a = self.coord_pair()?;
                    self.expect(Kw::Minus)?;
                    let b = self.coord_pair()?;
                    node("World", vec![a, b])
                } else {
                    Tree::Empty
                };
                Ok(node("WINDOW", vec![w]))
            }
            Kw::SCREEN | Kw::COLOR | Kw::LOCATE | Kw::CLEAR => {
                self.i += 1;
                let a = self.sep(Self::opt_exp)?;
                let rule = match k {
                    Kw::SCREEN => "SCREEN",
                    Kw::COLOR => "COLOR",
                    Kw::LOCATE => "LOCATE",
                    _ => "CLEAR",
                };
                Ok(node(rule, vec![a]))
            }
            Kw::PALETTE => {
                self.i += 1;
                let w = if self.eat(Kw::USING) {
                    let v = self.name_ref_exp()?;
                    node("Using", vec![v])
                } else if self.starts_exp() {
                    let a = self.exp()?;
                    self.expect(Kw::Comma)?;
                    let b = self.exp()?;
                    Tree::List(vec![a, b])
                } else {
                    Tree::Empty
                };
                Ok(node("PALETTE", vec![w]))
            }
            Kw::PCOPY | Kw::OUT | Kw::POKE | Kw::SOUND => {
                self.i += 1;
                let a = self.exp()?;
                self.expect(Kw::Comma)?;
                let b = self.exp()?;
                let rule = match k {
                    Kw::PCOPY => "PCOPY",
                    Kw::OUT => "OUT",
                    Kw::POKE => "POKE",
                    _ => "SOUND",
                };
                Ok(node(rule, vec![a, b]))
            }
            Kw::BEEP => self.bare("BEEP"),
            Kw::TRON => self.bare("TRON"),
            Kw::TROFF => self.bare("TROFF"),
            Kw::PLAY => {
                self.i += 1;
                if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::STOP)) {
                    let s = self.word()?;
                    return Ok(node("PLAYSWITCH", vec![s]));
                }
                let e = self.exp()?;
                Ok(node("PLAY", vec![e]))
            }
            Kw::WAIT => {
                self.i += 1;
                let a = self.exp()?;
                self.expect(Kw::Comma)?;
                let b = self.exp()?;
                let c = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
                Ok(node("WAIT", vec![a, b, c]))
            }
            Kw::CALL | Kw::CALLS => {
                self.i += 1;
                let n = self.proc_name()?;
                let a = if self.eat(Kw::LParen) {
                    let a = if self.is(Kw::RParen) {
                        Tree::Empty
                    } else if k == Kw::CALL {
                        self.sep(Self::call_arg)?
                    } else {
                        self.sep(Self::exp)?
                    };
                    self.expect(Kw::RParen)?;
                    a
                } else {
                    Tree::Empty
                };
                Ok(node(if k == Kw::CALL { "CALL" } else { "CALLS" }, vec![n, a]))
            }
            Kw::COM | Kw::SIGNAL | Kw::STRIG | Kw::PEN | Kw::TIMER | Kw::UEVENT => self.event_switch(k),
            Kw::KEY => {
                self.i += 1;
                if matches!(self.kw(), Some(Kw::OFF | Kw::ON | Kw::LIST)) {
                    let w = self.word()?;
                    return Ok(node("KEY", vec![w]));
                }
                let n = self.exp()?;
                let t = if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::STOP)) {
                    self.word()?
                } else {
                    self.expect(Kw::Comma)?;
                    self.exp()?
                };
                Ok(node("KEY", vec![n, t]))
            }
            Kw::REM => self.bare("REM"),
            _ => self.fail(),
        }
    }

    fn bare(&mut self, rule: &'static str) -> R {
        self.i += 1;
        Ok(node(rule, vec![]))
    }

    fn one_exp(&mut self, rule: &'static str) -> R {
        self.i += 1;
        let e = self.exp()?;
        Ok(node(rule, vec![e]))
    }

    fn opt_exp_stmt(&mut self, rule: &'static str) -> R {
        self.i += 1;
        let e = if self.starts_exp() { self.exp()? } else { Tree::Empty };
        Ok(node(rule, vec![e]))
    }

    fn loop_cond(&mut self) -> R {
        match self.kw() {
            Some(k @ (Kw::WHILE | Kw::UNTIL)) => {
                self.i += 1;
                let e = self.exp()?;
                Ok(node(if k == Kw::WHILE { "WHILE" } else { "UNTIL" }, vec![e]))
            }
            _ => Ok(Tree::Empty),
        }
    }

    fn case_item(&mut self) -> R {
        if self.eat(Kw::IS) {
            let r = self.rel_op()?;
            let e = self.exp()?;
            return Ok(node("Is", vec![r, e]));
        }
        if self.is_rel_op() {
            let r = self.rel_op()?;
            let e = self.exp()?;
            return Ok(node("Is", vec![r, e]));
        }
        let a = self.exp()?;
        let b = if self.eat(Kw::TO) { self.exp()? } else { Tree::Empty };
        Ok(node("Item", vec![a, b]))
    }

    fn on(&mut self) -> R {
        self.i += 1;
        let local = self.is(Kw::LOCAL);
        if (self.is(Kw::ERROR) && (self.is_n(1, Kw::GOTO) || self.is_n(1, Kw::RESUME)))
            || (local && self.is_n(1, Kw::ERROR))
        {
            self.i += 1 + usize::from(local);
            if self.eat(Kw::GOTO) {
                let l = self.lab_ln()?;
                return Ok(node(if local { "ONLOCALERROR" } else { "ONERROR" }, vec![l]));
            }
            self.expect(Kw::RESUME)?;
            self.expect(Kw::NEXT)?;
            return Ok(node(if local { "ONLOCALERRORRESUME" } else { "ONERRORRESUME" }, vec![]));
        }
        let event = match self.kw() {
            Some(k @ (Kw::KEY | Kw::COM | Kw::SIGNAL | Kw::TIMER)) if self.is_n(1, Kw::LParen) => Some((k, true)),
            Some(k @ (Kw::PEN | Kw::UEVENT)) if self.is_n(1, Kw::GOSUB) => Some((k, false)),
            _ => None,
        };
        if let Some((k, paren)) = event {
            self.i += 1;
            let rule = match k {
                Kw::KEY => "KEY",
                Kw::COM => "COM",
                Kw::SIGNAL => "SIGNAL",
                Kw::TIMER => "TIMER",
                Kw::PEN => "PEN",
                _ => "UEVENT",
            };
            let e = node(rule, if paren { vec![self.paren1()?] } else { vec![] });
            self.expect(Kw::GOSUB)?;
            let l = self.lab_ln()?;
            return Ok(node("ONEVENT", vec![e, l]));
        }
        let e = self.imp(First::NoErr)?;
        let g = match self.kw() {
            Some(Kw::GOTO | Kw::GOSUB) => self.word()?,
            _ => return self.fail(),
        };
        let l = self.sep(Self::lab_ln)?;
        Ok(node("ON", vec![e, g, l]))
    }

    fn event_switch(&mut self, k: Kw) -> R {
        self.i += 1;
        let has_number = matches!(k, Kw::COM | Kw::SIGNAL | Kw::STRIG) && self.is(Kw::LParen);
        let n = if has_number { Some(self.paren1()?) } else { None };
        if !matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::STOP)) {
            return self.fail();
        }
        if k == Kw::STRIG && n.is_none() {
            if self.is(Kw::STOP) {
                return self.fail();
            }
            let s = self.word()?;
            return Ok(node("STRIG", vec![s]));
        }
        let s = self.word()?;
        let rule = match k {
            Kw::COM => "COMSWITCH",
            Kw::SIGNAL => "SIGNALSWITCH",
            Kw::STRIG => "STRIGSWITCH",
            Kw::PEN => "PENSWITCH",
            Kw::TIMER => "TIMERSWITCH",
            _ => "UEVENTSWITCH",
        };
        match n {
            Some(n) => Ok(node(rule, vec![n, s])),
            None if matches!(k, Kw::COM | Kw::SIGNAL) => self.fail(),
            None => Ok(node(rule, vec![s])),
        }
    }

    fn paren1(&mut self) -> R {
        self.expect(Kw::LParen)?;
        let e = self.exp()?;
        self.expect(Kw::RParen)?;
        Ok(e)
    }

    fn call_arg(&mut self) -> R {
        self.opt_kw(Kw::BYVAL);
        self.opt_kw(Kw::SEG);
        let e = self.exp()?;
        Ok(node("CallArg", vec![e]))
    }

    // ---- declarations -----------------------------------------------------------------------

    fn proc_stmt(&mut self) -> R {
        match self.kw() {
            Some(Kw::FUNCTION) => {
                self.i += 1;
                let n = self.proc_name()?;
                let p = self.parms()?;
                let r = self.opt_return_as()?;
                let s = self.opt_kw(Kw::STATIC);
                Ok(node("FUNCTION", vec![n, p, r, s]))
            }
            Some(Kw::SUB) => {
                self.i += 1;
                let n = self.proc_name()?;
                let p = self.parms()?;
                let s = self.opt_kw(Kw::STATIC);
                Ok(node("SUB", vec![n, p, s]))
            }
            _ => self.fail(),
        }
    }

    fn proc_name(&mut self) -> R {
        let base = self.proc_word()?;
        self.dotted(base)
    }

    fn proc_word(&mut self) -> R {
        match self.tok_n(0) {
            Some(Tok::Ident) => self.ident(),
            Some(Tok::Kw(k)) if class(*k) & FN_WORD != 0 => self.word(),
            _ => self.fail(),
        }
    }

    fn dotted(&mut self, mut r: Tree) -> R {
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            self.i += 1;
            let f = self.field_name()?;
            r = node("Field", vec![r, f]);
        }
        Ok(r)
    }

    fn type_name(&mut self) -> R {
        self.proc_word()
    }

    fn parms(&mut self) -> R {
        if !self.eat(Kw::LParen) {
            return Ok(Tree::Empty);
        }
        let l = if self.is(Kw::RParen) { Tree::Empty } else { self.sep(Self::parm)? };
        self.expect(Kw::RParen)?;
        Ok(l)
    }

    fn parm(&mut self) -> R {
        let m = match self.kw() {
            Some(Kw::BYVAL | Kw::SEG) => self.word()?,
            _ => Tree::Empty,
        };
        let i = self.var_word_base()?;
        if self.eat(Kw::LParen) {
            self.arg_list()?;
            self.expect(Kw::RParen)?;
        }
        let a = self.opt_as()?;
        Ok(node("Parm", vec![m, i, a]))
    }

    fn opt_return_as(&mut self) -> R {
        if self.eat(Kw::AS) { self.as_clause() } else { Ok(Tree::Empty) }
    }

    fn opt_as(&mut self) -> R {
        if self.is(Kw::AS) { self.as_part() } else { Ok(Tree::Empty) }
    }

    /// `AS VOLATILE INTEGER * 4`: a modifier is a name before the type.
    fn as_part(&mut self) -> R {
        self.expect(Kw::AS)?;
        while matches!(self.tok_n(0), Some(Tok::Ident))
            && match self.tok_n(1) {
                Some(Tok::Ident) => true,
                Some(Tok::Kw(k)) => {
                    class(*k) & FN_WORD != 0
                        || matches!(k, Kw::INTEGER | Kw::LONG | Kw::SINGLE | Kw::DOUBLE | Kw::STRING | Kw::ANY)
                }
                _ => false,
            }
        {
            self.i += 1;
        }
        let c = self.as_clause()?;
        let n = if self.eat(Kw::Mult) { self.exp()? } else { Tree::Empty };
        Ok(node("As", vec![c, n]))
    }

    fn as_clause(&mut self) -> R {
        let rule = match self.kw() {
            Some(Kw::INTEGER) => "INTEGER",
            Some(Kw::LONG) => "LONG",
            Some(Kw::SINGLE) => "SINGLE",
            Some(Kw::DOUBLE) => "DOUBLE",
            Some(Kw::STRING) => "STRING",
            Some(Kw::ANY) => "ANY",
            _ => {
                let n = self.type_name()?;
                return Ok(node("TypeName", vec![n]));
            }
        };
        self.i += 1;
        Ok(node(rule, vec![]))
    }

    fn var_word_base(&mut self) -> R {
        match self.tok_n(0) {
            Some(Tok::Ident) => self.ident(),
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                if c & FN_WORD != 0 {
                    self.word()
                } else if c & CTX_WORD != 0 {
                    let w = self.word()?;
                    Ok(node("Name", vec![w]))
                } else if *k == Kw::ERROR {
                    self.i += 1;
                    Ok(node("Name", vec![]))
                } else {
                    self.fail()
                }
            }
            _ => self.fail(),
        }
    }

    fn var_word(&mut self) -> R {
        let base = self.var_word_base()?;
        self.dotted(base)
    }

    fn dim_name(&mut self) -> R {
        let n = self.var_word()?;
        let d = if self.eat(Kw::LParen) {
            let d = if self.is(Kw::RParen) { Tree::Empty } else { self.sep(Self::arg_item)? };
            self.expect(Kw::RParen)?;
            d
        } else {
            Tree::Empty
        };
        let a = self.opt_as()?;
        Ok(node("Dim", vec![n, d, a]))
    }

    fn empty_parens(&mut self) -> Result<(), usize> {
        if self.is(Kw::LParen) && self.is_n(1, Kw::RParen) {
            self.i += 2;
        }
        Ok(())
    }

    fn var_name(&mut self) -> R {
        let n = self.var_word()?;
        self.empty_parens()?;
        let a = self.opt_as()?;
        Ok(node("Var", vec![n, a]))
    }

    fn array_name(&mut self) -> R {
        let n = self.var_word()?;
        self.empty_parens()?;
        Ok(n)
    }

    fn const_assign(&mut self) -> R {
        let mut i = match self.tok_n(0) {
            Some(Tok::Ident) => self.ident()?,
            Some(Tok::Kw(Kw::MOD)) => {
                self.i += 1;
                node("Name", vec![])
            }
            Some(Tok::Kw(k)) if class(*k) & CTX_WORD != 0 => {
                let w = self.word()?;
                node("Name", vec![w])
            }
            _ => return self.fail(),
        };
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            self.i += 1;
            let f = self.field_name()?;
            i = node("Field", vec![i, f]);
        }
        self.expect(Kw::EQ)?;
        let e = self.exp()?;
        Ok(node("ConstAssign", vec![i, e]))
    }

    fn letter_ranges(&mut self) -> R {
        self.sep(|p| {
            let a = p.ident()?;
            let b = if p.eat(Kw::Minus) { p.ident()? } else { Tree::Empty };
            Ok(node("Range", vec![a, b]))
        })
    }

    // ---- files and graphics ------------------------------------------------------------------

    fn filenum(&mut self) -> R {
        if self.eat(Kw::Lbs) { self.exp() } else { self.exp() }
    }

    fn starts_filenum(&self) -> bool {
        self.is(Kw::Lbs) || self.starts_exp()
    }

    fn channel_comma(&mut self) -> R {
        if self.eat(Kw::Lbs) {
            let e = self.exp()?;
            self.expect(Kw::Comma)?;
            Ok(e)
        } else {
            Ok(Tree::Empty)
        }
    }

    fn input_prefix(&mut self) -> R {
        if self.eat(Kw::Lbs) {
            let e = self.exp()?;
            self.expect(Kw::Comma)?;
            return Ok(node("Channel", vec![e]));
        }
        if self.eat(Kw::SColon) {
            return if matches!(self.tok_n(0), Some(Tok::Str)) { self.prompt_sep() } else { Ok(Tree::Empty) };
        }
        if matches!(self.tok_n(0), Some(Tok::Str)) { self.prompt_sep() } else { Ok(Tree::Empty) }
    }

    fn prompt_sep(&mut self) -> R {
        let s = self.string()?;
        if !(self.eat(Kw::Comma) || self.eat(Kw::SColon)) {
            return self.fail();
        }
        Ok(s)
    }

    fn open_tail(&mut self) -> R {
        if self.eat(Kw::Comma) {
            let f = self.filenum()?;
            self.expect(Kw::Comma)?;
            let n = self.exp()?;
            let r = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
            return Ok(node("Classic", vec![f, n, r]));
        }
        let m = if self.eat(Kw::FOR) {
            match self.tok_n(0) {
                Some(Tok::Ident) => {
                    let i = self.ident()?;
                    let t = self.ident()?;
                    let n = self.exp()?;
                    node("ISAM", vec![i, t, n])
                }
                _ => match self.kw() {
                    Some(Kw::APPEND | Kw::INPUT | Kw::OUTPUT | Kw::RANDOM | Kw::BINARY) => self.word()?,
                    _ => return self.fail(),
                },
            }
        } else {
            Tree::Empty
        };
        let a = if self.eat(Kw::ACCESS) {
            if self.eat(Kw::READ) {
                let w = self.opt_kw(Kw::WRITE);
                node("Access", vec![w])
            } else {
                self.expect(Kw::WRITE)?;
                node("Access", vec![])
            }
        } else {
            Tree::Empty
        };
        let l = if self.eat(Kw::LOCK) {
            if self.eat(Kw::READ) {
                let w = self.opt_kw(Kw::WRITE);
                node("Lock", vec![w])
            } else {
                self.expect(Kw::WRITE)?;
                node("Lock", vec![])
            }
        } else if self.eat(Kw::SHARED) {
            node("Shared", vec![])
        } else {
            Tree::Empty
        };
        self.expect(Kw::AS)?;
        let f = self.filenum()?;
        let n = if self.is(Kw::LEN) && self.is_n(1, Kw::EQ) {
            self.i += 2;
            self.exp()?
        } else {
            Tree::Empty
        };
        Ok(node("Modern", vec![m, a, l, f, n]))
    }

    fn get_put(&mut self, k: Kw) -> R {
        self.i += 1;
        let graphic = self.is(Kw::STEP) || self.is(Kw::LParen) && {
            let save = self.i;
            let found = self.coord_pair().is_ok();
            self.i = save;
            found
        };
        if graphic {
            let c = self.coord_step()?;
            if k == Kw::GET {
                self.expect(Kw::Minus)?;
                let d = self.coord_step()?;
                self.expect(Kw::Comma)?;
                let v = self.name_ref_exp()?;
                return Ok(node("GETGFX", vec![c, d, v]));
            }
            self.expect(Kw::Comma)?;
            let v = self.name_ref_exp()?;
            let a = if self.eat(Kw::Comma) {
                match self.kw() {
                    Some(Kw::AND | Kw::OR | Kw::PRESET | Kw::PSET | Kw::XOR) => self.word()?,
                    _ => return self.fail(),
                }
            } else {
                Tree::Empty
            };
            return Ok(node("PUTGFX", vec![c, v, a]));
        }
        let f = self.filenum()?;
        let t = if self.eat(Kw::Comma) {
            let r = if self.starts_exp() { self.exp()? } else { Tree::Empty };
            let v = if self.eat(Kw::Comma) { self.name_ref_exp()? } else { Tree::Empty };
            node("Record", vec![r, v])
        } else {
            Tree::Empty
        };
        Ok(node(if k == Kw::GET { "GET" } else { "PUT" }, vec![f, t]))
    }

    fn coord_step(&mut self) -> R {
        if self.eat(Kw::STEP) {
            let p = self.coord_pair()?;
            Ok(node("Step", vec![p]))
        } else {
            self.coord_pair()
        }
    }

    fn coord_pair(&mut self) -> R {
        self.expect(Kw::LParen)?;
        let a = self.exp()?;
        self.expect(Kw::Comma)?;
        let b = self.exp()?;
        self.expect(Kw::RParen)?;
        Ok(Tree::List(vec![a, b]))
    }

    fn opt_rest(&mut self) -> R {
        if !self.is(Kw::Comma) {
            return Ok(Tree::Empty);
        }
        let mut items = Vec::new();
        while self.eat(Kw::Comma) {
            items.push(self.opt_exp()?);
        }
        Ok(Tree::List(items))
    }

    fn opt_exp(&mut self) -> R {
        if self.starts_exp() { self.exp() } else { Ok(Tree::Empty) }
    }

    fn view_rest(&mut self) -> R {
        if self.eat(Kw::PRINT) {
            let r = if self.starts_exp() {
                let a = self.exp()?;
                self.expect(Kw::TO)?;
                let b = self.exp()?;
                Tree::List(vec![a, b])
            } else {
                Tree::Empty
            };
            return Ok(node("Print", vec![r]));
        }
        if self.eat(Kw::SCREEN) {
            let a = self.coord_pair()?;
            self.expect(Kw::Minus)?;
            let b = self.coord_pair()?;
            let r = self.opt_rest()?;
            return Ok(node("Screen", vec![a, b, r]));
        }
        if self.is(Kw::LParen) {
            let a = self.coord_pair()?;
            self.expect(Kw::Minus)?;
            let b = self.coord_pair()?;
            let r = self.opt_rest()?;
            return Ok(node("Port", vec![a, b, r]));
        }
        Ok(Tree::Empty)
    }

    // ---- PRINT --------------------------------------------------------------------------------

    /// `PRINT a; b, TAB(3) c USING f$; x`: the steps, the final run, and what follows USING.
    fn print_head(&mut self) -> Result<(Tree, Tree), usize> {
        let mut head = Vec::new();
        loop {
            match self.kw() {
                Some(k @ (Kw::TAB | Kw::SPC)) if self.is_n(1, Kw::LParen) => {
                    self.i += 1;
                    let a = self.paren1()?;
                    head.push(node(if k == Kw::TAB { "TAB" } else { "SPC" }, vec![a]));
                }
                Some(Kw::Comma | Kw::SColon) => {
                    self.i += 1;
                    head.push(Tree::Empty);
                }
                _ if self.starts_exp() => {
                    let run = self.print_run()?;
                    if self.eat(Kw::Comma) || self.eat(Kw::SColon) {
                        head.push(run);
                    } else {
                        return Ok((Tree::List(head), run));
                    }
                }
                _ => return Ok((Tree::List(head), Tree::Empty)),
            }
        }
    }

    fn print_list(&mut self) -> R {
        let (h, f) = self.print_head()?;
        let u = if self.eat(Kw::USING) {
            let e = self.exp()?;
            self.expect(Kw::SColon)?;
            let (h, f) = self.print_head()?;
            Tree::List(vec![e, h, f])
        } else {
            Tree::Empty
        };
        Ok(node("PrintList", vec![h, f, u]))
    }

    fn print_run(&mut self) -> R {
        let mut r = self.exp()?;
        while self.starts_primary_base() {
            let e = self.imp(First::Joined)?;
            r = node("Joined", vec![r, e]);
        }
        Ok(r)
    }

    // ---- names and argument lists ---------------------------------------------------------------

    fn field_name(&mut self) -> R {
        match self.tok_n(0) {
            Some(Tok::Ident) => self.ident(),
            Some(Tok::Kw(k)) if class(*k) & ANY_WORD != 0 => self.word(),
            _ => self.fail(),
        }
    }

    /// The head of a NameRef, and whether it is already an applied form (`name(...)`).
    fn name_base(&mut self) -> Result<(Tree, bool), usize> {
        let named = match self.tok_n(0) {
            Some(Tok::Ident) => true,
            Some(Tok::FnWord) => {
                self.i += 1;
                true
            }
            _ => false,
        };
        if named {
            let n = self.ident()?;
            if self.eat(Kw::LParen) {
                let a = self.arg_list()?;
                self.expect(Kw::RParen)?;
                return Ok((node("Apply", vec![n, a]), true));
            }
            return Ok((node("Name", vec![n]), false));
        }
        match self.tok_n(0) {
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                if c & OBJ_WORD != 0 && self.period_n(1) {
                    return Ok((self.word()?, false));
                }
                if c & FN_WORD != 0 {
                    let w = self.word()?;
                    return Ok((node("Name", vec![w]), false));
                }
                self.fail()
            }
            _ => self.fail(),
        }
    }

    /// A NameRef in an expression: every `.field` and `.field(args)` belongs to it.
    fn name_ref_exp(&mut self) -> R {
        let (mut r, _) = self.name_base()?;
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            self.i += 1;
            let f = self.field_name()?;
            if self.eat(Kw::LParen) {
                let a = self.arg_list()?;
                self.expect(Kw::RParen)?;
                r = node("FieldApply", vec![r, f, a]);
            } else {
                r = node("Field", vec![r, f]);
            }
        }
        Ok(r)
    }

    fn starts_name_ref(&self) -> bool {
        match self.tok_n(0) {
            Some(Tok::Ident | Tok::FnWord) => true,
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                c & FN_WORD != 0 || (c & OBJ_WORD != 0 && self.period_n(1))
            }
            _ => false,
        }
    }

    fn arg_list(&mut self) -> R {
        if self.is(Kw::RParen) { Ok(Tree::Empty) } else { self.exp_list() }
    }

    fn exp_list(&mut self) -> R {
        self.sep(Self::arg_item)
    }

    fn arg_item(&mut self) -> R {
        if matches!(self.kw(), Some(Kw::BYVAL | Kw::SEG)) {
            let m = self.word()?;
            let e = self.exp()?;
            return Ok(node("Mode", vec![m, e]));
        }
        let a = self.exp()?;
        if self.eat(Kw::TO) {
            let b = self.exp()?;
            return Ok(node("To", vec![a, b]));
        }
        Ok(a)
    }

    /// `foo a, b`: arguments of a call with no parentheses, the first not starting with one.
    fn exp_list_np(&mut self) -> R {
        let first = if matches!(self.kw(), Some(Kw::BYVAL | Kw::SEG)) {
            let m = self.word()?;
            let e = self.exp()?;
            node("Mode", vec![m, e])
        } else {
            self.imp(First::NoParen)?
        };
        let mut list = vec![first];
        while self.eat(Kw::Comma) {
            list.push(self.arg_item()?);
        }
        Ok(Tree::List(list))
    }

    // ---- expressions ------------------------------------------------------------------------------

    fn starts_primary_base(&self) -> bool {
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

    fn starts_exp(&self) -> bool {
        self.starts_primary_base()
            || matches!(self.kw(), Some(Kw::LParen | Kw::Add | Kw::Minus | Kw::NOT))
            || (self.is(Kw::MOD) && self.period_n(1))
    }

    fn exp(&mut self) -> R {
        self.imp(First::Full)
    }

    fn imp(&mut self, f: First) -> R {
        let mut l = self.eqv(f)?;
        while self.is(Kw::IMP) {
            let o = self.word()?;
            let r = self.eqv(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn eqv(&mut self, f: First) -> R {
        let mut l = self.xor(f)?;
        while self.is(Kw::EQV) {
            let o = self.word()?;
            let r = self.xor(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn xor(&mut self, f: First) -> R {
        let mut l = self.or(f)?;
        while self.is(Kw::XOR) {
            let o = self.word()?;
            let r = self.or(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn or(&mut self, f: First) -> R {
        let mut l = self.and(f)?;
        while self.is(Kw::OR) {
            let o = self.word()?;
            let r = self.and(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn and(&mut self, f: First) -> R {
        let mut l = self.not(f)?;
        while self.is(Kw::AND) {
            let o = self.word()?;
            let r = self.not(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn not(&mut self, f: First) -> R {
        if f != First::Joined && self.injected.is_none() && self.is(Kw::NOT) {
            let o = self.word()?;
            let e = self.not(First::Full)?;
            return Ok(node("Unary", vec![o, e]));
        }
        self.rel(f)
    }

    fn is_rel_op(&self) -> bool {
        matches!(self.tok_n(0), Some(Tok::Le | Tok::Ne | Tok::Ge | Tok::Kw(Kw::EQ | Kw::LT | Kw::GT)))
    }

    fn rel_op(&mut self) -> R {
        match self.tok_n(0) {
            Some(Tok::Le | Tok::Ne | Tok::Ge) => {
                self.i += 1;
                Ok(Tree::At(self.i - 1))
            }
            Some(Tok::Kw(Kw::EQ | Kw::LT | Kw::GT)) => self.word(),
            _ => self.fail(),
        }
    }

    fn rel(&mut self, f: First) -> R {
        let mut l = self.add(f)?;
        while self.is_rel_op() {
            let o = self.rel_op()?;
            let r = self.add(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn add(&mut self, f: First) -> R {
        let mut l = self.modulo(f)?;
        while matches!(self.kw(), Some(Kw::Add | Kw::Minus)) {
            let o = self.word()?;
            let r = self.modulo(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn modulo(&mut self, f: First) -> R {
        let mut l = self.idiv(f)?;
        while self.is(Kw::MOD) {
            let o = self.word()?;
            let r = self.idiv(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn idiv(&mut self, f: First) -> R {
        let mut l = self.mul(f)?;
        while self.is(Kw::Idiv) {
            let o = self.word()?;
            let r = self.mul(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn mul(&mut self, f: First) -> R {
        let mut l = self.neg(f)?;
        while matches!(self.kw(), Some(Kw::Mult | Kw::Div)) {
            let o = self.word()?;
            let r = self.neg(First::Full)?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn neg(&mut self, f: First) -> R {
        if f != First::Joined && self.injected.is_none() && matches!(self.kw(), Some(Kw::Add | Kw::Minus)) {
            let o = self.word()?;
            let e = self.neg(First::Full)?;
            return Ok(node("Unary", vec![o, e]));
        }
        self.pow(f)
    }

    fn pow(&mut self, f: First) -> R {
        let mut l = self.prim(f)?;
        while self.is(Kw::Pwr) {
            let o = self.word()?;
            let r = self.pow_operand()?;
            l = bin(l, o, r);
        }
        Ok(l)
    }

    fn pow_operand(&mut self) -> R {
        if matches!(self.kw(), Some(Kw::Add | Kw::Minus)) {
            let o = self.word()?;
            let e = self.pow_operand()?;
            return Ok(node("Unary", vec![o, e]));
        }
        self.prim(First::Full)
    }

    fn prim(&mut self, f: First) -> R {
        if let Some(t) = self.injected.take() {
            return Ok(t);
        }
        if self.is(Kw::LParen) {
            if !matches!(f, First::Full | First::NoErr) {
                return self.fail();
            }
            self.i += 1;
            let e = self.exp()?;
            self.expect(Kw::RParen)?;
            return Ok(node("Paren", vec![e]));
        }
        self.prim_base(f)
    }

    fn prim_base(&mut self, f: First) -> R {
        let k = match self.tok_n(0) {
            Some(Tok::Int | Tok::Real | Tok::Str) => {
                self.i += 1;
                return Ok(Tree::At(self.i - 1));
            }
            Some(Tok::Ident | Tok::FnWord) => return self.name_ref_exp(),
            Some(Tok::Kw(k)) => *k,
            _ => return self.fail(),
        };
        let c = class(k);
        if c & OBJ_WORD != 0 && self.period_n(1) {
            return self.name_ref_exp();
        }
        if k == Kw::MOD && self.period_n(1) {
            if !matches!(f, First::Full | First::NoParen) {
                return self.fail();
            }
            self.i += 2;
            let g = self.field_name()?;
            return Ok(node("Field", vec![node("Name", vec![]), g]));
        }
        if k == Kw::RND {
            self.i += 1;
            let a = if self.is(Kw::LParen) { self.paren1()? } else { Tree::Empty };
            return Ok(node("Fn", vec![a]));
        }
        if self.is_n(1, Kw::LParen) {
            let function = self.function(k, c)?;
            if let Some(t) = function {
                return Ok(t);
            }
        }
        if c & FN0 != 0 {
            self.i += 1;
            return Ok(node("Fn", vec![Tree::Kw(k)]));
        }
        if c & (FN_WORD | CTX_WORD) != 0 {
            return self.name_ref_exp_from_word(c);
        }
        if k == Kw::ERROR && f != First::NoErr {
            self.i += 1;
            return Ok(node("Name", vec![]));
        }
        self.fail()
    }

    /// A function word before a parenthesis; `None` where the word is no function of that shape.
    fn function(&mut self, k: Kw, c: u16) -> Result<Option<Tree>, usize> {
        let t = Tree::Kw(k);
        if c & FN1 != 0 {
            self.i += 2;
            let a = self.exp()?;
            self.expect(Kw::RParen)?;
            return Ok(Some(node("Fn", vec![t, a])));
        }
        if c & FN2 != 0 {
            self.i += 2;
            let a = self.exp()?;
            self.expect(Kw::Comma)?;
            let b = self.exp()?;
            self.expect(Kw::RParen)?;
            return Ok(Some(node("Fn", vec![t, a, b])));
        }
        if c & FN23 != 0 {
            self.i += 2;
            let a = self.exp()?;
            self.expect(Kw::Comma)?;
            let b = self.exp()?;
            let d = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
            self.expect(Kw::RParen)?;
            return Ok(Some(node("Fn", vec![t, a, b, d])));
        }
        if c & FN_BOUND != 0 {
            self.i += 2;
            let a = self.name_ref_exp()?;
            let b = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
            self.expect(Kw::RParen)?;
            return Ok(Some(node("Fn", vec![t, a, b])));
        }
        if c & FN_REF != 0 {
            self.i += 2;
            let a = self.name_ref_exp()?;
            self.expect(Kw::RParen)?;
            return Ok(Some(node("Fn", vec![t, a])));
        }
        match k {
            Kw::POINT => {
                self.i += 2;
                let a = self.exp()?;
                let b = if self.eat(Kw::Comma) { self.exp()? } else { Tree::Empty };
                self.expect(Kw::RParen)?;
                Ok(Some(node("Fn", vec![a, b])))
            }
            Kw::ERROR => {
                self.i += 1;
                let a = self.paren1()?;
                Ok(Some(node("Fn", vec![a])))
            }
            Kw::INPUT_ => {
                self.i += 2;
                let a = self.exp()?;
                let b = if self.eat(Kw::Comma) { self.filenum()? } else { Tree::Empty };
                self.expect(Kw::RParen)?;
                Ok(Some(node("Fn", vec![a, b])))
            }
            Kw::IOCTL_ => {
                self.i += 2;
                let a = self.filenum()?;
                self.expect(Kw::RParen)?;
                Ok(Some(node("Fn", vec![a])))
            }
            _ => Ok(None),
        }
    }

    /// A function word used as a variable (a NameRef, fields and all), or a statement word (a name).
    fn name_ref_exp_from_word(&mut self, c: u16) -> R {
        if c & FN_WORD != 0 { self.name_ref_exp() } else {
            let w = self.word()?;
            Ok(node("Name", vec![w]))
        }
    }
}
