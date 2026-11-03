//! The encoding of `divu.l` and `divs.l` for the 32 / 32 -> 32 forms.
//!
//! DEVIATE from the pin's generated rows, which leave the extension word's
//! remainder register at 0 and so store the remainder in `%d0`, a register the
//! instruction must not write. Exit: the pin's row is fixed upstream.

use crate::dagisel::enc::{Enc, Piece};

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
        "UDIVd32d32" => (32, &DIVL[0]),
        "SDIVd32d32" => (32, &DIVL[1]),
        _ => return None,
    })
}
