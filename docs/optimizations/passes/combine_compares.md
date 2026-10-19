> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_compares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_compares.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_compares.rs sha256:0785831225f7201ff3661862c6011c01acaaacedcabb32818d7fb2ea7c1f549f -->

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
`foldICmpTruncWithTruncOrExt`, `canonicalizeICmpBool`,
`canonicalizeICmpPredicate` (needs `canFreelyInvertAllUsersOf`,
uselist-port's shared helper), `foldICmpWithConstant`,
`foldICmpWithDominatingICmp`, `foldICmpUsingBoolRange`,
`foldIsMultipleOfAPowerOfTwo`, `foldICmpBinOp`, `foldICmpInstWithConstant`,
`foldSignBitTest`, `foldOpIntoPhi`, `foldICmpInstWithConstantNotInt`,
`foldICmpCommutative`, `foldCmpSelectOfConstants` and the select pair fold,
the zero folds `foldIRemByPowerOfTwoToBitTest` and `stripNullTest`, and the
`dyn_castNegVal` select fold.

DEVIATE 1 (permanent): `samesign` is not a flag of MIR's `icmp`; the folds
that read or set it are left out (the unsigned-to-`samesign` step of
`foldICmpUsingKnownBits`; the signed-to-unsigned step is made, which is what
sets the flag in LLVM).

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
