> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gepoffset.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gepoffset.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gepoffset.rs sha256:88894832579a6594c2a579787d917b574c3f54d17a78d28cf4e6a36372601306 -->

# gepoffset

LLVM's SeparateConstOffsetFromGEP: the constant an index adds, under a
scale or through an extension its flags allow, moves into a `gep` of its
own, where every address mode takes it as a displacement.

```text
gep T, p, (i + 8) * 2   =>   gep T, (gep i8, p, 32), i * 2
```

An index is `rest + constant`: the walk goes through add, sub, a constant
multiply or shift, truncation, and a `sext` or `zext` only over adds
the matching `nsw` or `nuw` keeps from wrapping (and not over a multiply:
the rest would wrap where the whole does not). An index narrower than
its pointer's is sign-extended by the `gep`, so it is read as under a
`sext`. The width is the address space's, from the datalayout.

The constant `gep` is free where isel folds it, so a split is kept when
it frees something and the instructions it makes cost no more than those
it frees (`profit::operation`): `i + 8` read by two addresses is split in
both and the add goes, `(i + 8) * 2` costs a multiply for an add and a
multiply. A sum an add or a compare also reads stays: the address would
read `i` where it read the sum, one register more. Neither new `gep` is
`inbounds`: the constant may take the first outside the object.

It runs last, after `hoist`: a hoisted constant `gep` would be a register
held across the loop instead of a displacement.
