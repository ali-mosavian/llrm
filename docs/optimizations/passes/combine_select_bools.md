> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_bools.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_bools.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_bools.rs sha256:b36be05a98d677dc14126e3c5138f81dbc2029af286674f2d3ea90a44b18f6c1 -->

# combine_select_bools

`foldSelectOfBools`: a `select` of `i1` as a logical operation.

Translation of
[`InstCombinerImpl::foldSelectOfBools`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3754)
at d1106deb71cc, in its order, each fold a function in `FOLDS`.

MIRROR: `select c, true, x` is `or c, x` where `x` is poison only if `c`
is; `select c, x, false` is `and`; the nested logical and/or reassociate
(`(A || B) || C` as `A || (B | C)`); a constant arm moves the condition's
negation; De Morgan in select form; a select of a select that decides
itself.

DEVIATE 1: `impliesPoisonOrCond` is `impliesPoison` alone: the `samesign`
compare and `trunc nuw` cases are not found (it finds less).

DEVIATE 2 (temporary): the folds that need another family's helper are left
out, named where they stood: the factorization of `(A && B) || (C && B)`,
`~(A & B) & (A | B)`, the `select (~a | c)`, `(c & b)`, `(a | c)` and
`(c & ~b)` forms (`getFreelyInverted`), `isCheckForZeroAndMulWithOverflow`,
`foldBooleanAndOr` (and/or/xor family), the implied-condition
simplifications and `FoldOrOfLogicalAnds`.

DEVIATE 3: `prof` branch weights are dropped where the arms swap.

## Example
```text
%r = select i1 %a, i1 true, i1 %b        %r = or i1 %a, %b       ; %b is poison only if %a is
```

## Used by
Named by `combine_visit_select`.
