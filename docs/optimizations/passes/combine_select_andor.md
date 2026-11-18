> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_andor.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_andor.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_andor.rs sha256:c746b8f7a2af780d7fe345f332d6a9503ac2d6d564fc928e34eaaabe3cafe96b -->

# combine_select_andor

`FoldSelectWithAndOrCond`: a select on an `and` or `or` of two conditions,
when one of them settles the arms.

Translation of the lambda in
[`visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L5250)
and its dispatch on the condition
([:5296](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L5296))
at d1106deb71cc.

## How it works
- `select (A && B), T, F` is `select A, (select B, T, F), F` and `select (A
  || B), T, F` is `select A, T, (select B, T, F)`, when the inner select
  simplifies. A plain `and`/`or` tries both operand orders; a logical
  (select-form) one only the written order.

## Example
```text
%c = or i1 %a, %n ; %t = or i1 %b, %n
%r = select i1 %c, i1 %t, i1 false        %r = select i1 %a, i1 %t, i1 %n
```

MIRROR: the dispatch order, `simplifySelectInst(B, T, F)` asked at the
select, the branch metadata kept only when both arms are unchanged.

DEVIATE 1: the `canonicalizeSPF` step on a compare `B` is not here (it is
`combine_select_spf`'s next strip).

## Used by
`visitSelectInst`.
