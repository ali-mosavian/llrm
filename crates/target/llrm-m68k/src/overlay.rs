//! Instructions the pin's M68k `.td` does not define, and the rewrites that
//! use them: `addq`/`subq` for a constant 1 to 8 and `tst` for a compare with
//! zero. Each is smaller and faster on the 68000 than the pin's text
//! (`add.l #1, %d1` is 6 bytes and 16 clocks, `addq.l #1, %d1` 2 and 8;
//! `cmpi.l #0, %d1` 6 and 14, `tst.l %d1` 2 and 4), with the same condition
//! codes: `addq`/`subq` on a data register set them as `add`/`sub` do, `tst`
//! as `cmpi #0` does. On an address register `addq`/`subq` leave them alone,
//! as `adda`/`suba` do.
//!
//! DEVIATE from the pin's output (`M68kInstrArithmetic.td` marks `ADDQ` and
//! `SUBQ` as not implemented, and there is no `TST`). Reason: the user's
//! target is code smaller and faster than the reference. Exit: never; a
//! target choice, switched off by [`crate::compile::Options::overlay`] for the
//! tests that compare with `llc`.
//!
//! Encodings are written here by hand from the 68000 manual and checked
//! against binutils `m68k-linux-gnu-as` in `tests/overlay.rs`.

use crate::dagisel::asm::AsmOperand;
use crate::dagisel::enc::{Enc, Piece};
use crate::mc::{Inst, Operand};

const IMM_THEN_DST: &[AsmOperand] = &[
    AsmOperand { name: "imm", at: 1, method: "printImmediate" },
    AsmOperand { name: "dst", at: 0, method: "printOperand" },
];
const DST: &[AsmOperand] = &[AsmOperand { name: "dst", at: 0, method: "printOperand" }];

/// `(name, asm string, operands)`, as the pin's `asm` rows.
pub fn asm(name: &str) -> Option<(&'static str, &'static [AsmOperand])> {
    Some(match name {
        "ADDQ8d" => ("addq.b\t$imm, $dst", IMM_THEN_DST),
        "ADDQ16d" => ("addq.w\t$imm, $dst", IMM_THEN_DST),
        "ADDQ32d" | "ADDQ32a" => ("addq.l\t$imm, $dst", IMM_THEN_DST),
        "SUBQ8d" => ("subq.b\t$imm, $dst", IMM_THEN_DST),
        "SUBQ16d" => ("subq.w\t$imm, $dst", IMM_THEN_DST),
        "SUBQ32d" | "SUBQ32a" => ("subq.l\t$imm, $dst", IMM_THEN_DST),
        "TST8d" => ("tst.b\t$dst", DST),
        "TST16d" => ("tst.w\t$dst", DST),
        "TST32d" => ("tst.l\t$dst", DST),
        _ => return None,
    })
}

const fn field(
    lo: u16,
    width: u8,
    at: u8,
    from: u8,
) -> Piece {
    Piece::Operand { lo, width, at, from, enc: Enc::Plain }
}

const fn bits(
    lo: u16,
    width: u8,
    value: u64,
) -> Piece {
    Piece::Const { lo, width, value }
}

const fn quick(
    kind: u64,
    size: u64,
) -> [Piece; 6] {
    // `0101 ddd k ss mmm rrr`: `k` 0 for add, 1 for subtract; the data field
    // holds 8 as 0 (the mask drops the high bit)
    [bits(12, 4, 0b0101), field(9, 3, 1, 0), bits(8, 1, kind), bits(6, 2, size), field(3, 3, 0, 3), field(0, 3, 0, 0)]
}

const fn test(size: u64) -> [Piece; 4] {
    // `0100 1010 ss mmm rrr`
    [bits(8, 8, 0b0100_1010), bits(6, 2, size), field(3, 3, 0, 3), field(0, 3, 0, 0)]
}

static ADDQ: [[Piece; 6]; 3] = [quick(0, 0), quick(0, 1), quick(0, 2)];
static SUBQ: [[Piece; 6]; 3] = [quick(1, 0), quick(1, 1), quick(1, 2)];
static TST: [[Piece; 4]; 3] = [test(0), test(1), test(2)];

/// `(width in bits, pieces)`, as the pin's `enc` rows.
pub fn encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    Some(match name {
        "ADDQ8d" => (16, &ADDQ[0]),
        "ADDQ16d" => (16, &ADDQ[1]),
        "ADDQ32d" | "ADDQ32a" => (16, &ADDQ[2]),
        "SUBQ8d" => (16, &SUBQ[0]),
        "SUBQ16d" => (16, &SUBQ[1]),
        "SUBQ32d" | "SUBQ32a" => (16, &SUBQ[2]),
        "TST8d" => (16, &TST[0]),
        "TST16d" => (16, &TST[1]),
        "TST32d" => (16, &TST[2]),
        _ => return None,
    })
}

/// The smaller instruction for `inst`, if it has one.
pub fn rewrite(inst: Inst) -> Inst {
    let quickly = |name: &'static str, register, amount: i64| Inst {
        opcode: name,
        operands: vec![Operand::Reg(register), Operand::Imm(amount)],
    };
    match (inst.opcode, &inst.operands[..]) {
        ("ADD8di", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("ADDQ8d", dst, value)
        }
        ("ADD16di", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("ADDQ16d", dst, value)
        }
        ("ADD32di", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("ADDQ32d", dst, value)
        }
        ("ADD32ai", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("ADDQ32a", dst, value)
        }
        ("SUB8di", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("SUBQ8d", dst, value)
        }
        ("SUB16di", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("SUBQ16d", dst, value)
        }
        ("SUB32di", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("SUBQ32d", dst, value)
        }
        ("SUB32ai", &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ 1..=8)]) if dst == src => {
            quickly("SUBQ32a", dst, value)
        }
        ("CMP8di", &[Operand::Imm(0), Operand::Reg(register)]) => {
            Inst { opcode: "TST8d", operands: vec![Operand::Reg(register)] }
        }
        ("CMP16di", &[Operand::Imm(0), Operand::Reg(register)]) => {
            Inst { opcode: "TST16d", operands: vec![Operand::Reg(register)] }
        }
        ("CMP32di", &[Operand::Imm(0), Operand::Reg(register)]) => {
            Inst { opcode: "TST32d", operands: vec![Operand::Reg(register)] }
        }
        _ => inst,
    }
}
