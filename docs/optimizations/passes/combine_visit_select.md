> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_visit_select.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_visit_select.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_visit_select.rs sha256:55701c45493a09d6a5916498ded83f98d8654fc3d3d8de716748d04f07c23071 -->

# combine_visit_select

`visitSelectInst`: the canonical forms of a `select`.

Translation of
[`InstCombinerImpl::visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4598)
at d1106deb71cc. `visit` has already asked `simplifySelectInst`.

MIRROR: the folds below in the pin's order, each its own function in
`FOLDS`: a new one is a function and an entry.

DEVIATE 1: no vectors or `shufflevector` (MIR has none):
`canonicalizeSelectToShuffle`, `canonicalizeScalarSelectOfVecs` and
`foldVectorSelect` are not made.

DEVIATE 2 (temporary): the folds that call a helper not yet translated are
left out, each named where it stood: `foldSelectOfBools`,
`foldSelectNegNot`, `foldSelectAndOrPowerOfTwo`, the floating-point folds
(`fabs`, `minnum`, `canonicalize`...: need #2209),
`foldSelectValueEquivalence`, the rest of `foldSelectInstWithICmp`
(its `canonicalizeSPF` step is `combine_select_spf`),
`foldAddSubSelect`,
`foldOverflowingAddSubSelect`, `foldSetClearBits`,
`foldSelectZeroOrFixedOp`, `foldSelectIntrinsic`,
`foldSelectWithSRem`, the
`foldSPFofSPF`, and everything after the not-condition
(`foldSelectCmpBitcasts` ... `FoldSelectWithAndOrCond`).

DEVIATE 3: `replaceInInstruction` (the arms' instructions that read the
condition get its value) is not made:
`isSafeToSpeculativelyExecuteWithVariableReplaced` is private to
`lazyvalueinfo.rs`.

DEVIATE 4: when the operands of a select swap, its `prof` branch weights
are dropped, not swapped.

## Example
```text
%r = select i1 %c, i16 1, i16 0        %r = zext i1 %c to i16
```

## Used by
Named by `combine_select_bit_test`, `combine_select_bools`, `combine_select_ext_const`, `combine_select_into_op`, `combine_select_op_op`, `combine_select_sign_bit`, `combine_select_spf`.
