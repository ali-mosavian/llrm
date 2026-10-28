> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_sub.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_sub.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_sub.rs sha256:75004d1d1d5b944ebcc9c2792fd53edfe7124153010f0b70f2ae77b1d3634bd6 -->

# combine_sub

`InstCombinerImpl::visitSub`: the integer subtraction folds.

Translation of
[`visitSub`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L2413)
in the order it reaches them. `-x` is sunk into its operand by
[`combine_negator`](combine_negator.md).

MIRROR: the C++ order, including branches a later fold shadows.

DEVIATE 1: no vectors: `foldVectorBinop` and the `vector_reduce_add` fold
(:2602) are not translated.

DEVIATE 2: `isFreeToInvert` (`~X - ~Y`, :2587),
`tryFoldInstWithCtpopWithNot`, `foldSubOfMinMax` and everything from the
select-of-sub fold (:2799) on are the next strip, but the `usub.sat`,
`umin` and `umax` identities (:2978-3006).

DEVIATE 3: constant expressions are skipped (`combine_addsub` DEVIATE 4).

## How it works
After the shared prologue (simplify, phi operands, `x - -y`, shl
factoring), a constant minuend is folded into an add/select/phi, a
negation is sunk, and the rest are pattern rewrites over `add`, `sub`,
`and`, `or`, `xor` of shared operands. Each rewrite builds with the
`Combiner` builder.

## Used by
`combine::visit` (the `Sub` arm).

## Example
`(x | y) - (x & y)` is `x ^ y`:

```text
%o = or i32 %x, %y
%a = and i32 %x, %y
%r = sub i32 %o, %a   ; %r = xor i32 %x, %y
```
