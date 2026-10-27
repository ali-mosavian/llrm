> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gepoffset.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gepoffset.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gepoffset.rs sha256:47af1195ce5f00b6e3f615d4d949f415de6905ed795a7b37b1c3800273778ac3 -->

# gepoffset

Moves the constant an index adds into a `gep` of its own, where address
modes take it as a displacement: `a[i + 8]` then needs no add, and no
register for the sum, to reach its element.

## How it works
An index is read as `rest + constant`: the walk goes through add, sub, a
constant multiply or shift, truncation, and a `sext` or `zext` only over
adds that the matching `nsw` or `nuw` keeps from wrapping (not over a
multiply: the rest would wrap where the whole does not). An index narrower
than its pointer's is sign-extended by the `gep`, so it is read as under a
`sext`. The width is the address space's, from the datalayout.

The constant `gep` is free where isel folds it, so a split is kept when it
frees something and the instructions it makes cost no more than those it
frees (`profit::operation`): `i + 8` read by two addresses is split in both
and the add goes; `(i + 8) * 2` costs a multiply for an add and a multiply.
A sum that an add or a compare also reads stays: the address would read `i`
where it read the sum, one register more. Neither new `gep` is `inbounds`:
the constant may take the first outside the object. A negative constant
drops `inbounds` too.

DEVIATE LP-2 (no pin pass: LLVM gets `a[i + 8]` as a displacement from
InstCombine's gep canonicalization, LSR and CodeGenPrepare's address-mode
matching (row CG DEVIATE C1); exit: that matching translated, then deleted;
owner w-6; review 2026-11-13).

## Example
Before:

```mir
define i16 @f(i16 %i) {
b0:
  %s = add i16 %i, 8
  %m = mul i16 %s, 2
  %p = getelementptr i16, ptr @g, i16 %m
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}
```

After:

```mir
define i16 @f(i16 %i) {
b0:
  %0 = mul i16 %i, 2
  %1 = getelementptr i8, ptr @g, i16 32
  %p = getelementptr i16, ptr %1, i16 %0
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}
```

The `+ 8` under the scales is 32 bytes, now a constant `gep` of its own; the
add is gone.

From unit test `test_a_constant_added_under_a_scale_is_a_displacement` in
`gepoffset_tests.rs`.

## Runs
Once, after the body fixed point and after `hoist`: a hoisted constant `gep`
would be a register held across the loop instead of a displacement. Off with
`-fno-strength-reduce`.

## Keeps
Dominators and loops stay valid.

## See also
LLVM's SeparateConstOffsetFromGEP (SeparateConstOffsetFromGEP.cpp).
