//! `jmp (d8,%pc,Xn.w)`: the jump of a word-offset jump table.
//!
//! The table follows the `jmp` directly, so the `d8` is 2 (the table starts
//! 4 bytes past the opcode, 2 past the extension word the `pc` names) and
//! `Xn.w`, a sign-extended 16-bit offset from the table, reaches the case.
//! [`crate::wordtable`] makes it, with `move.w (table,%pc,%dn.l), %dn` before.
//!
//! DEVIATE M68K-WORD-TABLE: the pin's M68k has no PC-relative `jmp` and keeps
//! 32-bit entries (`EK_BlockAddress`). Reason: gcc's `casesi` makes this form;
//! each entry is 2 bytes, not 4. Owner x87-b. Exit: 2026-11-24.

use std::sync::LazyLock;

use llrm_dagisel::instr::InstrDesc;

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Code of the row, above the `bfffo` forms.
pub const FIRST: u16 = 0xF800;

/// The row: operand 0 is the table (the `d8` of the `pc`), operand 1 the index.
pub const NAME: &str = "JMP32kw";

pub fn code_of(name: &str) -> Option<u16> {
    (name == NAME).then_some(FIRST)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    (code == u32::from(FIRST)).then_some(NAME)
}

static DESC: LazyLock<&'static InstrDesc> = LazyLock::new(|| {
    let find = |name: &str| {
        let code = MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code).expect("a pin row");
        instrinfo::desc(code).expect("a described row")
    };
    // the `(d8,%pc,Xn)` operands of `move.w`, the control effects of `jmp (An)`
    let (word, jump) = (find("MOV16dk"), find("JMP32j"));
    &*Box::leak(Box::new(InstrDesc { operands: &word.operands[1..], num_operands: 2, ..*jump }))
});

pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    (code == FIRST).then_some(*DESC)
}

static ASM: (&str, &str, &[AsmOperand]) =
    (NAME, "jmp\t$dst", &[AsmOperand { name: "dst", at: 0, method: "printPCI32Mem" }]);

pub fn wordjump_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    (name == NAME).then_some(&ASM)
}

/// `jsr (d8,%pc,Xn)` with the opcode of `jmp` and a word index (bit 11 clear).
static PIECES: [Piece; 10] = [
    Piece::Const { lo: 0, width: 3, value: 3 },
    Piece::Const { lo: 3, width: 3, value: 7 },
    Piece::Const { lo: 6, width: 2, value: 3 },
    Piece::Const { lo: 8, width: 4, value: 14 },
    Piece::Const { lo: 12, width: 4, value: 4 },
    Piece::Operand { lo: 16, width: 8, at: 0, from: 0, enc: Enc::PcRelImm(8) },
    Piece::Const { lo: 24, width: 1, value: 0 },
    Piece::Const { lo: 25, width: 2, value: 0 },
    Piece::Const { lo: 27, width: 1, value: 0 },
    Piece::Operand { lo: 28, width: 4, at: 1, from: 0, enc: Enc::Plain },
];

pub fn wordjump_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    (name == NAME).then_some((32, &PIECES[..]))
}
