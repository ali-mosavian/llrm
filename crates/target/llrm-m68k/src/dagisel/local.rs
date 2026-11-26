//! The instructions this target has above the pin's table, by code and name:
//! the writebacks of [`super::indexed`], the short forms of
//! [`super::short`], the divide-remainders of [`super::divrem`] and the
//! `pea` push of [`super::pea`], which only [`crate::callpush`] makes, and the
//! `cmpa.l` compares of [`super::cmpa`].

use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::instr::InstrDesc;
use llrm_dagisel::matcher::Vt;

use super::asm::{AsmOperand, asm};
use super::enc::{Piece, encoding};
use super::{adda, bfextu, bfffo, cmpa, dbf, divrem, indexed, pea, scaled, short, widemul, wordjump};

/// An instruction's `asm` row.
pub type Text = (&'static str, &'static str, &'static [AsmOperand]);

/// A name's answer from the tables, kept. A lookup that is not in a table
/// misses it before the next is asked, and the late passes ask again for every
/// instruction they read (`code_of`, `text` and `bits`: 6 % of lexer.c's
/// compile). The pin indexes its tables by opcode.
type Kept<T> = std::thread::LocalKey<std::cell::RefCell<llrm_support::hash::HashMap<Box<str>, Option<T>>>>;

thread_local! {
    static CODES: std::cell::RefCell<llrm_support::hash::HashMap<Box<str>, Option<u16>>> = Default::default();
    static TEXTS: std::cell::RefCell<llrm_support::hash::HashMap<Box<str>, Option<&'static Text>>> = Default::default();
    static BITS: std::cell::RefCell<llrm_support::hash::HashMap<Box<str>, Option<(u16, &'static [Piece])>>> =
        Default::default();
    static SEARCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn cached<T: Copy>(
    kept: &'static Kept<T>,
    name: &str,
    search: impl FnOnce(&str) -> Option<T>,
) -> Option<T> {
    if let Some(found) = kept.with_borrow(|map| map.get(name).copied()) {
        return found;
    }
    SEARCHES.set(SEARCHES.get() + 1);
    let found = search(name);
    kept.with_borrow_mut(|map| map.insert(name.into(), found));
    found
}

/// How many names this thread has looked up in the tables.
pub fn table_searches() -> usize {
    SEARCHES.get()
}

/// The machine code of the instruction named `name`, the pin's or a local one.
pub fn code_of(name: &str) -> Option<u16> {
    cached(&CODES, name, search_code)
}

fn search_code(name: &str) -> Option<u16> {
    short::code_of(name)
        .or_else(|| scaled::code_of(name))
        .or_else(|| indexed::code_of(name))
        .or_else(|| divrem::code_of(name))
        .or_else(|| widemul::code_of(name))
        .or_else(|| cmpa::code_of(name))
        .or_else(|| adda::code_of(name))
        .or_else(|| bfffo::code_of(name))
        .or_else(|| bfextu::code_of(name))
        .or_else(|| wordjump::code_of(name))
        .or_else(|| dbf::code_of(name))
}

/// The name of machine code `code`.
pub fn name_of(code: u32) -> Option<&'static str> {
    short::name_of(code)
        .or_else(|| scaled::name_of(code))
        .or_else(|| indexed::name_of(code))
        .or_else(|| divrem::name_of(code))
        .or_else(|| widemul::name_of(code))
        .or_else(|| cmpa::name_of(code))
        .or_else(|| adda::name_of(code))
        .or_else(|| bfffo::name_of(code))
        .or_else(|| bfextu::name_of(code))
        .or_else(|| wordjump::name_of(code))
        .or_else(|| dbf::name_of(code))
}

/// The description of machine code `code`.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    short::desc(code)
        .or_else(|| scaled::desc(code))
        .or_else(|| indexed::desc(code))
        .or_else(|| divrem::desc(code))
        .or_else(|| widemul::desc(code))
        .or_else(|| cmpa::desc(code))
        .or_else(|| adda::desc(code))
        .or_else(|| bfffo::desc(code))
        .or_else(|| bfextu::desc(code))
        .or_else(|| wordjump::desc(code))
        .or_else(|| dbf::desc(code))
}

/// The pin's `asm` row of `name`, or the local instruction's.
pub fn text(name: &str) -> Option<&'static Text> {
    TEXT_LOOKUPS.with(|count| count.set(count.get() + 1));
    cached(&TEXTS, name, search_text)
}

fn search_text(name: &str) -> Option<&'static Text> {
    asm(name)
        .or_else(|| short::short_asm(name))
        .or_else(|| scaled::scaled_asm(name))
        .or_else(|| indexed::source_asm(name))
        .or_else(|| divrem::divrem_asm(name))
        .or_else(|| widemul::widemul_asm(name))
        .or_else(|| pea::pea_asm(name))
        .or_else(|| cmpa::cmpa_asm(name))
        .or_else(|| adda::adda_asm(name))
        .or_else(|| bfffo::bfffo_asm(name))
        .or_else(|| bfextu::bfextu_asm(name))
        .or_else(|| wordjump::wordjump_asm(name))
        .or_else(|| dbf::dbf_asm(name))
}

thread_local! {
    static TEXT_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times [`text`] ran on this thread: a lookup misses every local
/// table in turn, so a caller that repeats it is paying for it
/// (`encode` took 57% of matmul -O2's compile instructions).
pub fn text_lookups() -> usize {
    TEXT_LOOKUPS.with(std::cell::Cell::get)
}

/// The pin's encoding of `name`, or the local instruction's.
pub fn bits(name: &str) -> Option<(u16, &'static [Piece])> {
    cached(&BITS, name, search_bits)
}

fn search_bits(name: &str) -> Option<(u16, &'static [Piece])> {
    encoding(name)
        .map(|&(_, width, pieces)| (width, pieces))
        .or_else(|| short::short_encoding(name))
        .or_else(|| scaled::scaled_encoding(name))
        .or_else(|| indexed::source_encoding(name))
        .or_else(|| divrem::divrem_encoding(name))
        .or_else(|| widemul::widemul_encoding(name))
        .or_else(|| pea::pea_encoding(name))
        .or_else(|| cmpa::cmpa_encoding(name))
        .or_else(|| adda::adda_encoding(name))
        .or_else(|| bfffo::bfffo_encoding(name))
        .or_else(|| bfextu::bfextu_encoding(name))
        .or_else(|| wordjump::wordjump_encoding(name))
        .or_else(|| dbf::dbf_encoding(name))
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
    fn a_name_is_searched_for_once() {
        let ask = || (code_of("CMPA32p"), text("MOVE32rr").map(|row| row.0), bits("MOVE32rr").map(|row| row.0));
        let first = ask();
        let searched = table_searches();
        assert_eq!(ask(), first);
        assert_eq!(table_searches(), searched, "a name already found was searched for again");
    }

    fn instrinfo_desc(name: &str) -> &'static InstrDesc {
        super::super::generated::instrinfo::desc(super::super::opcodes::code_of(name).unwrap()).unwrap()
    }
}

/// `MorphNodeTo` of `node`, and `ReplaceUses` when an equal node existed: a node whose machine form is already in the
/// DAG is replaced by it (what the pin's matcher does after `MorphNodeTo`). A selector that leaves `node` where it is
/// leaves its users reading an unselected node ("operand AnyExtend before its definition").
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
