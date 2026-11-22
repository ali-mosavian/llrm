> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_selects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_selects.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_selects.rs sha256:c419c852d52824b77f0fa654d45468e5ad70ba85aa059dba0a4133020150f72b -->

# combine_icmp_selects

`visitICmpInst`'s compare of two selects on one condition: when the compare
of one pair of arms simplifies, only the other pair is compared and the
condition picks the result.

Translation of
[`InstCombinerImpl::visitICmpInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L8128)
at d1106deb71cc.

MIRROR: the two selects share the condition and one has no other user.

## Example
```text
%x = select i1 %c, i8 %q, i8 2          %i = icmp ugt i8 %p, %q
%y = select i1 %c, i8 %p, i8 0          %r = select i1 %c, i1 %i, i1 false
%r = icmp ult i8 %x, %y          ==>
```

## Used by
Named by `combine_compares`.
