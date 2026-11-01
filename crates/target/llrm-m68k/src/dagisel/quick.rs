//! `addq` and `subq`: the add or subtract of 1 to 8 in the short instruction
//! word, with no immediate extension.
//!
//! # How it works
//! Each quick instruction is the pin's add or subtract of an immediate with
//! the same operands, flags and classes ([`QUICK`] names the pin's row);
//! only its text and encoding differ, and both are built from that row.
//! [`narrow`] runs after the generated selector (X86's
//! `PostprocessISelDAG` is the same step) and morphs a selected add or
//! subtract whose immediate is in 1 to 8 into the quick instruction.
//!
//! DEVIATE: the pin has no `ADDQ` or `SUBQ`, so it prints `add.l #1, %d0` for
//! the two bytes `addq.l #1, %d0` takes. Reason: best of gcc and clang is the
//! goal, and both use the quick forms. Exit: none.
//!
//! # See also
//! `X86DAGToDAGISel::PostprocessISelDAG`.

use std::collections::HashSet;
use std::sync::LazyLock;

use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::{SDValue as Value, SelectionDAG};
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::instr::InstrDesc;

use super::asm::{AsmOperand, asm};
use super::enc::{Enc, Piece, encoding};
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Codes of the quick instructions, above those of the writebacks.
pub const FIRST: u16 = 0xF100;

/// A quick form: the pin's row it narrows, and what it prints as.
struct Quick {
    name: &'static str,
    base: &'static str,
    mnemonic: &'static str,
    /// Operand count of the effective-address register field, and the mode
    /// bits of the destination.
    mode: u64,
    sub: bool,
    size: u64,
}

/// The pin's rows that add or subtract an immediate: to a data register, to
/// an address register (long only), and to memory at `(An)`, `(d16,An)` and
/// `(d8,An,Xn)`; with their mode bits.
const CLASSES: [(&str, u64); 5] = [("di", 0), ("ai", 1), ("ji", 2), ("pi", 5), ("fi", 6)];
const SIZES: [(u32, u64); 3] = [(8, 0), (16, 1), (32, 2)];

static QUICKS: LazyLock<Vec<Quick>> = LazyLock::new(|| {
    let mut all = Vec::new();
    for (sub, mnemonic) in [(false, "addq"), (true, "subq")] {
        let pin = if sub { "SUB" } else { "ADD" };
        for (bits, size) in SIZES {
            for (class, mode) in CLASSES {
                let base = leak(format!("{pin}{bits}{class}"));
                // the pin has no word or byte add to an address register
                if instrinfo_code(base).is_none() {
                    continue;
                }
                all.push(Quick {
                    name: leak(format!("{}{bits}{class}", mnemonic.to_uppercase())),
                    base,
                    mnemonic,
                    mode,
                    sub,
                    size,
                });
            }
        }
    }
    all
});

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn instrinfo_code(name: &str) -> Option<u16> {
    MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code)
}

/// The names of the quick instructions.
pub fn names() -> impl Iterator<Item = &'static str> {
    QUICKS.iter().map(|one| one.name)
}

/// The machine code of the quick instruction `name`.
pub fn code_of(name: &str) -> Option<u16> {
    QUICKS.iter().position(|one| one.name == name).map(|at| FIRST + at as u16)
}

/// The name of machine code `code`, when it is a quick instruction's.
pub fn name_of(code: u32) -> Option<&'static str> {
    let at = u16::try_from(code).ok()?.checked_sub(FIRST)?;
    QUICKS.get(usize::from(at)).map(|one| one.name)
}

/// The description of a quick instruction: its pin row's.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    let one = QUICKS.get(usize::from(code.checked_sub(FIRST)?))?;
    instrinfo::desc(instrinfo_code(one.base)?)
}

/// The operand holding the immediate, from the pin row's text.
fn immediate_at(base: &str) -> Option<u8> {
    let &(_, _, operands) = asm(base)?;
    operands.iter().find(|one| one.method == "printImmediate").map(|one| one.at)
}

static ASM: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
    let mut rows: Vec<_> = QUICKS
        .iter()
        .map(|one| {
            let &(_, text, operands) = asm(one.base).expect("the pin row prints");
            let (_, rest) = text.split_once('.').expect("a sized mnemonic");
            (one.name, leak(format!("{}.{rest}", one.mnemonic)), operands)
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
});

/// The text of a quick instruction, as the pin's `asm` rows.
pub fn quick_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    ASM.binary_search_by_key(&name, |row| row.0).ok().map(|at| &ASM[at])
}

/// The slot the pin's immediate takes in the instruction: a byte is a word.
fn immediate_bits(size: u64) -> u16 {
    if size == 2 { 32 } else { 16 }
}

static ENCODINGS: LazyLock<Vec<(&'static str, u16, &'static [Piece])>> = LazyLock::new(|| {
    let mut rows: Vec<_> = QUICKS
        .iter()
        .map(|one| {
            let &(_, width, base) = encoding(one.base).expect("the pin row encodes");
            let imm_at = immediate_at(one.base).expect("an immediate");
            let slot = immediate_bits(one.size);
            // `0101 data s ss mode reg`: the register field is the pin's, at
            // bit 0 for memory and at bit 9 for a register destination
            let register = base
                .iter()
                .find_map(|piece| match *piece {
                    Piece::Operand { lo: 0 | 9, width: 3, at, .. } if at != imm_at => Some(at),
                    _ => None,
                })
                .expect("a base register");
            let mut pieces = vec![
                Piece::Operand { lo: 0, width: 3, at: register, from: 0, enc: Enc::Plain },
                Piece::Const { lo: 3, width: 3, value: one.mode },
                Piece::Const { lo: 6, width: 2, value: one.size },
                Piece::Const { lo: 8, width: 1, value: u64::from(one.sub) },
                // 1 to 7 as they are, 8 as 0
                Piece::Operand { lo: 9, width: 3, at: imm_at, from: 0, enc: Enc::Plain },
                Piece::Const { lo: 12, width: 4, value: 5 },
            ];
            // what follows the pin's immediate moves up over it
            for piece in base {
                match *piece {
                    Piece::Operand { lo, width, at, from, enc } if lo >= 16 + slot && at != imm_at => {
                        pieces.push(Piece::Operand { lo: lo - slot, width, at, from, enc })
                    }
                    Piece::Const { lo, width, value } if lo >= 16 + slot => {
                        pieces.push(Piece::Const { lo: lo - slot, width, value })
                    }
                    _ => {}
                }
            }
            (one.name, width - slot, &*Box::leak(pieces.into_boxed_slice()))
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
});

/// The encoding of a quick instruction: `(width in bits, pieces)`.
pub fn quick_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    ENCODINGS.binary_search_by_key(&name, |row| row.0).ok().map(|at| (ENCODINGS[at].1, ENCODINGS[at].2))
}

/// After the selector: narrows each selected add or subtract of an immediate
/// from 1 to 8 to its quick instruction, in place.
pub fn narrow(dag: &mut SelectionDAG) {
    let mut adapter = DagisAdapter::new(dag);
    let mut seen = HashSet::new();
    let mut work = vec![adapter.dag.get_root().node];
    while let Some(node) = work.pop() {
        if seen.insert(node) {
            work.extend(adapter.dag.node(Value { node, res: 0 }).ops.iter().map(|op| op.node));
        }
    }
    let mut nodes: Vec<u32> = seen.into_iter().collect();
    nodes.sort_unstable();
    for node in nodes {
        narrow_node(&mut adapter, node);
    }
}

fn narrow_node(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) {
    let ISD::Machine(code) = adapter.dag.node(Value { node, res: 0 }).opcode else { return };
    let Some(one) = QUICKS.iter().position(|one| instrinfo_code(one.base).is_some_and(|base| u32::from(base) == code))
    else {
        return;
    };
    let Some(base) = instrinfo_code(QUICKS[one].base).and_then(instrinfo::desc) else { return };
    // the machine operand numbers count the results, the node's operands do not
    let Some(imm_at) = immediate_at(QUICKS[one].base).and_then(|at| at.checked_sub(base.num_defs)) else { return };
    let mut operands: Vec<_> = (0..adapter.num_operands(node)).map(|index| adapter.operand(node, index)).collect();
    let Some(&immediate) = operands.get(usize::from(imm_at)) else { return };
    let Some(value) = adapter.constant(immediate) else { return };
    let mut target = one;
    if (-8..=-1).contains(&value) && !flags_read(adapter, node, base) {
        // x + -n is x - n, and the other way round; the carry and extend bits
        // differ, so only where nothing reads the flags
        target = QUICKS.iter().position(|other| other.name == opposite(QUICKS[one].name)).expect("the opposite");
        let vt = adapter.value_type(immediate);
        operands[usize::from(imm_at)] = adapter.target_constant(-value, vt);
    } else if !(1..=8).contains(&value) {
        return;
    }
    let vts: Vec<_> =
        (0..adapter.num_values(node)).map(|res| adapter.value_type(SDValue { node, res: res as u8 })).collect();
    let (morphed, _) = adapter.morph_node_to(node, FIRST + target as u16, &vts, &operands);
    if morphed != node {
        adapter.replace_node(node, morphed);
    }
}

/// `ADDQ32di` for `SUBQ32di` and back.
fn opposite(name: &str) -> String {
    let (head, rest) = name.split_at(3);
    format!("{}{rest}", if head == "ADD" { "SUB" } else { "ADD" })
}

/// Whether anything reads the condition codes `node` defines: the result
/// after its explicit ones.
fn flags_read(
    adapter: &DagisAdapter<'_>,
    node: u32,
    desc: &InstrDesc,
) -> bool {
    !desc.implicit_defs.is_empty() && !adapter.has_n_uses_of_value(SDValue { node, res: desc.num_defs }, 0)
}
