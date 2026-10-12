> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/graph.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/graph.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/graph.rs sha256:bd91046f2582b2456a92ee4d1bb8ed08b52344308d08af806b9233951598e621 -->

# graph

Block-graph algorithms (loops, depth, reducibility, dominators) over any IR
whose blocks implement `loops::Node`.

## How it works
The module holds one submodule, `loops`. A `Node` is a block id and its
successor ids; nothing here knows an IR. The rich MIR adapts a `Function`
through [`crate::cfg`](cfg.md), which turns it into `Block`s in layout order.

```text
 Function --cfg::graph--> [Block{at,succ}] --> graph::loops
                                                 (Loop, depth, ...)
```

The backend's `LirBlock` has its own copy of these walks in
`crates/backend/llrm-core/src/analysis/loops.rs`; both build on
`llrm_support::graph`.

## Provides
The algorithms of `loops`. Consumers: [`crate::cfg`](cfg.md) (`Loop`, `Node`,
`irreducible_under`) and the loop passes of `llrm-transforms`, which read
`Loop` out of [`crate::cfg::Shape`](cfg.md).
