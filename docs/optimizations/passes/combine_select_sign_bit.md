> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_sign_bit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_sign_bit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_sign_bit.rs sha256:148bd4793520c7411eda3b22e65ac1a1cd6b12d44b3c9c63bde5e839133ef903 -->

# combine_select_sign_bit

`foldSelectInstWithICmp`'s sign-bit step: a select on `x > -1` tests
`x < 0` with its arms swapped.

Translation of
[the step at
:2449](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2449)
of
[`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2419)
at d1106deb71cc.

MIRROR: no constant arm, and a compare with one use (the pin's guards
against looping with other canonicalizations).

DEVIATE 1: the steps of `foldSelectInstWithICmp` before this one
(`foldSelectInstWithICmpConst`, `canonicalizeClampLike`,
`tryToReuseConstantFromSelectInComparison`,
`foldICmpUSubSatWithAndForMostSignificantBitCmp`, `foldSelectICmpEq`) and
after it (`foldSelectICmpMinMax` ... `foldSelectWithConstOpToBinOp`) are
not translated.

DEVIATE 2: the select's `prof` branch weights are dropped, not swapped.

## Used by
`visitSelectInst`, right after `canonicalizeSPF`.

## Example
```text
%c = icmp sgt i16 %x, -1                  %c = icmp slt i16 %x, 0
%r = select i1 %c, i16 %a, i16 %b   ==>   %r = select i1 %c, i16 %b, i16 %a
```
