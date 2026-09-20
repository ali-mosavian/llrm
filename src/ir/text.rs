//! Deterministic textual assembly for the portable SSA IR.
//!
//! `.qir` is deliberately a replay format rather than a permissive exchange
//! syntax.  The grammar is line-oriented, closed, and maps one-for-one onto
//! the owned IR model.  This keeps textual fixtures useful for reviewing a
//! transformation without making the text format another semantic layer.

use std::collections::BTreeSet;
use std::fmt::{self, Write};

use super::*;

/// A malformed or unsupported `.qir` input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl TextError {
    fn new(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            column,
            message: message.into(),
        }
    }
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "qir:{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for TextError {}

/// Render a module in canonical `.qir` form.
pub fn write(module: &Module) -> String {
    let mut out = String::new();
    writeln!(out, "qir {FORMAT_VERSION}").expect("string writes cannot fail");
    out.push_str("module ");
    push_string(&mut out, &module.name);
    out.push('\n');
    for type_ in &module.types {
        write_type(&mut out, type_);
    }
    for global in &module.globals {
        write_global(&mut out, global);
    }
    for function in &module.functions {
        write_function(&mut out, function);
    }
    out.push_str("end\n");
    out
}

/// Parse a complete `.qir` module.
///
/// Syntax is checked while reading; the completed module then passes through
/// [`Module::verify`] for cross-reference, type, and CFG validation.
pub fn parse(input: &str) -> Result<Module, TextError> {
    let mut parser = Parser::new(input)?;
    parser.expect_word("qir")?;
    let version = parser.u32()?;
    if version != FORMAT_VERSION {
        return Err(parser.error(format!(
            "unsupported qir version {version}; expected {FORMAT_VERSION}"
        )));
    }
    parser.end_line()?;
    parser.expect_word("module")?;
    let name = parser.string()?;
    parser.end_line()?;

    let mut module = Module {
        name,
        types: Vec::new(),
        globals: Vec::new(),
        functions: Vec::new(),
    };
    let mut type_ids = BTreeSet::new();
    let mut global_ids = BTreeSet::new();
    let mut function_ids = BTreeSet::new();
    loop {
        match parser.peek_word()? {
            "type" => {
                let type_ = parse_type(&mut parser)?;
                if !type_ids.insert(type_.id.get()) {
                    return Err(parser.error("duplicate type id"));
                }
                module.types.push(type_);
            }
            "global" => {
                let global = parse_global(&mut parser)?;
                if !global_ids.insert(global.id.get()) {
                    return Err(parser.error("duplicate global id"));
                }
                module.globals.push(global);
            }
            "function" => {
                let function = parse_function(&mut parser)?;
                if !function_ids.insert(function.id.get()) {
                    return Err(parser.error("duplicate function id"));
                }
                module.functions.push(function);
            }
            "end" => {
                parser.expect_word("end")?;
                parser.end_line()?;
                if !parser.at_eof() {
                    return Err(parser.error("content after end"));
                }
                module.verify().map_err(|diagnostics| {
                    let messages = diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.message)
                        .collect::<Vec<_>>()
                        .join("; ");
                    parser.error(format!("invalid module: {messages}"))
                })?;
                return Ok(module);
            }
            _ => return Err(parser.error("expected type, global, function, or end")),
        }
    }
}

fn write_type(out: &mut String, type_: &Type) {
    write!(out, "type {} ", type_.id).expect("string writes cannot fail");
    match &type_.kind {
        TypeKind::Void => out.push_str("void"),
        TypeKind::Integer { bits } => {
            write!(out, "integer {bits}").expect("string writes cannot fail")
        }
        TypeKind::Float(kind) => {
            write!(out, "float {}", float_kind_name(*kind)).expect("string writes cannot fail")
        }
        TypeKind::Pointer { address_space } => {
            write!(out, "pointer {}", address_space_name(*address_space))
                .expect("string writes cannot fail")
        }
        TypeKind::Array { element, length } => {
            write!(out, "array {element} {length}").expect("string writes cannot fail")
        }
        TypeKind::Structure { fields, packed } => {
            write!(out, "structure {} ", bool_name(*packed)).expect("string writes cannot fail");
            push_type_list(out, fields);
        }
    }
    out.push('\n');
}

fn write_global(out: &mut String, global: &Global) {
    write!(out, "global {} ", global.id).expect("string writes cannot fail");
    push_string(out, &global.name);
    write!(
        out,
        " {} {} {} ",
        global.type_id,
        linkage_name(global.linkage),
        bool_name(global.constant)
    )
    .expect("string writes cannot fail");
    match &global.initializer {
        Some(value) => {
            out.push_str("some ");
            push_constant(out, value);
        }
        None => out.push_str("none"),
    }
    out.push('\n');
}

fn write_function(out: &mut String, function: &Function) {
    write!(out, "function {} ", function.id).expect("string writes cannot fail");
    push_string(out, &function.name);
    write!(
        out,
        " linkage {} result {} parameters ",
        linkage_name(function.linkage),
        function.signature.result
    )
    .expect("string writes cannot fail");
    push_type_list(out, &function.signature.parameters);
    write!(
        out,
        " variadic {} cc {} attributes ",
        bool_name(function.signature.variadic),
        calling_convention_name(function.signature.calling_convention)
    )
    .expect("string writes cannot fail");
    push_attributes(out, &function.attributes);
    out.push('\n');
    for parameter in &function.parameters {
        writeln!(out, "param {} {}", parameter.id, parameter.type_id)
            .expect("string writes cannot fail");
    }
    for block in &function.blocks {
        writeln!(out, "block {}", block.id).expect("string writes cannot fail");
        for instruction in &block.instructions {
            write_instruction(out, instruction);
        }
        write_terminator(out, &block.terminator);
        out.push_str("endblock\n");
    }
    out.push_str("endfunction\n");
}

fn write_instruction(out: &mut String, instruction: &Instruction) {
    write!(out, "inst {} results ", instruction.id).expect("string writes cannot fail");
    push_values(out, &instruction.results);
    out.push(' ');
    match &instruction.kind {
        InstructionKind::Phi { incoming } => {
            out.push_str("phi ");
            out.push('[');
            for (index, item) in incoming.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                write!(out, "{}:", item.predecessor).expect("string writes cannot fail");
                push_operand(out, &item.value);
            }
            out.push(']');
        }
        InstructionKind::StackAlloc {
            size,
            alignment,
            address_space,
        } => {
            write!(
                out,
                "alloca {size} {alignment} {}",
                address_space_name(*address_space)
            )
            .expect("string writes cannot fail");
        }
        InstructionKind::Unary { op, operand } => {
            write!(out, "unary {} ", unary_name(*op)).expect("string writes cannot fail");
            push_operand(out, operand);
        }
        InstructionKind::Binary { op, left, right } => {
            write!(out, "binary {} ", binary_name(*op)).expect("string writes cannot fail");
            push_operand(out, left);
            out.push(' ');
            push_operand(out, right);
        }
        InstructionKind::Compare {
            predicate,
            left,
            right,
        } => {
            write!(out, "compare {} ", predicate_name(*predicate))
                .expect("string writes cannot fail");
            push_operand(out, left);
            out.push(' ');
            push_operand(out, right);
        }
        InstructionKind::Cast { op, operand, to } => {
            write!(out, "cast {} ", cast_name(*op)).expect("string writes cannot fail");
            push_operand(out, operand);
            write!(out, " {to}").expect("string writes cannot fail");
        }
        InstructionKind::Load {
            address,
            alignment,
            volatile,
        } => {
            write!(out, "load {alignment} {} ", bool_name(*volatile))
                .expect("string writes cannot fail");
            push_operand(out, address);
        }
        InstructionKind::Store {
            address,
            value,
            alignment,
            volatile,
        } => {
            write!(out, "store {alignment} {} ", bool_name(*volatile))
                .expect("string writes cannot fail");
            push_operand(out, address);
            out.push(' ');
            push_operand(out, value);
        }
        InstructionKind::GetElementPointer { base, indices } => {
            out.push_str("gep ");
            push_operand(out, base);
            out.push(' ');
            push_operands(out, indices);
        }
        InstructionKind::Select {
            condition,
            then_value,
            else_value,
        } => {
            out.push_str("select ");
            push_operand(out, condition);
            out.push(' ');
            push_operand(out, then_value);
            out.push(' ');
            push_operand(out, else_value);
        }
        InstructionKind::Call {
            callee,
            arguments,
            effects,
        } => {
            out.push_str("call ");
            push_callee(out, callee);
            out.push(' ');
            push_operands(out, arguments);
            out.push(' ');
            push_effects(out, *effects);
        }
        InstructionKind::Intrinsic {
            intrinsic,
            arguments,
        } => {
            write!(out, "intrinsic {} ", intrinsic_name(*intrinsic))
                .expect("string writes cannot fail");
            push_operands(out, arguments);
        }
    }
    out.push('\n');
}

fn write_terminator(out: &mut String, terminator: &Terminator) {
    out.push_str("term ");
    match terminator {
        Terminator::Jump(target) => {
            write!(out, "jump {target}").expect("string writes cannot fail");
        }
        Terminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            out.push_str("branch ");
            push_operand(out, condition);
            write!(out, " {then_block} {else_block}").expect("string writes cannot fail");
        }
        Terminator::Switch {
            selector,
            cases,
            default,
        } => {
            out.push_str("switch ");
            push_operand(out, selector);
            out.push_str(" cases [");
            for (index, (value, target)) in cases.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                write!(out, "{value}:{target}").expect("string writes cannot fail");
            }
            write!(out, "] default {default}").expect("string writes cannot fail");
        }
        Terminator::Return(value) => match value {
            Some(value) => {
                out.push_str("return some ");
                push_operand(out, value);
            }
            None => out.push_str("return none"),
        },
        Terminator::Unreachable => out.push_str("unreachable"),
    }
    out.push('\n');
}

fn push_values(out: &mut String, values: &[Value]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        write!(out, "{}:{}", value.id, value.type_id).expect("string writes cannot fail");
    }
    out.push(']');
}

fn push_type_list(out: &mut String, values: &[TypeId]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        write!(out, "{value}").expect("string writes cannot fail");
    }
    out.push(']');
}

fn push_attributes(out: &mut String, attributes: &[FunctionAttribute]) {
    out.push('[');
    for (index, attribute) in attributes.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        out.push_str(attribute_name(*attribute));
    }
    out.push(']');
}

fn push_operands(out: &mut String, operands: &[Operand]) {
    out.push('[');
    for (index, operand) in operands.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        push_operand(out, operand);
    }
    out.push(']');
}

fn push_operand(out: &mut String, operand: &Operand) {
    match operand {
        Operand::Value(value) => write!(out, "value {value}").expect("string writes cannot fail"),
        Operand::Constant(value) => {
            write!(out, "const type {} ", value.type_id).expect("string writes cannot fail");
            push_constant(out, &value.value);
        }
    }
}

fn push_callee(out: &mut String, callee: &Callee) {
    match callee {
        Callee::Direct(id) => write!(out, "direct {id}").expect("string writes cannot fail"),
        Callee::Indirect(operand) => {
            out.push_str("indirect ");
            push_operand(out, operand);
        }
    }
}

fn push_effects(out: &mut String, effects: Effects) {
    write!(
        out,
        "effects {} {} {}",
        memory_effects_name(effects.memory),
        bool_name(effects.may_trap),
        bool_name(effects.observable)
    )
    .expect("string writes cannot fail");
}

fn push_constant(out: &mut String, constant: &Constant) {
    match constant {
        Constant::Integer(value) => {
            write!(out, "integer {value}").expect("string writes cannot fail")
        }
        Constant::Float(value) => {
            out.push_str("float ");
            push_string(out, value);
        }
        Constant::Null => out.push_str("null"),
        Constant::Undefined => out.push_str("undefined"),
        Constant::Bytes(bytes) => {
            out.push_str("bytes ");
            push_string(out, &hex(bytes));
        }
        Constant::RelocatableBytes { bytes, relocations } => {
            out.push_str("relocbytes ");
            push_string(out, &hex(bytes));
            out.push_str(" [");
            for (index, relocation) in relocations.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                write!(
                    out,
                    "{} {} {} {}",
                    relocation.offset,
                    relocation.target,
                    relocation.addend,
                    address_space_name(relocation.address_space)
                )
                .expect("string writes cannot fail");
            }
            out.push(']');
        }
        Constant::Aggregate(values) => {
            out.push_str("aggregate [");
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push('(');
                write!(out, "type {} ", value.type_id).expect("string writes cannot fail");
                push_constant(out, &value.value);
                out.push(')');
            }
            out.push(']');
        }
        Constant::GlobalAddress { global, addend } => {
            write!(out, "globaladdr {global} {addend}").expect("string writes cannot fail")
        }
        Constant::FunctionAddress(function) => {
            write!(out, "functionaddr {function}").expect("string writes cannot fail")
        }
    }
}

fn push_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character.is_control() => {
                write!(out, "\\x{:02x}", character as u32).expect("string writes cannot fail");
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

fn bool_name(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
fn linkage_name(value: Linkage) -> &'static str {
    match value {
        Linkage::Internal => "internal",
        Linkage::External => "external",
    }
}
fn float_kind_name(value: FloatKind) -> &'static str {
    match value {
        FloatKind::Binary32 => "binary32",
        FloatKind::Binary64 => "binary64",
        FloatKind::Extended80 => "extended80",
    }
}
fn address_space_name(value: AddressSpace) -> &'static str {
    match value {
        AddressSpace::Generic => "generic",
        AddressSpace::NearData => "neardata",
        AddressSpace::FarData => "fardata",
        AddressSpace::HugeData => "hugedata",
        AddressSpace::Code => "code",
        AddressSpace::Segment => "segment",
    }
}
fn calling_convention_name(value: CallingConvention) -> &'static str {
    match value {
        CallingConvention::C => "c",
        CallingConvention::FarPascal => "far_pascal",
    }
}
fn attribute_name(value: FunctionAttribute) -> &'static str {
    match value {
        FunctionAttribute::NoReturn => "noreturn",
        FunctionAttribute::NoUnwind => "nounwind",
        FunctionAttribute::ReadOnly => "readonly",
        FunctionAttribute::AlwaysInline => "alwaysinline",
        FunctionAttribute::NeverInline => "neverinline",
    }
}
fn unary_name(value: UnaryOp) -> &'static str {
    match value {
        UnaryOp::Negate => "negate",
        UnaryOp::Not => "not",
        UnaryOp::FloatNegate => "floatnegate",
        UnaryOp::FloatAbsolute => "floatabsolute",
    }
}
fn binary_name(value: BinaryOp) -> &'static str {
    match value {
        BinaryOp::Add => "add",
        BinaryOp::Subtract => "subtract",
        BinaryOp::Multiply => "multiply",
        BinaryOp::SignedDivide => "signeddivide",
        BinaryOp::UnsignedDivide => "unsigneddivide",
        BinaryOp::SignedRemainder => "signedremainder",
        BinaryOp::UnsignedRemainder => "unsignedremainder",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
        BinaryOp::Xor => "xor",
        BinaryOp::ShiftLeft => "shiftleft",
        BinaryOp::LogicalShiftRight => "logicalshiftright",
        BinaryOp::ArithmeticShiftRight => "arithmeticshiftright",
        BinaryOp::FloatAdd => "floatadd",
        BinaryOp::FloatSubtract => "floatsubtract",
        BinaryOp::FloatMultiply => "floatmultiply",
        BinaryOp::FloatDivide => "floatdivide",
    }
}
fn predicate_name(value: ComparePredicate) -> &'static str {
    match value {
        ComparePredicate::Equal => "equal",
        ComparePredicate::NotEqual => "notequal",
        ComparePredicate::SignedLessThan => "signedlessthan",
        ComparePredicate::SignedLessEqual => "signedlessequal",
        ComparePredicate::SignedGreaterThan => "signedgreaterthan",
        ComparePredicate::SignedGreaterEqual => "signedgreaterequal",
        ComparePredicate::UnsignedLessThan => "unsignedlessthan",
        ComparePredicate::UnsignedLessEqual => "unsignedlessequal",
        ComparePredicate::UnsignedGreaterThan => "unsignedgreaterthan",
        ComparePredicate::UnsignedGreaterEqual => "unsignedgreaterequal",
        ComparePredicate::OrderedEqual => "orderedequal",
        ComparePredicate::OrderedNotEqual => "orderednotequal",
        ComparePredicate::OrderedLessThan => "orderedlessthan",
        ComparePredicate::OrderedLessEqual => "orderedlessequal",
        ComparePredicate::OrderedGreaterThan => "orderedgreaterthan",
        ComparePredicate::OrderedGreaterEqual => "orderedgreaterequal",
    }
}
fn cast_name(value: CastOp) -> &'static str {
    match value {
        CastOp::Truncate => "truncate",
        CastOp::SignExtend => "signextend",
        CastOp::ZeroExtend => "zeroextend",
        CastOp::IntegerToFloat => "integertofloat",
        CastOp::FloatToInteger => "floattointeger",
        CastOp::FloatExtend => "floatextend",
        CastOp::FloatTruncate => "floattruncate",
        CastOp::PointerToInteger => "pointertointeger",
        CastOp::IntegerToPointer => "integertopointer",
        CastOp::Bitcast => "bitcast",
    }
}
fn memory_effects_name(value: MemoryEffects) -> &'static str {
    match value {
        MemoryEffects::None => "none",
        MemoryEffects::Read => "read",
        MemoryEffects::Write => "write",
        MemoryEffects::ReadWrite => "readwrite",
        MemoryEffects::Unknown => "unknown",
    }
}
fn intrinsic_name(value: Intrinsic) -> &'static str {
    match value {
        Intrinsic::MemoryCopy => "memorycopy",
        Intrinsic::MemoryMove => "memorymove",
        Intrinsic::MemorySet => "memoryset",
        Intrinsic::SquareRoot => "squareroot",
        Intrinsic::Sine => "sine",
        Intrinsic::Cosine => "cosine",
        Intrinsic::Arctangent => "arctangent",
        Intrinsic::Log2 => "log2",
        Intrinsic::Exp2 => "exp2",
        Intrinsic::Trap => "trap",
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 15) as usize] as char);
    }
    text
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TokenKind {
    Word(String),
    String(String),
    Punctuation(char),
    Eol,
    Eof,
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    line: usize,
    column: usize,
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(input: &str) -> Result<Self, TextError> {
        Ok(Self {
            tokens: tokenize(input)?,
            index: 0,
        })
    }
    fn token(&self) -> &Token {
        &self.tokens[self.index]
    }
    fn error(&self, message: impl Into<String>) -> TextError {
        TextError::new(self.token().line, self.token().column, message)
    }
    fn at_eof(&self) -> bool {
        matches!(&self.token().kind, TokenKind::Eof)
    }
    fn expect_word(&mut self, expected: &str) -> Result<(), TextError> {
        match &self.token().kind {
            TokenKind::Word(word) if word == expected => {
                self.index += 1;
                Ok(())
            }
            _ => Err(self.error(format!("expected `{expected}`"))),
        }
    }
    fn word(&mut self) -> Result<String, TextError> {
        match self.token().kind.clone() {
            TokenKind::Word(word) => {
                self.index += 1;
                Ok(word)
            }
            _ => Err(self.error("expected word")),
        }
    }
    fn peek_word(&self) -> Result<&str, TextError> {
        match &self.token().kind {
            TokenKind::Word(word) => Ok(word),
            _ => Err(self.error("expected record keyword")),
        }
    }
    fn string(&mut self) -> Result<String, TextError> {
        match self.token().kind.clone() {
            TokenKind::String(value) => {
                self.index += 1;
                Ok(value)
            }
            _ => Err(self.error("expected quoted string")),
        }
    }
    fn punctuation(&mut self, expected: char) -> Result<(), TextError> {
        match &self.token().kind {
            TokenKind::Punctuation(actual) if *actual == expected => {
                self.index += 1;
                Ok(())
            }
            _ => Err(self.error(format!("expected `{expected}`"))),
        }
    }
    fn end_line(&mut self) -> Result<(), TextError> {
        match &self.token().kind {
            TokenKind::Eol => {
                self.index += 1;
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            _ => Err(self.error("unexpected content at end of line")),
        }
    }
    fn u16(&mut self) -> Result<u16, TextError> {
        parse_number(&self.word()?, self, "u16")
    }
    fn u32(&mut self) -> Result<u32, TextError> {
        parse_number(&self.word()?, self, "u32")
    }
    fn u64(&mut self) -> Result<u64, TextError> {
        parse_number(&self.word()?, self, "u64")
    }
    fn i64(&mut self) -> Result<i64, TextError> {
        parse_number(&self.word()?, self, "i64")
    }
    fn i128(&mut self) -> Result<i128, TextError> {
        parse_number(&self.word()?, self, "i128")
    }
    fn bool(&mut self) -> Result<bool, TextError> {
        match self.word()?.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(self.error("expected true or false")),
        }
    }
}

fn parse_number<T: std::str::FromStr>(
    word: &str,
    parser: &Parser,
    name: &str,
) -> Result<T, TextError> {
    word.parse()
        .map_err(|_| parser.error(format!("invalid {name} `{word}`")))
}

fn tokenize(input: &str) -> Result<Vec<Token>, TextError> {
    let mut tokens = Vec::new();
    let mut characters = input.char_indices().peekable();
    let mut line = 1;
    let mut column = 1;
    while let Some((_, character)) = characters.next() {
        match character {
            ' ' | '\t' => column += 1,
            '\n' => {
                tokens.push(Token {
                    kind: TokenKind::Eol,
                    line,
                    column,
                });
                line += 1;
                column = 1;
            }
            '\r' => {
                if matches!(characters.peek(), Some((_, '\n'))) {
                    characters.next();
                }
                tokens.push(Token {
                    kind: TokenKind::Eol,
                    line,
                    column,
                });
                line += 1;
                column = 1;
            }
            '[' | ']' | '(' | ')' | ',' | ':' => {
                tokens.push(Token {
                    kind: TokenKind::Punctuation(character),
                    line,
                    column,
                });
                column += 1;
            }
            '"' => {
                let start = column;
                let mut value = String::new();
                loop {
                    let Some((_, current)) = characters.next() else {
                        return Err(TextError::new(line, start, "unterminated string"));
                    };
                    match current {
                        '"' => {
                            column += 1;
                            break;
                        }
                        '\n' | '\r' => {
                            return Err(TextError::new(line, column, "newline in string"));
                        }
                        '\\' => {
                            let Some((_, escape)) = characters.next() else {
                                return Err(TextError::new(line, column, "unterminated escape"));
                            };
                            match escape {
                                '\\' => value.push('\\'),
                                '"' => value.push('"'),
                                'n' => value.push('\n'),
                                'r' => value.push('\r'),
                                't' => value.push('\t'),
                                'x' => {
                                    let Some((_, first)) = characters.next() else {
                                        return Err(TextError::new(
                                            line,
                                            column,
                                            "incomplete hex escape",
                                        ));
                                    };
                                    let Some((_, second)) = characters.next() else {
                                        return Err(TextError::new(
                                            line,
                                            column,
                                            "incomplete hex escape",
                                        ));
                                    };
                                    let high = hex_digit(first).ok_or_else(|| {
                                        TextError::new(line, column, "invalid hex escape")
                                    })?;
                                    let low = hex_digit(second).ok_or_else(|| {
                                        TextError::new(line, column, "invalid hex escape")
                                    })?;
                                    value.push((high * 16 + low) as char);
                                    column += 2;
                                }
                                _ => {
                                    return Err(TextError::new(
                                        line,
                                        column,
                                        "unknown string escape",
                                    ));
                                }
                            }
                            column += 2;
                        }
                        current if current.is_control() => {
                            return Err(TextError::new(line, column, "control character in string"));
                        }
                        current => {
                            value.push(current);
                            column += 1;
                        }
                    }
                }
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    line,
                    column: start,
                });
            }
            _ => {
                let start = column;
                let mut word = String::new();
                word.push(character);
                column += 1;
                while let Some((_, next)) = characters.peek().copied() {
                    if next.is_whitespace()
                        || matches!(next, '[' | ']' | '(' | ')' | ',' | ':' | '"')
                    {
                        break;
                    }
                    characters.next();
                    word.push(next);
                    column += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Word(word),
                    line,
                    column: start,
                });
            }
        }
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        line,
        column,
    });
    Ok(tokens)
}

fn hex_digit(character: char) -> Option<u8> {
    character
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
}

fn parse_type(parser: &mut Parser) -> Result<Type, TextError> {
    parser.expect_word("type")?;
    let id = TypeId::new(parser.u32()?);
    let kind = match parser.word()?.as_str() {
        "void" => TypeKind::Void,
        "integer" => TypeKind::Integer {
            bits: parser.u16()?,
        },
        "float" => TypeKind::Float(parse_float_kind(parser)?),
        "pointer" => TypeKind::Pointer {
            address_space: parse_address_space(parser)?,
        },
        "array" => TypeKind::Array {
            element: TypeId::new(parser.u32()?),
            length: parser.u64()?,
        },
        "structure" => TypeKind::Structure {
            packed: parser.bool()?,
            fields: parse_type_list(parser)?,
        },
        _ => return Err(parser.error("unknown type kind")),
    };
    parser.end_line()?;
    Ok(Type { id, kind })
}

fn parse_global(parser: &mut Parser) -> Result<Global, TextError> {
    parser.expect_word("global")?;
    let id = GlobalId::new(parser.u32()?);
    let name = parser.string()?;
    let type_id = TypeId::new(parser.u32()?);
    let linkage = parse_linkage(parser)?;
    let constant = parser.bool()?;
    let initializer = match parser.word()?.as_str() {
        "none" => None,
        "some" => Some(parse_constant(parser)?),
        _ => return Err(parser.error("expected none or some")),
    };
    parser.end_line()?;
    Ok(Global {
        id,
        name,
        type_id,
        linkage,
        constant,
        initializer,
    })
}

fn parse_function(parser: &mut Parser) -> Result<Function, TextError> {
    parser.expect_word("function")?;
    let id = FunctionId::new(parser.u32()?);
    let name = parser.string()?;
    parser.expect_word("linkage")?;
    let linkage = parse_linkage(parser)?;
    parser.expect_word("result")?;
    let result = TypeId::new(parser.u32()?);
    parser.expect_word("parameters")?;
    let signature_parameters = parse_type_list(parser)?;
    parser.expect_word("variadic")?;
    let variadic = parser.bool()?;
    parser.expect_word("cc")?;
    let calling_convention = parse_calling_convention(parser)?;
    parser.expect_word("attributes")?;
    let attributes = parse_attributes(parser)?;
    parser.end_line()?;

    let mut parameters = Vec::new();
    let mut blocks = Vec::new();
    let mut parameter_ids = BTreeSet::new();
    let mut block_ids = BTreeSet::new();
    let mut saw_block = false;
    loop {
        match parser.peek_word()? {
            "param" => {
                if saw_block {
                    return Err(parser.error("parameter after first block"));
                }
                parser.expect_word("param")?;
                let value = Value {
                    id: ValueId::new(parser.u32()?),
                    type_id: TypeId::new(parser.u32()?),
                };
                parser.end_line()?;
                if !parameter_ids.insert(value.id.get()) {
                    return Err(parser.error("duplicate parameter value id"));
                }
                parameters.push(value);
            }
            "block" => {
                saw_block = true;
                let block = parse_block(parser)?;
                if !block_ids.insert(block.id.get()) {
                    return Err(parser.error("duplicate block id"));
                }
                blocks.push(block);
            }
            "endfunction" => {
                parser.expect_word("endfunction")?;
                parser.end_line()?;
                break;
            }
            _ => return Err(parser.error("expected param, block, or endfunction")),
        }
    }
    Ok(Function {
        id,
        name,
        signature: Signature {
            result,
            parameters: signature_parameters,
            variadic,
            calling_convention,
        },
        linkage,
        attributes,
        parameters,
        blocks,
    })
}

fn parse_block(parser: &mut Parser) -> Result<Block, TextError> {
    parser.expect_word("block")?;
    let id = BlockId::new(parser.u32()?);
    parser.end_line()?;
    let mut instructions = Vec::new();
    let mut instruction_ids = BTreeSet::new();
    loop {
        match parser.peek_word()? {
            "inst" => {
                let instruction = parse_instruction(parser)?;
                if !instruction_ids.insert(instruction.id.get()) {
                    return Err(parser.error("duplicate instruction id"));
                }
                instructions.push(instruction);
            }
            "term" => {
                let terminator = parse_terminator(parser)?;
                parser.expect_word("endblock")?;
                parser.end_line()?;
                return Ok(Block {
                    id,
                    instructions,
                    terminator,
                });
            }
            "endblock" => return Err(parser.error("block has no terminator")),
            _ => return Err(parser.error("expected inst or term")),
        }
    }
}

fn parse_instruction(parser: &mut Parser) -> Result<Instruction, TextError> {
    parser.expect_word("inst")?;
    let id = InstructionId::new(parser.u32()?);
    parser.expect_word("results")?;
    let results = parse_values(parser)?;
    let kind = match parser.word()?.as_str() {
        "phi" => InstructionKind::Phi {
            incoming: parse_phi_incoming(parser)?,
        },
        "alloca" => InstructionKind::StackAlloc {
            size: parser.u32()?,
            alignment: parser.u32()?,
            address_space: parse_address_space(parser)?,
        },
        "unary" => InstructionKind::Unary {
            op: parse_unary(parser)?,
            operand: parse_operand(parser)?,
        },
        "binary" => InstructionKind::Binary {
            op: parse_binary(parser)?,
            left: parse_operand(parser)?,
            right: parse_operand(parser)?,
        },
        "compare" => InstructionKind::Compare {
            predicate: parse_predicate(parser)?,
            left: parse_operand(parser)?,
            right: parse_operand(parser)?,
        },
        "cast" => {
            let op = parse_cast(parser)?;
            let operand = parse_operand(parser)?;
            let to = TypeId::new(parser.u32()?);
            InstructionKind::Cast { op, operand, to }
        }
        "load" => {
            let alignment = parser.u32()?;
            let volatile = parser.bool()?;
            let address = parse_operand(parser)?;
            InstructionKind::Load {
                address,
                alignment,
                volatile,
            }
        }
        "store" => {
            let alignment = parser.u32()?;
            let volatile = parser.bool()?;
            let address = parse_operand(parser)?;
            let value = parse_operand(parser)?;
            InstructionKind::Store {
                address,
                value,
                alignment,
                volatile,
            }
        }
        "gep" => {
            let base = parse_operand(parser)?;
            let indices = parse_operands(parser)?;
            InstructionKind::GetElementPointer { base, indices }
        }
        "select" => InstructionKind::Select {
            condition: parse_operand(parser)?,
            then_value: parse_operand(parser)?,
            else_value: parse_operand(parser)?,
        },
        "call" => {
            let callee = parse_callee(parser)?;
            let arguments = parse_operands(parser)?;
            let effects = parse_effects(parser)?;
            InstructionKind::Call {
                callee,
                arguments,
                effects,
            }
        }
        "intrinsic" => {
            let intrinsic = parse_intrinsic(parser)?;
            let arguments = parse_operands(parser)?;
            InstructionKind::Intrinsic {
                intrinsic,
                arguments,
            }
        }
        _ => return Err(parser.error("unknown instruction kind")),
    };
    parser.end_line()?;
    Ok(Instruction { id, results, kind })
}

fn parse_terminator(parser: &mut Parser) -> Result<Terminator, TextError> {
    parser.expect_word("term")?;
    let terminator = match parser.word()?.as_str() {
        "jump" => Terminator::Jump(BlockId::new(parser.u32()?)),
        "branch" => Terminator::Branch {
            condition: parse_operand(parser)?,
            then_block: BlockId::new(parser.u32()?),
            else_block: BlockId::new(parser.u32()?),
        },
        "switch" => {
            let selector = parse_operand(parser)?;
            parser.expect_word("cases")?;
            parser.punctuation('[')?;
            let mut cases = Vec::new();
            if !matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                loop {
                    let value = parser.i128()?;
                    parser.punctuation(':')?;
                    let target = BlockId::new(parser.u32()?);
                    cases.push((value, target));
                    if matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                        break;
                    }
                    parser.punctuation(',')?;
                }
            }
            parser.punctuation(']')?;
            parser.expect_word("default")?;
            let default = BlockId::new(parser.u32()?);
            Terminator::Switch {
                selector,
                cases,
                default,
            }
        }
        "return" => match parser.word()?.as_str() {
            "none" => Terminator::Return(None),
            "some" => Terminator::Return(Some(parse_operand(parser)?)),
            _ => return Err(parser.error("expected none or some")),
        },
        "unreachable" => Terminator::Unreachable,
        _ => return Err(parser.error("unknown terminator")),
    };
    parser.end_line()?;
    Ok(terminator)
}

fn parse_values(parser: &mut Parser) -> Result<Vec<Value>, TextError> {
    parser.punctuation('[')?;
    let mut values = Vec::new();
    if !matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
        loop {
            let id = ValueId::new(parser.u32()?);
            parser.punctuation(':')?;
            let type_id = TypeId::new(parser.u32()?);
            values.push(Value { id, type_id });
            if matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                break;
            }
            parser.punctuation(',')?;
        }
    }
    parser.punctuation(']')?;
    Ok(values)
}

fn parse_type_list(parser: &mut Parser) -> Result<Vec<TypeId>, TextError> {
    parse_list(parser, |parser| Ok(TypeId::new(parser.u32()?)))
}
fn parse_operands(parser: &mut Parser) -> Result<Vec<Operand>, TextError> {
    parse_list(parser, parse_operand)
}
fn parse_list<T>(
    parser: &mut Parser,
    mut item: impl FnMut(&mut Parser) -> Result<T, TextError>,
) -> Result<Vec<T>, TextError> {
    parser.punctuation('[')?;
    let mut values = Vec::new();
    if !matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
        loop {
            values.push(item(parser)?);
            if matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                break;
            }
            parser.punctuation(',')?;
        }
    }
    parser.punctuation(']')?;
    Ok(values)
}

fn parse_phi_incoming(parser: &mut Parser) -> Result<Vec<PhiIncoming>, TextError> {
    parser.punctuation('[')?;
    let mut values = Vec::new();
    if !matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
        loop {
            let predecessor = BlockId::new(parser.u32()?);
            parser.punctuation(':')?;
            let value = parse_operand(parser)?;
            values.push(PhiIncoming { predecessor, value });
            if matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                break;
            }
            parser.punctuation(',')?;
        }
    }
    parser.punctuation(']')?;
    Ok(values)
}

fn parse_operand(parser: &mut Parser) -> Result<Operand, TextError> {
    match parser.word()?.as_str() {
        "value" => Ok(Operand::Value(ValueId::new(parser.u32()?))),
        "const" => {
            parser.expect_word("type")?;
            let type_id = TypeId::new(parser.u32()?);
            Ok(Operand::Constant(TypedConstant {
                type_id,
                value: parse_constant(parser)?,
            }))
        }
        _ => Err(parser.error("expected value or const operand")),
    }
}

fn parse_constant(parser: &mut Parser) -> Result<Constant, TextError> {
    match parser.word()?.as_str() {
        "integer" => Ok(Constant::Integer(parser.i128()?)),
        "float" => Ok(Constant::Float(parser.string()?)),
        "null" => Ok(Constant::Null),
        "undefined" => Ok(Constant::Undefined),
        "bytes" => parse_hex(&parser.string()?)
            .map(Constant::Bytes)
            .map_err(|message| parser.error(message)),
        "relocbytes" => {
            let bytes = parse_hex(&parser.string()?).map_err(|message| parser.error(message))?;
            parser.punctuation('[')?;
            let mut relocations = Vec::new();
            if !matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                loop {
                    relocations.push(GlobalRelocation {
                        offset: parser.u64()?,
                        target: GlobalId::new(parser.u32()?),
                        addend: parser.i64()?,
                        address_space: parse_address_space(parser)?,
                    });
                    if matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                        break;
                    }
                    parser.punctuation(',')?;
                }
            }
            parser.punctuation(']')?;
            Ok(Constant::RelocatableBytes { bytes, relocations })
        }
        "aggregate" => {
            parser.punctuation('[')?;
            let mut values = Vec::new();
            if !matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                loop {
                    parser.punctuation('(')?;
                    parser.expect_word("type")?;
                    let type_id = TypeId::new(parser.u32()?);
                    let value = parse_constant(parser)?;
                    parser.punctuation(')')?;
                    values.push(TypedConstant { type_id, value });
                    if matches!(&parser.token().kind, TokenKind::Punctuation(']')) {
                        break;
                    }
                    parser.punctuation(',')?;
                }
            }
            parser.punctuation(']')?;
            Ok(Constant::Aggregate(values))
        }
        "globaladdr" => Ok(Constant::GlobalAddress {
            global: GlobalId::new(parser.u32()?),
            addend: parser.i64()?,
        }),
        "functionaddr" => Ok(Constant::FunctionAddress(FunctionId::new(parser.u32()?))),
        _ => Err(parser.error("unknown constant")),
    }
}

fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    if text.len() % 2 != 0 {
        return Err("hex bytes must have an even number of digits".to_owned());
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    let raw = text.as_bytes();
    for index in (0..raw.len()).step_by(2) {
        let high = hex_digit(raw[index] as char).ok_or_else(|| "invalid hex byte".to_owned())?;
        let low = hex_digit(raw[index + 1] as char).ok_or_else(|| "invalid hex byte".to_owned())?;
        bytes.push(high * 16 + low);
    }
    Ok(bytes)
}

fn parse_callee(parser: &mut Parser) -> Result<Callee, TextError> {
    match parser.word()?.as_str() {
        "direct" => Ok(Callee::Direct(FunctionId::new(parser.u32()?))),
        "indirect" => Ok(Callee::Indirect(parse_operand(parser)?)),
        _ => Err(parser.error("expected direct or indirect callee")),
    }
}
fn parse_effects(parser: &mut Parser) -> Result<Effects, TextError> {
    parser.expect_word("effects")?;
    Ok(Effects {
        memory: parse_memory_effects(parser)?,
        may_trap: parser.bool()?,
        observable: parser.bool()?,
    })
}

fn parse_float_kind(parser: &mut Parser) -> Result<FloatKind, TextError> {
    match parser.word()?.as_str() {
        "binary32" => Ok(FloatKind::Binary32),
        "binary64" => Ok(FloatKind::Binary64),
        "extended80" => Ok(FloatKind::Extended80),
        _ => Err(parser.error("unknown float kind")),
    }
}
fn parse_address_space(parser: &mut Parser) -> Result<AddressSpace, TextError> {
    match parser.word()?.as_str() {
        "generic" => Ok(AddressSpace::Generic),
        "neardata" => Ok(AddressSpace::NearData),
        "fardata" => Ok(AddressSpace::FarData),
        "hugedata" => Ok(AddressSpace::HugeData),
        "code" => Ok(AddressSpace::Code),
        "segment" => Ok(AddressSpace::Segment),
        _ => Err(parser.error("unknown address space")),
    }
}
fn parse_linkage(parser: &mut Parser) -> Result<Linkage, TextError> {
    match parser.word()?.as_str() {
        "internal" => Ok(Linkage::Internal),
        "external" => Ok(Linkage::External),
        _ => Err(parser.error("unknown linkage")),
    }
}
fn parse_calling_convention(parser: &mut Parser) -> Result<CallingConvention, TextError> {
    match parser.word()?.as_str() {
        "c" => Ok(CallingConvention::C),
        "far_pascal" => Ok(CallingConvention::FarPascal),
        _ => Err(parser.error("unknown calling convention")),
    }
}
fn parse_attributes(parser: &mut Parser) -> Result<Vec<FunctionAttribute>, TextError> {
    parse_list(parser, |parser| match parser.word()?.as_str() {
        "noreturn" => Ok(FunctionAttribute::NoReturn),
        "nounwind" => Ok(FunctionAttribute::NoUnwind),
        "readonly" => Ok(FunctionAttribute::ReadOnly),
        "alwaysinline" => Ok(FunctionAttribute::AlwaysInline),
        "neverinline" => Ok(FunctionAttribute::NeverInline),
        _ => Err(parser.error("unknown function attribute")),
    })
}
fn parse_unary(parser: &mut Parser) -> Result<UnaryOp, TextError> {
    match parser.word()?.as_str() {
        "negate" => Ok(UnaryOp::Negate),
        "not" => Ok(UnaryOp::Not),
        "floatnegate" => Ok(UnaryOp::FloatNegate),
        "floatabsolute" => Ok(UnaryOp::FloatAbsolute),
        _ => Err(parser.error("unknown unary operation")),
    }
}
fn parse_binary(parser: &mut Parser) -> Result<BinaryOp, TextError> {
    match parser.word()?.as_str() {
        "add" => Ok(BinaryOp::Add),
        "subtract" => Ok(BinaryOp::Subtract),
        "multiply" => Ok(BinaryOp::Multiply),
        "signeddivide" => Ok(BinaryOp::SignedDivide),
        "unsigneddivide" => Ok(BinaryOp::UnsignedDivide),
        "signedremainder" => Ok(BinaryOp::SignedRemainder),
        "unsignedremainder" => Ok(BinaryOp::UnsignedRemainder),
        "and" => Ok(BinaryOp::And),
        "or" => Ok(BinaryOp::Or),
        "xor" => Ok(BinaryOp::Xor),
        "shiftleft" => Ok(BinaryOp::ShiftLeft),
        "logicalshiftright" => Ok(BinaryOp::LogicalShiftRight),
        "arithmeticshiftright" => Ok(BinaryOp::ArithmeticShiftRight),
        "floatadd" => Ok(BinaryOp::FloatAdd),
        "floatsubtract" => Ok(BinaryOp::FloatSubtract),
        "floatmultiply" => Ok(BinaryOp::FloatMultiply),
        "floatdivide" => Ok(BinaryOp::FloatDivide),
        _ => Err(parser.error("unknown binary operation")),
    }
}
fn parse_predicate(parser: &mut Parser) -> Result<ComparePredicate, TextError> {
    match parser.word()?.as_str() {
        "equal" => Ok(ComparePredicate::Equal),
        "notequal" => Ok(ComparePredicate::NotEqual),
        "signedlessthan" => Ok(ComparePredicate::SignedLessThan),
        "signedlessequal" => Ok(ComparePredicate::SignedLessEqual),
        "signedgreaterthan" => Ok(ComparePredicate::SignedGreaterThan),
        "signedgreaterequal" => Ok(ComparePredicate::SignedGreaterEqual),
        "unsignedlessthan" => Ok(ComparePredicate::UnsignedLessThan),
        "unsignedlessequal" => Ok(ComparePredicate::UnsignedLessEqual),
        "unsignedgreaterthan" => Ok(ComparePredicate::UnsignedGreaterThan),
        "unsignedgreaterequal" => Ok(ComparePredicate::UnsignedGreaterEqual),
        "orderedequal" => Ok(ComparePredicate::OrderedEqual),
        "orderednotequal" => Ok(ComparePredicate::OrderedNotEqual),
        "orderedlessthan" => Ok(ComparePredicate::OrderedLessThan),
        "orderedlessequal" => Ok(ComparePredicate::OrderedLessEqual),
        "orderedgreaterthan" => Ok(ComparePredicate::OrderedGreaterThan),
        "orderedgreaterequal" => Ok(ComparePredicate::OrderedGreaterEqual),
        _ => Err(parser.error("unknown comparison predicate")),
    }
}
fn parse_cast(parser: &mut Parser) -> Result<CastOp, TextError> {
    match parser.word()?.as_str() {
        "truncate" => Ok(CastOp::Truncate),
        "signextend" => Ok(CastOp::SignExtend),
        "zeroextend" => Ok(CastOp::ZeroExtend),
        "integertofloat" => Ok(CastOp::IntegerToFloat),
        "floattointeger" => Ok(CastOp::FloatToInteger),
        "floatextend" => Ok(CastOp::FloatExtend),
        "floattruncate" => Ok(CastOp::FloatTruncate),
        "pointertointeger" => Ok(CastOp::PointerToInteger),
        "integertopointer" => Ok(CastOp::IntegerToPointer),
        "bitcast" => Ok(CastOp::Bitcast),
        _ => Err(parser.error("unknown cast operation")),
    }
}
fn parse_memory_effects(parser: &mut Parser) -> Result<MemoryEffects, TextError> {
    match parser.word()?.as_str() {
        "none" => Ok(MemoryEffects::None),
        "read" => Ok(MemoryEffects::Read),
        "write" => Ok(MemoryEffects::Write),
        "readwrite" => Ok(MemoryEffects::ReadWrite),
        "unknown" => Ok(MemoryEffects::Unknown),
        _ => Err(parser.error("unknown memory effects")),
    }
}
fn parse_intrinsic(parser: &mut Parser) -> Result<Intrinsic, TextError> {
    match parser.word()?.as_str() {
        "memorycopy" => Ok(Intrinsic::MemoryCopy),
        "memorymove" => Ok(Intrinsic::MemoryMove),
        "memoryset" => Ok(Intrinsic::MemorySet),
        "squareroot" => Ok(Intrinsic::SquareRoot),
        "sine" => Ok(Intrinsic::Sine),
        "cosine" => Ok(Intrinsic::Cosine),
        "arctangent" => Ok(Intrinsic::Arctangent),
        "log2" => Ok(Intrinsic::Log2),
        "exp2" => Ok(Intrinsic::Exp2),
        "trap" => Ok(Intrinsic::Trap),
        _ => Err(parser.error("unknown intrinsic")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operand(value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id: TypeId::new(1),
            value: Constant::Integer(value),
        })
    }

    #[test]
    fn comprehensive_round_trip() {
        let module = Module {
            name: "quoted \\\" name\n".to_owned(),
            types: vec![
                Type {
                    id: TypeId::new(0),
                    kind: TypeKind::Void,
                },
                Type {
                    id: TypeId::new(1),
                    kind: TypeKind::Integer { bits: 128 },
                },
                Type {
                    id: TypeId::new(2),
                    kind: TypeKind::Float(FloatKind::Extended80),
                },
                Type {
                    id: TypeId::new(3),
                    kind: TypeKind::Pointer {
                        address_space: AddressSpace::HugeData,
                    },
                },
                Type {
                    id: TypeId::new(4),
                    kind: TypeKind::Array {
                        element: TypeId::new(1),
                        length: 9,
                    },
                },
                Type {
                    id: TypeId::new(5),
                    kind: TypeKind::Structure {
                        fields: vec![TypeId::new(1), TypeId::new(4)],
                        packed: true,
                    },
                },
            ],
            globals: vec![
                Global {
                    id: GlobalId::new(0),
                    name: "g".to_owned(),
                    type_id: TypeId::new(1),
                    linkage: Linkage::Internal,
                    constant: true,
                    initializer: Some(Constant::Aggregate(vec![
                        TypedConstant {
                            type_id: TypeId::new(1),
                            value: Constant::Integer(i128::MIN),
                        },
                        TypedConstant {
                            type_id: TypeId::new(1),
                            value: Constant::Bytes(vec![0, 255]),
                        },
                    ])),
                },
                Global {
                    id: GlobalId::new(1),
                    name: "f".to_owned(),
                    type_id: TypeId::new(2),
                    linkage: Linkage::External,
                    constant: false,
                    initializer: Some(Constant::Float("-0.0e+10".to_owned())),
                },
            ],
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "main".to_owned(),
                linkage: Linkage::External,
                signature: Signature {
                    result: TypeId::new(1),
                    parameters: vec![TypeId::new(1)],
                    variadic: true,
                    calling_convention: CallingConvention::FarPascal,
                },
                attributes: vec![FunctionAttribute::NoReturn, FunctionAttribute::AlwaysInline],
                parameters: vec![Value {
                    id: ValueId::new(0),
                    type_id: TypeId::new(1),
                }],
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions: vec![
                        Instruction {
                            id: InstructionId::new(0),
                            results: vec![Value {
                                id: ValueId::new(1),
                                type_id: TypeId::new(1),
                            }],
                            kind: InstructionKind::Phi {
                                incoming: vec![PhiIncoming {
                                    predecessor: BlockId::new(0),
                                    value: Operand::Value(ValueId::new(0)),
                                }],
                            },
                        },
                        Instruction {
                            id: InstructionId::new(1),
                            results: vec![Value {
                                id: ValueId::new(2),
                                type_id: TypeId::new(1),
                            }],
                            kind: InstructionKind::Call {
                                callee: Callee::Indirect(Operand::Constant(TypedConstant {
                                    type_id: TypeId::new(3),
                                    value: Constant::GlobalAddress {
                                        global: GlobalId::new(0),
                                        addend: -2,
                                    },
                                })),
                                arguments: vec![operand(1)],
                                effects: Effects {
                                    memory: MemoryEffects::Unknown,
                                    may_trap: true,
                                    observable: true,
                                },
                            },
                        },
                        Instruction {
                            id: InstructionId::new(2),
                            results: vec![],
                            kind: InstructionKind::Intrinsic {
                                intrinsic: Intrinsic::MemorySet,
                                arguments: vec![Operand::Constant(TypedConstant {
                                    type_id: TypeId::new(1),
                                    value: Constant::FunctionAddress(FunctionId::new(0)),
                                })],
                            },
                        },
                    ],
                    terminator: Terminator::Switch {
                        selector: Operand::Value(ValueId::new(2)),
                        cases: vec![(i128::MIN, BlockId::new(0))],
                        default: BlockId::new(0),
                    },
                }],
            }],
        };
        let text = write(&module);
        assert_eq!(parse(&text), Ok(module));
    }

    #[test]
    fn stack_allocation_round_trips() {
        let module = Module {
            name: "stack-allocation".into(),
            types: vec![
                Type {
                    id: TypeId::new(0),
                    kind: TypeKind::Void,
                },
                Type {
                    id: TypeId::new(1),
                    kind: TypeKind::Pointer {
                        address_space: AddressSpace::NearData,
                    },
                },
            ],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "main".into(),
                linkage: Linkage::Internal,
                signature: Signature {
                    result: TypeId::new(0),
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::FarPascal,
                },
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: InstructionId::new(0),
                        results: vec![Value {
                            id: ValueId::new(0),
                            type_id: TypeId::new(1),
                        }],
                        kind: InstructionKind::StackAlloc {
                            size: 8,
                            alignment: 4,
                            address_space: AddressSpace::NearData,
                        },
                    }],
                    terminator: Terminator::Return(None),
                }],
            }],
        };

        let text = write(&module);
        assert!(text.contains("alloca 8 4 neardata"));
        assert_eq!(parse(&text), Ok(module));
    }

    #[test]
    fn relocatable_bytes_round_trip_preserves_patch_order() {
        let module = Module {
            name: "relocations".to_owned(),
            types: vec![
                Type {
                    id: TypeId::new(0),
                    kind: TypeKind::Integer { bits: 8 },
                },
                Type {
                    id: TypeId::new(1),
                    kind: TypeKind::Array {
                        element: TypeId::new(0),
                        length: 8,
                    },
                },
            ],
            globals: vec![
                Global {
                    id: GlobalId::new(0),
                    name: "target".to_owned(),
                    type_id: TypeId::new(1),
                    linkage: Linkage::Internal,
                    constant: true,
                    initializer: Some(Constant::Bytes(vec![0; 8])),
                },
                Global {
                    id: GlobalId::new(1),
                    name: "data".to_owned(),
                    type_id: TypeId::new(1),
                    linkage: Linkage::Internal,
                    constant: true,
                    initializer: Some(Constant::RelocatableBytes {
                        bytes: vec![0; 8],
                        relocations: vec![
                            GlobalRelocation {
                                offset: 4,
                                target: GlobalId::new(0),
                                addend: -4,
                                address_space: AddressSpace::FarData,
                            },
                            GlobalRelocation {
                                offset: 0,
                                target: GlobalId::new(1),
                                addend: 7,
                                address_space: AddressSpace::Segment,
                            },
                        ],
                    }),
                },
            ],
            functions: Vec::new(),
        };

        let text = write(&module);

        assert!(text.contains("relocbytes \"0000000000000000\" [4 0 -4 fardata,0 1 7 segment]"));
        assert_eq!(parse(&text), Ok(module));
    }

    #[test]
    fn writes_only_the_language_neutral_far_pascal_abi_name() {
        let module = Module {
            name: "abi".into(),
            types: vec![Type {
                id: TypeId::new(0),
                kind: TypeKind::Void,
            }],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "callee".into(),
                signature: Signature {
                    result: TypeId::new(0),
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::FarPascal,
                },
                linkage: Linkage::External,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: Vec::new(),
            }],
        };

        let text = write(&module);

        assert!(text.contains(" cc far_pascal "));
        assert!(!text.contains(" cc basic "));
        assert!(!text.contains(" cc runtime "));
        assert_eq!(parse(&text), Ok(module));
    }

    #[test]
    fn rejects_unknown_and_malformed_input() {
        for obsolete in ["basic", "runtime"] {
            let source = format!(
                "qir 2\nmodule \"m\"\ntype 0 void\nfunction 0 \"f\" linkage external result 0 parameters [] variadic false cc {obsolete} attributes []\nendfunction\nend\n"
            );
            let error = parse(&source).expect_err("source-language ABI labels must not enter IR");
            assert!(error.message.contains("unknown calling convention"));
        }
        assert!(parse("qir 3\nmodule \"m\"\nend\n").is_err());
        assert!(parse("qir 1\nmodule \"m\"\nend\n").is_err());
        assert!(parse("qir 2\nmodule \"m\"\ntype 0 nope\nend\n").is_err());
        assert!(
            parse("qir 2\nmodule \"m\"\nglobal 0 \"g\" 0 internal false some bytes \"f\"\nend\n")
                .is_err()
        );
        assert!(parse("qir 2\nmodule \"m\"\nglobal 0 \"g\" 9 external false none\nend\n").is_err());
    }
}
