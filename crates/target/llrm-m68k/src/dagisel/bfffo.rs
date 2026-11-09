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
//! expands over it as `31 - bfffo(x & -x)` (`expandCTTZ`).
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

/// Code of the instruction, above the `adda` forms.
pub const FIRST: u16 = 0xF700;

pub const NAME: &str = "BFFFO32";

/// The operands: the index (the result) and the register searched.
pub const DESTINATION: usize = 0;
pub const SOURCE: usize = 1;

pub fn code_of(name: &str) -> Option<u16> {
    (name == NAME).then_some(FIRST)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    (code == u32::from(FIRST)).then_some(NAME)
}

static DESC: LazyLock<&'static InstrDesc> = LazyLock::new(|| {
    let code = MACHINE_OPCODES.iter().find(|(_, one)| *one == "ADD32dd").map(|(code, _)| *code).expect("a pin row");
    let base = instrinfo::desc(code).expect("a described row");
    // the pin's [dst, src (tied to dst), opd]: the data register result and the
    // data register searched
    let operands = vec![base.operands[0], base.operands[2]];
    Box::leak(Box::new(InstrDesc {
        num_operands: 2,
        num_defs: 1,
        operands: Box::leak(operands.into_boxed_slice()),
        ..*base
    }))
});

pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    (code == FIRST).then_some(*DESC)
}

static ASM: (&str, &str, &[AsmOperand]) = (
    NAME,
    "bfffo\t$src{#0:#0}, $dst",
    &[
        AsmOperand { name: "src", at: SOURCE as u8, method: "printOperand" },
        AsmOperand { name: "dst", at: DESTINATION as u8, method: "printOperand" },
    ],
);

pub fn bfffo_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    (name == NAME).then_some(&ASM)
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

/// `1110 1101 11 000 Dn` (`0xEDC0`) and the extension word with `Dm` in
/// bits 14-12.
static PIECES: [Piece; 4] =
    [field(0, 3, SOURCE as u8), bits(3, 3, 0), bits(6, 10, 0x3B7), field(28, 3, DESTINATION as u8)];

pub fn bfffo_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    (name == NAME).then_some((32, &PIECES[..]))
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
