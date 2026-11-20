> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_visit_select.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_visit_select.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_visit_select.rs sha256:6ddf149ff4ab82a7d7cf6997367b92b08b64b1a9ff6aba9f2188960652e6df26 -->

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
(`minnum`, `canonicalize`...: need #2209; `fabs` is `combine_select_fabs`),
the rest of `foldSelectInstWithICmp`
(its `canonicalizeSPF` step is `combine_select_spf`),
`foldAddSubSelect`,
`foldOverflowingAddSubSelect`, `foldSetClearBits`,
`foldSelectZeroOrFixedOp`, `foldSelectIntrinsic`,
`foldSelectWithSRem`, the
`foldSPFofSPF` (the min/max block after it is `combine_select_minmax_cast`),
and everything after the not-condition but
`foldSelectToCmp` (`foldSelectCmpBitcasts` ... `foldSelectEqualityTest`);
`FoldSelectWithAndOrCond` is `combine_select_andor`.

DEVIATE 3: when the operands of a select swap, its `prof` branch weights
are dropped, not swapped.

## Example
```text
%r = select i1 %c, i16 1, i16 0        %r = zext i1 %c to i16
```

## Used by
Named by `combine_select_andor`, `combine_select_bit_test`, `combine_select_bools`, `combine_select_cttz_ctlz`, `combine_select_equivalence`, `combine_select_ext_const`, `combine_select_fabs`, `combine_select_fcmp_swap`, `combine_select_into_op`, `combine_select_minmax_cast`, `combine_select_op_op`, `combine_select_spf`, `combine_select_tail`, `combine_select_to_cmp`, `combine_select_to_phi`.
