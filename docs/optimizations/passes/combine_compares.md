> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_compares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_compares.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_compares.rs sha256:640e0e46a06a3ce03f1e1d909bdeb588e40ec2aea748eacb5bba750da743b658 -->

# combine_compares

InstCombine's `visitICmpInst` and the helpers it calls, from
[`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
at d1106deb71cc.

`visit_icmp` is
[`visitICmpInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L7941):
the same steps in the same order, each a function named for the C++ one with
its own permalink. A step that returns `nullptr` there returns `None` here,
one that returns an instruction returns `Some(Visit)`.

MIRROR: the order of the steps and each condition of the steps below.

This is the first slice of the family. A step not translated yet stays in
its place as a `// OWED:` comment naming the C++ function, so the order
stays LLVM's. The owed steps, with their exits:
`foldICmpTruncWithTruncOrExt`,
`foldICmpWithConstant`, `foldICmpUsingBoolRange`,
`foldIsMultipleOfAPowerOfTwo`, `foldICmpBinOp`, `foldICmpInstWithConstant`,
`foldSignBitTest`, `foldICmpInstWithConstantNotInt`,
`foldICmpCommutative`, `foldCmpSelectOfConstants` and the select pair fold,
the zero folds `foldIRemByPowerOfTwoToBitTest` and `stripNullTest`, and the
`dyn_castNegVal` select fold.

OPEN: two shapes the pin's first InstCombine folds and `visit_icmp` leaves,
the producing fold unidentified (not `foldICmpAndConstConst`,
`foldICmpWithLowBitMaskedVal`, InstSimplify's and-add or
`getDemandedBitsLHSMask`; a known-bits or range fold in the icmp visitor is
the lead). `(a & -4) == C` folds at C = 0 and signmin only
(`a s< -32764` at i16); `((b + M) & M) u< b` is `b != 0` for M = 3, 7.

DEVIATE 2 (temporary): `foldICmpUsingKnownBits` computes the known bits of
the operands, where LLVM first runs `SimplifyDemandedBits` on them
(InstCombineSimplifyDemanded.cpp, not translated): the same facts or fewer.

## Used by
`combine::visit`, once the compares slot is flipped.

## How it works
`visit_icmp` orders the operands (the more complex first), asks
InstSimplify, and then rewrites the compare to a canonical or cheaper one: a
non-strict predicate against a constant becomes the strict one (`x <= 5` is
`x < 6`), a compare the known bits decide or reduce (`A <u B` where `max(A)
== min(B)` is `A != B`) is rewritten.

## Example
`x u<= 5` is `x u< 6`:

```text
%r = icmp ule i16 %x, 5   ; %r = icmp ult i16 %x, 6
```
