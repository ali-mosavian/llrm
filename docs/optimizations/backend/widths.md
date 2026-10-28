> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/widths.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/widths.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/widths.rs sha256:4def2865c0320b6ead4600b8d022f74e3461a94b88ba548d38ad063d2f92173d -->

# widths

How wide each value is where an instruction names it.

## How it works
`_named_widths` lists the (value, width) pairs one instruction names;
`_widest` keeps the largest per value over a body. `_read_widths` and
`_widest_read` count only the places an instruction reads, so a value no
instruction reads wider than 2 bytes never needs its high half, however
wide its definition is.

## Used by
The greedy allocator's inputs (`greedy::shadow`), which size each value's
register and decide which values need only a half register.

## Example
`v1 := mov32 [bp+8]; v2 := mov16 v1`: `v1` is named at 4 bytes and 2, so
`_widest` is 4; only 2 bytes of it are read, so `_widest_read` is 2.
