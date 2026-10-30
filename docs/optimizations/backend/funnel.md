> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/funnel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/funnel.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/funnel.rs sha256:9d6bff26a617b59db7104224efdb665e71e50789c121975a9f6abfc4d21f369a -->

# funnel

Expands `llvm.fshl` and `llvm.fshr` into shifts and an or in MIR before
instruction selection, for the targets whose description lists `fshl` or
`fshr` under `expand`.

## How it works
A constant amount is taken modulo the width: zero keeps the half that is not
shifted, any other `c` is `x << c | y >> (w - c)` (`fshl`) or
`x << (w - c) | y >> c` (`fshr`). A variable amount cannot shift by the
width, so the second operand is shifted by one first and then by the
complement: `x << s | (y >> 1) >> (~z & (w - 1))` with `s = z & (w - 1)`
(`urem` and `sub` where the width is not a power of two).

## Example
`fshl` of an `i16` by a variable amount:

```text
%r = call i16 @llvm.fshl.i16(i16 %x, i16 %y, i16 %z)
==> %s = and %z, 15 ; %n = xor %z, -1 ; %i = and %n, 15
    %a = shl %x, %s ; %b = lshr %y, 1 ; %c = lshr %b, %i ; %r = or %a, %c
```

## Runs
Once per module in `driver::optimized`, for a target that lists `fshl` or
`fshr` under `expand`; a call on one that does not reaches selection, which
refuses it by name.

DEVIATE 1: the `isNonZeroModBitWidthOrUndef` two-shift form for a variable
amount known to be non-zero modulo the width is not made: the general form
is always right.

## See also
LLVM's `TargetLowering::expandFunnelShift`
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L8697)).
