//! Reads the text `print` writes. Names resolve anywhere in their function,
//! so a loop's phi may name a value defined further down.

use std::collections::HashMap;
use std::fmt;

use crate::function::{Block, BlockId, Constant, Edge, EdgeId, Function, Instruction, InstructionId, Module, Operand, ValueId, ValueInfo};
use crate::opcode::{Opcode, Predicate, Slot};
use crate::types::{FloatFormat, MirContext, Type, TypeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.message)
    }
}

type Parsed<T> = Result<T, ParseError>;

fn error<T>(line: usize, message: impl Into<String>) -> Parsed<T> {
    Err(ParseError { line, message: message.into() })
}

pub fn module(context: &mut MirContext, text: &str) -> Parsed<Module> {
    let lines = lines(text)?;
    let mut at = 0;
    let name = match lines.first() {
        Some(first) if first.word(0) == Some("module") && first.tokens.len() == 2 => first.ident(1)?,
        Some(first) => return error(first.number, "expected `module NAME`"),
        None => return error(1, "expected `module NAME`"),
    };
    at += 1;
    let mut functions = Vec::new();
    while at < lines.len() {
        let (function, next) = raw_function(context, &lines, at)?;
        functions.push(build(context, function)?);
        at = next;
    }
    Ok(Module { name, functions })
}

pub fn function(context: &mut MirContext, text: &str) -> Parsed<Function> {
    let lines = lines(text)?;
    let (raw, next) = raw_function(context, &lines, 0)?;
    if let Some(extra) = lines.get(next) {
        return error(extra.number, "text after the function's `end`");
    }
    build(context, raw)
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Int(i128),
    Punct(&'static str),
}

struct Line {
    number: usize,
    indented: bool,
    tokens: Vec<Token>,
}

impl Line {
    fn word(&self, at: usize) -> Option<&str> {
        match self.tokens.get(at) {
            Some(Token::Ident(word)) => Some(word),
            _ => None,
        }
    }

    fn ident(&self, at: usize) -> Parsed<String> {
        match self.word(at) {
            Some(word) => Ok(word.to_owned()),
            None => error(self.number, format!("expected a name at token {}", at + 1)),
        }
    }
}

const PUNCTUATION: [&str; 17] = ["...", "->", "==", "!=", "<=", ">=", "<", ">", ":", ",", "=", "(", ")", "[", "]", "{", "}"];

fn lines(text: &str) -> Parsed<Vec<Line>> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let code = raw.split(';').next().unwrap_or("");
        if code.trim().is_empty() {
            continue;
        }
        let mut tokens = Vec::new();
        let mut rest = code.trim_start();
        let indented = rest.len() != code.len();
        while !rest.is_empty() {
            let first = rest.chars().next().unwrap();
            if first.is_whitespace() {
                rest = rest.trim_start();
                continue;
            }
            let negative = first == '-' && rest[1..].starts_with(|one: char| one.is_ascii_digit());
            if first.is_ascii_digit() || negative {
                let end = rest[1..].find(|one: char| !one.is_ascii_digit()).map_or(rest.len(), |at| at + 1);
                let Ok(value) = rest[..end].parse::<i128>() else { return error(number, format!("{} is out of range", &rest[..end])) };
                tokens.push(Token::Int(value));
                rest = &rest[end..];
            } else if first.is_ascii_alphabetic() || first == '_' {
                let end = rest.find(|one: char| !(one.is_ascii_alphanumeric() || one == '_' || one == '.')).unwrap_or(rest.len());
                tokens.push(Token::Ident(rest[..end].to_owned()));
                rest = &rest[end..];
            } else if let Some(&punct) = PUNCTUATION.iter().find(|one| rest.starts_with(**one)) {
                tokens.push(Token::Punct(punct));
                rest = &rest[punct.len()..];
            } else {
                return error(number, format!("unexpected {first:?}"));
            }
        }
        out.push(Line { number, indented, tokens });
    }
    Ok(out)
}

/// A cursor over one line's tokens.
struct Cursor<'a> {
    line: &'a Line,
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(line: &'a Line, at: usize) -> Self {
        Self { line, at }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.line.tokens.get(self.at)
    }

    fn done(&self) -> bool {
        self.at == self.line.tokens.len()
    }

    fn fail<T>(&self, wanted: &str) -> Parsed<T> {
        let found = match self.peek() {
            None => "the end of the line".to_owned(),
            Some(Token::Ident(word)) => format!("`{word}`"),
            Some(Token::Int(value)) => format!("`{value}`"),
            Some(Token::Punct(punct)) => format!("`{punct}`"),
        };
        error(self.line.number, format!("expected {wanted}, found {found}"))
    }

    fn eat(&mut self, punct: &str) -> bool {
        if matches!(self.peek(), Some(Token::Punct(one)) if *one == punct) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, punct: &str) -> Parsed<()> {
        if self.eat(punct) { Ok(()) } else { self.fail(&format!("`{punct}`")) }
    }

    fn keyword(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Token::Ident(one)) if one == word) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Parsed<String> {
        match self.peek() {
            Some(Token::Ident(word)) => {
                self.at += 1;
                Ok(word.clone())
            }
            _ => self.fail("a name"),
        }
    }

    fn int(&mut self) -> Parsed<i128> {
        match self.peek() {
            Some(Token::Int(value)) => {
                self.at += 1;
                Ok(*value)
            }
            _ => self.fail("a number"),
        }
    }

    fn end(&self) -> Parsed<()> {
        if self.done() { Ok(()) } else { self.fail("the end of the line") }
    }

    fn ty(&mut self, context: &mut MirContext) -> Parsed<TypeId> {
        if self.eat("[") || self.eat("<") {
            let vector = self.line.tokens[self.at - 1] == Token::Punct("<");
            let elements = u32::try_from(self.int()?).or_else(|_| self.fail("an element count"))?;
            if !self.keyword("x") {
                return self.fail("`x`");
            }
            let element = self.ty(context)?;
            self.expect(if vector { ">" } else { "]" })?;
            return Ok(context.intern(if vector { Type::Vector { element, elements } } else { Type::Array { element, elements } }));
        }
        if self.eat("{") {
            let fields = self.types_until(context, "}")?;
            return Ok(context.intern(Type::Struct { fields, name: None }));
        }
        let word = self.ident()?;
        let simple = match word.as_str() {
            "void" => Some(Type::Void),
            "f32" => Some(Type::Float(FloatFormat::Binary32)),
            "f64" => Some(Type::Float(FloatFormat::Binary64)),
            "f80" => Some(Type::Float(FloatFormat::Extended80)),
            _ => word.strip_prefix('i').and_then(|bits| bits.parse::<u32>().ok()).filter(|bits| *bits > 0).map(Type::Int),
        };
        if let Some(simple) = simple {
            return Ok(context.intern(simple));
        }
        match word.as_str() {
            "ptr" => {
                self.expect("(")?;
                let space = self.ident()?;
                self.expect(")")?;
                Ok(context.intern(Type::Pointer(space)))
            }
            "fn" => {
                self.expect("(")?;
                let mut parameters = Vec::new();
                let mut variadic = false;
                while !self.eat(")") {
                    if self.eat("...") {
                        variadic = true;
                        self.expect(")")?;
                        break;
                    }
                    parameters.push(self.ty(context)?);
                    if !self.eat(",") {
                        self.expect(")")?;
                        break;
                    }
                }
                let returns = self.returns(context)?;
                Ok(context.intern(Type::Function { parameters, returns, variadic }))
            }
            _ if self.eat("{") => {
                let fields = self.types_until(context, "}")?;
                Ok(context.intern(Type::Struct { fields, name: Some(word) }))
            }
            _ => error(self.line.number, format!("`{word}` is not a type")),
        }
    }

    fn types_until(&mut self, context: &mut MirContext, close: &str) -> Parsed<Vec<TypeId>> {
        let mut types = Vec::new();
        if self.eat(close) {
            return Ok(types);
        }
        loop {
            types.push(self.ty(context)?);
            if self.eat(close) {
                return Ok(types);
            }
            self.expect(",")?;
        }
    }

    /// ` -> T`, ` -> (T, U)`, or nothing.
    fn returns(&mut self, context: &mut MirContext) -> Parsed<Vec<TypeId>> {
        if !self.eat("->") {
            return Ok(Vec::new());
        }
        if self.eat("(") { self.types_until(context, ")") } else { Ok(vec![self.ty(context)?]) }
    }

    fn operand(&mut self, context: &mut MirContext) -> Parsed<RawOperand> {
        match self.peek() {
            Some(Token::Int(_)) => {
                let value = self.int()?;
                let ty = if self.eat(":") { Some(self.ty(context)?) } else { None };
                Ok(RawOperand::Int(value, ty))
            }
            Some(Token::Ident(word)) if word == "true" || word == "false" => {
                let value = word == "true";
                self.at += 1;
                Ok(RawOperand::Bool(value))
            }
            _ => Ok(RawOperand::Name(self.ident()?)),
        }
    }

    fn operands(&mut self, context: &mut MirContext) -> Parsed<Vec<RawOperand>> {
        let mut operands = Vec::new();
        if self.done() {
            return Ok(operands);
        }
        loop {
            operands.push(self.operand(context)?);
            if !self.eat(",") {
                return Ok(operands);
            }
        }
    }

    fn target(&mut self) -> Parsed<RawTarget> {
        let label = self.ident()?;
        let edge = if self.keyword("as") { Some(self.ident()?) } else { None };
        Ok(RawTarget { label, edge })
    }
}

#[derive(Clone, Debug)]
enum RawOperand {
    Name(String),
    Int(i128, Option<TypeId>),
    Bool(bool),
}

struct RawTarget {
    label: String,
    edge: Option<String>,
}

enum RawBody {
    Op(Opcode, Vec<RawOperand>),
    Phi(Vec<(String, RawOperand)>),
    Goto(RawTarget),
    If(RawOperand, RawTarget, RawTarget),
    Return(Vec<RawOperand>),
    Unreachable,
}

struct RawInstruction {
    line: usize,
    results: Vec<(String, Option<TypeId>)>,
    body: RawBody,
}

struct RawBlock {
    name: String,
    line: usize,
    instructions: Vec<RawInstruction>,
}

struct RawFunction {
    name: String,
    line: usize,
    parameters: Vec<(String, TypeId)>,
    returns: Vec<TypeId>,
    blocks: Vec<RawBlock>,
}

/// One function from `lines[at]`, and the line after its `end`.
fn raw_function(context: &mut MirContext, lines: &[Line], at: usize) -> Parsed<(RawFunction, usize)> {
    let header = &lines[at];
    let mut cursor = Cursor::new(header, 0);
    if !cursor.keyword("function") {
        return cursor.fail("`function`");
    }
    let name = cursor.ident()?;
    cursor.expect("(")?;
    let mut parameters = Vec::new();
    if !cursor.eat(")") {
        loop {
            let parameter = cursor.ident()?;
            cursor.expect(":")?;
            parameters.push((parameter, cursor.ty(context)?));
            if cursor.eat(")") {
                break;
            }
            cursor.expect(",")?;
        }
    }
    let returns = cursor.returns(context)?;
    cursor.end()?;
    let mut function = RawFunction { name, line: header.number, parameters, returns, blocks: Vec::new() };
    let mut next = at + 1;
    while let Some(line) = lines.get(next) {
        next += 1;
        if !line.indented && line.word(0) == Some("end") && line.tokens.len() == 1 {
            return Ok((function, next));
        }
        if !line.indented {
            let mut cursor = Cursor::new(line, 0);
            let label = cursor.ident()?;
            cursor.expect(":")?;
            cursor.end()?;
            function.blocks.push(RawBlock { name: label, line: line.number, instructions: Vec::new() });
            continue;
        }
        let Some(block) = function.blocks.last_mut() else { return error(line.number, "an instruction before any block label") };
        if line.word(0) == Some("from") {
            match block.instructions.last_mut() {
                Some(RawInstruction { body: RawBody::Phi(inputs), .. }) => {
                    let mut cursor = Cursor::new(line, 1);
                    inputs.push(phi_input(context, &mut cursor)?);
                    cursor.end()?;
                    continue;
                }
                _ => return error(line.number, "`from` outside a phi"),
            }
        }
        block.instructions.push(instruction(context, line)?);
    }
    error(header.number, format!("function {} has no `end`", function.name))
}

fn phi_input(context: &mut MirContext, cursor: &mut Cursor) -> Parsed<(String, RawOperand)> {
    let edge = cursor.ident()?;
    cursor.expect(":")?;
    Ok((edge, cursor.operand(context)?))
}

fn instruction(context: &mut MirContext, line: &Line) -> Parsed<RawInstruction> {
    let mut cursor = Cursor::new(line, 0);
    let bare = |body| Ok(RawInstruction { line: line.number, results: Vec::new(), body });
    if cursor.keyword("goto") {
        let target = cursor.target()?;
        cursor.end()?;
        return bare(RawBody::Goto(target));
    }
    if cursor.keyword("if") {
        let condition = cursor.operand(context)?;
        if !cursor.keyword("goto") {
            return cursor.fail("`goto`");
        }
        let yes = cursor.target()?;
        if !cursor.keyword("else") {
            return cursor.fail("`else`");
        }
        let no = cursor.target()?;
        cursor.end()?;
        return bare(RawBody::If(condition, yes, no));
    }
    if cursor.keyword("return") {
        let operands = cursor.operands(context)?;
        cursor.end()?;
        return bare(RawBody::Return(operands));
    }
    if cursor.keyword("unreachable") {
        cursor.end()?;
        return bare(RawBody::Unreachable);
    }
    let mut results = Vec::new();
    loop {
        let name = cursor.ident()?;
        let ty = if cursor.eat(":") { Some(cursor.ty(context)?) } else { None };
        results.push((name, ty));
        if cursor.eat("=") {
            break;
        }
        cursor.expect(",")?;
    }
    let mnemonic = cursor.ident()?;
    let body = if mnemonic == "phi" {
        let mut inputs = Vec::new();
        if cursor.keyword("from") {
            inputs.push(phi_input(context, &mut cursor)?);
        }
        RawBody::Phi(inputs)
    } else if mnemonic.starts_with("compare") {
        let left = cursor.operand(context)?;
        let operator = match cursor.peek() {
            Some(Token::Punct(punct)) => *punct,
            _ => return cursor.fail("a comparison"),
        };
        cursor.at += 1;
        let right = cursor.operand(context)?;
        let Some(predicate) = Predicate::ALL.into_iter().find(|one| one.spelling() == (mnemonic.as_str(), operator)) else {
            return error(line.number, format!("`{mnemonic}` has no `{operator}`"));
        };
        RawBody::Op(Opcode::Compare(predicate), vec![left, right])
    } else {
        let Some(opcode) = Opcode::ORDINARY.into_iter().find(|one| one.mnemonic() == mnemonic) else {
            return error(line.number, format!("`{mnemonic}` is not an opcode"));
        };
        RawBody::Op(opcode, cursor.operands(context)?)
    };
    cursor.end()?;
    Ok(RawInstruction { line: line.number, results, body })
}

/// Resolves names, creates edges, and fixes every type.
fn build(context: &mut MirContext, raw: RawFunction) -> Parsed<Function> {
    let mut blocks: HashMap<&str, BlockId> = HashMap::new();
    for (at, block) in raw.blocks.iter().enumerate() {
        if blocks.insert(&block.name, BlockId(at as u32)).is_some() {
            return error(block.line, format!("block {} is defined twice", block.name));
        }
    }
    if raw.blocks.is_empty() {
        return error(raw.line, format!("function {} has no blocks", raw.name));
    }

    let mut values: HashMap<&str, ValueId> = HashMap::new();
    let mut types: Vec<Option<TypeId>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let defined = raw.parameters.iter().map(|(name, ty)| (raw.line, name, Some(*ty))).chain(raw.blocks.iter().flat_map(|block| {
        block.instructions.iter().flat_map(|one| one.results.iter().map(move |(name, ty)| (one.line, name, *ty)))
    }));
    for (line, name, ty) in defined {
        if values.insert(name, ValueId(types.len() as u32)).is_some() {
            return error(line, format!("{name} is defined twice"));
        }
        types.push(ty);
        names.push(name.clone());
    }

    // Edges, in terminator order: each successor a terminator names is one.
    let mut edges = Vec::new();
    let mut sources = Vec::new();
    let mut named_edges: HashMap<String, EdgeId> = HashMap::new();
    let mut targets: Vec<Vec<EdgeId>> = Vec::new();
    for (at, block) in raw.blocks.iter().enumerate() {
        let mut owned = Vec::new();
        for one in &block.instructions {
            let listed: Vec<&RawTarget> = match &one.body {
                RawBody::Goto(target) => vec![target],
                RawBody::If(_, yes, no) => vec![yes, no],
                _ => continue,
            };
            for target in listed {
                let Some(&to) = blocks.get(target.label.as_str()) else {
                    return error(one.line, format!("no block is labelled {}", target.label));
                };
                let id = EdgeId(edges.len() as u32);
                if let Some(name) = &target.edge
                    && named_edges.insert(name.clone(), id).is_some()
                {
                    return error(one.line, format!("edge {name} is named twice"));
                }
                edges.push(Edge { target: to, name: target.edge.clone() });
                sources.push(BlockId(at as u32));
                owned.push(id);
            }
        }
        targets.push(owned);
    }

    // Types: stated, or read off operands, until nothing changes.
    let known = |types: &[Option<TypeId>], operand: &RawOperand, context: &MirContext| match operand {
        RawOperand::Name(name) => values.get(name.as_str()).and_then(|id| types[id.0 as usize]),
        RawOperand::Int(_, ty) => *ty,
        RawOperand::Bool(_) => Some(context.bool()),
    };
    loop {
        let mut progress = false;
        for one in raw.blocks.iter().flat_map(|block| &block.instructions) {
            let RawBody::Op(opcode, operands) = &one.body else { continue };
            let id = values[one.results[0].0.as_str()];
            if types[id.0 as usize].is_some() {
                continue;
            }
            let found: Vec<Option<TypeId>> = operands.iter().map(|operand| known(&types, operand, context)).collect();
            if let Some(ty) = opcode.infer(context, &found) {
                types[id.0 as usize] = Some(ty);
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }

    let mut function_blocks = Vec::new();
    let mut next_instruction = 0u32;
    for (at, block) in raw.blocks.iter().enumerate() {
        let here = BlockId(at as u32);
        let mut owned = targets[at].iter().copied();
        let mut instructions = Vec::new();
        for one in &block.instructions {
            let mut results = Vec::new();
            for (name, _) in &one.results {
                let id = values[name.as_str()];
                if types[id.0 as usize].is_none() {
                    return error(one.line, format!("{name}'s type cannot be read off its operands; state it"));
                }
                results.push(id);
            }
            let result = results.first().and_then(|id| types[id.0 as usize]);
            let (opcode, raw_operands, mut operands): (Opcode, Vec<&RawOperand>, Vec<Operand>) = match &one.body {
                RawBody::Op(opcode, list) => (*opcode, list.iter().collect(), Vec::new()),
                RawBody::Return(list) => (Opcode::Return, list.iter().collect(), Vec::new()),
                RawBody::Unreachable => (Opcode::Unreachable, Vec::new(), Vec::new()),
                RawBody::Goto(_) => (Opcode::Goto, Vec::new(), vec![Operand::Edge(owned.next().unwrap())]),
                RawBody::If(condition, _, _) => (Opcode::If, vec![condition], Vec::new()),
                RawBody::Phi(inputs) => {
                    let mut list = Vec::new();
                    for (label, value) in inputs {
                        let edge = phi_edge(&edges, &sources, &named_edges, &blocks, here, label)
                            .map_or_else(|message| error(one.line, message), Ok)?;
                        list.push(Operand::Edge(edge));
                        list.push(operand(context, &values, value, result, one.line)?);
                    }
                    (Opcode::Phi, Vec::new(), list)
                }
            };
            let slots = opcode.slots(raw_operands.len());
            let shared = slots.iter().zip(&raw_operands).find_map(|(slot, one)| known(&types, one, context).filter(|_| *slot == Slot::Shared));
            for (slot, raw_operand) in slots.into_iter().zip(raw_operands) {
                let expected = match slot {
                    Slot::Bool => Some(context.bool()),
                    Slot::Result => result,
                    Slot::Shared => shared,
                    Slot::Return(index) => raw.returns.get(index).copied(),
                    Slot::Free | Slot::Edge => None,
                };
                operands.push(operand(context, &values, raw_operand, expected, one.line)?);
            }
            if opcode == Opcode::If {
                operands.push(Operand::Edge(owned.next().unwrap()));
                operands.push(Operand::Edge(owned.next().unwrap()));
            }
            instructions.push(Instruction { id: InstructionId(next_instruction), opcode, results, operands });
            next_instruction += 1;
        }
        function_blocks.push(Block { name: Some(block.name.clone()), instructions });
    }

    let values = types.into_iter().zip(names).map(|(ty, name)| ValueInfo { ty: ty.unwrap(), name: Some(name) }).collect();
    let parameters = (0..raw.parameters.len() as u32).map(ValueId).collect();
    Ok(Function { name: raw.name, parameters, returns: raw.returns, values, blocks: function_blocks, edges })
}

fn operand(
    context: &MirContext,
    values: &HashMap<&str, ValueId>,
    raw: &RawOperand,
    expected: Option<TypeId>,
    line: usize,
) -> Parsed<Operand> {
    match raw {
        RawOperand::Name(name) => match values.get(name.as_str()) {
            Some(&id) => Ok(Operand::Value(id)),
            None => error(line, format!("{name} is not defined")),
        },
        RawOperand::Bool(value) => Ok(Operand::Constant(Constant { ty: context.bool(), bits: u128::from(*value) })),
        RawOperand::Int(value, ty) => match ty.or(expected) {
            Some(ty) if context.int_bits(ty).is_some() => Ok(Operand::Constant(Constant::int(context, ty, *value))),
            Some(ty) => error(line, format!("{value} cannot be a {}", context.display(ty))),
            None => error(line, format!("{value}'s type cannot be read off its place; write {value}:TYPE")),
        },
    }
}

/// A phi's `from X`: the edge named X into this block, else the one edge from block X.
fn phi_edge(
    edges: &[Edge],
    sources: &[BlockId],
    named: &HashMap<String, EdgeId>,
    blocks: &HashMap<&str, BlockId>,
    here: BlockId,
    label: &str,
) -> Result<EdgeId, String> {
    if let Some(&edge) = named.get(label)
        && edges[edge.0 as usize].target == here
    {
        return Ok(edge);
    }
    let Some(&source) = blocks.get(label) else { return Err(format!("no edge or block is called {label}")) };
    let from: Vec<EdgeId> =
        (0..edges.len()).filter(|&at| sources[at] == source && edges[at].target == here).map(|at| EdgeId(at as u32)).collect();
    match from.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no edge leads here from {label}")),
        _ => Err(format!("more than one edge leads here from {label}; name them with `as`")),
    }
}
