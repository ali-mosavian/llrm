> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_const.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_const.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_const.rs sha256:2723f36439aef68f15dad5def4fbe6fb6330e01fb8a9b7cd3c4c0371dc8cf3a3 -->

# combine_icmp_const

`icmp (binop ...), C`: the folds of a compare of a binary operator with a
constant, so far the `and` family.

Translation, from
[`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
at d1106deb71cc, of
[`foldICmpInstWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3644),
[`foldICmpBinOpWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4124),
[`foldICmpAndConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1951),
[`foldICmpAndConstConst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1807)
and
[`foldICmpAndShift`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1720).

MIRROR: the order of the folds and each condition.

OWED, each at its place as a `// OWED:` comment: the other opcodes of
`foldICmpBinOpWithConstant` (`foldICmpXorConstant`, `foldICmpOrConstant`,
`foldICmpShlConstant`, `foldICmpShrConstant`,
`foldICmpSRemConstant`, `foldICmpUDivConstant`, `foldICmpDivConstant`,
),
the `or`-of-select case of `foldICmpBinOpEqualityWithConstant`, the
truth-table fold, and the select, truncate, intrinsic and `extractvalue`
arms of `foldICmpInstWithConstant`.

DEVIATE 1: the load from an indexed global fold
(`foldCmpLoadFromIndexedGlobal`) and the floating-point class folds on a
bitcast (`is.fpclass`) are not translated.

## How it works
The compare's left operand is an `and` of a value with a constant `C2`,
compared with a constant `C1`. The `and` goes where the compare says what
it keeps: `(X & signbit) != 0` is `X < 0`, `(X & -2^n) == 0` is `X u< 2^n`,
`(X & 1) != 0` is `trunc X`, a shift under the mask moves into the
constants, and a mask of a narrowed value widens.

## Used by
`combine_compares::visit_icmp`.

## Example
A sign test of a mask is a compare of the value:

```text
%m = and i16 %x, -32768
%r = icmp ne i16 %m, 0          ; %r = icmp slt i16 %x, 0
```
