//! `bfffo %dn{#0:#0}, %dm`: the index of the first set bit of a data register,
//! counted from the most significant bit, 32 for zero.
//!
//! # How it works
//! A copy of the pin's `ADD32dd` row (data registers, the codes defined) with
//! the destination and the second source as its two operands, its own text and
//! encoding: the first word `1110 1101 1100 0 Dn`, the
//! extension word `0 Dm 0 00000 0 00000 00000` (offset 0, width 0 = 32).
//! `CTLZ` and `CTLZ_ZERO_POISON` of an `i32` are legal, and [`select`] morphs
//! them to it, so the count of leading zeros needs no select for zero; `CTTZ`
//! expands over it as `31 - bfffo(x & -x)` (`expandCTTZ`). `BFFFO32dp` reads
//! `(d16,An)` in place of the register; only [`crate::foldarg`] makes it, from
//! a stack argument that nothing else reads, in a function built for size.
//!
//! DEVIATE M68K-BFFFO: the pin's M68k selects no bitfield instruction
//! (`M68kInstrBits.td` has `btst`, `bset`, `bclr` and `bchg` only) and marks
//! `CTLZ` `Expand` (`M68kISelLowering.cpp` line 156,
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelLowering.cpp#L156>).
//! Reason: gcc 15 counts `clz32`, `clz64` and `ctz32` with `bfffo` from the
//! 68020. Owner m68k. Exit: 2026-11-30, or when the pin selects `bfffo`.
//! The codes are N and Z of the field, V and C cleared: no pass may read them
//! as a test of the result.

use std::sync::LazyLock;

use llrm_codegen::backend::selectiondag::SDValue as Value;
use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::instr::InstrDesc;
use llrm_dagisel::interp::Outcome;

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Code of the first row, above the `adda` forms.
pub const FIRST: u16 = 0xF700;

/// A row: its name, the pin's row it takes its description from, and the
/// operand the searched value is: a data register, or `(d16,An)` (the two
/// operands after the result).
const ROWS: [(&str, &str); 2] = [("BFFFO32dd", "ADD32dd"), ("BFFFO32dp", "ADD32dp")];

/// The register row, which [`select`] makes.
pub const NAME: &str = ROWS[0].0;

/// The operands: the index (the result) and the register searched, or the
/// displacement and the base of the memory searched.
pub const DESTINATION: usize = 0;
pub const SOURCE: usize = 1;

pub fn code_of(name: &str) -> Option<u16> {
    ROWS.iter().position(|row| row.0 == name).map(|at| FIRST + at as u16)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    let at = u16::try_from(code).ok()?.checked_sub(FIRST)?;
    ROWS.get(usize::from(at)).map(|row| row.0)
}

/// The row that reads `(d16,An)` in place of the data register of `name`
/// (what `foldarg` makes of a stack argument that only the count reads).
pub fn memory_twin(name: &str) -> Option<&'static str> {
    (name == NAME).then_some(ROWS[1].0)
}

static DESCS: LazyLock<Vec<&'static InstrDesc>> = LazyLock::new(|| {
    ROWS.iter()
        .map(|&(_, from)| {
            let code = MACHINE_OPCODES.iter().find(|(_, one)| *one == from).map(|(code, _)| *code).expect("a pin row");
            let base = instrinfo::desc(code).expect("a described row");
            // the pin's [dst, src (tied to dst), opd...]: the result and the
            // operands searched
            let operands: Vec<_> = std::iter::once(base.operands[0]).chain(base.operands[2..].iter().copied()).collect();
            &*Box::leak(Box::new(InstrDesc {
                num_operands: operands.len() as u8,
                num_defs: 1,
                operands: Box::leak(operands.into_boxed_slice()),
                ..*base
            }))
        })
        .collect()
});

pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    DESCS.get(usize::from(code.checked_sub(FIRST)?)).copied()
}

static ASM: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
    ROWS.iter()
        .map(|&(name, _)| {
            let source = AsmOperand {
                name: "src",
                at: SOURCE as u8,
                method: if name == NAME { "printOperand" } else { "printARID32Mem" },
            };
            let operands: &'static [AsmOperand] =
                Box::leak(Box::new([source, AsmOperand { name: "dst", at: DESTINATION as u8, method: "printOperand" }]));
            (name, "bfffo\t$src{#0:#0}, $dst", operands)
        })
        .collect()
});

pub fn bfffo_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    ASM.iter().find(|row| row.0 == name)
}

const fn field(
    lo: u16,
    width: u8,
    at: u8,
) -> Piece {
    Piece::Operand { lo, width, at, from: 0, enc: Enc::Plain }
}

const fn bits(
    lo: u16,
    width: u8,
    value: u64,
) -> Piece {
    Piece::Const { lo, width, value }
}

/// `1110 1101 11 mode reg` (`0xEDC0` with mode 0, a data register) and the
/// extension word with `Dm` in bits 14-12.
static REGISTER: [Piece; 4] =
    [field(0, 3, SOURCE as u8), bits(3, 3, 0), bits(6, 10, 0x3B7), field(28, 3, DESTINATION as u8)];

/// Mode 5, `(d16,An)`: the base in the first word and the displacement in a
/// third, after the bit-field word.
static MEMORY: [Piece; 5] = [
    field(0, 3, SOURCE as u8 + 1),
    bits(3, 3, 5),
    bits(6, 10, 0x3B7),
    field(28, 3, DESTINATION as u8),
    Piece::Operand { lo: 32, width: 16, at: SOURCE as u8, from: 0, enc: Enc::RelocImm(16) },
];

pub fn bfffo_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    match name {
        _ if name == ROWS[0].0 => Some((32, &REGISTER[..])),
        _ if name == ROWS[1].0 => Some((48, &MEMORY[..])),
        _ => None,
    }
}

/// `CTLZ` and `CTLZ_ZERO_POISON` of an `i32` as `bfffo`.
pub fn select(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    let here = Value { node, res: 0 };
    if !matches!(adapter.dag.opcode(here), ISD::CtLz | ISD::CtLzZeroPoison) || adapter.dag.value_type(here) != EVT::I32
    {
        return None;
    }
    let value = Dag::operand(adapter, node, 0);
    let vt = adapter.value_type(SDValue { node, res: 0 });
    adapter.morph_node_to(node, FIRST, &[vt], &[value]);
    Some(Outcome::Selected)
}
