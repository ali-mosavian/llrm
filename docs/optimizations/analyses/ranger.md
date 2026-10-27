> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ranger.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ranger.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ranger.rs sha256:b4f04e7494f2179d1d4c0eb54d17ec0d432b334a1d075a9248f1021374f060cb -->

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

The walk up the blocks stops at the definition and does not start for a
block the definition does not dominate (nothing above holds the value), and
each block it passes keeps the answer, so a value is walked to once from
each block that asks, not once per question.

## Example
`if x < 5 { y = x + 1 }`:

```text
  b0:  %c = icmp slt %x, 5        b1 (taken):     %x in [lo, 4]   %y in [lo+1, 5]
       br %c, b1, b2              b2 (not taken): %x in [5, hi]
                                  b3 (join):      nothing known of %x
```

From unit test `a_guard_bounds_the_value_on_its_edge_and_not_at_the_join` in
`ranger_tests.rs`.

## Provides
`EdgeRanger::at`.

## Needs
[`crate::ranges`](ranges.md) (the transfer functions), [`crate::cfg`](cfg.md).

## See also
GCC `gimple-range-cache.cc`; LLVM `LazyValueInfo::getValueInBlock`.
