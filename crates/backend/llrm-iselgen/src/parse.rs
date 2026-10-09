//! The `.isel` pattern language's reader.

/// MIR opcodes a pattern may match, as `Opcode::mnemonic` spells them.
pub const OPCODES: [&str; 51] = [
    "ret",
    "br",
    "switch",
    "invoke",
    "resume",
    "unreachable",
    "fneg",
    "add",
    "sub",
    "mul",
    "udiv",
    "sdiv",
    "urem",
    "srem",
    "shl",
    "lshr",
    "ashr",
    "and",
    "or",
    "xor",
    "fadd",
    "fsub",
    "fmul",
    "fdiv",
    "frem",
    "trunc",
    "zext",
    "sext",
    "fptrunc",
    "fpext",
    "fptoui",
    "fptosi",
    "uitofp",
    "sitofp",
    "ptrtoint",
    "inttoptr",
    "bitcast",
    "addrspacecast",
    "extractvalue",
    "alloca",
    "load",
    "store",
    "getelementptr",
    "insertvalue",
    "icmp",
    "fcmp",
    "phi",
    "select",
    "freeze",
    "call",
    "landingpad",
];

/// Shorthands for sets of opcodes.
pub const OPCODE_SETS: [(&str, &[&str]); 2] = [
    (
        "cast",
        &[
            "trunc",
            "zext",
            "sext",
            "fptrunc",
            "fpext",
            "fptoui",
            "fptosi",
            "uitofp",
            "sitofp",
            "ptrtoint",
            "inttoptr",
            "bitcast",
            "addrspacecast",
        ],
    ),
    (
        "binary",
        &[
            "add", "sub", "mul", "udiv", "sdiv", "urem", "srem", "shl", "lshr", "ashr", "and", "or", "xor", "fadd",
            "fsub", "fmul", "fdiv", "frem",
        ],
    ),
];

/// A value's type, as the selector classes it: `ptr` is near, `far` is
/// addrspace(1), `float` any float.
pub const TYPES: [&str; 11] = ["none", "void", "i1", "i8", "i16", "i32", "i64", "ptr", "far", "float", "other"];
/// Shorthands for sets of types.
pub const TYPE_SETS: [(&str, &[&str]); 1] = [("int", &["i1", "i8", "i16", "i32"])];

/// What an operand is: a value, an integer constant (null and zero among
/// them), poison, a float constant, another constant, a block, or absent.
pub const KINDS: [&str; 7] = ["none", "value", "int", "poison", "fconst", "const", "block"];

/// Operands the automaton looks at; a pattern may name more, left to
/// predicates and hooks.
pub const OPERANDS: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct OperandPattern {
    pub binding: Option<String>,
    pub kinds: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
    /// An integer constant of exactly this value.
    pub literal: Option<i64>,
    /// A value an instruction of this shape defines.
    pub nested: Option<Box<Nested>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Nested {
    pub opcodes: Vec<String>,
    pub result: Option<Vec<String>>,
    pub operands: Vec<OperandPattern>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Vec<Expr>,
    pub negated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Name(String),
    Int(i64),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Let(String, Expr),
    Emit {
        name: String,
        dests: Vec<Expr>,
        sources: Vec<Expr>,
        volatile: bool,
    },
    Hook(Call),
    /// A binary operation on an integer wider than the native one, as its
    /// halves: `first` for the low, `rest` for each above it (the form that
    /// takes the carry, or the same).
    Chain {
        first: String,
        rest: String,
        left: Expr,
        right: Expr,
    },
    /// Refused with this message, or the opcode's mnemonic.
    Refuse(Option<String>),
    Nothing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub name: String,
    pub line: usize,
    /// None matches any opcode.
    pub opcodes: Option<Vec<String>>,
    pub result: Option<Vec<String>>,
    pub operands: Vec<OperandPattern>,
    pub group: Option<String>,
    pub when: Vec<Call>,
    pub cost: Option<Call>,
    /// Nested instructions this pattern selects in its root's place.
    pub covers: Vec<String>,
    pub body: Vec<Step>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patterns {
    pub patterns: Vec<Pattern>,
    /// Opcodes whose constant first operand is taken second.
    pub commutative: Vec<String>,
}

struct Lexer<'a> {
    text: &'a str,
    at: usize,
    line: usize,
}

fn error<T>(
    line: usize,
    what: impl std::fmt::Display,
) -> Result<T, String> {
    Err(format!("patterns.isel:{line}: {what}"))
}

impl<'a> Lexer<'a> {
    fn skip(&mut self) {
        self.at += self.text[self.at..].len() - self.text[self.at..].trim_start().len();
    }

    fn done(&mut self) -> bool {
        self.skip();
        self.at == self.text.len()
    }

    fn eat(
        &mut self,
        token: &str,
    ) -> bool {
        self.skip();
        let found = self.text[self.at..].starts_with(token);
        if found {
            self.at += token.len();
        }
        found
    }

    fn expect(
        &mut self,
        token: &str,
    ) -> Result<(), String> {
        if self.eat(token) {
            Ok(())
        } else {
            error(self.line, format!("expected `{token}` at `{}`", &self.text[self.at..]))
        }
    }

    fn word(&mut self) -> Option<&'a str> {
        self.skip();
        let rest = &self.text[self.at..];
        let length =
            rest.find(|one: char| !(one.is_ascii_alphanumeric() || one == '_' || one == '$')).unwrap_or(rest.len());
        (length > 0).then(|| {
            self.at += length;
            &rest[..length]
        })
    }

    fn name(&mut self) -> Result<&'a str, String> {
        match self.word() {
            Some(word) => Ok(word),
            None => error(self.line, format!("expected a name at `{}`", &self.text[self.at..])),
        }
    }

    fn int(&mut self) -> Result<i64, String> {
        let negative = self.eat("-");
        let word = self.name()?;
        let value: i64 = word.parse().or_else(|_| error(self.line, format!("`{word}` is no integer")))?;
        Ok(if negative { -value } else { value })
    }

    fn alternatives(&mut self) -> Result<Vec<String>, String> {
        let mut out = vec![self.name()?.to_owned()];
        while self.eat("|") {
            out.push(self.name()?.to_owned());
        }
        Ok(out)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.skip();
        if self.text[self.at..].starts_with(|one: char| one == '-' || one.is_ascii_digit()) {
            return self.int().map(Expr::Int);
        }
        let name = self.name()?.to_owned();
        if !self.eat("(") {
            return Ok(Expr::Name(name));
        }
        let mut args = Vec::new();
        if !self.eat(")") {
            loop {
                args.push(self.expr()?);
                if self.eat(")") {
                    break;
                }
                self.expect(",")?;
            }
        }
        Ok(Expr::Call(name, args))
    }

    fn call(&mut self) -> Result<Call, String> {
        let negated = self.eat("!");
        match self.expr()? {
            Expr::Call(name, args) => Ok(Call { name, args, negated }),
            Expr::Name(name) => Ok(Call { name, args: Vec::new(), negated }),
            Expr::Int(_) => error(self.line, "an integer is no predicate or hook"),
        }
    }

    fn list(&mut self) -> Result<Vec<Expr>, String> {
        let mut out = vec![self.expr()?];
        while self.eat(",") {
            out.push(self.expr()?);
        }
        Ok(out)
    }
}

fn types(
    line: usize,
    names: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for name in names {
        if let Some((_, set)) = TYPE_SETS.iter().find(|(set, _)| *set == name) {
            out.extend(set.iter().map(|one| one.to_string()));
        } else if TYPES.contains(&name.as_str()) {
            out.push(name);
        } else {
            return error(line, format!("no type `{name}`"));
        }
    }
    Ok(out)
}

fn opcodes(lexer: &mut Lexer) -> Result<Vec<String>, String> {
    let mut opcodes = Vec::new();
    for name in lexer.alternatives()? {
        if let Some((_, set)) = OPCODE_SETS.iter().find(|(set, _)| *set == name) {
            opcodes.extend(set.iter().map(|one| one.to_string()));
        } else if OPCODES.contains(&name.as_str()) {
            opcodes.push(name);
        } else {
            return error(lexer.line, format!("no MIR opcode `{name}`"));
        }
    }
    Ok(opcodes)
}

fn operands(lexer: &mut Lexer) -> Result<Vec<OperandPattern>, String> {
    let mut out = Vec::new();
    if lexer.eat("(") && !lexer.eat(")") {
        loop {
            out.push(operand(lexer)?);
            if lexer.eat(")") {
                break;
            }
            lexer.expect(",")?;
        }
    }
    Ok(out)
}

fn operand(lexer: &mut Lexer) -> Result<OperandPattern, String> {
    let line = lexer.line;
    let mut one = OperandPattern { binding: None, kinds: None, types: None, literal: None, nested: None };
    if lexer.eat("#") {
        one.literal = Some(lexer.int()?);
        one.kinds = Some(vec!["int".into()]);
    } else if !lexer.eat("_") {
        one.binding = Some(lexer.name()?.to_owned());
        if lexer.eat("=") {
            let opcodes = opcodes(lexer)?;
            let result = if lexer.eat(".") { Some(types(line, lexer.alternatives()?)?) } else { None };
            let operands = operands(lexer)?;
            one.kinds = Some(vec!["value".into()]);
            one.types = result.clone();
            one.nested = Some(Box::new(Nested { opcodes, result, operands }));
            return Ok(one);
        }
    }
    if lexer.eat(".") {
        one.types = Some(types(line, lexer.alternatives()?)?);
    }
    if lexer.eat(":") {
        let kinds = lexer.alternatives()?;
        if let Some(bad) = kinds.iter().find(|one| !KINDS.contains(&one.as_str())) {
            return error(line, format!("no operand kind `{bad}`"));
        }
        one.kinds = Some(kinds);
    }
    Ok(one)
}

fn matched(
    pattern: &mut Pattern,
    text: &str,
    line: usize,
) -> Result<(), String> {
    let mut lexer = Lexer { text, at: 0, line };
    if !lexer.eat("*") {
        pattern.opcodes = Some(opcodes(&mut lexer)?);
    }
    if lexer.eat(".") {
        pattern.result = Some(types(line, lexer.alternatives()?)?);
    }
    pattern.operands = operands(&mut lexer)?;
    if !lexer.done() {
        return error(line, format!("`{}` after the match", &lexer.text[lexer.at..]));
    }
    fn bound<'a>(
        operands: &'a [OperandPattern],
        into: &mut Vec<&'a String>,
    ) -> Option<&'a String> {
        for one in operands {
            if let Some(binding) = &one.binding {
                if into.contains(&binding) {
                    return Some(binding);
                }
                into.push(binding);
            }
            if let Some(twice) = one.nested.as_ref().and_then(|nested| bound(&nested.operands, into)) {
                return Some(twice);
            }
        }
        None
    }
    if let Some(twice) = bound(&pattern.operands, &mut Vec::new()) {
        return error(line, format!("`{twice}` bound twice"));
    }
    Ok(())
}

fn step(
    keyword: &str,
    rest: &str,
    line: usize,
) -> Result<Step, String> {
    let mut lexer = Lexer { text: rest, at: 0, line };
    let step = match keyword {
        "let" => {
            let name = lexer.name()?.to_owned();
            lexer.expect("=")?;
            Step::Let(name, lexer.expr()?)
        }
        "emit" => {
            let name = lexer.name()?.to_owned();
            let dests = if lexer.eat("<-") {
                Vec::new()
            } else {
                let dests = lexer.list()?;
                lexer.expect("<-")?;
                dests
            };
            let mut sources = Vec::new();
            let mut volatile = false;
            loop {
                lexer.skip();
                if lexer.done() {
                    break;
                }
                if lexer.text[lexer.at..].trim() == "volatile" {
                    lexer.at = lexer.text.len();
                    volatile = true;
                    break;
                }
                if !sources.is_empty() {
                    lexer.expect(",")?;
                }
                sources.push(lexer.expr()?);
            }
            Step::Emit { name, dests, sources, volatile }
        }
        "call" => Step::Hook(lexer.call()?),
        "chain" => {
            let first = lexer.name()?.to_owned();
            let rest = lexer.name()?.to_owned();
            lexer.expect("<-")?;
            let left = lexer.expr()?;
            lexer.expect(",")?;
            Step::Chain { first, rest, left, right: lexer.expr()? }
        }
        "refuse" => {
            lexer.skip();
            let text = lexer.text[lexer.at..].trim();
            lexer.at = lexer.text.len();
            if text == "mnemonic" {
                Step::Refuse(None)
            } else if let Some(quoted) = text.strip_prefix('"').and_then(|one| one.strip_suffix('"')) {
                Step::Refuse(Some(quoted.to_owned()))
            } else {
                return error(line, "refuse takes a \"message\" or `mnemonic`");
            }
        }
        "nothing" => Step::Nothing,
        _ => unreachable!("a body keyword"),
    };
    if !lexer.done() {
        return error(line, format!("`{}` after the {keyword}", &lexer.text[lexer.at..]));
    }
    Ok(step)
}

/// Each `for` instance of a pattern's lines: `$mir` and `$x86` replaced.
fn instances(
    lines: &[(usize, String)],
    pairs: &[(String, String)],
) -> Vec<Vec<(usize, String)>> {
    if pairs.is_empty() {
        return vec![lines.to_vec()];
    }
    pairs
        .iter()
        .map(|(mir, x86)| {
            lines.iter().map(|(line, text)| (*line, text.replace("$mir", mir).replace("$x86", x86))).collect()
        })
        .collect()
}

fn pattern(
    name: &str,
    line: usize,
    lines: &[(usize, String)],
) -> Result<Pattern, String> {
    let mut pattern = Pattern {
        name: name.to_owned(),
        line,
        opcodes: None,
        result: None,
        operands: Vec::new(),
        group: None,
        when: Vec::new(),
        cost: None,
        covers: Vec::new(),
        body: Vec::new(),
    };
    let mut matches = 0;
    for (line, text) in lines {
        let line = *line;
        let (keyword, rest) =
            text.split_once(char::is_whitespace).map_or((text.as_str(), ""), |(keyword, rest)| (keyword, rest.trim()));
        match keyword {
            "match" => {
                matches += 1;
                matched(&mut pattern, rest, line)?;
            }
            "cover" => {
                let mut lexer = Lexer { text: rest, at: 0, line };
                loop {
                    pattern.covers.push(lexer.name()?.to_owned());
                    if lexer.done() {
                        break;
                    }
                    lexer.expect(",")?;
                }
            }
            "group" => pattern.group = Some(Lexer { text: rest, at: 0, line }.name()?.to_owned()),
            "when" => {
                let mut lexer = Lexer { text: rest, at: 0, line };
                loop {
                    pattern.when.push(lexer.call()?);
                    if lexer.done() {
                        break;
                    }
                    lexer.expect(",")?;
                }
            }
            "cost" => {
                let mut lexer = Lexer { text: rest, at: 0, line };
                pattern.cost = Some(lexer.call()?);
                if !lexer.done() {
                    return error(line, "one cost hook");
                }
            }
            "let" | "emit" | "call" | "chain" | "refuse" | "nothing" => pattern.body.push(step(keyword, rest, line)?),
            other => return error(line, format!("unknown line `{other}` in pattern `{name}`")),
        }
    }
    if matches != 1 {
        return error(line, format!("pattern `{name}` has {matches} match lines, not one"));
    }
    if pattern.body.is_empty() {
        return error(line, format!("pattern `{name}` selects nothing; say `nothing` if it means to"));
    }
    let nested: Vec<&String> =
        pattern.operands.iter().filter(|one| one.nested.is_some()).filter_map(|one| one.binding.as_ref()).collect();
    if let Some(bad) = pattern.covers.iter().find(|one| !nested.contains(one)) {
        return error(line, format!("pattern `{name}` covers `{bad}`, which is no instruction its match nests"));
    }
    if !pattern.covers.is_empty() && pattern.group.is_some() {
        return error(line, format!("pattern `{name}` covers and competes; a cover is decided before costs are"));
    }
    if pattern.cost.is_some() != pattern.group.is_some() {
        return error(line, format!("pattern `{name}`: a group member has a cost, and only it"));
    }
    Ok(pattern)
}

pub fn parse(text: &str) -> Result<Patterns, String> {
    let mut out = Patterns::default();
    let mut open: Option<(String, usize, Vec<(String, String)>, Vec<(usize, String)>)> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        // A comment is `#` and a space; `#0` is a literal.
        let content = raw.split("# ").next().unwrap_or("").trim().trim_end_matches('#').trim();
        if content.is_empty() {
            continue;
        }
        let (keyword, rest) =
            content.split_once(char::is_whitespace).map_or((content, ""), |(keyword, rest)| (keyword, rest.trim()));
        match (keyword, &mut open) {
            ("commutative", None) => out.commutative.extend(rest.split_whitespace().map(str::to_owned)),
            ("pattern", None) => {
                let mut words = rest.split_whitespace();
                let Some(name) = words.next() else { return error(line, "a pattern has no name") };
                let mut pairs = Vec::new();
                match words.next() {
                    None => {}
                    Some("for") => {
                        for pair in words {
                            let Some((mir, x86)) = pair.split_once('=') else {
                                return error(line, format!("`{pair}` is not mir=x86"));
                            };
                            pairs.push((mir.to_owned(), x86.to_owned()));
                        }
                    }
                    Some(other) => return error(line, format!("`{other}` after the pattern's name")),
                }
                if out.patterns.iter().any(|one| one.name == name) {
                    return error(line, format!("pattern `{name}` twice"));
                }
                open = Some((name.to_owned(), line, pairs, Vec::new()));
            }
            ("end", Some(_)) => {
                let (name, start, pairs, lines) = open.take().expect("an open pattern");
                for (index, lines) in instances(&lines, &pairs).into_iter().enumerate() {
                    let name = if pairs.is_empty() { name.clone() } else { format!("{name}_{}", pairs[index].0) };
                    out.patterns.push(pattern(&name, start, &lines)?);
                }
            }
            (_, Some((_, _, _, lines))) => lines.push((line, content.to_owned())),
            (other, None) => return error(line, format!("`{other}` outside a pattern")),
        }
    }
    if let Some((name, line, _, _)) = open {
        return error(line, format!("pattern `{name}` has no `end`"));
    }
    if let Some(bad) = out.commutative.iter().find(|one| !OPCODES.contains(&one.as_str())) {
        return error(0, format!("commutative `{bad}` is no MIR opcode"));
    }
    Ok(out)
}
