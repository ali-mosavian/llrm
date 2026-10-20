> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_ext_const.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_ext_const.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_ext_const.rs sha256:4c201d3833958902c66b5e0b06be77b9ea660c385891e4a1f55f307e767b0ff6 -->

# combine_select_ext_const

`foldSelectExtConst`: `select c, (ext x), C` as `ext (select c, x, C')`.

Translation of
[`InstCombinerImpl::foldSelectExtConst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2711)
at d1106deb71cc, with `getLosslessInvCast` (ConstantFolding.cpp:5017) for
the `zext` and `sext` cases.

MIRROR: narrowed only when the extension has one use, the constant is the
same after truncation and extension back, and the extended type is `i1` or
the condition is an `icmp` or `trunc` of that type.

DEVIATE 1: no vectors; only an integer constant narrows (a `poison` arm
never reaches here: `simplifySelectInst` takes it first).

## Example
```text
%e = zext i1 %x to i16                    %n = select i1 %c, i1 %x, i1 true
%r = select i1 %c, i16 %e, i16 1    ==>   %r = zext i1 %n to i16
```
