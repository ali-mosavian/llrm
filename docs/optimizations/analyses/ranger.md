> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ranger.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ranger.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ranger.rs sha256:dbd9a7cd6bfc87f80fb41f5e0050cdafe8f6a91f36d968660aae7bb9a98e4cbd -->

# ranger

The bounds the branch edges give a value at a block, worked out when asked
and kept by (value, block), in place of a table of every value at every
block (`ranges::edges_solved`).

## How it works
GCC's ranger asks for a name's range on entry to a block and walks the
dominating edges back to its definition, filling a cache on the way
(`ranger_cache::range_on_entry`). The same here: a value's interval at a
block is what the block's own instructions compute of it, else what the
block starts from (its sole predecessor's, narrowed by the edge; or its
immediate dominator's), else what the program declares of it. Nothing is
solved for a value nobody asks of, and an answer is the one the table
gives: `LLRM_CHECK_REPLAY` asks every value at every block both ways.

A loop's blocks are asked the same way (`LoopView`): the facts the edges and
assumes above a block give, and the loop's operations worked from them, one
value at a block. An answer keeps the boxed values it read, so the next
round of `ranges::inductive_boxes` works out only what a changed box
reaches.

## Provides
`EdgeRanger::at`, `LoopView::at`.

## Needs
[`crate::ranges`](ranges.md) (the transfer functions), [`crate::cfg`](cfg.md).

## See also
GCC `gimple-range-cache.cc`; LLVM `LazyValueInfo::getValueInBlock`.
