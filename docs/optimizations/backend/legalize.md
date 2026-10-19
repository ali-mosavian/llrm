> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/legalize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/legalize.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/legalize.rs sha256:61d4982be0d6aa2beb7901ec5f4e235db988d4c8e1a2aa3ea711a3b8cbf215cb -->

# legalize

Promotes the integers of an odd width (i2..i63 but 8, 16 and 32) to the
next machine width, in MIR before instruction selection.

## How it works
An operation on `iN` is made on `iM`, M the next of 8, 16, 32 and 64. The
bits above N of a promoted value are not defined, so an operation that
reads them first extends its operands in the register: `and` with the mask
to zero them (`lshr`, `udiv`, `urem`, unsigned compares, `zext`), `shl` and
`ashr` to copy the sign (`ashr`, `sdiv`, `srem`, signed compares, `sext`).
`add`, `sub`, `mul`, the bit operations, `shl`, `select`, `phi`, `freeze`
and `trunc` take them as they are. Poison and overflow flags are dropped,
which only makes the result more defined.

A function that gives an odd width to anything else (an argument, a load,
a store, a call, a return, a switch, a GEP index) is left as it is:
selection refuses it as before.

## Runs
Once per module in `driver::optimized`, after `minmax_expanded` and before
`selects`, at every -O level. SCEV's closed forms (`n * (n - 1) / 2` in
`i33`) made the first.

## See also
LLVM's `DAGTypeLegalizer::PromoteIntegerResult` and `PromoteIntegerOperand`.
