> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_intrinsic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_intrinsic.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_intrinsic.rs sha256:885bd80ac13ecccc46e61a5571e64b77e737c1445a876a81f9758c24516ae215 -->

# combine_icmp_intrinsic

`icmp` of an intrinsic and a constant.

Translation of the pin's (d1106deb71cc, InstCombineCompares.cpp)
[`foldICmpIntrinsicWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4329),
its equality half
[`foldICmpEqIntrinsicWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3929)
and the intrinsic arm of
[`foldICmpInstWithConstantAllowPoison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4102).

MIRROR: the arms and their order. `abs`, `bswap`, `bitreverse`, `ctlz`,
`cttz`, `ctpop`, `fshl`, `fshr`, `umax`, `uadd.sat`, `ssub.sat` and
`usub.sat` against a constant, equality first, then the relational arms of
`ctpop`, `ctlz`, `cttz`, `ssub.sat` and `abs`.

Also its helpers `foldCtpopPow2Test`,
`foldICmpUSubSatOrUAddSatWithConstant`, `foldICmpOfCmpIntrinsicWithConstant`
(`scmp`/`ucmp`) and, for two intrinsics, `foldICmpIntrinsicWithIntrinsic`.

## Used by
`fold_icmp_inst_with_constant`.

## Example
```text
%c = call i64 @llvm.ctpop.i64(i64 %x)
%r = icmp eq i64 %c, 0                    ==>   %r = icmp eq i64 %x, 0
```
