> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_fp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_fp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_fp.rs sha256:b6bad086ffdf34238020fd3665a1b903d12680c1d07f29178fcae65f696b3e70 -->

# combine_fp

`visitFDiv`'s `foldFDivConstantDivisor`: a division by a constant is a
multiply by its reciprocal.

Translation of
[`foldFDivConstantDivisor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp)
at d1106deb71cc.

MIRROR: `X / C --> X * (1/C)` when `C` has an exact inverse (a normal power
of two whose reciprocal is normal), or with `arcp` when `C` is a normal
number; the product takes the division's fast-math flags.

DEVIATE 1: the `-X / C --> X / -C` arm and `foldFDivConstantDividend`,
`foldFDivPowDivisor` and the sqrt/exp rules of `visitFDiv`, and everything
in `visitFAdd` and `visitFMul` after `SimplifyAssociativeOrCommutative`, are
not translated.

DEVIATE 2: an `x86_fp80` constant is held as a double: its reciprocal is
formed only where exact.

## Used by
`combine::visitors` for `fdiv`.

## Example
```text
%r = fdiv arcp double %x, 3.0     ==>   %r = fmul arcp double %x, 0x3FD5555555555555
%r = fdiv double %x, 4.0          ==>   %r = fmul double %x, 2.5e-1
```
