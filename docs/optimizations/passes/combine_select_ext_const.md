> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_ext_const.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_ext_const.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_ext_const.rs sha256:e882deeefa0ca17650457779528ba471a22d288d051505eb29b8f65e558144d9 -->

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

## Used by
Named by `combine_visit_select`.
