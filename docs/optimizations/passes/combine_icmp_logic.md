> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_logic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_logic.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_logic.rs sha256:88191bbee51a1f28039f03b79c428fa2321bba3a0c7b8a77715958ea1e67bbdc -->

# combine_icmp_logic

`foldBooleanAndOr`: an `and` or `or` of two `i1` compares, folded to one
compare.

Translation of
[`foldBooleanAndOr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L3685)
at d1106deb71cc: `foldLogOpOfMaskedICmps`
([L536](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L536))
with `getMaskedTypeForICmpPair`
([L215](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L215))
and its two helpers, then the part of `foldAndOrOfICmps`
([L3445](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L3445))
listed below.

## How it works
Both operands are `i1` compares. `getMaskedTypeForICmpPair` views each as
`(A & B) ==/!= C` (a compare of a mask, a bit test, or a compare with an
all-ones mask), finds the `A` they share and classifies each side
(all-ones, all-zeros, mixed). The classes both sides satisfy say how to
merge: OR the masks, AND them, or one side implies the other. The part of
`foldAndOrOfICmps` kept merges two compares of the same operands by their
compare codes and turns pairs of zero, all-ones and sign tests into one
test of the OR, AND or XOR.

## Example
A sign test of each of two values is one test of their `or`
(`(icmp slt a, 0)` is `(a & signbit) != 0`, so the sign bit is the common
`A` of the masked form):

```text
%x = icmp slt i16 %a, 0                     %o = or i16 %a, %b
%y = icmp slt i16 %b, 0           ==>       %r = icmp slt i16 %o, 0
%r = or i1 %x, %y
```

MIRROR: the C++ order and the one-use and poison-safety conditions.

DEVIATE 1: the `isNaN` idiom of `foldLogOpOfMaskedICmps_NotAllZeros_BMask_
Mixed` (a floating-point class test) is not translated.

DEVIATE 2: of `foldAndOrOfICmps` only the compare-code merge, the sign-bit
pair, `(ne A, 0) | (ne B, 0)` and `(ne A, -1) | (ne B, -1)` are here. Not
yet: `foldAndOrOfICmpEqConstantAndICmp`, `foldAndOrOfICmpsWithConstEq`,
`foldIsPowerOf2OrZero`, `foldSignedTruncationCheck`,
`foldIsPowerOf2`, `foldPowerOf2AndShiftedMask`,
`foldUnsignedUnderflowCheck`, `foldAndOrOfICmpsWithPow2AndWithZero`, the
`trunc`/`and` fold and the `isnormal` fold; `simplifyRangeCheck` and
`foldAndOrOfICmpsUsingRanges` are in `combine_icmp_ranges`; also
`foldLogicOfFCmps` and `foldEqOfParts`.

`reassociateBooleanAndOr` is here: `visit_and` and `visit_or` call it after
the fold of the pair.

## Used by
`visit_and` and `visit_or` (the bitwise forms).
