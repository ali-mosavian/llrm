> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_ranges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_ranges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_ranges.rs sha256:1c8b23648b12b06e93675dadb3d18ec43f126083ccaa64c8865cab3ed0a81659 -->

# combine_icmp_ranges

Range folds of `foldAndOrOfICmps`: two compares of one value, each a
range of it, are one compare.

Translation of
[`simplifyRangeCheck`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L726)
and
[`foldAndOrOfICmpsUsingRanges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L1341)
at d1106deb71cc.

## How it works
`foldAndOrOfICmpsUsingRanges` reads each compare of a constant as the
`ConstantRange` of values it accepts (of `X` for `(X + C1) pred C`, or of
`X` for `(X & -2^k) ==/!= C`). Where both are ranges of the same `X`, an
`or` accepts their union and an `and` the inverse of the union of the
inverses. A union the range type holds exactly is one compare
(`getEquivalentICmp`: `X u< n`, or `(X + offset) u< n`); two equal-size
ranges that differ in one bit of both ends are one compare of `X` with
that bit cleared. `simplifyRangeCheck` is the signed case with a lower bound
of 0 and a variable upper bound known not negative: one unsigned compare.

## Example
A lower-case test of a `char` (`c >= 'a' && c <= 'z'`):

```text
%a = icmp sgt i8 %c, 96                     %s = add i8 %c, -97
%b = icmp slt i8 %c, 123          ==>       %r = icmp ult i8 %s, 26
%r = and i1 %a, %b
```

MIRROR: the C++ order of the cases and its one-use conditions.

DEVIATE 1: scalars only. The pin matches a splat vector constant as well
(`m_APInt`). Exit: the vector `ConstantRange`.

## Used by
`combine_icmp_logic::fold_boolean_and_or`.
