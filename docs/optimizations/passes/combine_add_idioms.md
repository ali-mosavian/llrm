> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_add_idioms.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_add_idioms.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_add_idioms.rs sha256:a9a0038121fb4bc5158d361d2ec9d22dea3e53f7c7fd7ba51b50133d2339f1fc -->

# combine_add_idioms

`visitAdd` after `x + x`: the negation and not identities, the
reassociations of an add with an add or sub, and the bitwise identities.

Translation of the middle of
[`InstCombinerImpl::visitAdd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L1645)
(`-A + -B` through `(or A, B) + (and A, B)`, :1646-:1819) and of
[`checkForNegativeOperand`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L752).

MIRROR: the C++ order. Each fold is one function; `fold_add_rest` tries
them in the order `visitAdd` reaches them.

DEVIATE 1: `SimplifyAddWithRemainder` (:1739), the align-up and
`(A & -A) - 1` idioms, `~(A*C)+A`, signum, `select` of `icmp eq`,
ceiling division, `~X + ~Y` (`isFreeToInvert`) and everything after
`canonicalizeLowbitMask` are the next strip.

DEVIATE 2: `sext(A<B) + zext(A>B)` (:1766) and `X + (or X, -X)`'s
sibling folds that build `umax`/`scmp` need `CreateIntrinsic`
(`combine_build` DEVIATE 2).

## How it works
Each fold matches with `pattern` and the shared helpers, builds the new
operands with the `Combiner` builder (which simplifies first) and returns
the `Visit` of `BinaryOperator::Create`.

## Used by
`combine_addsub::visit_add`.

## Example
`-a + b` is `b - a`:

```text
%n = sub i32 0, %a
%r = add i32 %n, %b   ; %r = sub i32 %b, %a
```
