> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_and.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_and.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_and.rs sha256:ccfe0966e92f24cd85a7caacd4006eb2eca23e5daec7757dbae4f2861a0c8d68 -->

# combine_and

`InstCombinerImpl::visitAnd`: the integer `and` folds.

Translation of
[`visitAnd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2475)
and
[`foldAndToXor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L1944)
in the order `visitAnd` reaches them, up to the constant-mask folds.

MIRROR: the C++ order, including branches a later fold shadows.

DEVIATE 1: no vectors (`foldVectorBinop`).

DEVIATE 2: `foldComplexAndOrPatterns` is the next strip or another family's.

DEVIATE 3: of the shifted-power-of-two folds (:2688-:2740) only the
single-shift one is translated; the two double-shift ones follow.

DEVIATE 4: from the `and` of `icmp`s (`foldAndOrOfICmps`) on, the rest of
`visitAnd` is the next strip.

## How it works
The prologue (simplify, associativity, phi operands, `foldAndToXor`) runs
first; then folds of an `and` with a constant mask, each rebuilding the
operands with the `Combiner` builder.

## Used by
`combine::visit` (the `And` arm, once wired).

## Example
`(x ^ 12) & 10` is `(x & 10) ^ 8`:

```text
%e = xor i32 %x, 12
%r = and i32 %e, 10   ; %a = and i32 %x, 10 ; %r = xor i32 %a, 8
```
