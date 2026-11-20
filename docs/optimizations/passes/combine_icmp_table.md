> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_table.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_table.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_table.rs sha256:180485506df3734d54da9a03fc761ae8430ba3d27efb04a055e7bbe21c533397 -->

# combine_icmp_table

`icmp pred (load (gep @table, i)), C`: the compare of an element of a
constant table with a constant, as a compare of the index.

Translation, from
[`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
at d1106deb71cc, of
[`foldCmpLoadFromIndexedGlobal`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L119)
and of
[`decomposeLinearExpression`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp#L972).

MIRROR: the checks, the scan and the order of the state machines.

OWED: the `fcmp` call site (`foldFCmpInstWithConstantNotFloat`).

## How it works
The table is scanned once: each element is loaded from the initializer
(masked by `AndCst` when the load is under an `and`) and compared with the
constant. The compare is then rebuilt from the elements it is true (or
false) for: one or two indices, a range of them, or a bit of a constant
bitvector shifted by the index. A multi-use load stays.

## Used by
`combine_icmp_const::fold_icmp_and_constant` (the masked load) and
`combine_compares::visit_icmp` (the plain load).

## Example
A table of `{0, 1, 2, 17, 64, 100}` compared `> 9` is true for indices 3
to 5:

```text
%v = load i16, ptr %p        ; %p = gep [2 x i8], ptr @ns, i32 %i
%r = icmp sgt i16 %v, 9      ; %t = sub i32 %i, 3
                             ; %r = icmp ult i32 %t, 3
```
