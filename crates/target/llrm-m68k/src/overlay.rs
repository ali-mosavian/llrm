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
//! The `(An)+` fold is one too: LLVM forms post-increment accesses in the
//! indexed-load/store combines when the target marks them legal, and the pin's
//! M68k marks none. Same exit, same switch.
//!
//! Encodings are written here by hand from the 68000 manual and checked
//! against binutils `m68k-linux-gnu-as` in `tests/overlay.rs`.

use crate::dagisel::asm::AsmOperand;
use crate::dagisel::enc::{Enc, Piece};

const SRC: &[AsmOperand] = &[AsmOperand { name: "src", at: 0, method: "printOperand" }];
const MASK: &[AsmOperand] = &[AsmOperand { name: "mask", at: 0, method: "printMoveMask" }];
const DST: &[AsmOperand] = &[AsmOperand { name: "dst", at: 0, method: "printOperand" }];

/// `(name, asm string, operands)`, as the pin's `asm` rows.
pub fn asm(name: &str) -> Option<(&'static str, &'static [AsmOperand])> {
    Some(match name {
        "PUSH32" => ("move.l\t$src, -(%sp)", SRC),
        "POP32" => ("move.l\t(%sp)+, $dst", DST),
        "PUSHM32" => ("movem.l\t$mask, -(%sp)", MASK),
        "POPM32" => ("movem.l\t(%sp)+, $mask", MASK),
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

const fn test(size: u64) -> [Piece; 4] {
    // `0100 1010 ss mmm rrr`
    [bits(8, 8, 0b0100_1010), bits(6, 2, size), field(3, 3, 0, 3), field(0, 3, 0, 0)]
}

// `move.l reg, -(%sp)`: 0010 111 100 mmm rrr
static PUSH: [Piece; 5] = [bits(12, 4, 0b0010), bits(9, 3, 7), bits(6, 3, 0b100), field(3, 3, 0, 3), field(0, 3, 0, 0)];
// `move.l (%sp)+, reg`: 0010 rrr mmm 011 111
static POP: [Piece; 5] = [bits(12, 4, 0b0010), field(9, 3, 0, 0), field(6, 3, 0, 3), bits(3, 3, 0b011), bits(0, 3, 7)];
// `movem.l regs, -(%sp)`: 0x48E7, then the mask with d0 as bit 15
static PUSHM: [Piece; 2] =
    [bits(0, 16, 0x48E7), Piece::Operand { lo: 16, width: 16, at: 0, from: 0, enc: Enc::InverseMoveMask }];
// `movem.l (%sp)+, regs`: 0x4CDF, then the mask with d0 as bit 0
static POPM: [Piece; 2] = [bits(0, 16, 0x4CDF), field(16, 16, 0, 0)];

static TST: [[Piece; 4]; 3] = [test(0), test(1), test(2)];

// `divu.l`/`divs.l %src, %dq` (32 / 32 -> 32): the pin's rows leave the
// extension word's remainder register at 0, which makes the form that stores
// the remainder in `%d0`. Dr = Dq stores none.
const fn divide_long(signed: u64) -> [Piece; 9] {
    [
        field(0, 3, 2, 0),
        bits(3, 3, 0),
        bits(6, 10, 305),
        field(16, 3, 0, 0),
        bits(19, 7, 0),
        bits(26, 1, 0),
        bits(27, 1, signed),
        field(28, 3, 0, 0),
        bits(31, 1, 0),
    ]
}

static DIVL: [[Piece; 9]; 2] = [divide_long(0), divide_long(1)];

/// `(width in bits, pieces)`, as the pin's `enc` rows.
pub fn encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    Some(match name {
        "PUSH32" => (16, &PUSH),
        "POP32" => (16, &POP),
        "PUSHM32" => (32, &PUSHM),
        "POPM32" => (32, &POPM),
        "TST8d" => (16, &TST[0]),
        "TST16d" => (16, &TST[1]),
        "TST32d" => (16, &TST[2]),
        "UDIVd32d32" => (32, &DIVL[0]),
        "SDIVd32d32" => (32, &DIVL[1]),
        _ => return None,
    })
}

/// An instruction whose `dr` form takes a register of either kind as its
/// source commutes when its `dd` form (data registers only) does: `ADD32dr`
/// from `ADD32dd`'s `isCommutable`. The swap is checked against the register
/// classes by `commute_tied`.
///
/// DEVIATE from the pin, which selects `ADD32dr` for a value in an `xr32`
/// register (every phi is) and so never swaps its sources. Reason: swapping
/// is what lets the result take the register of the source that lives on, and
/// saves a move on a loop that carries a sum. Exit: never; a target choice,
/// off with [`crate::compile::Options::overlay`].
pub fn mark_commutable(function: &mut crate::mfunc::MFunction) {
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        if let llrm_spike_mf::Kind::Op(name) = inst.kind
            && let Some(stem) = name.strip_suffix("dr")
            && crate::mfunc::pin_commutable(&format!("{stem}dd"))
        {
            inst.commutable = true;
        }
    }
}
