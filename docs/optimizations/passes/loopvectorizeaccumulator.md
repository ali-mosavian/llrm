> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorizeaccumulator.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorizeaccumulator.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorizeaccumulator.rs sha256:903a9221e8c0c01cd5fbbd294490844a7f08ebeb1e060462eb7f2bf5fe78db62 -->

# loopvectorizeaccumulator

A reduction wider than a vector register keeps one register of lanes: gcc's
single accumulator, `vectorizable_reduction` and `vect_transform_reduction`
of
[`tree-vect-loop.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/tree-vect-loop.cc)
(`single_defuse_cycle`: the copies of one scalar statement chain through one
accumulator).

## How it works
`acc = acc + x` with `x` a `<16 x i64>` is `<2 x i64>` lanes of `acc` and
eight pieces of `x` added in turn: the same adds, one live accumulator where
LLVM keeps `<16 x i64>` (eight registers, and a byte sum spills). The pieces
are the shuffles `x[k * 2 .. k * 2 + 2]`, which the backend takes as the
registers `x` is already split into.

## DEVIATE rows
- LVA1 one accumulator per interleave part (gcc's one per reduction): the
  parts' chains stay independent, as the pin's. Exit: none.
- LVA2 only an update `acc op x` whose `acc` is the reduction's own phi and
  `x` a widened value; a chain of several operations keeps the wide
  accumulator.

## Runs
Inside `loopvectorize`, on a reduction whose vector is wider than a
register.

## Example
`acc = acc + x` with `x: <16 x i64>` on a 128-bit target becomes eight adds
of `<2 x i64>` pieces of `x` into one `<2 x i64>` accumulator.
