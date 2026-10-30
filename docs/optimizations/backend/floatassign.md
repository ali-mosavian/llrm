> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/floatassign.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/floatassign.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/floatassign.rs sha256:e7db8a2e059d255bdcd29fd7398acabf8455315ba41d1e08e24a49ba7d78a99b -->

# floatassign

Makes the floating code ready for register allocation: x87 integer
conversions and compares go through memory, and loads of one cell are one
value.

## How it works
Runs before `RegAlloc` as `FloatPrep` (`prepared`); the allocator then gives
each floating value a flat x87 register and [`crate::backend::fpstack`](fpstack.md)
turns those into stack form. This pass chooses no register.

An integer a `fild` reads from a register or an immediate goes through a
frame cell (a constant through the constant pool, 0 and 1 as `fldz`/`fld1`).
A float-to-integer store lands in an owned cell, with `wait` around it in a
BASIC module, and the integer is read back only if something reads it.
Later loads of a cell nothing wrote since are given the first load's value
(`_aliased_by`). Instructions that must keep their order on the stack are
sequenced (`_sequenced`), and a compare, which `fnstsw` answers in AX, is
marked as clobbering it (`_compares_clobber`).

A spilled value takes a ten-byte frame cell in extended format, so no
precision is lost.

## Example
`fild st, [bp+6]` of a word held in a register becomes a store of the
register to a frame cell and a `fild` of the cell; `fild 1` becomes `fld1`.

## Runs
Machine phase after `floatfold` and `phielim`, before `RegAlloc`, once per
function; a no-op when the body has no floating value. Not gated by an -O
level.

## Needs
`live_among`, [`crate::backend::frame`](frame.md) for cells, `constpool` for float
constants.

## See also
[`crate::backend::fpstack`](fpstack.md), `docs/optimizations/floating-pop-reuse.md`.
