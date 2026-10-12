> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gepoffset.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gepoffset.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gepoffset.rs sha256:d749bfb570554a979296e8b319bbc3a17fbd927c5bb1e8786058059459b83195 -->

# gepoffset

Move the constant an index adds into a `gep` of its own, where address modes
take it as a displacement.

## How it works
An index is read as `rest + constant`: the walk goes through add, sub, a
constant multiply or shift, truncation, and a `sext` or `zext` only over
adds that the matching `nsw` or `nuw` keeps from wrapping (not over a
multiply: the rest would wrap where the whole does not). An index narrower
than its pointer's is sign-extended by the `gep`, so it is read as under a
`sext`. The width is the address space's, from the datalayout.

```text
gep T, p, (i + 8) * 2   =>   gep T, (gep i8, p, 32), i * 2
```

The constant `gep` is free where isel folds it, so a split is kept when it
frees something and the instructions it makes cost no more than those it
frees (`profit::operation`): `i + 8` read by two addresses is split in both
and the add goes; `(i + 8) * 2` costs a multiply for an add and a multiply.
A sum that an add or a compare also reads stays: the address would read `i`
where it read the sum, one register more. Neither new `gep` is `inbounds`:
the constant may take the first outside the object. A negative constant
drops `inbounds` too.

## Runs
Once, after the body fixed point and after `hoist`: a
hoisted constant `gep` would be a register held across the loop instead of a
displacement. Off with the `gepoffset` or strength-reduction option.

## Keeps
Dominators and loops stay valid.

## See also
LLVM's SeparateConstOffsetFromGEP
(SeparateConstOffsetFromGEP.cpp).
