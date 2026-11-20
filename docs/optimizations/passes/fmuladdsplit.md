> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fmuladdsplit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fmuladdsplit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fmuladdsplit.rs sha256:417c393e6d649627a9cc0bbe066f4cb83031b9b6ff9fd2d327ae552fca4ccdd1 -->

# fmuladdsplit

Splits an `llvm.fmuladd` whose product a loop does not change, so that
LICM hoists the multiply.

## How it works
clang contracts `x + a * b` into `llvm.fmuladd(a, b, x)`. The intrinsic is
one instruction to LICM, which cannot hoist half of it: a product of values
the loop does not write is made on every trip, until the DAG splits the call
into `fmul` and `fadd` (the target having no fused multiply-add). The
target says (`Tti::splits_invariant_fmuladd`) that no fused form will come,
and a call in a loop whose two factors are defined outside it and whose
addend is not becomes `fmul` and `fadd`, each with the call's flags, in
its place.

```text
loop:  %p = call fmuladd(%r, 6.7e7, %x)        loop:  %m = fmul %r, 6.7e7
                                         =>           %p = fadd %m, %x
                                                 (LICM: %m leaves the loop)
```

DEVIATE FMULADD-SPLIT (no pin pass: clang's `llvm.fmuladd` stays one
instruction until `SelectionDAGBuilder::visitIntrinsicCall` splits it, after
the loop passes; gcc's `convert_mult_to_fma` runs after
`pass_lim` and only where the target has the instruction; owner x87-c, exit
2026-11-24).

## See also
`-ffp-contract=off` for every call gave the same hoist and cost nbody_single
+3.2% clocks and +289 B at -O2: the split of a call whose product varies
changes the unrolling price of the loop that holds it. Only the invariant
product is split here.

## Example
A `llvm.fmuladd` in a loop whose factors are defined outside it and whose addend
is not becomes `fmul` and `fadd`, so LICM hoists the multiply.

```text
loop:  %p = call fmuladd(%r, 6.7e7, %x)        loop:  %m = fmul %r, 6.7e7
                                         =>           %p = fadd %m, %x
                                                 (LICM: %m leaves the loop)
```

## Used by
Named by `pipeline`.
