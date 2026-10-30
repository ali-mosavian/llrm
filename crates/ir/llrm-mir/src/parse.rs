//! Reads MIR's subset of LLVM's assembly language.
//!
//! A pre-scan gives each global value its id in definition order and reads
//! the attribute groups, which LLVM prints at the end, so both may be
//! referred to before they appear. A function's values and blocks may be
//! used before their definition; the definition must agree with the use.

use crate::context::{Constant, ConstantExpr, ConstantId, ConstantKind, GlobalId, mask};
use crate::hash::{HashMap, HashSet};
use crate::intrinsics;
use crate::lexer::{Name, ParseError, Token, lex};
use crate::module::{
    Block, BlockId, DebugRecord, DebugWhat, Function, GlobalKind, GlobalValue, GlobalVariable, InstId, Instruction,
    LINKAGE, Linkage, MetadataId, MetadataNode, MetadataOperand, Module, Operand, Special, UnnamedAddr, ValueData,
    ValueDef, ValueId,
};
use crate::opcode::{
    AtomicInfo, AtomicKind, AtomicOrdering, Attribute, BINARY, BinaryOp, CAST, CallInfo, CastOp, Clause,
    FLAG_ATTRIBUTES, FLOAT_PREDICATE, Flags, INT_ATTRIBUTES, INT_PREDICATE, Opcode, RMW_OPS, TARGET_FLAG_ATTRIBUTES,
    TYPE_ATTRIBUTES, Tail, spelled,
};
use crate::types::{FloatKind, StructBody, Type, TypeId};

type Parsed<T> = Result<T, ParseError>;

pub fn module(text: &str) -> Parsed<Module> {
    let mut parser = Parser::new(lex(text)?);
    parser.prescan()?;
    parser.top_level()?;
    parser.finish()
}

/// Constructs llrm has no producer for, named so the refusal says why.
const OUTSIDE_SUBSET: [&str; 8] =
    ["fp128", "ppc_fp128", "half", "bfloat", "blockaddress", "indirectbr", "dso_local", "triple"];

struct Parser {
    tokens: Vec<(Token, usize)>,
    at: usize,
    module: Module,
    globals: HashMap<Name, GlobalId>,
    /// Global values referred to, with the pointer type each use gave them.
    global_uses: Vec<(GlobalId, TypeId, usize)>,
    defined_globals: HashSet<GlobalId>,
    groups: HashMap<u32, Vec<Attribute>>,
    metadata: HashMap<u32, MetadataId>,
    defined_metadata: HashSet<MetadataId>,
    /// Token ranges of the attribute groups, skipped by the main pass.
    group_spans: Vec<(usize, usize)>,
}

/// A function being read: names to arena ids, and uses still undefined.
struct Local {
    function: Function,
    values: HashMap<Name, ValueId>,
    blocks: HashMap<Name, BlockId>,
    pending_values: HashMap<ValueId, usize>,
    pending_blocks: HashMap<BlockId, usize>,
    /// Records read, to stand before the next instruction.
    pending_records: Vec<(MetadataId, DebugWhat, Option<MetadataId>, Option<MetadataId>)>,
    next_slot: u32,
}

impl Parser {
    fn new(tokens: Vec<(Token, usize)>) -> Self {
        Self {
            tokens,
            at: 0,
            module: Module::default(),
            globals: HashMap::default(),
            global_uses: Vec::new(),
            defined_globals: HashSet::default(),
            groups: HashMap::default(),
            metadata: HashMap::default(),
            defined_metadata: HashSet::default(),
            group_spans: Vec::new(),
        }
    }

    // ---- tokens

    fn peek(&self) -> &Token {
        &self.tokens[self.at].0
    }

    fn peek_at(
        &self,
        ahead: usize,
    ) -> &Token {
        &self.tokens[(self.at + ahead).min(self.tokens.len() - 1)].0
    }

    fn line(&self) -> usize {
        self.tokens[self.at].1
    }

    fn next(&mut self) -> Token {
        let token = self.tokens[self.at].0.clone();
        if self.at + 1 < self.tokens.len() {
            self.at += 1;
        }
        token
    }

    fn fail<T>(
        &self,
        message: impl Into<String>,
    ) -> Parsed<T> {
        self.fail_on(self.line(), message)
    }

    fn fail_on<T>(
        &self,
        line: usize,
        message: impl Into<String>,
    ) -> Parsed<T> {
        Err(ParseError { line, message: message.into() })
    }

    fn is_word(
        &self,
        word: &str,
    ) -> bool {
        matches!(self.peek(), Token::Word(one) if one == word)
    }

    fn eat_word(
        &mut self,
        word: &str,
    ) -> bool {
        let found = self.is_word(word);
        if found {
            self.next();
        }
        found
    }

    fn expect_word(
        &mut self,
        word: &str,
    ) -> Parsed<()> {
        if self.eat_word(word) { Ok(()) } else { self.fail(format!("expected `{word}`, found {}", self.describe())) }
    }

    fn is_punct(
        &self,
        c: char,
    ) -> bool {
        *self.peek() == Token::Punct(c)
    }

    fn eat_punct(
        &mut self,
        c: char,
    ) -> bool {
        let found = self.is_punct(c);
        if found {
            self.next();
        }
        found
    }

    fn expect_punct(
        &mut self,
        c: char,
    ) -> Parsed<()> {
        if self.eat_punct(c) { Ok(()) } else { self.fail(format!("expected `{c}`, found {}", self.describe())) }
    }

    fn describe(&self) -> String {
        match self.peek() {
            Token::Word(word) => format!("`{word}`"),
            Token::Punct(c) => format!("`{c}`"),
            Token::Eof => "the end".to_owned(),
            other => format!("{other:?}"),
        }
    }

    fn word(&mut self) -> Parsed<String> {
        match self.next() {
            Token::Word(word) => Ok(word),
            _ => {
                self.at -= 1;
                self.fail(format!("expected a keyword, found {}", self.describe()))
            }
        }
    }

    /// An `i64`.
    fn offset(&mut self) -> Parsed<i64> {
        match self.next() {
            Token::Int { negative, magnitude } if magnitude <= u128::from(u64::MAX >> 1) + u128::from(negative) => {
                Ok(if negative { (magnitude as i128).wrapping_neg() as i64 } else { magnitude as i64 })
            }
            _ => {
                self.at -= 1;
                self.fail(format!("expected an i64, found {}", self.describe()))
            }
        }
    }

    fn unsigned(&mut self) -> Parsed<u64> {
        match self.next() {
            Token::Int { negative: false, magnitude } if magnitude <= u128::from(u64::MAX) => Ok(magnitude as u64),
            _ => {
                self.at -= 1;
                self.fail(format!("expected a count, found {}", self.describe()))
            }
        }
    }

    fn string(&mut self) -> Parsed<String> {
        match self.next() {
            Token::Str(bytes) => String::from_utf8(bytes).or_else(|_| self.fail("a string is not UTF-8")),
            _ => {
                self.at -= 1;
                self.fail(format!("expected a string, found {}", self.describe()))
            }
        }
    }

    fn refuse_subset(&self) -> Parsed<()> {
        if let Token::Word(word) = self.peek()
            && OUTSIDE_SUBSET.contains(&word.as_str())
        {
            return self.fail(format!("`{word}` is outside MIR's subset of LLVM"));
        }
        Ok(())
    }

    // ---- the pre-scan

    fn prescan(&mut self) -> Parsed<()> {
        let mut at = 0;
        let mut awaiting_function = false;
        while at < self.tokens.len() {
            match &self.tokens[at].0 {
                Token::Word(word) if word == "define" || word == "declare" => awaiting_function = true,
                Token::Word(word)
                    if word == "attributes"
                        && matches!(self.tokens.get(at + 1), Some((Token::AttributeGroup(_), _))) =>
                {
                    let Token::AttributeGroup(group) = self.tokens[at + 1].0 else { unreachable!() };
                    self.at = at + 2;
                    self.expect_punct('=')?;
                    self.expect_punct('{')?;
                    let attrs = self.attributes(false)?;
                    self.expect_punct('}')?;
                    self.groups.insert(group, attrs);
                    self.group_spans.push((at, self.at));
                    at = self.at;
                    continue;
                }
                Token::Global(name) => {
                    let defines = awaiting_function
                        || self.tokens.get(at + 1).is_some_and(|(next, _)| *next == Token::Punct('='));
                    if defines {
                        if self.globals.contains_key(name) {
                            let line = self.tokens[at].1;
                            return Err(ParseError { line, message: format!("@{} is defined twice", display(name)) });
                        }
                        let id = GlobalId(self.module.globals.len() as u32);
                        let void = self.module.context.types.void();
                        self.module
                            .globals
                            .push(
                                GlobalValue {
                                    name: match name {
                                        Name::Named(one) => Some(one.clone()),
                                        Name::Numbered(_) => None,
                                    }
                                    .into(),
                                    linkage: Linkage::External,
                                    unnamed_addr: UnnamedAddr::None,
                                    address_space: 0,
                                    kind: GlobalKind::Variable(GlobalVariable {
                                        externally_initialized: false,
                                        ty: void,
                                        constant: false,
                                        initializer: None,
                                        align: None,
                                    }),
                                },
                            );
                        self.globals.insert(name.clone(), id);
                    }
                    awaiting_function = false;
                }
                _ => {}
            }
            at += 1;
        }
        self.at = 0;
        Ok(())
    }

    // ---- the module

    fn top_level(&mut self) -> Parsed<()> {
        loop {
            if let Some(&(_, end)) = self.group_spans.iter().find(|(start, _)| *start == self.at) {
                self.at = end;
                continue;
            }
            self.refuse_subset()?;
            match self.peek().clone() {
                Token::Eof => return Ok(()),
                Token::Word(word) if word == "target" => {
                    self.next();
                    self.refuse_subset()?;
                    self.expect_word("datalayout")?;
                    self.expect_punct('=')?;
                    self.module.datalayout = Some(self.string()?);
                }
                Token::Word(word) if word == "source_filename" => {
                    self.next();
                    self.expect_punct('=')?;
                    self.string()?;
                }
                Token::Local(Name::Named(name)) => self.named_type(name)?,
                Token::Global(name) => self.global_variable(name)?,
                Token::Word(word) if word == "declare" || word == "define" => self.function(word == "define")?,
                Token::MetadataName(name) => self.named_metadata(name)?,
                Token::MetadataId(number) => self.metadata_definition(number)?,
                _ => return self.fail(format!("unexpected {} at the top level", self.describe())),
            }
        }
    }

    fn finish(mut self) -> Parsed<Module> {
        for (name, id) in &self.globals {
            if !self.defined_globals.contains(id) {
                return Err(ParseError { line: 0, message: format!("@{} is used but never defined", display(name)) });
            }
        }
        for (id, ty, line) in std::mem::take(&mut self.global_uses) {
            let space = self.module.global(id).address_space;
            if self.module.context.types.get(ty) != &Type::Pointer(space) {
                let used = self.module.context.types.display(ty);
                return Err(ParseError { line, message: format!("a global in address space {space} used as {used}") });
            }
        }
        for (number, id) in &self.metadata {
            if !self.defined_metadata.contains(id) {
                return Err(ParseError { line: 0, message: format!("!{number} is used but never defined") });
            }
        }
        self.renumber_metadata();
        Ok(self.module)
    }

    /// Nodes in the order of their numbers in the text, inline nodes after,
    /// so text numbered densely prints back as it was read.
    fn renumber_metadata(&mut self) {
        let count = self.module.metadata.len();
        let mut numbered: Vec<(u32, MetadataId)> = self.metadata.iter().map(|(number, id)| (*number, *id)).collect();
        numbered.sort();
        let mut order: Vec<MetadataId> = numbered.into_iter().map(|(_, id)| id).collect();
        let named: HashSet<MetadataId> = order.iter().copied().collect();
        order.extend((0..count as u32).map(MetadataId).filter(|id| !named.contains(id)));
        let mut new = vec![MetadataId(0); count];
        for (at, old) in order.iter().enumerate() {
            new[old.0 as usize] = MetadataId(at as u32);
        }
        let map = |id: &mut MetadataId| *id = new[id.0 as usize];
        let mut nodes: Vec<MetadataNode> =
            order.iter().map(|old| self.module.metadata[old.0 as usize].clone()).collect();
        for node in &mut nodes {
            for operand in &mut node.operands {
                if let MetadataOperand::Node(id) = operand {
                    map(id);
                }
            }
        }
        self.module.metadata = nodes;
        for (_, list) in &mut self.module.named_metadata {
            list.iter_mut().for_each(map);
        }
        for list in self.module.global_metadata.values_mut() {
            list.iter_mut().for_each(|(_, id)| map(id));
        }
        self.module.special = std::mem::take(&mut self.module.special)
            .into_iter()
            .map(|(id, special)| (new[id.0 as usize], special))
            .collect();
        for global in &mut self.module.globals {
            if let GlobalKind::Function(function) = &mut global.kind {
                function.metadata.iter_mut().for_each(|(_, id)| map(id));
                for record in &mut function.debug_records {
                    map(&mut record.variable);
                    record.expression.iter_mut().for_each(map);
                    record.location.iter_mut().for_each(map);
                    if let DebugWhat::Assign { id, address_expression, .. } = &mut record.what {
                        map(id);
                        map(address_expression);
                    }
                }
                for instruction in &mut function.instructions {
                    instruction.metadata.iter_mut().for_each(|(_, id)| map(id));
                }
            }
        }
    }

    fn named_type(
        &mut self,
        name: String,
    ) -> Parsed<()> {
        self.next();
        self.expect_punct('=')?;
        self.expect_word("type")?;
        let body = if self.eat_word("opaque") {
            None
        } else {
            let ty = self.ty()?;
            match self.module.context.types.get(ty).clone() {
                Type::Struct { fields, packed } => Some(StructBody { fields, packed }),
                _ => return self.fail("a named type is a struct"),
            }
        };
        let types = &mut self.module.context.types;
        match types.named.iter_mut().find(|(one, _)| *one == name) {
            Some((_, slot)) if slot.is_none() => *slot = body,
            Some(_) => return self.fail(format!("%{name} is defined twice")),
            None => types.named.push((name, body)),
        }
        Ok(())
    }

    fn linkage(&mut self) -> Linkage {
        if let Token::Word(word) = self.peek()
            && let Some(linkage) = spelled(&LINKAGE, word)
        {
            self.next();
            return linkage;
        }
        self.eat_word("external");
        Linkage::External
    }

    fn unnamed_addr(&mut self) -> UnnamedAddr {
        if self.eat_word("unnamed_addr") {
            UnnamedAddr::Global
        } else if self.eat_word("local_unnamed_addr") {
            UnnamedAddr::Local
        } else {
            UnnamedAddr::None
        }
    }

    /// `ccc`, `fastcc` and their kin, `cc N` or `ccN`; C's when none is
    /// written.
    fn calling_convention(&mut self) -> Parsed<u32> {
        let Token::Word(word) = self.peek().clone() else { return Ok(0) };
        if let Some(&(_, number)) = crate::opcode::CONVENTIONS.iter().find(|(name, _)| *name == word) {
            self.next();
            return Ok(number);
        }
        if word == "cc" {
            self.next();
            return u32::try_from(self.unsigned()?).or_else(|_| self.fail("a calling convention is too large"));
        }
        match word.strip_prefix("cc").map(str::parse::<u32>) {
            Some(Ok(number)) => {
                self.next();
                Ok(number)
            }
            _ => Ok(0),
        }
    }

    fn address_space(&mut self) -> Parsed<u32> {
        if !self.eat_word("addrspace") {
            return Ok(0);
        }
        self.expect_punct('(')?;
        let space = u32::try_from(self.unsigned()?).or_else(|_| self.fail("an address space is too large"))?;
        self.expect_punct(')')?;
        Ok(space)
    }

    fn global_variable(
        &mut self,
        name: Name,
    ) -> Parsed<()> {
        self.next();
        self.expect_punct('=')?;
        let id = self.globals[&name];
        let linkage = self.linkage();
        let unnamed_addr = self.unnamed_addr();
        let address_space = self.address_space()?;
        let externally_initialized = self.eat_word("externally_initialized");
        let constant = match self.word()?.as_str() {
            "global" => false,
            "constant" => true,
            other => return self.fail(format!("expected `global` or `constant`, found `{other}`")),
        };
        let ty = self.ty()?;
        let initializer = if matches!(linkage, Linkage::External | Linkage::ExternWeak) && !self.starts_constant() {
            None
        } else {
            Some(self.constant(ty)?)
        };
        let mut align = None;
        while self.eat_punct(',') {
            if let Token::MetadataName(kind) = self.peek().clone() {
                let Token::MetadataId(number) = self.peek_at(1).clone() else {
                    return self.fail("a variable's attachment is `!kind !N`");
                };
                self.next();
                self.next();
                let node = self.metadata_id(number);
                self.module.global_metadata.entry(id.0).or_default().push((kind, node));
            } else if self.eat_word("align") {
                align = Some(self.unsigned()?);
            } else if self.eat_word("section") {
                let section = self.string()?;
                if let Some(name) = self.module.globals[id.0 as usize].name.owned() {
                    self.module.sections.insert(name, section);
                }
            } else {
                return self.fail(format!("unexpected {} after a global", self.describe()));
            }
        }
        self.module.globals[id.0 as usize] = GlobalValue {
            name: self.module.globals[id.0 as usize].name.clone(),
            linkage,
            unnamed_addr,
            address_space,
            kind: GlobalKind::Variable(GlobalVariable { externally_initialized, ty, constant, initializer, align }),
        };
        self.defined_globals.insert(id);
        Ok(())
    }

    /// Whether a constant starts here, rather than the next statement.
    fn starts_constant(&self) -> bool {
        match self.peek() {
            Token::Int { .. }
            | Token::Float(..)
            | Token::FloatBits { .. }
            | Token::HexFloat(_)
            | Token::Float80(_)
            | Token::Bytes(_)
            | Token::Global(_) => !matches!(self.peek_at(1), Token::Punct('=')),
            Token::Punct('[' | '{' | '<') => true,
            Token::Word(word) => {
                ["true", "false", "null", "poison", "undef", "zeroinitializer", "getelementptr"]
                    .contains(&word.as_str())
                    || spelled(&CAST, word).is_some()
                    || constant_binary(word)
            }
            _ => false,
        }
    }

    fn function(
        &mut self,
        define: bool,
    ) -> Parsed<()> {
        self.next();
        // `declare !dbg !36 i32 @f(...)`: clang's attachments stand before the
        // linkage; a `define`'s stand after the attributes.
        let mut early = Vec::new();
        while let Token::MetadataName(kind) = self.peek().clone() {
            let Token::MetadataId(number) = self.peek_at(1).clone() else { break };
            self.next();
            self.next();
            early.push((kind, self.metadata_id(number)));
        }
        let linkage = self.linkage();
        let calling_convention = self.calling_convention()?;
        let return_attrs = self.attributes(false)?;
        let returns = self.ty()?;
        let Token::Global(name) = self.next() else {
            self.at -= 1;
            return self.fail(format!("expected the function's name, found {}", self.describe()));
        };
        let id = self.globals[&name];
        let void = self.module.context.types.void();
        let mut function = Function::new(returns, void);
        function.return_attrs = return_attrs;
        function.calling_convention = calling_convention;
        let mut local = Local {
            function,
            values: HashMap::default(),
            blocks: HashMap::default(),
            pending_values: HashMap::default(),
            pending_blocks: HashMap::default(),
            pending_records: Vec::new(),
            next_slot: 0,
        };
        self.expect_punct('(')?;
        let mut parameters = Vec::new();
        let mut variadic = false;
        while !self.is_punct(')') {
            if !parameters.is_empty() {
                self.expect_punct(',')?;
            }
            if matches!(self.peek(), Token::Dots) {
                self.next();
                variadic = true;
                break;
            }
            let ty = self.ty()?;
            let attrs = self.attributes(false)?;
            let at = parameters.len() as u32;
            let value = ValueId(local.function.values.len() as u32);
            let name = match self.peek().clone() {
                Token::Local(name) => {
                    self.next();
                    Some(name)
                }
                _ => None,
            };
            local.function.values.push(ValueData { ty, name: None, def: ValueDef::Argument(at) });
            let line = self.line();
            self.name_value(&mut local, value, name, line)?;
            local.function.parameters.push(value);
            local.function.parameter_attrs.push(attrs);
            parameters.push(ty);
        }
        self.expect_punct(')')?;
        let unnamed_addr = self.unnamed_addr();
        let address_space = self.address_space()?;
        local.function.attrs = self.attributes(true)?;
        if self.eat_word("section") {
            let section = self.string()?;
            if let Some(name) = self.module.globals[id.0 as usize].name.owned() {
                self.module.sections.insert(name, section);
            }
        }
        if self.eat_word("personality") {
            let ty = self.ty()?;
            local.function.personality = Some(self.constant(ty)?);
        }
        while let Token::MetadataName(kind) = self.peek().clone() {
            let Token::MetadataId(number) = self.peek_at(1).clone() else { break };
            self.next();
            self.next();
            let node = self.metadata_id(number);
            local.function.metadata.push((kind, node));
        }
        local.function.metadata.splice(0..0, early);
        local.function.ty = self.module.context.types.intern(Type::Function { returns, parameters, variadic });
        if let Some(name) = self.module.globals[id.0 as usize].name.as_deref() {
            intrinsics::declare(&mut local.function, &self.module.context.types, name);
        }
        if define {
            self.body(&mut local)?;
        }
        local.function.index();
        self.module.globals[id.0 as usize] = GlobalValue {
            name: self.module.globals[id.0 as usize].name.clone(),
            linkage,
            unnamed_addr,
            address_space,
            kind: GlobalKind::Function(Box::new(local.function)),
        };
        self.defined_globals.insert(id);
        Ok(())
    }

    // ---- attributes

    /// Attributes until none follow; `groups` admits `#N` references.
    fn attributes(
        &mut self,
        groups: bool,
    ) -> Parsed<Vec<Attribute>> {
        let mut out = Vec::new();
        loop {
            match self.peek().clone() {
                Token::AttributeGroup(group) if groups => {
                    self.next();
                    let Some(attrs) = self.groups.get(&group) else {
                        return self.fail(format!("#{group} is never defined"));
                    };
                    out.extend(attrs.iter().cloned());
                }
                Token::Str(_) => {
                    let key = self.string()?;
                    let value = if self.eat_punct('=') { Some(self.string()?) } else { None };
                    out.push(Attribute::Str(key, value));
                }
                // `captures(none)` is `nocapture`; any other capture kind says
                // what the callee may do with the pointer, which no pass here
                // reads.
                Token::Word(word) if word == "captures" => {
                    self.next();
                    self.expect_punct('(')?;
                    let none = self.is_word("none");
                    while !self.eat_punct(')') {
                        if *self.peek() == Token::Eof {
                            return self.fail("`captures(` is never closed");
                        }
                        self.next();
                    }
                    if none {
                        out.push(Attribute::Flag("nocapture".to_owned()));
                    }
                }
                Token::Word(word) if TARGET_FLAG_ATTRIBUTES.contains(&word.as_str()) => {
                    self.next();
                    // An argument list (`uwtable(sync)`, `allocsize(0, 1)`) is
                    // not read.
                    if self.eat_punct('(') {
                        while !self.eat_punct(')') {
                            if *self.peek() == Token::Eof {
                                return self.fail(format!("`{word}(` is never closed"));
                            }
                            self.next();
                        }
                    }
                    out.push(Attribute::Flag(word));
                }
                Token::Word(word) if FLAG_ATTRIBUTES.contains(&word.as_str()) => {
                    self.next();
                    out.push(Attribute::Flag(word));
                }
                Token::Word(word) if INT_ATTRIBUTES.contains(&word.as_str()) => {
                    self.next();
                    let parenthesized = self.eat_punct('(');
                    let value = self.unsigned()?;
                    if parenthesized {
                        self.expect_punct(')')?;
                    }
                    out.push(Attribute::Int(word, value));
                }
                Token::Word(word) if TYPE_ATTRIBUTES.contains(&word.as_str()) => {
                    self.next();
                    self.expect_punct('(')?;
                    let ty = self.ty()?;
                    self.expect_punct(')')?;
                    out.push(Attribute::Type(word, ty));
                }
                Token::Word(word) if word == "range" => {
                    self.next();
                    self.expect_punct('(')?;
                    let ty = self.ty()?;
                    let Some(bits) = self.module.context.types.int_bits(ty) else {
                        return self.fail("a range is of an integer type");
                    };
                    let lower = self.bound(bits)?;
                    self.expect_punct(',')?;
                    let upper = self.bound(bits)?;
                    self.expect_punct(')')?;
                    out.push(Attribute::Range { ty, lower, upper });
                }
                Token::Word(word) if word == "memory" => {
                    self.next();
                    self.expect_punct('(')?;
                    let mut effects = Vec::new();
                    loop {
                        let location = match self.peek().clone() {
                            Token::Label(Name::Named(location)) => {
                                self.next();
                                Some(location)
                            }
                            _ => None,
                        };
                        effects.push((location, self.word()?));
                        if !self.eat_punct(',') {
                            break;
                        }
                    }
                    self.expect_punct(')')?;
                    out.push(Attribute::Memory(effects));
                }
                Token::Word(word) if word == "initializes" => {
                    self.next();
                    self.expect_punct('(')?;
                    let mut ranges = Vec::new();
                    loop {
                        self.expect_punct('(')?;
                        let lower = self.offset()?;
                        self.expect_punct(',')?;
                        let upper = self.offset()?;
                        self.expect_punct(')')?;
                        ranges.push((lower, upper));
                        if !self.eat_punct(',') {
                            break;
                        }
                    }
                    self.expect_punct(')')?;
                    out.push(Attribute::Initializes(ranges));
                }
                _ => return Ok(out),
            }
        }
    }

    /// A range's bound: an integer that fits `bits`, as bits.
    fn bound(
        &mut self,
        bits: u32,
    ) -> Parsed<u128> {
        let line = self.line();
        match self.next() {
            Token::Int { negative, magnitude } => {
                let fits = if negative { magnitude <= 1u128 << (bits - 1).min(127) } else { magnitude <= mask(bits) };
                if !fits {
                    return self
                        .fail_on(line, format!("{}{magnitude} does not fit i{bits}", if negative { "-" } else { "" }));
                }
                Ok(if negative { magnitude.wrapping_neg() } else { magnitude } & mask(bits))
            }
            _ => self.fail_on(line, "a range's bound is an integer"),
        }
    }

    // ---- types

    fn ty(&mut self) -> Parsed<TypeId> {
        self.refuse_subset()?;
        let base = match self.next() {
            Token::Word(word) => match word.as_str() {
                "void" => Type::Void,
                "label" => Type::Label,
                "metadata" => Type::Metadata,
                "token" => Type::Token,
                "float" => Type::Float(FloatKind::Float),
                "double" => Type::Float(FloatKind::Double),
                "x86_fp80" => Type::Float(FloatKind::X86Fp80),
                "ptr" => Type::Pointer(self.address_space()?),
                _ => match word.strip_prefix('i').and_then(|bits| bits.parse::<u32>().ok()) {
                    Some(bits) if (1..=128).contains(&bits) => Type::Int(bits),
                    Some(bits) => return self.fail(format!("i{bits}: MIR's integers are 1 to 128 bits")),
                    None => {
                        self.at -= 1;
                        return self.fail(format!("expected a type, found `{word}`"));
                    }
                },
            },
            Token::Punct('[') => {
                let count = self.unsigned()?;
                self.expect_word("x")?;
                let element = self.ty()?;
                self.expect_punct(']')?;
                Type::Array { element, count }
            }
            Token::Punct('<') if self.is_punct('{') => {
                self.next();
                let fields = self.type_list('}')?;
                self.expect_punct('>')?;
                Type::Struct { fields, packed: true }
            }
            Token::Punct('<') => {
                let count = u32::try_from(self.unsigned()?).or_else(|_| self.fail("a vector is too long"))?;
                self.expect_word("x")?;
                let element = self.ty()?;
                self.expect_punct('>')?;
                Type::Vector { element, count }
            }
            Token::Punct('{') => Type::Struct { fields: self.type_list('}')?, packed: false },
            Token::Local(Name::Named(name)) => {
                let types = &mut self.module.context.types;
                if !types.named.iter().any(|(one, _)| *one == name) {
                    types.named.push((name.clone(), None));
                }
                Type::Named(name)
            }
            _ => {
                self.at -= 1;
                return self.fail(format!("expected a type, found {}", self.describe()));
            }
        };
        let mut ty = self.module.context.types.intern(base);
        while self.is_punct('(') {
            self.next();
            let mut parameters = Vec::new();
            let mut variadic = false;
            while !self.is_punct(')') {
                if !parameters.is_empty() {
                    self.expect_punct(',')?;
                }
                if matches!(self.peek(), Token::Dots) {
                    self.next();
                    variadic = true;
                    break;
                }
                parameters.push(self.ty()?);
            }
            self.expect_punct(')')?;
            ty = self.module.context.types.intern(Type::Function { returns: ty, parameters, variadic });
        }
        if self.is_punct('*') {
            return self.fail("typed pointers are outside MIR's subset: use `ptr`");
        }
        Ok(ty)
    }

    /// Types separated by commas, up to and including `close`.
    fn type_list(
        &mut self,
        close: char,
    ) -> Parsed<Vec<TypeId>> {
        let mut out = Vec::new();
        while !self.eat_punct(close) {
            if !out.is_empty() {
                self.expect_punct(',')?;
            }
            out.push(self.ty()?);
        }
        Ok(out)
    }

    // ---- constants

    fn global_ref(
        &mut self,
        name: &Name,
        ty: TypeId,
        line: usize,
    ) -> Parsed<ConstantId> {
        let Some(&id) = self.globals.get(name) else {
            return self.fail_on(line, format!("@{} is never defined", display(name)));
        };
        if !matches!(self.module.context.types.get(ty), Type::Pointer(_)) {
            return self.fail_on(line, format!("@{} is a pointer", display(name)));
        }
        self.global_uses.push((id, ty, line));
        Ok(self.module.context.constant(Constant { ty, kind: ConstantKind::Global(id) }))
    }

    fn constant(
        &mut self,
        ty: TypeId,
    ) -> Parsed<ConstantId> {
        self.refuse_subset()?;
        let line = self.line();
        let shape = self.module.context.types.get(ty).clone();
        let kind = match self.next() {
            Token::Int { negative, magnitude } => {
                let Type::Int(bits) = shape else {
                    return self
                        .fail_on(line, format!("an integer given as {}", self.module.context.types.display(ty)));
                };
                let fits = if negative { magnitude <= 1u128 << (bits - 1).min(127) } else { magnitude <= mask(bits) };
                if !fits {
                    return self
                        .fail_on(line, format!("{}{magnitude} does not fit i{bits}", if negative { "-" } else { "" }));
                }
                let value = if negative { magnitude.wrapping_neg() } else { magnitude };
                ConstantKind::Int(value & mask(bits))
            }
            Token::Word(word) if word == "true" || word == "false" => {
                if shape != Type::Int(1) {
                    return self.fail_on(line, format!("`{word}` is an i1"));
                }
                ConstantKind::Int(u128::from(word == "true"))
            }
            Token::Float(value, narrow) => match shape {
                Type::Float(FloatKind::Float) => ConstantKind::Float(u64::from(narrow.to_bits())),
                _ => self.float(&shape, value, line)?,
            },
            Token::FloatBits { bits, digits } => match shape {
                Type::Float(FloatKind::Float) if digits == 8 => ConstantKind::Float(bits as u64),
                Type::Float(FloatKind::Double) if digits == 16 => ConstantKind::Float(bits as u64),
                Type::Float(FloatKind::X86Fp80) if digits == 20 => ConstantKind::Float(x86_fp80_to_f64(bits).to_bits()),
                Type::Float(kind) => {
                    let (name, width) = match kind {
                        FloatKind::Float => ("float", 8),
                        FloatKind::Double => ("double", 16),
                        FloatKind::X86Fp80 => ("x86_fp80", 20),
                    };
                    return self.fail_on(line, format!("f0x{bits:0digits$X} is not a {name}'s {width} hex digits"));
                }
                _ => return self.fail_on(line, "a floating constant given a non-floating type"),
            },
            Token::Float80(bits) => match shape {
                Type::Float(FloatKind::X86Fp80) => {
                    crate::fp80::kind(&crate::apfloat::ApFloat::from_bits(crate::apfloat::X87_DOUBLE_EXTENDED, bits))
                }
                _ => return self.fail_on(line, "`0xK` is an x86_fp80"),
            },
            Token::HexFloat(bits) => self.float(&shape, f64::from_bits(bits), line)?,
            Token::Word(word) if word == "null" => {
                if !matches!(shape, Type::Pointer(_)) {
                    return self.fail_on(line, "`null` is a pointer");
                }
                ConstantKind::Null
            }
            Token::Word(word) if word == "poison" => ConstantKind::Poison,
            Token::Word(word) if word == "undef" => ConstantKind::Undef,
            Token::Word(word) if word == "zeroinitializer" => ConstantKind::Zero,
            Token::Global(name) => return self.global_ref(&name, ty, line),
            Token::Bytes(bytes) => {
                let byte = self.module.context.types.int(8);
                if shape != (Type::Array { element: byte, count: bytes.len() as u64 }) {
                    return self.fail_on(
                        line,
                        format!("c\"...\" of {} bytes given as {}", bytes.len(), self.module.context.types.display(ty)),
                    );
                }
                ConstantKind::Bytes(bytes)
            }
            Token::Punct(open @ ('[' | '{' | '<')) => {
                let packed = open == '<' && self.eat_punct('{');
                let close = match open {
                    '[' => ']',
                    '<' if !packed => '>',
                    _ => '}',
                };
                let mut members = Vec::new();
                while !self.eat_punct(close) {
                    if !members.is_empty() {
                        self.expect_punct(',')?;
                    }
                    let member_ty = self.ty()?;
                    let expected = self.module.context.types.member(ty, members.len() as u64);
                    if expected != Some(member_ty) {
                        return self.fail_on(
                            line,
                            format!(
                                "member {} of {} is not a {}",
                                members.len(),
                                self.module.context.types.display(ty),
                                self.module.context.types.display(member_ty)
                            ),
                        );
                    }
                    members.push(self.constant(member_ty)?);
                }
                if packed {
                    self.expect_punct('>')?;
                }
                let count = match &shape {
                    Type::Array { count, .. } => *count as usize,
                    Type::Vector { count, .. } => *count as usize,
                    _ => self.module.context.types.fields(ty).map_or(0, <[TypeId]>::len),
                };
                if members.len() != count {
                    return self.fail_on(
                        line,
                        format!("{} members for {}", members.len(), self.module.context.types.display(ty)),
                    );
                }
                ConstantKind::Aggregate(members)
            }
            Token::Word(word) if word == "getelementptr" => {
                let mut flags = Flags::default();
                self.gep_flags(&mut flags);
                self.expect_punct('(')?;
                let source = self.ty()?;
                let mut operands = Vec::new();
                while self.eat_punct(',') {
                    let operand_ty = self.ty()?;
                    operands.push(self.constant(operand_ty)?);
                }
                self.expect_punct(')')?;
                ConstantKind::Expr(ConstantExpr::GetElementPtr {
                    source,
                    inbounds: flags.contains(Flags::INBOUNDS),
                    nusw: flags.contains(Flags::INBOUNDS) || flags.contains(Flags::NUSW),
                    nuw: flags.contains(Flags::NUW),
                    operands,
                })
            }
            Token::Word(word) if constant_binary(&word) => {
                let op = spelled(&BINARY, &word).expect("a binary operator");
                let mut flags = Flags::default();
                if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                    flags = self.flags(&["nuw", "nsw"]);
                }
                let _ = flags;
                self.expect_punct('(')?;
                let left_ty = self.ty()?;
                let lhs = self.constant(left_ty)?;
                self.expect_punct(',')?;
                let right_ty = self.ty()?;
                let rhs = self.constant(right_ty)?;
                self.expect_punct(')')?;
                if left_ty != ty || right_ty != ty {
                    return self.fail_on(line, "a constant binary operator's operands are of its own type");
                }
                ConstantKind::Expr(ConstantExpr::Binary { op, lhs, rhs })
            }
            Token::Word(word) if spelled(&CAST, &word).is_some() => {
                let op = spelled(&CAST, &word).expect("a cast");
                self.expect_punct('(')?;
                let from = self.ty()?;
                let value = self.constant(from)?;
                self.expect_word("to")?;
                let to = self.ty()?;
                self.expect_punct(')')?;
                if to != ty {
                    return self.fail_on(line, "a constant cast's type is the one it is given as");
                }
                ConstantKind::Expr(ConstantExpr::Cast { op, value })
            }
            _ => {
                self.at -= 1;
                return self.fail_on(line, format!("expected a constant, found {}", self.describe()));
            }
        };
        Ok(self.module.context.constant(Constant { ty, kind }))
    }

    fn float(
        &self,
        shape: &Type,
        value: f64,
        line: usize,
    ) -> Parsed<ConstantKind> {
        match shape {
            Type::Float(FloatKind::Double | FloatKind::X86Fp80) => Ok(ConstantKind::Float(value.to_bits())),
            Type::Float(FloatKind::Float) => {
                let narrow = value as f32;
                if f64::from(narrow).to_bits() != value.to_bits() && !value.is_nan() {
                    return self.fail_on(line, format!("{value} is not exactly a float"));
                }
                Ok(ConstantKind::Float(u64::from(narrow.to_bits())))
            }
            _ => self.fail_on(line, "a floating constant given a non-floating type"),
        }
    }

    // ---- metadata

    fn metadata_id(
        &mut self,
        number: u32,
    ) -> MetadataId {
        if let Some(&id) = self.metadata.get(&number) {
            return id;
        }
        let id = MetadataId(self.module.metadata.len() as u32);
        self.module.metadata.push(MetadataNode { distinct: false, operands: Vec::new() });
        self.metadata.insert(number, id);
        id
    }

    fn named_metadata(
        &mut self,
        name: String,
    ) -> Parsed<()> {
        self.next();
        self.expect_punct('=')?;
        if !matches!(self.next(), Token::Exclaim) {
            self.at -= 1;
            return self.fail("named metadata is `!{...}`");
        }
        self.expect_punct('{')?;
        let mut nodes = Vec::new();
        while !self.eat_punct('}') {
            if !nodes.is_empty() {
                self.expect_punct(',')?;
            }
            let Token::MetadataId(number) = self.next() else {
                self.at -= 1;
                return self.fail("named metadata lists `!N` nodes");
            };
            nodes.push(self.metadata_id(number));
        }
        self.module.named_metadata.push((name, nodes));
        Ok(())
    }

    fn metadata_definition(
        &mut self,
        number: u32,
    ) -> Parsed<()> {
        self.next();
        self.expect_punct('=')?;
        let distinct = self.eat_word("distinct");
        let id = self.metadata_id(number);
        if !self.defined_metadata.insert(id) {
            return self.fail(format!("!{number} is defined twice"));
        }
        if self.special_ahead() {
            let (special, operands) = self.special_node()?;
            self.module.metadata[id.0 as usize] = MetadataNode { distinct, operands };
            self.module.special.insert(id, special);
            return Ok(());
        }
        let operands = self.metadata_tuple()?;
        self.module.metadata[id.0 as usize] = MetadataNode { distinct, operands };
        Ok(())
    }

    /// `!Kind(`: specialized metadata (`!DILocation(line: 3, scope: !10)`).
    fn special_ahead(&self) -> bool {
        matches!(self.peek(), Token::MetadataName(_)) && matches!(self.peek_at(1), Token::Punct('('))
    }

    /// `!Kind(field: value, ...)`: the node's kind and field names, and its
    /// values in order. A value is a node, a string, an integer, a nested
    /// node, or a word and the `|`-joined words after it (a flag set).
    fn special_node(&mut self) -> Parsed<(Special, Vec<MetadataOperand>)> {
        let Token::MetadataName(kind) = self.next() else { unreachable!("special_ahead checked") };
        self.expect_punct('(')?;
        let (mut fields, mut operands) = (Vec::new(), Vec::new());
        while !self.eat_punct(')') {
            if !operands.is_empty() {
                self.expect_punct(',')?;
            }
            let field = match self.peek().clone() {
                Token::Label(Name::Named(field)) => {
                    self.next();
                    field
                }
                _ => String::new(),
            };
            fields.push(field);
            operands.push(self.special_value()?);
        }
        Ok((Special { kind, fields }, operands))
    }

    fn special_value(&mut self) -> Parsed<MetadataOperand> {
        match self.peek().clone() {
            Token::MetadataId(number) => {
                self.next();
                Ok(MetadataOperand::Node(self.metadata_id(number)))
            }
            Token::Str(_) => Ok(MetadataOperand::String(self.string()?)),
            Token::Int { negative, magnitude } => {
                self.next();
                let value = if negative { -(magnitude as i128) } else { magnitude as i128 };
                let i64 = self.module.context.types.int(64);
                Ok(MetadataOperand::Constant(self.module.context.int(i64, value)))
            }
            Token::Word(word) => {
                self.next();
                let mut text = word;
                while self.eat_punct('|') {
                    text.push_str(" | ");
                    text.push_str(&self.word()?);
                }
                Ok(MetadataOperand::Ident(text))
            }
            Token::Exclaim | Token::MetadataName(_) => self.metadata_operand(),
            _ => self.fail(format!("a value of a specialized node, found {}", self.describe())),
        }
    }

    /// A debug record's expression or location: `!N` or a node written in
    /// place.
    /// `operand`, unless it is `poison` or `undef`: what a record says of a
    /// value that is gone.
    fn live(
        &self,
        operand: Operand,
    ) -> Option<Operand> {
        match operand {
            Operand::Constant(id)
                if matches!(
                    self.module.context.get(id).kind,
                    ConstantKind::Poison | ConstantKind::Undef
                ) =>
            {
                None
            }
            other => Some(other),
        }
    }

    fn record_node(&mut self) -> Parsed<MetadataId> {
        match self.metadata_operand()? {
            MetadataOperand::Node(id) => Ok(id),
            _ => self.fail("a debug record names a node, `!N` or `!DIExpression()`"),
        }
    }

    /// `!{...}`.
    fn metadata_tuple(&mut self) -> Parsed<Vec<MetadataOperand>> {
        if !matches!(self.next(), Token::Exclaim) {
            self.at -= 1;
            return self.fail("MIR's metadata nodes are tuples, `!{...}`");
        }
        self.expect_punct('{')?;
        let mut operands = Vec::new();
        while !self.eat_punct('}') {
            if !operands.is_empty() {
                self.expect_punct(',')?;
            }
            operands.push(self.metadata_operand()?);
        }
        Ok(operands)
    }

    fn metadata_operand(&mut self) -> Parsed<MetadataOperand> {
        match self.peek().clone() {
            Token::Word(word) if word == "null" => {
                self.next();
                Ok(MetadataOperand::Null)
            }
            Token::MetadataId(number) => {
                self.next();
                Ok(MetadataOperand::Node(self.metadata_id(number)))
            }
            Token::MetadataName(_) if self.special_ahead() => {
                let (special, operands) = self.special_node()?;
                let id = MetadataId(self.module.metadata.len() as u32);
                self.module.metadata.push(MetadataNode { distinct: false, operands });
                self.module.special.insert(id, special);
                self.defined_metadata.insert(id);
                Ok(MetadataOperand::Node(id))
            }
            Token::Exclaim if matches!(self.peek_at(1), Token::Str(_)) => {
                self.next();
                Ok(MetadataOperand::String(self.string()?))
            }
            Token::Exclaim => {
                let operands = self.metadata_tuple()?;
                let id = MetadataId(self.module.metadata.len() as u32);
                self.module.metadata.push(MetadataNode { distinct: false, operands });
                self.defined_metadata.insert(id);
                Ok(MetadataOperand::Node(id))
            }
            _ => {
                let ty = self.ty()?;
                Ok(MetadataOperand::Constant(self.constant(ty)?))
            }
        }
    }

    /// `, !kind !N` attachments, after an instruction.
    fn attachments(&mut self) -> Parsed<Vec<(String, MetadataId)>> {
        let mut out = Vec::new();
        while self.is_punct(',') && matches!(self.peek_at(1), Token::MetadataName(_)) {
            self.next();
            let Token::MetadataName(kind) = self.next() else { unreachable!() };
            let Token::MetadataId(number) = self.next() else {
                self.at -= 1;
                return self.fail("an attachment names a node, `!N`");
            };
            out.push((kind, self.metadata_id(number)));
        }
        Ok(out)
    }

    // ---- function bodies

    fn name_value(
        &mut self,
        local: &mut Local,
        value: ValueId,
        name: Option<Name>,
        line: usize,
    ) -> Parsed<()> {
        match name {
            None | Some(Name::Numbered(_)) => {
                if let Some(Name::Numbered(number)) = name
                    && number != local.next_slot
                {
                    return self.fail_on(line, format!("expected to be numbered %{}", local.next_slot));
                }
                let slot = Name::Numbered(local.next_slot);
                local.next_slot += 1;
                self.bind(local, value, slot, line)
            }
            Some(name) => {
                if let Name::Named(text) = &name {
                    local.function.values[value.0 as usize].name = Some(text.clone());
                }
                self.bind(local, value, name, line)
            }
        }
    }

    /// `name` now means `value`, which a forward use may already have taken.
    fn bind(
        &mut self,
        local: &mut Local,
        value: ValueId,
        name: Name,
        line: usize,
    ) -> Parsed<()> {
        match local.values.get(&name).copied() {
            None => {
                local.values.insert(name, value);
                Ok(())
            }
            Some(used) if local.pending_values.remove(&used).is_some() => {
                let (defined, forward) =
                    (local.function.values[value.0 as usize].clone(), local.function.values[used.0 as usize].ty);
                if defined.ty != forward {
                    let types = &self.module.context.types;
                    return self.fail_on(
                        line,
                        format!(
                            "%{} is used as {} but defined as {}",
                            display(&name),
                            types.display(forward),
                            types.display(defined.ty)
                        ),
                    );
                }
                // The forward use took the id; the definition moves into it.
                local.function.values[used.0 as usize] = defined;
                local.function.values[value.0 as usize].name = None;
                for instruction in &mut local.function.instructions {
                    if instruction.result == Some(value) {
                        instruction.result = Some(used);
                    }
                }
                if let Some(slot) = local.function.parameters.iter_mut().find(|one| **one == value) {
                    *slot = used;
                }
                local.function.values.truncate(value.0 as usize);
                Ok(())
            }
            Some(_) => self.fail_on(line, format!("%{} is defined twice", display(&name))),
        }
    }

    fn block_id(
        &mut self,
        local: &mut Local,
        name: Name,
    ) -> BlockId {
        if let Some(&id) = local.blocks.get(&name) {
            return id;
        }
        let id = BlockId(local.function.blocks.len() as u32);
        local.function.blocks.push(Block::default());
        local.blocks.insert(name, id);
        local.pending_blocks.insert(id, self.line());
        id
    }

    fn start_block(
        &mut self,
        local: &mut Local,
        label: Option<Name>,
    ) -> Parsed<BlockId> {
        let name = match label {
            None | Some(Name::Numbered(_)) => {
                if let Some(Name::Numbered(number)) = label
                    && number != local.next_slot
                {
                    return self.fail(format!("label expected to be numbered {}", local.next_slot));
                }
                local.next_slot += 1;
                Name::Numbered(local.next_slot - 1)
            }
            Some(name) => name,
        };
        let id = self.block_id(local, name.clone());
        if local.pending_blocks.remove(&id).is_none() {
            return self.fail(format!("block {} is defined twice", display(&name)));
        }
        if let Name::Named(text) = name {
            local.function.blocks[id.0 as usize].name = Some(text);
        }
        local.function.insert_block(id, None).expect("a block defined once and not yet placed");
        Ok(id)
    }

    fn body(
        &mut self,
        local: &mut Local,
    ) -> Parsed<()> {
        self.expect_punct('{')?;
        let mut current: Option<BlockId> = None;
        loop {
            match self.peek().clone() {
                Token::Punct('}') => {
                    self.next();
                    break;
                }
                Token::Label(name) => {
                    if current.is_some() {
                        return self.fail("a block ends with its terminator");
                    }
                    self.next();
                    current = Some(self.start_block(local, Some(name))?);
                }
                _ => {
                    let block = match current {
                        Some(block) => block,
                        None => self.start_block(local, None)?,
                    };
                    current = Some(block);
                    if matches!(self.peek(), Token::Word(word) if word.starts_with("#dbg_")) {
                        self.debug_record(local)?;
                        continue;
                    }
                    let inst = self.instruction(local)?;
                    local.function.link_last(block, inst);
                    if local.function.instruction(inst).opcode.is_terminator() {
                        current = None;
                    }
                }
            }
        }
        if current.is_some() {
            return self.fail("the last block has no terminator");
        }
        if let Some((&block, &line)) = local.pending_blocks.iter().next() {
            let name =
                local.blocks.iter().find(|(_, id)| **id == block).map(|(name, _)| display(name)).unwrap_or_default();
            return Err(ParseError { line, message: format!("block %{name} is used but never defined") });
        }
        if let Some((&value, &line)) = local.pending_values.iter().next() {
            let name =
                local.values.iter().find(|(_, id)| **id == value).map(|(name, _)| display(name)).unwrap_or_default();
            return Err(ParseError { line, message: format!("%{name} is used but never defined") });
        }
        Ok(())
    }

    /// A use of the local `name` as `ty`.
    fn local_use(
        &mut self,
        local: &mut Local,
        name: Name,
        ty: TypeId,
        line: usize,
    ) -> Parsed<Operand> {
        if let Some(&id) = local.values.get(&name) {
            let had = local.function.value(id).ty;
            if had != ty {
                let types = &self.module.context.types;
                return self.fail_on(
                    line,
                    format!("%{} is {} but used as {}", display(&name), types.display(had), types.display(ty)),
                );
            }
            return Ok(Operand::Value(id));
        }
        let id = ValueId(local.function.values.len() as u32);
        local
            .function
            .values
            .push(
                ValueData {
                    ty,
                    name: match &name {
                        Name::Named(text) => Some(text.clone()),
                        Name::Numbered(_) => None,
                    },
                    def: ValueDef::Instruction(InstId(u32::MAX)),
                },
            );
        local.values.insert(name, id);
        local.pending_values.insert(id, line);
        Ok(Operand::Value(id))
    }

    fn value(
        &mut self,
        local: &mut Local,
        ty: TypeId,
    ) -> Parsed<Operand> {
        match self.peek().clone() {
            Token::Local(name) => {
                let line = self.line();
                self.next();
                self.local_use(local, name, ty, line)
            }
            _ => Ok(Operand::Constant(self.constant(ty)?)),
        }
    }

    fn typed_value(
        &mut self,
        local: &mut Local,
    ) -> Parsed<(TypeId, Operand)> {
        let ty = self.ty()?;
        Ok((ty, self.value(local, ty)?))
    }

    fn label(
        &mut self,
        local: &mut Local,
    ) -> Parsed<Operand> {
        self.expect_word("label")?;
        match self.next() {
            Token::Local(name) => Ok(Operand::Block(self.block_id(local, name))),
            _ => {
                self.at -= 1;
                self.fail(format!("expected a block, found {}", self.describe()))
            }
        }
    }

    fn gep_flags(
        &mut self,
        flags: &mut Flags,
    ) {
        loop {
            let flag = match self.peek() {
                Token::Word(word) if word == "inbounds" => Flags::INBOUNDS,
                Token::Word(word) if word == "nusw" => Flags::NUSW,
                Token::Word(word) if word == "nuw" => Flags::NUW,
                _ => return,
            };
            self.next();
            flags.insert(flag);
        }
    }

    /// Flags among `allowed`, in any order.
    fn flags(
        &mut self,
        allowed: &[&str],
    ) -> Flags {
        let mut flags = Flags::default();
        while let Token::Word(word) = self.peek() {
            let Some((flag, _)) = Flags::NAMES.iter().find(|(_, name)| name == word && allowed.contains(name)) else {
                break;
            };
            flags.insert(*flag);
            self.next();
        }
        flags
    }

    /// An atomic ordering word.
    fn ordering(&mut self) -> Parsed<AtomicOrdering> {
        match self.next() {
            Token::Word(word) => match AtomicOrdering::named(&word) {
                Some(ordering) => Ok(ordering),
                None => self.fail("an atomic ordering"),
            },
            _ => self.fail("an atomic ordering"),
        }
    }

    /// `syncscope("singlethread")`, if there, and the ordering after it.
    fn scope_and_ordering(&mut self) -> Parsed<(bool, AtomicOrdering)> {
        let mut single_thread = false;
        if self.eat_word("syncscope") {
            self.expect_punct('(')?;
            single_thread = self.string()? == "singlethread";
            self.expect_punct(')')?;
        }
        Ok((single_thread, self.ordering()?))
    }

    fn align(&mut self) -> Parsed<Option<u64>> {
        if self.is_punct(',') && self.peek_at(1) == &Token::Word("align".to_owned()) {
            self.next();
            self.next();
            return Ok(Some(self.unsigned()?));
        }
        Ok(None)
    }

    /// `, 4, 8` after a piece's variable: where in it the piece is, and how
    /// long.
    fn piece_span(&mut self) -> Parsed<(u32, u32)> {
        self.expect_punct(',')?;
        let offset = u32::try_from(self.unsigned()?).or_else(|_| self.fail("a piece's offset is a u32"))?;
        self.expect_punct(',')?;
        let bytes = u32::try_from(self.unsigned()?).or_else(|_| self.fail("a piece's length is a u32"))?;
        Ok((offset, bytes))
    }

    /// `#dbg_declare(ptr %x, !5)`, `#dbg_value(i16 %v, !5)` or `#dbg_gone(!5)`:
    /// what is said of a variable before the instruction that follows.
    fn debug_record(
        &mut self,
        local: &mut Local,
    ) -> Parsed<()> {
        let Token::Word(word) = self.next() else { unreachable!("a record") };
        self.expect_punct('(')?;
        let what = match word.as_str() {
            "#dbg_value" if matches!(self.peek(), Token::MetadataName(name) if name == "DIArgList") => {
                // clang's `!DIArgList(i32 %a, i32 %b)`: the values an
                // expression's `DW_OP_LLVM_arg n` names.
                self.next();
                self.expect_punct('(')?;
                let mut values = Vec::new();
                while !self.eat_punct(')') {
                    if !values.is_empty() {
                        self.expect_punct(',')?;
                    }
                    values.push(self.typed_value(local)?.1);
                }
                self.expect_punct(',')?;
                DebugWhat::Args(local.function.add_debug_args(values))
            }
            "#dbg_declare" | "#dbg_value" | "#dbg_piece" => {
                let (_, at) = self.typed_value(local)?;
                self.expect_punct(',')?;
                match word.as_str() {
                    "#dbg_declare" => DebugWhat::Declare(at),
                    "#dbg_value" => DebugWhat::Value(at),
                    // The piece's place in the variable follows its name.
                    _ => DebugWhat::Piece { value: at, offset: 0, bytes: 0 },
                }
            }
            "#dbg_assign" => {
                let (_, value) = self.typed_value(local)?;
                let value = self.live(value);
                self.expect_punct(',')?;
                // The assignment's id, address and their expression follow
                // the variable and the value's expression.
                DebugWhat::Assign { value, address: None, id: MetadataId(0), address_expression: MetadataId(0) }
            }
            "#dbg_gonepiece" => DebugWhat::GonePiece { offset: 0, bytes: 0 },
            "#dbg_gone" => DebugWhat::Gone,
            other => return self.fail(format!("`{other}` is not a debug record")),
        };
        let Token::MetadataId(number) = self.next() else {
            self.at -= 1;
            return self.fail("a debug record names its variable, `!N`");
        };
        let variable = self.metadata_id(number);
        let what = match what {
            DebugWhat::Piece { value, .. } => {
                let (offset, bytes) = self.piece_span()?;
                DebugWhat::Piece { value, offset, bytes }
            }
            DebugWhat::GonePiece { .. } => {
                let (offset, bytes) = self.piece_span()?;
                DebugWhat::GonePiece { offset, bytes }
            }
            other => other,
        };
        let (mut expression, mut location) = (None, None);
        let mut what = what;
        if let DebugWhat::Assign { value, .. } = what {
            // `, !DIExpression(), !id, ptr %a, !DIExpression(), !loc`
            self.expect_punct(',')?;
            expression = Some(self.record_node()?);
            self.expect_punct(',')?;
            let id = self.record_node()?;
            self.expect_punct(',')?;
            let (_, address) = self.typed_value(local)?;
            let address = self.live(address);
            self.expect_punct(',')?;
            let address_expression = self.record_node()?;
            self.expect_punct(',')?;
            location = Some(self.record_node()?);
            what = DebugWhat::Assign { value, address, id, address_expression };
        }
        // clang's `, !DIExpression(), !loc` after a declare or value's
        // variable.
        if matches!(
            what,
            DebugWhat::Declare(_) | DebugWhat::Value(_) | DebugWhat::Args(_)
        ) && self.eat_punct(',') {
            expression = Some(self.record_node()?);
            if self.eat_punct(',') {
                location = Some(self.record_node()?);
            }
        }
        self.expect_punct(')')?;
        local.pending_records.push((variable, what, expression, location));
        Ok(())
    }

    fn instruction(
        &mut self,
        local: &mut Local,
    ) -> Parsed<InstId> {
        let result_name = match (self.peek().clone(), self.peek_at(1).clone()) {
            (Token::Local(name), Token::Punct('=')) => {
                self.next();
                self.next();
                Some(name)
            }
            _ => None,
        };
        let line = self.line();
        let tail = if self.eat_word("tail") {
            Tail::Tail
        } else if self.eat_word("musttail") {
            Tail::MustTail
        } else if self.eat_word("notail") {
            Tail::NoTail
        } else {
            Tail::None
        };
        self.refuse_subset()?;
        let word = self.word()?;
        if tail != Tail::None && word != "call" {
            return self.fail("a tail marker precedes `call`");
        }
        let (void, bool) = (self.module.context.types.void(), self.module.context.types.int(1));
        let fast = ["fast", "reassoc", "nnan", "ninf", "nsz", "arcp", "contract", "afn"];
        let mut flags = Flags::default();
        let (opcode, ty, operands) = if let Some(op) = spelled(&BINARY, &word) {
            flags = match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl => self.flags(&["nuw", "nsw"]),
                BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::LShr | BinaryOp::AShr => self.flags(&["exact"]),
                BinaryOp::Or => self.flags(&["disjoint"]),
                BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem => self.flags(&fast),
                _ => Flags::default(),
            };
            let (ty, left) = self.typed_value(local)?;
            self.expect_punct(',')?;
            let right = self.value(local, ty)?;
            (Opcode::Binary(op), ty, vec![left, right])
        } else if let Some(op) = spelled(&CAST, &word) {
            flags = match op {
                CastOp::Trunc => self.flags(&["nuw", "nsw"]),
                CastOp::ZExt | CastOp::UIToFP => self.flags(&["nneg"]),
                CastOp::FPTrunc | CastOp::FPExt | CastOp::FPRound => self.flags(&fast),
                _ => Flags::default(),
            };
            let (_, value) = self.typed_value(local)?;
            self.expect_word("to")?;
            let to = self.ty()?;
            (Opcode::Cast(op), to, vec![value])
        } else {
            match word.as_str() {
                "ret" => {
                    if self.eat_word("void") {
                        (Opcode::Ret, void, vec![])
                    } else {
                        let (_, value) = self.typed_value(local)?;
                        (Opcode::Ret, void, vec![value])
                    }
                }
                "br" => {
                    if self.is_word("label") {
                        let dest = self.label(local)?;
                        (Opcode::Br, void, vec![dest])
                    } else {
                        let (ty, condition) = self.typed_value(local)?;
                        if ty != bool {
                            return self.fail("a branch condition is an i1");
                        }
                        self.expect_punct(',')?;
                        let yes = self.label(local)?;
                        self.expect_punct(',')?;
                        let no = self.label(local)?;
                        (Opcode::Br, void, vec![condition, yes, no])
                    }
                }
                "switch" => {
                    flags = self.flags(&["table"]);
                    let (ty, condition) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let default = self.label(local)?;
                    self.expect_punct('[')?;
                    let mut operands = vec![condition, default];
                    while !self.eat_punct(']') {
                        let case_ty = self.ty()?;
                        if case_ty != ty {
                            return self.fail("a case has the condition's type");
                        }
                        operands.push(Operand::Constant(self.constant(ty)?));
                        self.expect_punct(',')?;
                        operands.push(self.label(local)?);
                    }
                    (Opcode::Switch, void, operands)
                }
                "unreachable" => (Opcode::Unreachable, void, vec![]),
                "resume" => {
                    let (_, value) = self.typed_value(local)?;
                    (Opcode::Resume, void, vec![value])
                }
                "fneg" => {
                    flags = self.flags(&fast);
                    let (ty, value) = self.typed_value(local)?;
                    (Opcode::FNeg, ty, vec![value])
                }
                "freeze" => {
                    let (ty, value) = self.typed_value(local)?;
                    (Opcode::Freeze, ty, vec![value])
                }
                "icmp" | "fcmp" => {
                    flags = if word == "icmp" { self.flags(&["samesign"]) } else { self.flags(&fast) };
                    let predicate = self.word()?;
                    let opcode = if word == "icmp" {
                        Opcode::ICmp(
                            spelled(&INT_PREDICATE, &predicate)
                                .ok_or_else(|| self.error(format!("`{predicate}` is not an icmp predicate")))?,
                        )
                    } else {
                        Opcode::FCmp(
                            spelled(&FLOAT_PREDICATE, &predicate)
                                .ok_or_else(|| self.error(format!("`{predicate}` is not an fcmp predicate")))?,
                        )
                    };
                    let (ty, left) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let right = self.value(local, ty)?;
                    let result = match self.module.context.types.get(ty).clone() {
                        Type::Vector { count, .. } => {
                            self.module.context.types.intern(Type::Vector { element: bool, count })
                        }
                        _ => bool,
                    };
                    (opcode, result, vec![left, right])
                }
                "select" => {
                    flags = self.flags(&fast);
                    let (_, condition) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let (ty, yes) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let (other, no) = self.typed_value(local)?;
                    if other != ty {
                        return self.fail("a select's two values have one type");
                    }
                    (Opcode::Select, ty, vec![condition, yes, no])
                }
                "phi" => {
                    flags = self.flags(&fast);
                    let ty = self.ty()?;
                    let mut operands = Vec::new();
                    loop {
                        self.expect_punct('[')?;
                        operands.push(self.value(local, ty)?);
                        self.expect_punct(',')?;
                        let Token::Local(name) = self.next() else {
                            self.at -= 1;
                            return self.fail("a phi input names its block");
                        };
                        operands.push(Operand::Block(self.block_id(local, name)));
                        self.expect_punct(']')?;
                        if !(self.is_punct(',') && self.peek_at(1) == &Token::Punct('[')) {
                            break;
                        }
                        self.next();
                    }
                    (Opcode::Phi, ty, operands)
                }
                "extractvalue" | "insertvalue" => {
                    let (aggregate_ty, aggregate) = self.typed_value(local)?;
                    let mut operands = vec![aggregate];
                    let mut inserted = None;
                    if word == "insertvalue" {
                        self.expect_punct(',')?;
                        let (ty, value) = self.typed_value(local)?;
                        operands.push(value);
                        inserted = Some(ty);
                    }
                    let mut indices = Vec::new();
                    let mut member = aggregate_ty;
                    while self.is_punct(',') && matches!(self.peek_at(1), Token::Int { .. }) {
                        self.next();
                        let index = self.unsigned()?;
                        member = self
                            .module
                            .context
                            .types
                            .member(member, index)
                            .ok_or_else(|| self.error(format!("no member {index}")))?;
                        indices.push(u32::try_from(index).or_else(|_| self.fail("an index is too large"))?);
                    }
                    if indices.is_empty() {
                        return self.fail(format!("{word} takes at least one index"));
                    }
                    match inserted {
                        None => (Opcode::ExtractValue(indices), member, operands),
                        Some(ty) if ty == member => (Opcode::InsertValue(indices), aggregate_ty, operands),
                        Some(_) => return self.fail("the inserted value has the member's type"),
                    }
                }
                "alloca" => {
                    let allocated = self.ty()?;
                    let mut operands = Vec::new();
                    if self.is_punct(',')
                        && !matches!(
                            self.peek_at(1),
                            Token::Word(word) if word == "align" || word == "addrspace"
                        )
                        && !matches!(self.peek_at(1), Token::MetadataName(_))
                    {
                        self.next();
                        operands.push(self.typed_value(local)?.1);
                    }
                    let align = self.align()?;
                    let mut address_space = 0;
                    if self.is_punct(',') && self.peek_at(1) == &Token::Word("addrspace".to_owned()) {
                        self.next();
                        address_space = self.address_space()?;
                    }
                    let ty = self.module.context.types.ptr(address_space);
                    (Opcode::Alloca { allocated, align, address_space }, ty, operands)
                }
                "load" => {
                    let atomic = self.eat_word("atomic");
                    let volatile = self.eat_word("volatile");
                    let ty = self.ty()?;
                    self.expect_punct(',')?;
                    let (_, pointer) = self.typed_value(local)?;
                    if atomic {
                        let (single_thread, ordering) = self.scope_and_ordering()?;
                        let align = self.align()?;
                        let info = AtomicInfo { kind: AtomicKind::Load, ordering, single_thread, volatile, align };
                        (Opcode::Atomic(info), ty, vec![pointer])
                    } else {
                        let align = self.align()?;
                        (Opcode::Load { align, volatile }, ty, vec![pointer])
                    }
                }
                "store" => {
                    let atomic = self.eat_word("atomic");
                    let volatile = self.eat_word("volatile");
                    let (_, value) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let (_, pointer) = self.typed_value(local)?;
                    if atomic {
                        let (single_thread, ordering) = self.scope_and_ordering()?;
                        let align = self.align()?;
                        let info = AtomicInfo { kind: AtomicKind::Store, ordering, single_thread, volatile, align };
                        (Opcode::Atomic(info), void, vec![value, pointer])
                    } else {
                        let align = self.align()?;
                        (Opcode::Store { align, volatile }, void, vec![value, pointer])
                    }
                }
                "atomicrmw" => {
                    let volatile = self.eat_word("volatile");
                    let op = match self.next() {
                        Token::Word(word) => RMW_OPS.iter().find(|(_, name)| *name == word).map(|(op, _)| *op),
                        _ => None,
                    };
                    let Some(op) = op else { return self.fail("an atomicrmw operation") };
                    let (_, pointer) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let (ty, value) = self.typed_value(local)?;
                    let (single_thread, ordering) = self.scope_and_ordering()?;
                    let align = self.align()?;
                    let info = AtomicInfo { kind: AtomicKind::Rmw(op), ordering, single_thread, volatile, align };
                    (Opcode::Atomic(info), ty, vec![pointer, value])
                }
                "cmpxchg" => {
                    let weak = self.eat_word("weak");
                    let volatile = self.eat_word("volatile");
                    let (_, pointer) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let (ty, expected) = self.typed_value(local)?;
                    self.expect_punct(',')?;
                    let (_, new) = self.typed_value(local)?;
                    let (single_thread, ordering) = self.scope_and_ordering()?;
                    let failure = self.ordering()?;
                    let align = self.align()?;
                    let flag = self.module.context.types.int(1);
                    let result =
                        self.module.context.types.intern(Type::Struct { fields: vec![ty, flag], packed: false });
                    let info = AtomicInfo {
                        kind: AtomicKind::CmpXchg { failure, weak },
                        ordering,
                        single_thread,
                        volatile,
                        align,
                    };
                    (Opcode::Atomic(info), result, vec![pointer, expected, new])
                }
                "fence" => {
                    let (single_thread, ordering) = self.scope_and_ordering()?;
                    let info =
                        AtomicInfo { kind: AtomicKind::Fence, ordering, single_thread, volatile: false, align: None };
                    (Opcode::Atomic(info), void, Vec::new())
                }
                "getelementptr" => {
                    self.gep_flags(&mut flags);
                    let source = self.ty()?;
                    self.expect_punct(',')?;
                    let (ty, base) = self.typed_value(local)?;
                    let mut operands = vec![base];
                    while self.is_punct(',') && !matches!(self.peek_at(1), Token::MetadataName(_)) {
                        self.next();
                        operands.push(self.typed_value(local)?.1);
                    }
                    (Opcode::GetElementPtr { source }, ty, operands)
                }
                "call" | "invoke" => {
                    if word == "call" {
                        flags = self.flags(&fast);
                    }
                    let calling_convention = self.calling_convention()?;
                    let return_attrs = self.attributes(false)?;
                    let space = self.address_space()?;
                    let written = self.ty()?;
                    let pointer = self.module.context.types.ptr(space);
                    let callee = self.value(local, pointer)?;
                    self.expect_punct('(')?;
                    let mut operands = Vec::new();
                    let mut argument_attrs = Vec::new();
                    let mut argument_types = Vec::new();
                    while !self.eat_punct(')') {
                        if !operands.is_empty() {
                            self.expect_punct(',')?;
                        }
                        let ty = self.ty()?;
                        argument_attrs.push(self.attributes(false)?);
                        operands.push(self.value(local, ty)?);
                        argument_types.push(ty);
                    }
                    let attrs = self.attributes(true)?;
                    let function_type = match self.module.context.types.get(written) {
                        Type::Function { .. } => written,
                        _ => self.module.context.types.intern(Type::Function {
                            returns: written,
                            parameters: argument_types,
                            variadic: false,
                        }),
                    };
                    let (returns, _, _) = self.module.signature(function_type);
                    let info = Box::new(CallInfo {
                        function_type,
                        calling_convention,
                        return_attrs,
                        argument_attrs,
                        attrs,
                        tail,
                    });
                    if word == "call" {
                        operands.push(callee);
                        (Opcode::Call(info), returns, operands)
                    } else {
                        self.expect_word("to")?;
                        operands.push(self.label(local)?);
                        self.expect_word("unwind")?;
                        operands.push(self.label(local)?);
                        operands.push(callee);
                        (Opcode::Invoke(info), returns, operands)
                    }
                }
                "landingpad" => {
                    let ty = self.ty()?;
                    let cleanup = self.eat_word("cleanup");
                    let mut clauses = Vec::new();
                    let mut operands = Vec::new();
                    loop {
                        let clause = if self.eat_word("catch") {
                            Clause::Catch
                        } else if self.eat_word("filter") {
                            Clause::Filter
                        } else {
                            break;
                        };
                        clauses.push(clause);
                        operands.push(self.typed_value(local)?.1);
                    }
                    (Opcode::LandingPad { cleanup, clauses }, ty, operands)
                }
                _ => {
                    return Err(ParseError {
                        line,
                        message: format!("`{word}` is not an instruction of MIR's subset"),
                    });
                }
            }
        };
        let metadata = self.attachments()?;
        let id = InstId(local.function.instructions.len() as u32);
        let result = if self.module.context.types.is_void(ty) {
            if result_name.is_some() {
                return Err(ParseError { line, message: "a void instruction has no result to name".to_owned() });
            }
            None
        } else {
            let value = ValueId(local.function.values.len() as u32);
            local.function.values.push(ValueData { ty, name: None, def: ValueDef::Instruction(id) });
            Some(value)
        };
        local.function.instructions.push(Instruction { opcode, ty, operands, flags, result, metadata, link: (0, 0) });
        for (variable, what, expression, location) in std::mem::take(&mut local.pending_records) {
            local.function.debug_records.push(DebugRecord { before: id, variable, what, expression, location });
        }
        if let Some(value) = result {
            self.name_value(local, value, result_name, line)?;
        }
        Ok(id)
    }

    fn error(
        &self,
        message: String,
    ) -> ParseError {
        ParseError { line: self.line(), message }
    }
}

pub fn display(name: &Name) -> String {
    match name {
        Name::Named(text) => crate::print::quoted(text),
        Name::Numbered(number) => number.to_string(),
    }
}

/// The double nearest an x87 extended value (16 bits of sign and exponent, then
/// the 64-bit significand with its integer bit explicit). The significand
/// rounds to 53 bits once and the scaling is exact, so a result below the
/// double normals is rounded twice.
fn x86_fp80_to_f64(bits: u128) -> f64 {
    let negative = bits >> 79 & 1 == 1;
    let exponent = (bits >> 64 & 0x7FFF) as i32;
    let significand = bits as u64;
    let magnitude = match exponent {
        0x7FFF if significand << 1 == 0 => f64::INFINITY,
        0x7FFF => f64::NAN,
        _ => {
            // value = significand * 2^(exponent - 16383 - 63); a denormal has
            // exponent 1's scale.
            let scale = exponent.max(1) - 16383 - 63;
            let (half, rest) = (scale / 2, scale - scale / 2);
            significand as f64 * 2f64.powi(half) * 2f64.powi(rest)
        }
    };
    if negative { -magnitude } else { magnitude }
}

/// LLVM's auto-upgrade, the other way: `llvm.lifetime.start` and `.end` lost
/// their size argument (they cover the whole object), and llrm's passes read
/// `(i64 size, ptr)`. Each one-argument declaration and call becomes the
/// two-argument one with the size -1, unknown. Without it a marker is a call
/// that may capture its object, and nothing about the object is promoted or
/// forwarded (`wide_frame`: 824 bytes against the pin's 36). `parse::module`
/// does not call it: `llrm-cc` upgrades the text (`lifetimes.rs`); a reader of
/// the pin's IR as it is (`llrm-arm64-llc`) does.
pub fn upgrade_lifetimes(module: &mut Module) {
    let Module { context, globals, .. } = module;
    let mut upgraded: HashMap<GlobalId, (TypeId, TypeId)> = HashMap::default();
    let size = context.types.int(64);
    let void = context.types.void();
    for (at, global) in globals.iter_mut().enumerate() {
        let is_lifetime = matches!(
            global.intrinsic(),
            Some(intrinsics::Intrinsic::LifetimeStart | intrinsics::Intrinsic::LifetimeEnd)
        );
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        let Type::Function { parameters, variadic: false, .. } = context.types.get(function.ty).clone() else {
            continue;
        };
        let [pointer] = parameters[..] else { continue };
        if !is_lifetime || !function.is_declaration() {
            continue;
        }
        let new =
            context.types.intern(Type::Function { returns: void, parameters: vec![size, pointer], variadic: false });
        let old = std::mem::replace(function.as_mut(), Function::new(new, void));
        function.parameter_attrs = old.parameter_attrs.clone();
        function.parameter_attrs.insert(0, Vec::new());
        function.return_attrs = old.return_attrs.clone();
        function.attrs = old.attrs.clone();
        function.calling_convention = old.calling_convention;
        let pointer_parameter = old.value(old.parameters()[0]).clone();
        let size_parameter = ValueData { ty: size, name: None, def: ValueDef::Argument(0) };
        let pointer_parameter = ValueData { name: None, def: ValueDef::Argument(1), ..pointer_parameter };
        for data in [size_parameter, pointer_parameter] {
            function.parameters.push(ValueId(function.values.len() as u32));
            function.values.push(data);
            function.uses.push_value();
        }
        upgraded.insert(GlobalId(at as u32), (new, pointer));
    }
    if upgraded.is_empty() {
        return;
    }
    let unknown = Operand::Constant(context.int(size, -1));
    for global in globals.iter_mut() {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        let calls: Vec<_> = function
            .walk()
            .filter_map(|(_, inst)| {
                let callee = crate::memory::callee(context, function, inst)?;
                let &(new, _) = upgraded.get(&callee)?;
                matches!(function.instruction(inst).operands.len(), 2).then_some((inst, new))
            })
            .collect();
        for (inst, new) in calls {
            let Opcode::Call(info) = function.instruction(inst).opcode.clone() else { continue };
            let mut info = info;
            info.function_type = new;
            info.argument_attrs.insert(0, Vec::new());
            let operands = &function.instruction(inst).operands;
            let rewritten = vec![unknown, operands[0], operands[1]];
            function.set_opcode(inst, Opcode::Call(info));
            function.set_operands(inst, rewritten);
        }
    }
}

/// The binary operators LLVM still has as constant expressions.
fn constant_binary(word: &str) -> bool {
    matches!(
        spelled(&BINARY, word),
        Some(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Xor)
    )
}
