> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/canonexit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/canonexit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/canonexit.rs sha256:50a649fdf8c5dc177fcdeb34468b6f731f1b873b60eda28050c1f9a40e6f4b81 -->

# canonexit

A loop's exit compare of a zero-extended counter is made unsigned, and the
extension is moved off the counter onto the invariant side. LLVM's
`canonicalizeExitCondition` (`IndVarSimplify.cpp`).

## How it works
For each loop, innermost first, over its exiting blocks that end in a
one-use `icmp`:

- A signed compare of `zext X` with a loop-invariant `Y` whose unsigned
  range (under the loop's guards) fits `X`'s width becomes the unsigned
  compare: both sides are non-negative.
- An unsigned compare of `zext X` with such a `Y` becomes `X` against `trunc
  Y`, the truncation written in the preheader, when the `zext` has no other
  use or `X` is an add recurrence. `samesign` is dropped.

## Example
`icmp slt (zext i16 %i to i32), %n` with `%n` known below 65536 becomes
`icmp ult (zext i16 %i to i32), %n`, then
`icmp ult i16 %i, %n.trunc` with `%n.trunc = trunc i32 %n to i16` in the
preheader.

## Used by
Not wired in: [`crate::indvars`](indvars.md) will run `canonicalized` after the
congruent IVs, as `IndVarSimplify::run` does.

## See also
`canonicalizeExitCondition`, `docs/architecture/scev-port.md`.
