> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_minmax.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_minmax.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_minmax.rs sha256:fc4ecbaa3e4383a2afcf813c04cbe1258a28f3beb58944d05ad053c0f875c1b6 -->

# combine_icmp_minmax

`icmp pred min|max(X, Y), Z`.

Translation of the pin's (d1106deb71cc, InstCombineCompares.cpp)
[`foldICmpWithMinMax`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5832),
called from `foldICmpCommutative` (L7810) for the min/max in either operand.

MIRROR: the order of the checks and each table of the pin (equality, then
the eight relational predicates). `samesign` is dropped, as the pin does,
since a compare of `X` or `Y` against `Z` is speculated.

## Used by
`combine_compares::visit_icmp`.

## Example
```text
%m = call i32 @llvm.umin.i32(i32 %x, i32 %y)
%r = icmp ult i32 %m, 7              ; %x u< 7 is known: true
```
