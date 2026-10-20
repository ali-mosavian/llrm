> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_or.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_or.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_or.rs sha256:620684747eaeae2c7a333a6fda2864e5af7b23065170aea3771690d4c560f38a -->

# combine_or

`InstCombinerImpl::visitOr`: the integer `or` folds.

Translation of
[`visitOr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L4196)
at d1106deb71cc, the folds that need only the operand patterns, in the
order it reaches them (`FOLDS`: a new one is a function and an entry).

MIRROR: the C++ order, which operand of a commutative match is bound
first (a match that binds and then fails a condition does not retry the
other order), and the one-use conditions.

DEVIATE 1: no vectors, and only integer constants.

DEVIATE 2: the folds that call a helper not yet translated are left out,
each named where it stood: `SimplifyDemandedInstructionBits`
(`combine_demanded`), `foldOrToXor`, `foldComplexAndOrPatterns`,
`foldIntegerPackFromVector`, `foldOrOfInversions`,
`foldUsingDistributiveLaws`, the `i1` select folds
(`foldAndOrOfSelectUsingImpliedCond`, `FoldOrOfLogicalAnds`),
`matchBSwapOrBitReverse`, `matchFunnelShift` and `matchOrConcat` (MIR has
no `fshl`), `foldBinOpShiftWithShift`, `tryFoldInstWithCtpopWithNot`, the
disjoint-`or` folds, the `trunc`/`lshr` mask fold, `matchSelectFromAndOr`
and everything after it.

## Example
`(x & 12) | (y & -13)` with `y = x | 3` keeps one mask:

```text
%o = or i32 %x, %b                           %a = and i32 %x, 12
%p = and i32 %o, 12                          %r = or i32 %a, %b
%q = and i32 %b, -13            ==>
%r = or i32 %p, %q
```
