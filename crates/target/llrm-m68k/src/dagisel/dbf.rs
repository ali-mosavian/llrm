//! `dbf %dn, label`: decrement the low word of a data register and branch
//! unless it became -1 (`DBcc` with the condition false, GCC's `dbra`).
//!
//! # How it works
//! The operands are the counter (defined, and tied to its own use) and the
//! target. [`crate::doloop`] makes it from a counted loop's `addq` and `bne`;
//! the codes are not touched, as the `addq` it replaces set them. The first
//! word is `0101 0001 1100 1 Dn`, the second the 16-bit displacement from the
//! extension word, as `bra.w`'s: a loop of more than 32 KB is not made one.
//!
//! DEVIATE M68K-DOLOOP: the pin's M68k has no `DBcc` (`M68kInstrControl.td`
//! lists it unimplemented) and GCC's m68k states no `doloop_end`. Reason:
//! `move.b d1,(a0)+; dbf` is 18 clk a trip on the 68000 where `addq.w; bne` is
//! 26 with the store (mem_length_boundaries_m32 -Os: -15%, -16 B). Owner w8,
//! exit: when the pin selects `DBcc`.

use std::sync::LazyLock;

use llrm_dagisel::instr::InstrDesc;

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};
use super::generated::instrinfo;

/// Code of the row, above the `bfextu` forms.
pub const FIRST: u16 = 0xFA00;

/// The counter is operand 0 (tied to operand 1), the target operand 2.
pub const NAME: &str = "DBF16dl";

pub const COUNTER: usize = 0;
pub const TARGET: usize = 2;

pub fn code_of(name: &str) -> Option<u16> {
    (name == NAME).then_some(FIRST)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    (code == u32::from(FIRST)).then_some(NAME)
}

static DESC: LazyLock<&'static InstrDesc> = LazyLock::new(|| {
    let find = |name: &str| {
        let code = super::opcodes::code_of(name).expect("a pin row");
        instrinfo::desc(code).expect("a described row")
    };
    // the counter operands of `addq.w` (a short row of this target), the branch effects of `bra`
    let step = super::short::code_of("ADDQ16di").and_then(super::short::desc).expect("the short word addq");
    let branch = find("BRA8");
    let operands: Vec<_> = step.operands[..2].iter().copied().chain(branch.operands[..1].iter().copied()).collect();
    &*Box::leak(Box::new(InstrDesc {
        num_operands: 3,
        num_defs: 1,
        operands: Box::leak(operands.into_boxed_slice()),
        ..*branch
    }))
});

pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    (code == FIRST).then_some(*DESC)
}

static ASM: (&str, &str, &[AsmOperand]) = (
    NAME,
    "dbf\t$cnt, $dst",
    &[
        AsmOperand { name: "cnt", at: COUNTER as u8, method: "printOperand" },
        AsmOperand { name: "dst", at: TARGET as u8, method: "printPCRelImm" },
    ],
);

pub fn dbf_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    (name == NAME).then_some(&ASM)
}

static PIECES: [Piece; 5] = [
    Piece::Operand { lo: 0, width: 3, at: COUNTER as u8, from: 0, enc: Enc::Plain },
    Piece::Const { lo: 3, width: 5, value: 0x19 },
    Piece::Const { lo: 8, width: 4, value: 1 },
    Piece::Const { lo: 12, width: 4, value: 5 },
    Piece::Operand { lo: 16, width: 16, at: TARGET as u8, from: 0, enc: Enc::PcRelImm(16) },
];

pub fn dbf_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    (name == NAME).then_some((32, &PIECES[..]))
}
