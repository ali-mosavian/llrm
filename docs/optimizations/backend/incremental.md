> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/incremental.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/incremental.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/incremental.rs sha256:9025663f3045479611bfe4faa5583a55562583e052dcb56fa090a68bdb9d9216 -->

# incremental

A dataflow answer over a body's blocks, held between asks and brought up to
date from the blocks that changed.

## How it works
The sub-passes of a clean-up ask for the same fact about the body in turn
(the lanes live out of each block, the flags live out of each block, the
roots zero above their word) and each leaves most blocks as they were. A
`Problem` says what the fact is: what one block does to it (a summary,
worked out from the block's instructions), what enters a block from its
neighbours, and what the block makes of that. `Kept` holds the last answer
and the summaries.

```text
 ask(body)
   every block the same instructions (element by element) and successors  -> the held answer
   else summaries for the changed blocks only, then the strongly connected
   components in the order the facts flow:
     a component with a changed block, or fed by a component whose value
     changed, is solved again from the problem's start; the rest keep theirs
```

A component is solved from the problem's start (nothing live; every root
zero), never from the held answer: what is live round a loop may be live
only because of a read that is gone, and a root zero round a loop only
because of a write that is gone, and a solve from the old answer keeps both.

A block is the same while its instructions are the same allocations
(`Insns::same_insns`), so a pass that rebuilds a block it left as it was
costs nothing. The answer says which blocks to work out again for what is
derived from it per instruction (`Solution::refreshed`).

## Example
No example: the answers are printed by no dump. See
[`crate::backend::liveness`](liveness.md) for lanes, [`crate::backend::upperzero`](upperzero.md) for
roots and [`crate::backend::peephole`](peephole.md)'s `_flags_live_out` for flags.

## Used by
`liveness`, `upperzero` and `peephole` (flag liveness).
