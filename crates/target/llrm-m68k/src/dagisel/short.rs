//! `addq`, `subq` and `tst`: the short forms of an add or subtract of 1 to 8
//! and of a compare with zero, which need no immediate extension word.
//!
//! # How it works
//! Each short instruction is a pin instruction (the add, subtract or compare
//! of an immediate) with the same flags and classes; only its text and
//! encoding differ, and both are built from the pin's row. [`narrow`] runs
//! after the generated selector (X86's `PostprocessISelDAG` is the same step)
//! and morphs a selected instruction whose immediate is in range into its
//! short form: 1 to 8 for `addq` and `subq`, 0 for `tst`. `tst` sets the
//! codes as `cmpi #0` does, so the swap changes no reader.
//!
//! DEVIATE: the pin has no `ADDQ`, `SUBQ` or `TST`, so it prints
//! `add.l #1, %d0` for the two bytes `addq.l #1, %d0` takes, and `cmpi.l #0,
//! %d0` for the two bytes of `tst.l %d0`. Reason: best of gcc and clang is the
//! goal, and both use the short forms. Exit: none.
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

/// Codes of the short instructions, above those of the writebacks.
pub const FIRST: u16 = 0xF100;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Form {
    Add,
    Sub,
    Test,
}

/// A short form: the pin's row it narrows, and what it prints as.
struct Quick {
    name: &'static str,
    base: &'static str,
    mnemonic: &'static str,
    /// The mode bits of the destination (not for `tst`, which keeps the pin's).
    mode: u64,
    form: Form,
    size: u64,
}

/// The pin's rows that add or subtract an immediate: to a data register, to
/// an address register (long only), and to memory at `(An)`, `(d16,An)` and
/// `(d8,An,Xn)`; with their mode bits.
const CLASSES: [(&str, u64); 5] = [("di", 0), ("ai", 1), ("ji", 2), ("pi", 5), ("fi", 6)];
const SIZES: [(u32, u64); 3] = [(8, 0), (16, 1), (32, 2)];

static QUICKS: LazyLock<Vec<Quick>> = LazyLock::new(|| {
    let mut all = Vec::new();
    for (form, mnemonic) in [(Form::Add, "addq"), (Form::Sub, "subq")] {
        let pin = if form == Form::Sub { "SUB" } else { "ADD" };
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
                    form,
                    size,
                });
            }
        }
    }
    // `tst` of a data register and of memory but not of the program counter
    // forms, which the 68000 lacks
    for (bits, size) in SIZES {
        for class in ["di", "ji", "pi", "fi", "bi"] {
            let base = leak(format!("CMP{bits}{class}"));
            all.push(Quick {
                name: leak(format!("TST{bits}{}", &class[..1])),
                base,
                mnemonic: "tst",
                mode: 0,
                form: Form::Test,
                size,
            });
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

/// The names of the short instructions.
pub fn names() -> impl Iterator<Item = &'static str> {
    QUICKS.iter().map(|one| one.name)
}

/// The machine code of the short instruction `name`.
pub fn code_of(name: &str) -> Option<u16> {
    QUICKS.iter().position(|one| one.name == name).map(|at| FIRST + at as u16)
}

/// The name of machine code `code`, when it is a short instruction's.
pub fn name_of(code: u32) -> Option<&'static str> {
    let at = u16::try_from(code).ok()?.checked_sub(FIRST)?;
    QUICKS.get(usize::from(at)).map(|one| one.name)
}

static DESCS: LazyLock<Vec<&'static InstrDesc>> = LazyLock::new(|| {
    QUICKS
        .iter()
        .map(|one| {
            let base = instrinfo::desc(instrinfo_code(one.base).expect("a pin row")).expect("a described row");
            if one.form != Form::Test {
                return base;
            }
            // the same instruction without the immediate operand
            let at = usize::from(immediate_at(one.base).expect("an immediate"));
            let mut operands = base.operands.to_vec();
            operands.remove(at);
            &*Box::leak(Box::new(InstrDesc {
                num_operands: operands.len() as u8,
                operands: Box::leak(operands.into_boxed_slice()),
                ..*base
            }))
        })
        .collect()
});

/// The description of a short instruction: its pin row's, without the
/// immediate of a `tst`.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    DESCS.get(usize::from(code.checked_sub(FIRST)?)).copied()
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
            if one.form != Form::Test {
                return (one.name, leak(format!("{}.{rest}", one.mnemonic)), operands);
            }
            // `cmpi.l $imm, $reg` is `tst.l $reg`, and the operands after the
            // immediate move down one
            let gone = usize::from(immediate_at(one.base).expect("an immediate"));
            let text = leak(format!("tst.{}", rest.replace("$imm, ", "")));
            let kept: Vec<AsmOperand> = operands
                .iter()
                .filter(|one| usize::from(one.at) != gone)
                .map(|one| AsmOperand {
                    name: one.name,
                    at: one.at - u8::from(usize::from(one.at) > gone),
                    method: one.method,
                })
                .collect();
            (one.name, text, &*Box::leak(kept.into_boxed_slice()))
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
});

/// The text of a short instruction, as the pin's `asm` rows.
pub fn short_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
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
            if one.form == Form::Test {
                return (one.name, width - slot, test_pieces(base, imm_at, slot));
            }
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
                Piece::Const { lo: 8, width: 1, value: u64::from(one.form == Form::Sub) },
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

/// `0100 1010 ss mode reg` and the words that follow: the pin's `cmpi` with
/// its opcode byte `0000 1100` replaced, its immediate dropped and what
/// follows it moved up, the operands after the immediate one lower.
fn test_pieces(
    base: &[Piece],
    imm_at: u8,
    slot: u16,
) -> &'static [Piece] {
    let down = |at: u8| if at > imm_at { at - 1 } else { at };
    let pieces: Vec<Piece> = base
        .iter()
        .filter_map(|piece| match *piece {
            Piece::Const { lo: 8, width: 8, .. } => Some(Piece::Const { lo: 8, width: 8, value: 0x4A }),
            Piece::Operand { lo, width, at, from, enc } if lo < 16 => {
                Some(Piece::Operand { lo, width, at: down(at), from, enc })
            }
            Piece::Const { lo, .. } if lo < 16 => Some(*piece),
            Piece::Operand { lo, width, at, from, enc } if lo >= 16 + slot => {
                Some(Piece::Operand { lo: lo - slot, width, at: down(at), from, enc })
            }
            Piece::Const { lo, width, value } if lo >= 16 + slot => Some(Piece::Const { lo: lo - slot, width, value }),
            _ => None,
        })
        .collect();
    Box::leak(pieces.into_boxed_slice())
}

/// The encoding of a short instruction: `(width in bits, pieces)`.
pub fn short_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    ENCODINGS.binary_search_by_key(&name, |row| row.0).ok().map(|at| (ENCODINGS[at].1, ENCODINGS[at].2))
}

/// After the selector: narrows each selected add or subtract of an immediate
/// from 1 to 8 to its short instruction, in place.
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
    if QUICKS[one].form == Form::Test {
        if value != 0 {
            return;
        }
        operands.remove(usize::from(imm_at));
    } else if (-8..=-1).contains(&value) && !flags_read(adapter, node, base) {
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
