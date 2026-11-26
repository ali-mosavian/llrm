//! The instructions this target has above the pin's table, by code and name:
//! the writebacks of [`super::indexed`], the short forms of
//! [`super::short`], the divide-remainders of [`super::divrem`] and the
//! `pea` push of [`super::pea`], which only [`crate::callpush`] makes, and the
//! `cmpa.l` compares of [`super::cmpa`].
//!
//! # How it works
//! Each of those modules states one [`Table`]. What a name answers, the pin's
//! or a local one, is read from the tables once, into one [`Row`].

use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::instr::InstrDesc;
use llrm_dagisel::matcher::Vt;
use llrm_support::hash::HashMap;

use super::asm::{AsmOperand, asm};
use super::enc::{Piece, encoding};
use super::{adda, bfextu, bfffo, cmpa, dbf, divrem, indexed, pea, scaled, short, widemul, wordjump};

/// An instruction's `asm` row.
pub type Text = (&'static str, &'static str, &'static [AsmOperand]);

/// What one module adds to the pin's tables.
pub(crate) struct Table {
    pub code_of: fn(&str) -> Option<u16>,
    pub name_of: fn(u32) -> Option<&'static str>,
    pub desc: fn(u16) -> Option<&'static InstrDesc>,
    pub text: fn(&str) -> Option<&'static Text>,
    pub bits: fn(&str) -> Option<(u16, &'static [Piece])>,
}

/// The first table that answers wins.
const TABLES: [Table; 12] = [
    short::TABLE,
    scaled::TABLE,
    indexed::TABLE,
    divrem::TABLE,
    widemul::TABLE,
    pea::TABLE,
    cmpa::TABLE,
    adda::TABLE,
    bfffo::TABLE,
    bfextu::TABLE,
    wordjump::TABLE,
    dbf::TABLE,
];

/// What a name answers.
#[derive(Clone, Copy)]
struct Row {
    code: Option<u16>,
    text: Option<&'static Text>,
    bits: Option<(u16, &'static [Piece])>,
    memory: Option<(usize, usize)>,
}

thread_local! {
    /// The late passes ask again for every instruction they read (6 % of
    /// lexer.c's compile when each question walked the tables); the pin
    /// indexes its tables by opcode. Filled as names are asked: reading every
    /// name up front cost a small file's compile 4 %.
    static INDEX: std::cell::RefCell<HashMap<Box<str>, Row>> = Default::default();
}

#[cfg(test)]
thread_local! {
    static WALKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn row(name: &str) -> Row {
    if let Some(row) = INDEX.with_borrow(|index| index.get(name).copied()) {
        return row;
    }
    let row = read(name);
    INDEX.with_borrow_mut(|index| index.insert(name.into(), row));
    row
}

/// `name`'s row, from a walk of the tables.
fn read(name: &str) -> Row {
    #[cfg(test)]
    WALKS.set(WALKS.get() + 1);
    let text = asm(name).or_else(|| TABLES.iter().find_map(|table| (table.text)(name)));
    Row {
        code: TABLES.iter().find_map(|table| (table.code_of)(name)),
        text,
        bits: encoding(name)
            .map(|&(_, width, pieces)| (width, pieces))
            .or_else(|| TABLES.iter().find_map(|table| (table.bits)(name))),
        memory: memory_operands(name, text),
    }
}

/// The positions of the base and the index in an operand list of row `name`,
/// when it has exactly one indexed operand.
fn memory_operands(
    name: &str,
    text: Option<&Text>,
) -> Option<(usize, usize)> {
    // an extending load has no printed operands: `dst, disp, base, index`
    if scaled::base_of(name).is_none() && crate::expand::extension(name).is_some_and(|(_, _, mode, _)| mode == 'f') {
        return Some((2, 3));
    }
    let &(_, _, operands) = text?;
    let mut found = operands.iter().filter(|one| one.method.contains("ARII"));
    let first = found.next()?;
    found.next().is_none().then(|| (usize::from(first.at) + 1, usize::from(first.at) + 2))
}

/// The machine code of the instruction named `name`, the pin's or a local one.
pub fn code_of(name: &str) -> Option<u16> {
    row(name).code
}

/// Where `name`'s indexed operand has its base and its index.
pub(crate) fn memory_at(name: &str) -> Option<(usize, usize)> {
    row(name).memory
}

/// The name of machine code `code`.
pub fn name_of(code: u32) -> Option<&'static str> {
    TABLES.iter().find_map(|table| (table.name_of)(code))
}

/// The description of machine code `code`.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    TABLES.iter().find_map(|table| (table.desc)(code))
}

/// The pin's `asm` row of `name`, or the local instruction's.
pub fn text(name: &str) -> Option<&'static Text> {
    TEXT_LOOKUPS.with(|count| count.set(count.get() + 1));
    row(name).text
}

thread_local! {
    static TEXT_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times [`text`] ran on this thread: a caller that repeats it is
/// paying for it (`encode` took 57% of matmul -O2's compile instructions).
pub fn text_lookups() -> usize {
    TEXT_LOOKUPS.with(std::cell::Cell::get)
}

/// The pin's encoding of `name`, or the local instruction's.
pub fn bits(name: &str) -> Option<(u16, &'static [Piece])> {
    row(name).bits
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A segment that outgrew its range shadowed the next one: `CMPA32p` took
    /// a scaled row's description (a store with side effects), which stopped
    /// `select_of_constants` f281 -O2 sharing its `moveq` (+16 B, +1.6%
    /// clocks).
    #[test]
    fn every_local_code_names_its_own_row() {
        let names = short::names().chain(scaled::names()).chain(["CMPA32r", "CMPA32p", "CMPA32i", "CMPA16i"]);
        for name in names {
            let code = code_of(name).expect("a local row");
            assert_eq!(name_of(u32::from(code)), Some(name), "code {code:#x}");
        }
        let pin = |name| instrinfo_desc(name).flags;
        for (cmpa, base) in [("CMPA32p", "CMP32dp"), ("CMPA32r", "CMP32dr")] {
            let desc = desc(code_of(cmpa).unwrap()).unwrap();
            assert_eq!(desc.flags, pin(base), "{cmpa} describes {base}");
        }
    }

    /// Every lookup of a name walked the tables again: 6 % of lexer.c's
    /// compile.
    #[test]
    fn a_lookup_does_not_walk_the_tables() {
        let ask = || {
            let known = ["CMPA32p", "MOVE32rr"]
                .map(|name| (code_of(name), text(name).map(|row| row.0), bits(name).map(|row| row.0)));
            let rows = (memory_at("MOVE32rr"), scaled::base_of("MOVE32rr"));
            (known, rows, code_of("no such row"))
        };
        let first = ask();
        let searched = WALKS.get();
        assert_eq!(ask(), first);
        assert_eq!(WALKS.get(), searched, "a lookup walked the tables");
    }

    fn instrinfo_desc(name: &str) -> &'static InstrDesc {
        super::super::generated::instrinfo::desc(super::super::opcodes::code_of(name).unwrap()).unwrap()
    }
}

/// `MorphNodeTo` of `node`, and `ReplaceUses` when an equal node existed: a
/// node whose machine form is already in the DAG is replaced by it (what the
/// pin's matcher does after `MorphNodeTo`). A selector that leaves `node` where
/// it is leaves its users reading an unselected node ("operand AnyExtend before
/// its definition").
pub fn morph(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
    opcode: u16,
    vts: &[Vt],
    operands: &[SDValue],
) -> u32 {
    let (morphed, _) = adapter.morph_node_to(node, opcode, vts, operands);
    if morphed != node {
        adapter.replace_node(node, morphed);
    }
    morphed
}
