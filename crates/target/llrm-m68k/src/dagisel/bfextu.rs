//! `bfextu %dn{#off:#wid}, %dm`: an unsigned bit field of a data register, the
//! `(x >> c) & mask` of a contiguous low mask, in one instruction.
//!
//! # How it works
//! A copy of the pin's `ADD32dd` row with the result, the register and two
//! immediates (offset from the most significant bit, width) as its operands;
//! the first word `1110 1001 11 000 Dn`, the extension word
//! `0 Dm 0 offset(5) 0 width(5)` (a width of 32 is written 0). [`select`]
//! turns `and (srl x, c), 2^w - 1` into it when the field lies inside the
//! register (`c + w <= 32`): offset `32 - c - w`, width `w`.
//!
//! Translation: gcc's `extzv` / `*extzv_32_reg` pattern (`config/m68k/m68k.md`)
//! for `(zero_extract:SI x w off)`, and `bfextu` in `m68k.md`'s `extzv`
//! expander.
//!
//! DEVIATE M68K-BFEXTU: the pin's M68k selects no bitfield instruction
//! (`M68kInstrBits.td` has `btst`, `bset`, `bclr` and `bchg` only), so
//! `(x >> 23) & 255` is `moveq #23; lsr.l; and.l #255` and a spare register to
//! save. Reason: gcc 15 extracts the exponent in `fix_si` / `fix_usi` with
//! `bfextu`, 7 clocks a call cheaper on the 68020 (row M68K-FIX-USI-CLK).
//! Owner m68k. Exit: 2026-11-24, or when the pin selects `bfextu`.
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

pub(crate) const TABLE: super::local::Table =
    super::local::Table { code_of, name_of, desc, text: bfextu_asm, bits: bfextu_encoding };

/// Code of the row, above the `jmp` forms.
pub const FIRST: u16 = 0xF900;

/// The row and the pin's rows it takes its description from: the register
/// operands from `ADD32dd`, the immediates from `ADD32di`.
pub const NAME: &str = "BFEXTU32dd";

/// The operands: the field (the result), the register, the offset, the width.
pub const DESTINATION: usize = 0;
pub const SOURCE: usize = 1;
pub const OFFSET: usize = 2;
pub const WIDTH: usize = 3;

pub fn code_of(name: &str) -> Option<u16> {
    (name == NAME).then_some(FIRST)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    (code == u32::from(FIRST)).then_some(NAME)
}

static DESC: LazyLock<&'static InstrDesc> = LazyLock::new(|| {
    let register = instrinfo::desc(super::opcodes::code_of("ADD32dd").expect("a pin row")).expect("a described row");
    let immediate = instrinfo::desc(super::opcodes::code_of("ADD32di").expect("a pin row")).expect("a described row");
    // the pin's [dst, src (tied to dst), opd]: the result, the register, two
    // immediates
    let operands = vec![register.operands[0], register.operands[2], immediate.operands[2], immediate.operands[2]];
    &*Box::leak(Box::new(InstrDesc {
        num_operands: operands.len() as u8,
        num_defs: 1,
        operands: Box::leak(operands.into_boxed_slice()),
        ..*register
    }))
});

pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    (code == FIRST).then_some(*DESC)
}

static ASM: LazyLock<(&'static str, &'static str, &'static [AsmOperand])> = LazyLock::new(|| {
    let operands: &'static [AsmOperand] = Box::leak(Box::new([
        AsmOperand { name: "src", at: SOURCE as u8, method: "printOperand" },
        AsmOperand { name: "off", at: OFFSET as u8, method: "printImmediate" },
        AsmOperand { name: "wid", at: WIDTH as u8, method: "printImmediate" },
        AsmOperand { name: "dst", at: DESTINATION as u8, method: "printOperand" },
    ]));
    (NAME, "bfextu\t$src{$off:$wid}, $dst", operands)
});

pub fn bfextu_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    (name == NAME).then_some(&*ASM)
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

/// `1110 1001 11 mode reg` (`0xE9C0` with mode 0, a data register) and the
/// extension word: `Dn` in bits 14-12, the offset in 10-6, the width in 4-0
/// (written in the instruction as bits 16 and up).
static REGISTER: [Piece; 6] = [
    field(0, 3, SOURCE as u8),
    bits(3, 3, 0),
    bits(6, 10, 0x3A7),
    field(16, 5, WIDTH as u8),
    field(22, 5, OFFSET as u8),
    field(28, 3, DESTINATION as u8),
];

pub fn bfextu_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    (name == NAME).then_some((32, &REGISTER[..]))
}

/// `(x >> c) & (2^w - 1)` of an `i32` as `bfextu`: the offset from the most
/// significant bit and the width of the field.
fn field_of(
    adapter: &DagisAdapter<'_>,
    node: u32,
) -> Option<(u32, i64, i64)> {
    let here = Value { node, res: 0 };
    if adapter.dag.opcode(here) != ISD::And || adapter.dag.value_type(here) != EVT::I32 {
        return None;
    }
    let (shift, mask) = [(0, 1), (1, 0)]
        .into_iter()
        .find_map(
            |(s, m)| {
                let shift = adapter.dag.operand(here, s);
                let mask = adapter.dag.constant(adapter.dag.operand(here, m))?;
                (adapter.dag.opcode(shift) == ISD::Srl && adapter.dag.has_one_use(shift.node)).then_some((shift, mask))
            },
        )?;
    let count = adapter.dag.constant(adapter.dag.operand(shift, 1))?;
    let mask = u32::try_from(mask & 0xFFFF_FFFF).ok()?;
    let width = mask.trailing_ones();
    // a low mask of 1 to 31 bits, wholly inside what the shift leaves
    if width == 0 || width >= 32 || mask != (1u32 << width) - 1 || count == 0 || count + u128::from(width) > 32 {
        return None;
    }
    Some((shift.node, 32 - count as i64 - i64::from(width), i64::from(width)))
}

/// `and (srl x, c), mask` as `bfextu`, from the 68020.
pub fn select(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    let (shift, offset, width) = field_of(adapter, node)?;
    let value = Dag::operand(adapter, shift, 0);
    let vt = adapter.value_type(SDValue { node, res: 0 });
    let offset = adapter.target_constant(offset, vt);
    let width = adapter.target_constant(width, vt);
    super::local::morph(adapter, node, FIRST, &[vt], &[value, offset, width]);
    Some(Outcome::Selected)
}
