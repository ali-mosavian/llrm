//! The instruction description, `x86.instr`, as the rules need it: each
//! mnemonic's LIR operation, operand shapes and fixed registers, and the
//! flags its forms read and write.

use indexmap::IndexMap;
use llrm_target::instr as parse;

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
    /// The target's flags by name, and the bits of those an arithmetic
    /// instruction writes (its dialect's).
    pub flags: Vec<(String, u32)>,
    pub arithmetic: u32,
}

impl Table {
    pub fn flag(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.flags.iter().find(|(one, _)| one == name).map(|(_, bit)| *bit)
    }

    /// Each operation, with the shapes of every form of it.
    pub fn shapes_of(
        &self,
        op: &str,
    ) -> impl Iterator<Item = &Shape> {
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

fn operands(list: &[parse::Operand]) -> String {
    if list.is_empty() {
        return "-".into();
    }
    list.iter()
        .map(|one| one.tied.map_or_else(|| one.kinds.clone(), |dest| format!("^{dest}")))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn load(
    source: &str,
    file: &str,
    dialect: &parse::Dialect,
) -> Result<Table, String> {
    let forms = parse::parse(source, dialect).map_err(|error| format!("{file}: {error}"))?;
    let mut mnemonics: IndexMap<String, Mnem> = IndexMap::new();
    for form in forms {
        let op = operation(&form.operation)
            .ok_or_else(|| format!("{file}:{}: no operation {}", form.line, form.operation))?;
        let (reads, writes) = (form.flags_read, form.flags_written);
        let shape = Shape {
            dests: form.dests.len(),
            sources: form.sources.len(),
            text: format!("{}/{}", operands(&form.dests), operands(&form.sources)),
            fixed: form.fixed.iter().map(|(_, _, register)| register.clone()).collect(),
        };
        let entry = mnemonics
            .entry(form.name.clone())
            .or_insert_with(
                || Mnem {
                    op: op.to_owned(),
                    shapes: Vec::new(),
                    reads: 0,
                    writes: 0,
                },
            );
        if entry.op != op {
            return Err(format!("{file}:{}: {} is {} here and {} before", form.line, form.name, op, entry.op));
        }
        entry.shapes.push(shape);
        entry.reads |= reads;
        entry.writes |= writes;
    }
    Ok(Table { mnemonics, flags: dialect.flags.clone(), arithmetic: dialect.arithmetic_mask() })
}
