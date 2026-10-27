> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/bodyfacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/bodyfacts.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/bodyfacts.rs sha256:69fe23f447eb9296a0f700391e231554edf569ca0c545904748741d962239dfb -->

# bodyfacts

Facts about the definitions of a body's values that the passes around the
allocator ask: which values are literals, addresses or copies, which loads
of a cell hold until their last use, and what may write a cell.
Moved out of `spiller` unchanged.

## How it works
Each function reads the postings or the instructions of a body and answers
for a set of values. `_stable_loads_by` proves, per live range, what LLVM
reads from a load's invariance flag (DEVIATE D-I of `greedy::remat`).

## Example
`v1 := mov 5; v2 := mov v1; ret v2`: `_constants` finds `v1` and `v2` both
the literal 5 (a copy of a literal is the literal), `_copies` finds `v2`
a copy of `v1`.

## Used by
`spiller`, `ssaspill`, `constrain`, `floatassign`, `peep`, `greedy::remat`.
