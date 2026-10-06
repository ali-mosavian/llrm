//! The instruction description, `x86.instr`, as the rules need it: each
//! mnemonic's LIR operation, operand shapes and fixed registers, and the
//! flags iced-x86 says its forms read and write.

use std::collections::HashMap;

use iced_x86::{Code, Instruction, RflagsBits};
use indexmap::IndexMap;

#[path = "../../../target/llrm-x86-code16/src/instructions/parse.rs"]
#[allow(dead_code)]
mod parse;

/// The six arithmetic flags.
pub const ARITHMETIC: u32 =
    RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF;

pub const FLAGS: [(&str, u32); 7] = [
    ("OF", RflagsBits::OF),
    ("SF", RflagsBits::SF),
    ("ZF", RflagsBits::ZF),
    ("AF", RflagsBits::AF),
    ("CF", RflagsBits::CF),
    ("PF", RflagsBits::PF),
    ("DF", RflagsBits::DF),
];

pub fn flag(name: &str) -> Option<u32> {
    FLAGS.iter().find(|(one, _)| *one == name).map(|(_, bit)| *bit)
}

#[derive(Debug)]
pub struct Shape {
    pub dests: usize,
    pub sources: usize,
    /// As x86.instr writes it, `rm/^0,rmi`.
    pub text: String,
    /// Registers pinned to operands.
    pub fixed: Vec<String>,
}

#[derive(Debug)]
pub struct Mnem {
    /// The `ir::Operation` variant.
    pub op: String,
    pub shapes: Vec<Shape>,
    /// Flags any form reads, and writes or leaves undefined.
    pub reads: u32,
    pub writes: u32,
}

#[derive(Debug)]
pub struct Table {
    pub mnemonics: IndexMap<String, Mnem>,
}

impl Table {
    /// Each operation, with the shapes of every form of it.
    pub fn shapes_of(&self, op: &str) -> impl Iterator<Item = &Shape> {
        self.mnemonics.values().filter(move |one| one.op == op).flat_map(|one| &one.shapes)
    }
}

/// The variant of `ir::Operation` that x86.instr spells `spelled`.
pub fn operation(spelled: &str) -> Option<String> {
    llrm_lir::Operation::named(spelled).map(|op| format!("{op:?}"))
}

pub fn variant(name: &str) -> bool {
    llrm_lir::Operation::ALL.iter().any(|op| format!("{op:?}") == name)
}

/// As `llrm_x86_code16::instructions::flags`: what iced's Code reads, and
/// writes, sets, clears or leaves undefined.
fn flags(code: Code) -> (u32, u32) {
    let mut one = Instruction::default();
    one.set_code(code);
    let written = one.rflags_written() | one.rflags_cleared() | one.rflags_set() | one.rflags_undefined() | one.rflags_modified();
    (one.rflags_read(), written)
}

fn operands(list: &[parse::Operand]) -> String {
    if list.is_empty() {
        return "-".into();
    }
    list.iter().map(|one| one.tied.map_or_else(|| one.kinds.clone(), |dest| format!("^{dest}"))).collect::<Vec<_>>().join(",")
}

pub fn load(source: &str, file: &str) -> Result<Table, String> {
    let forms = parse::parse(source).map_err(|error| format!("{file}: {error}"))?;
    let codes: HashMap<String, Code> = Code::values().map(|code| (format!("{code:?}"), code)).collect();
    let mut mnemonics: IndexMap<String, Mnem> = IndexMap::new();
    for form in forms {
        let op = operation(&form.operation).ok_or_else(|| format!("{file}:{}: no operation {}", form.line, form.operation))?;
        let (reads, writes) = match form.code_name(None) {
            Some(name) => flags(*codes.get(&name).ok_or_else(|| format!("{file}:{}: iced-x86 has no code {name}", form.line))?),
            None => (0, 0),
        };
        let shape = Shape {
            dests: form.dests.len(),
            sources: form.sources.len(),
            text: format!("{}/{}", operands(&form.dests), operands(&form.sources)),
            fixed: form.fixed.iter().map(|(_, _, register)| register.clone()).collect(),
        };
        let entry = mnemonics.entry(form.name.clone()).or_insert_with(|| Mnem { op: op.to_owned(), shapes: Vec::new(), reads: 0, writes: 0 });
        if entry.op != op {
            return Err(format!("{file}:{}: {} is {} here and {} before", form.line, form.name, op, entry.op));
        }
        entry.shapes.push(shape);
        entry.reads |= reads;
        entry.writes |= writes;
    }
    Ok(Table { mnemonics })
}
