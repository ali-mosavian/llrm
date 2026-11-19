> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fmuladd.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fmuladd.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fmuladd.rs sha256:6c77f217a83a784a3c5fa8abae0f009625f6ce0a16f2b4a716f1c5ab1b0cc95b -->

# fmuladd

Expands `llvm.fmuladd` into an `fmul` and an `fadd` in MIR before the old
instruction selector, which has no fused multiply-add to select.

## How it works
`fmuladd(a, b, c)` is `fadd (fmul a, b), c`: the unfused form
`SelectionDAGBuilder` takes when the target has no faster FMA.

## Example
```text
%r = call float @llvm.fmuladd.f32(float %a, float %b, float %c)
==> %p = fmul %a, %b ; %r = fadd %p, %c
```

## Runs
Once per module in `driver::optimized`, unless the selector is the DAG,
which lowers `fmuladd` itself.

DEVIATE 1: the new instructions carry no fast-math flags; the call's
`contract`/`fast` bits are dropped. Exit: none (the old selector is the
only user).

## See also
LLVM's `SelectionDAGBuilder::visitIntrinsicCall`, `Intrinsic::fmuladd`
([SelectionDAGBuilder.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp)).
