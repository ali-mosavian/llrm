> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_tail.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_tail.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_tail.rs sha256:35f394f29bfdb284f406dedb38377a6414e3b6e0e091aa725733703e8fa05152 -->

# combine_select_tail

The select folds of `visitSelectInst` that rewrite a select whose arms or
condition are themselves selects:
[`foldSelectOfSymmetricSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3611)
and
[`foldNestedSelects`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3639).

## How it works
`select c2, (select c1, a, b), (select c1, b, a)` is
`select (xor c1, c2), b, a`. A select whose condition is a logical and/or
of its inner select's condition and another `alt` is two selects with the
inner condition outermost: the inner select moves into the arm that `alt`
decides.

## Example
```text
%c = select i1 %a, i1 %b, i1 false        %s = select i1 %b, i8 %x, i8 %y
%i = select i1 %a, i8 %y, i8 %z    ==>    %r = select i1 %a, i8 %s, i8 %z
%r = select i1 %c, i8 %x, i8 %i
```

MIRROR: the pattern, the one-use guards, the inversion swaps and the order
of the two folds.

DEVIATE 1: the new instruction is not named after the old one
(`takeName`): MIR names are not read by any pass.

## Used by
`combine_visit_select` (`FOLDS`).
