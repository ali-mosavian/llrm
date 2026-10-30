> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitpredicate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitpredicate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitpredicate.rs sha256:70357d823013f8fc40a87d30f943fa365c47890d3b2b939d5fd4c5b55d7c74b7 -->

# exitpredicate

A read-only loop's exits are rewritten to tests on the loop's trip count
made before it. LLVM's `predicateLoopExits` (`IndVarSimplify.cpp`).

## How it works
For each loop in LoopSimplify form, innermost first, whose exact backedge
count is known and safe to expand, and whose exits all dominate the latch:
the leading exits (in dominance order) that end in a conditional branch on a
non-constant, leave to a block with no phis, and have a known, expandable
count are predicated, when the loop writes nothing another thread could see
and has no implicit exit.

The exit's test becomes a constant where its count is the loop's, and else
`exit count != backedge count` (`==` where the loop is left on true),
compared in the wider of the two types. A loop with stores to its own
memory is predicated only where every exit leaves to a block that traps.

## Example
A loop with exits at counts `a` and `b`, backedge count `b`: the exit at `a`
tests `a != b` before the loop's first trip, the exit at `b` is `false`.

## Used by
Not wired in: [`crate::indvars`](indvars.md) will run [`predicated`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scev/predicated.rs) after
[`crate::exitopt`](exitopt.md), as `IndVarSimplify::run` does.

## See also
`docs/architecture/scev-port.md`.
