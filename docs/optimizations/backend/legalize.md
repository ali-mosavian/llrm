> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/legalize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/legalize.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/legalize.rs sha256:4047880443a146e77c2b2af933c28e803e5ef9b23f66d28167a72b94e72cd1eb -->

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

## Example
An `i33` operation is made on `i64`; a shift right first zeroes the high
bits:

```text
%a = add i33 %x, %y     ; add i64
%d = lshr i33 %a, 1     ; and i64 %a, 0x1ffffffff, then lshr i64 by 1
```
