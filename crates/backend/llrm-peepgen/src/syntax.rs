//! The `.peep` rule language, parsed.
//!
//! A file is stanzas. A stanza starts in column 0 with `meta`, `set`,
//! `group` or `rule`; its clauses (`walk`, `match`, `when`, `rewrite`) are
//! indented and may continue on further indented lines. `//` starts a
//! comment.

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Str(String),
    Punct(&'static str),
}

#[derive(Debug)]
pub struct Meta {
    pub line: usize,
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug)]
pub enum SetExpr {
    Name(String),
    Op(String),
    /// Mnemonics with a form of this shape, as x86.instr writes it.
    Shape(String),
    Reads(Vec<String>),
    Fixed,
    Bin(Box<SetExpr>, &'static str, Box<SetExpr>),
}

#[derive(Debug)]
pub struct SetDef {
    pub line: usize,
    pub name: String,
    pub expr: SetExpr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    None,
    /// `lir::Insn::is_meta`.
    Meta,
    /// An inert NOTHING without virtual edges.
    Inert,
    /// Any inert NOTHING.
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkKind {
    /// Each instruction alone, in place.
    Each,
    /// Consecutive instructions; a match consumes them.
    Window,
    /// Consecutive instructions in place; a match moves on `advance`.
    Slide,
    /// The first instruction, any run the gap guard lets through, then the rest, in place.
    Gap,
}

#[derive(Clone, Debug)]
pub struct Walk {
    pub line: usize,
    pub kind: WalkKind,
    pub skip: Skip,
    pub advance: usize,
    pub first_original: bool,
    pub resume_past: bool,
}

#[derive(Debug)]
pub struct Group {
    pub line: usize,
    pub name: String,
    pub walk: Option<Walk>,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub line: usize,
    pub name: String,
    pub pattern: Vec<Element>,
    pub guards: Vec<Guard>,
    pub rewrite: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Element {
    Insn(InsnPat),
    Gap(Call),
    End,
    /// `def v: mov v, c:mem`: the nearest instruction before the window
    /// that defines held value `v`.
    Def(String, InsnPat),
}

#[derive(Clone, Debug)]
pub struct InsnPat {
    pub bind: Option<String>,
    pub head: Head,
    pub operands: Vec<OperandPat>,
    /// How many operands are destinations, when `/` says.
    pub split: Option<usize>,
    pub rest: bool,
}

#[derive(Clone, Debug)]
pub enum Head {
    /// A mnemonic or a set.
    Name(String),
    Alt(Vec<String>),
    /// Any instruction of these operations.
    Any(Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    Reg,
    Mem,
    /// An immediate with no address.
    Imm,
    /// An immediate, relocated or not.
    Sym,
    Held,
    Any,
}

#[derive(Clone, Debug)]
pub enum OperandPat {
    Bind { name: String, kind: Option<(Class, Option<u32>)> },
    Lit { value: i64, width: Option<u32> },
    Wild,
}

#[derive(Clone, Debug)]
pub struct Guard {
    pub negate: bool,
    pub call: Call,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Vec<Arg>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Slot(usize),
    Built(usize),
    Crossed,
    Ident(String),
    Int(i64),
    /// `[16, 32]`.
    List(Vec<i64>),
    Call(Call),
}

#[derive(Clone, Debug)]
pub enum Item {
    Keep { slot: usize, over: Vec<Override> },
    Anchor { slot: usize, over: Vec<Override> },
    Drop { slot: usize },
    New { slot: usize, head: NewHead, operands: Operands, over: Vec<Override> },
}

#[derive(Clone, Debug)]
pub enum NewHead {
    Name(String),
    Copy(usize),
    Family(String, String),
}

#[derive(Clone, Debug)]
pub enum Operands {
    /// The operands, and how many are destinations when `/` says.
    List(Vec<OpExpr>, Option<usize>),
    /// Every operand of the copied instruction through this maker.
    Map(Call),
}

#[derive(Clone, Debug)]
pub enum OpExpr {
    Var(String),
    Call(Call),
    Lit(i64, u32),
}

#[derive(Clone, Debug)]
pub struct Override {
    pub field: String,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub enum Value {
    Field(usize, String),
    Empty,
    Bool(bool),
    Call(Call),
}

#[derive(Debug, Default)]
pub struct File {
    pub metas: Vec<Meta>,
    pub sets: Vec<SetDef>,
    pub groups: Vec<Group>,
}

const PUNCT: [&str; 23] =
    ["...", "..", "@", "$", "#", ":", ",", ";", "(", ")", "[", "]", "{", "}", "|", "&", "-", "=", "/", "*", ".", "!", "?"];

fn lex(text: &str, line: usize, file: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        if first.is_whitespace() {
            rest = &rest[first.len_utf8()..];
        } else if first.is_ascii_alphabetic() || first == '_' {
            let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
            out.push(Tok::Ident(rest[..end].to_owned()));
            rest = &rest[end..];
        } else if first == '"' {
            let end = rest[1..].find('"').ok_or_else(|| format!("{file}:{line}: unterminated string"))?;
            out.push(Tok::Str(rest[1..=end].to_owned()));
            rest = &rest[end + 2..];
        } else if first.is_ascii_digit() {
            let end = rest.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(rest.len());
            let digits = &rest[..end];
            let value = match digits.strip_prefix("0x") {
                Some(hex) => i64::from_str_radix(hex, 16),
                None => digits.parse(),
            }
            .map_err(|_| format!("{file}:{line}: bad number {digits}"))?;
            out.push(Tok::Int(value));
            rest = &rest[end..];
        } else if let Some(punct) = PUNCT.iter().find(|one| rest.starts_with(**one)) {
            out.push(Tok::Punct(punct));
            rest = &rest[punct.len()..];
        } else {
            return Err(format!("{file}:{line}: unexpected {first:?}"));
        }
    }
    Ok(out)
}

struct Parser<'a> {
    toks: Vec<Tok>,
    at: usize,
    line: usize,
    file: &'a str,
}

impl Parser<'_> {
    fn error<T>(&self, message: impl std::fmt::Display) -> Result<T, String> {
        Err(format!("{}:{}: {message}", self.file, self.line))
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn peek_at(&self, ahead: usize) -> Option<&Tok> {
        self.toks.get(self.at + ahead)
    }

    fn is(&self, punct: &str) -> bool {
        matches!(self.peek(), Some(Tok::Punct(one)) if *one == punct)
    }

    fn eat(&mut self, punct: &str) -> bool {
        let found = self.is(punct);
        if found {
            self.at += 1;
        }
        found
    }

    fn expect(&mut self, punct: &str) -> Result<(), String> {
        if self.eat(punct) { Ok(()) } else { self.error(format!("expected '{punct}', found {}", self.describe())) }
    }

    fn describe(&self) -> String {
        match self.peek() {
            None => "the end of the clause".into(),
            Some(Tok::Ident(one)) => format!("'{one}'"),
            Some(Tok::Int(one)) => format!("'{one}'"),
            Some(Tok::Str(one)) => format!("\"{one}\""),
            Some(Tok::Punct(one)) => format!("'{one}'"),
        }
    }

    fn ident(&mut self) -> Result<String, String> {
        match self.peek() {
            Some(Tok::Ident(one)) => {
                let one = one.clone();
                self.at += 1;
                Ok(one)
            }
            _ => self.error(format!("expected a name, found {}", self.describe())),
        }
    }

    fn keyword(&mut self, word: &str) -> bool {
        let found = matches!(self.peek(), Some(Tok::Ident(one)) if one == word);
        if found {
            self.at += 1;
        }
        found
    }

    fn int(&mut self) -> Result<i64, String> {
        match self.peek() {
            Some(Tok::Int(one)) => {
                let one = *one;
                self.at += 1;
                Ok(one)
            }
            _ => self.error(format!("expected a number, found {}", self.describe())),
        }
    }

    fn slot(&mut self, sigil: &str) -> Result<usize, String> {
        self.expect(sigil)?;
        usize::try_from(self.int()?).or_else(|_| self.error("a position is not negative"))
    }

    fn done(&self) -> Result<(), String> {
        if self.at == self.toks.len() { Ok(()) } else { self.error(format!("unexpected {}", self.describe())) }
    }

    fn set_expr(&mut self) -> Result<SetExpr, String> {
        let mut left = self.set_term()?;
        loop {
            let op = ["|", "&", "-"].into_iter().find(|op| self.is(op));
            let Some(op) = op else { return Ok(left) };
            self.at += 1;
            left = SetExpr::Bin(Box::new(left), op, Box::new(self.set_term()?));
        }
    }

    fn set_term(&mut self) -> Result<SetExpr, String> {
        if self.eat("(") {
            let inner = self.set_expr()?;
            self.expect(")")?;
            return Ok(inner);
        }
        let name = self.ident()?;
        if !self.eat("(") {
            return Ok(SetExpr::Name(name));
        }
        let made = match name.as_str() {
            "op" => SetExpr::Op(self.ident()?),
            "shape" => match self.peek() {
                Some(Tok::Str(one)) => {
                    let one = one.clone();
                    self.at += 1;
                    SetExpr::Shape(one)
                }
                _ => return self.error("shape(...) takes a quoted shape, as \"rm/^0,rmi\""),
            },
            "fixed" => SetExpr::Fixed,
            "reads" => {
                let mut flags = vec![self.ident()?];
                while self.eat("|") {
                    flags.push(self.ident()?);
                }
                SetExpr::Reads(flags)
            }
            _ => return self.error(format!("unknown set filter {name}(...)")),
        };
        self.expect(")")?;
        Ok(made)
    }

    fn call(&mut self) -> Result<Call, String> {
        let name = self.ident()?;
        self.expect("(")?;
        let mut args = Vec::new();
        if !self.eat(")") {
            loop {
                args.push(self.arg()?);
                if self.eat(")") {
                    break;
                }
                self.expect(",")?;
            }
        }
        Ok(Call { name, args })
    }

    fn arg(&mut self) -> Result<Arg, String> {
        if self.is("@") {
            return Ok(Arg::Slot(self.slot("@")?));
        }
        if self.is("$") {
            return Ok(Arg::Built(self.slot("$")?));
        }
        if self.eat("*") {
            return Ok(Arg::Crossed);
        }
        if self.eat("[") {
            let mut list = vec![self.int()?];
            while self.eat(",") {
                list.push(self.int()?);
            }
            self.expect("]")?;
            return Ok(Arg::List(list));
        }
        if let Some(Tok::Int(value)) = self.peek() {
            let value = *value;
            self.at += 1;
            return Ok(Arg::Int(value));
        }
        if matches!(self.peek_at(1), Some(Tok::Punct("("))) {
            return Ok(Arg::Call(self.call()?));
        }
        Ok(Arg::Ident(self.ident()?))
    }

    fn element(&mut self) -> Result<Element, String> {
        if self.eat("...") {
            return Ok(Element::Gap(self.call()?));
        }
        if self.eat("$") {
            return Ok(Element::End);
        }
        if self.keyword("def") {
            let value = self.ident()?;
            self.expect(":")?;
            let Element::Insn(pattern) = self.element()? else {
                return self.error("a def names an instruction");
            };
            return Ok(Element::Def(value, pattern));
        }
        let bind = if matches!(self.peek_at(1), Some(Tok::Punct("="))) {
            let name = self.ident()?;
            self.at += 1;
            Some(name)
        } else {
            None
        };
        let head = if self.eat("(") {
            let mut names = vec![self.ident()?];
            while self.eat("|") {
                names.push(self.ident()?);
            }
            self.expect(")")?;
            Head::Alt(names)
        } else if self.keyword("any") {
            self.expect("(")?;
            let mut ops = vec![self.ident()?];
            while self.eat("|") {
                ops.push(self.ident()?);
            }
            self.expect(")")?;
            Head::Any(ops)
        } else {
            Head::Name(self.ident()?)
        };
        let (mut operands, mut rest, mut split) = (Vec::new(), false, None);
        let mut separated = true;
        while self.peek().is_some() && !self.is(";") {
            if self.eat("/") {
                if split.is_some() {
                    return self.error("one '/' parts destinations from sources");
                }
                split = Some(operands.len());
                separated = true;
                continue;
            }
            if !separated {
                self.expect(",")?;
            }
            if rest {
                return self.error("'..' ends an operand list");
            }
            separated = false;
            if self.eat("..") {
                rest = true;
                continue;
            }
            operands.push(self.operand()?);
        }
        Ok(Element::Insn(InsnPat { bind, head, operands, split, rest }))
    }

    fn operand(&mut self) -> Result<OperandPat, String> {
        if self.eat("#") {
            let value = self.int()?;
            let width = if self.eat(":") { Some(self.bits()?) } else { None };
            return Ok(OperandPat::Lit { value, width });
        }
        let name = self.ident()?;
        if name == "_" {
            return Ok(OperandPat::Wild);
        }
        if !self.eat(":") {
            return Ok(OperandPat::Bind { name, kind: None });
        }
        let kind = self.ident()?;
        let split = kind.find(|c: char| c.is_ascii_digit()).unwrap_or(kind.len());
        let class = match &kind[..split] {
            "reg" => Class::Reg,
            "mem" => Class::Mem,
            "imm" => Class::Imm,
            "sym" => Class::Sym,
            "held" => Class::Held,
            "any" => Class::Any,
            other => return self.error(format!("unknown operand kind {other}")),
        };
        let width = match &kind[split..] {
            "" => None,
            "8" => Some(1),
            "16" => Some(2),
            "32" => Some(4),
            other => return self.error(format!("operand width {other} is not 8, 16 or 32")),
        };
        if class == Class::Any && width.is_some() {
            return self.error("an 'any' operand has no width");
        }
        Ok(OperandPat::Bind { name, kind: Some((class, width)) })
    }

    fn bits(&mut self) -> Result<u32, String> {
        match self.int()? {
            8 => Ok(1),
            16 => Ok(2),
            32 => Ok(4),
            other => self.error(format!("width {other} is not 8, 16 or 32")),
        }
    }

    fn item(&mut self) -> Result<Item, String> {
        if self.keyword("drop") {
            return Ok(Item::Drop { slot: self.slot("@")? });
        }
        if self.keyword("anchor") {
            let slot = self.slot("@")?;
            return Ok(Item::Anchor { slot, over: self.overrides()? });
        }
        let slot = self.slot("@")?;
        if !self.eat(":") {
            return Ok(Item::Keep { slot, over: self.overrides()? });
        }
        let head = if self.eat("=") {
            NewHead::Copy(self.slot("@")?)
        } else {
            let name = self.ident()?;
            if self.eat("[") {
                let key = self.ident()?;
                self.expect("]")?;
                NewHead::Family(name, key)
            } else {
                NewHead::Name(name)
            }
        };
        let operands = if self.keyword("map") {
            Operands::Map(self.call()?)
        } else {
            let (mut list, mut split, mut separated) = (Vec::new(), None, true);
            while self.peek().is_some() && !self.is(";") && !self.is("{") {
                if self.eat("/") {
                    if split.is_some() {
                        return self.error("one '/' parts destinations from sources");
                    }
                    split = Some(list.len());
                    separated = true;
                    continue;
                }
                if !separated {
                    self.expect(",")?;
                }
                separated = false;
                list.push(if self.eat("#") {
                    let value = self.int()?;
                    self.expect(":")?;
                    OpExpr::Lit(value, self.bits()?)
                } else if matches!(self.peek_at(1), Some(Tok::Punct("("))) {
                    OpExpr::Call(self.call()?)
                } else {
                    OpExpr::Var(self.ident()?)
                });
            }
            Operands::List(list, split)
        };
        Ok(Item::New { slot, head, operands, over: self.overrides()? })
    }

    fn overrides(&mut self) -> Result<Vec<Override>, String> {
        let mut out = Vec::new();
        if !self.eat("{") {
            return Ok(out);
        }
        loop {
            let field = self.ident()?;
            self.expect(":")?;
            let value = if self.is("@") {
                let slot = self.slot("@")?;
                self.expect(".")?;
                Value::Field(slot, self.ident()?)
            } else if self.eat("[") {
                self.expect("]")?;
                Value::Empty
            } else if self.keyword("true") {
                Value::Bool(true)
            } else if self.keyword("false") {
                Value::Bool(false)
            } else {
                Value::Call(self.call()?)
            };
            out.push(Override { field, value });
            if self.eat("}") {
                return Ok(out);
            }
            self.expect(",")?;
        }
    }
}

/// One clause: its keyword, the line it starts on, and its tokens.
struct Clause {
    keyword: String,
    line: usize,
    toks: Vec<Tok>,
}

pub fn parse(source: &str, file: &str) -> Result<File, String> {
    // Stanzas: (header line, header tokens, clauses).
    let mut stanzas: Vec<(usize, Vec<Tok>, Vec<Clause>)> = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        let line = index + 1;
        let text = raw.split("//").next().unwrap_or("");
        if text.trim().is_empty() {
            continue;
        }
        let toks = lex(text, line, file)?;
        if !raw.starts_with(char::is_whitespace) {
            stanzas.push((line, toks, Vec::new()));
            continue;
        }
        let Some((_, _, clauses)) = stanzas.last_mut() else {
            return Err(format!("{file}:{line}: an indented line belongs to a stanza"));
        };
        match toks.first() {
            Some(Tok::Ident(word)) if ["walk", "match", "when", "rewrite"].contains(&word.as_str()) => {
                clauses.push(Clause { keyword: word.clone(), line, toks: toks[1..].to_vec() });
            }
            _ => match clauses.last_mut() {
                Some(clause) => clause.toks.extend(toks),
                None => return Err(format!("{file}:{line}: expected walk, match, when or rewrite")),
            },
        }
    }

    let mut out = File::default();
    for (line, toks, clauses) in stanzas {
        let mut header = Parser { toks, at: 0, line, file };
        let word = header.ident()?;
        let clause = |keyword: &str| clauses.iter().filter(|one| one.keyword == keyword).collect::<Vec<_>>();
        let parser = |clause: &Clause| Parser { toks: clause.toks.clone(), at: 0, line: clause.line, file };
        let unexpected = |allowed: &[&str]| -> Result<(), String> {
            match clauses.iter().find(|one| !allowed.contains(&one.keyword.as_str())) {
                Some(one) => Err(format!("{file}:{}: {word} takes no {} clause", one.line, one.keyword)),
                None => Ok(()),
            }
        };
        match word.as_str() {
            "meta" => {
                unexpected(&[])?;
                let name = header.ident()?;
                header.expect("=")?;
                let mut fields = Vec::new();
                while header.peek().is_some() {
                    fields.push(header.ident()?);
                }
                out.metas.push(Meta { line, name, fields });
            }
            "set" => {
                unexpected(&[])?;
                let name = header.ident()?;
                header.expect("=")?;
                let expr = header.set_expr()?;
                header.done()?;
                out.sets.push(SetDef { line, name, expr });
            }
            "group" => {
                unexpected(&["walk"])?;
                let name = header.ident()?;
                header.done()?;
                let walks = clause("walk");
                let walk = match walks.as_slice() {
                    [] => None,
                    [one] => Some(walk(&mut parser(one))?),
                    [_, second, ..] => return Err(format!("{file}:{}: group {name} has two walks", second.line)),
                };
                out.groups.push(Group { line, name, walk, rules: Vec::new() });
            }
            "rule" => {
                unexpected(&["match", "when", "rewrite"])?;
                let name = header.ident()?;
                header.done()?;
                let single = |keyword: &str| -> Result<Option<&Clause>, String> {
                    match clause(keyword).as_slice() {
                        [] => Ok(None),
                        [one] => Ok(Some(*one)),
                        [_, second, ..] => Err(format!("{file}:{}: rule {name} has two {keyword} clauses", second.line)),
                    }
                };
                let Some(matched) = single("match")? else {
                    return Err(format!("{file}:{line}: rule {name} has no match clause"));
                };
                let Some(rewritten) = single("rewrite")? else {
                    return Err(format!("{file}:{line}: rule {name} has no rewrite clause"));
                };
                let mut p = parser(matched);
                let mut pattern = vec![p.element()?];
                while p.eat(";") {
                    pattern.push(p.element()?);
                }
                p.done()?;
                let mut guards = Vec::new();
                if let Some(when) = single("when")? {
                    let mut p = parser(when);
                    loop {
                        let negate = p.eat("!");
                        guards.push(Guard { negate, call: p.call()? });
                        if !p.eat(",") {
                            break;
                        }
                    }
                    p.done()?;
                }
                let mut p = parser(rewritten);
                let mut rewrite = vec![p.item()?];
                while p.eat(";") {
                    rewrite.push(p.item()?);
                }
                p.done()?;
                let Some(group) = out.groups.last_mut() else {
                    return Err(format!("{file}:{line}: rule {name} comes before any group"));
                };
                group.rules.push(Rule { line, name, pattern, guards, rewrite });
            }
            other => return Err(format!("{file}:{line}: expected meta, set, group or rule, found '{other}'")),
        }
    }
    Ok(out)
}

fn walk(p: &mut Parser) -> Result<Walk, String> {
    let line = p.line;
    let word = p.ident()?;
    let kind = match word.as_str() {
        "each" => WalkKind::Each,
        "window" => WalkKind::Window,
        "slide" => WalkKind::Slide,
        "gap" => WalkKind::Gap,
        other => return p.error(format!("unknown walk {other}; the walks are each, window, slide and gap")),
    };
    let mut made = Walk { line, kind, skip: Skip::None, advance: 1, first_original: false, resume_past: false };
    while p.peek().is_some() {
        match p.ident()?.as_str() {
            "skip" => {
                made.skip = match p.ident()?.as_str() {
                    "meta" => Skip::Meta,
                    "inert" => Skip::Inert,
                    "nothing" => Skip::Nothing,
                    other => return p.error(format!("unknown skip {other}; meta, inert or nothing")),
                };
            }
            "advance" if kind == WalkKind::Slide => made.advance = usize::try_from(p.int()?).or_else(|_| p.error("advance"))?,
            "first" if kind == WalkKind::Gap => {
                if !p.keyword("original") {
                    return p.error("expected 'first original'");
                }
                made.first_original = true;
            }
            "resume" if kind == WalkKind::Gap => {
                made.resume_past = match p.ident()?.as_str() {
                    "next" => false,
                    "past" => true,
                    other => return p.error(format!("unknown resume {other}; next or past")),
                };
            }
            other => return p.error(format!("walk {word} takes no '{other}'")),
        }
    }
    Ok(made)
}
