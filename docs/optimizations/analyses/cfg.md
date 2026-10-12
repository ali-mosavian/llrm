> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/cfg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/cfg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/cfg.rs sha256:1fba079736f2af01e44a2c25a30cbd0f856f045c98cba6a06af8d283236c89a2 -->

# cfg

A function's blocks as the graph walks see them: block ids, successors,
dominance, natural loops and reachability, built once per body.

## How it works
`graph` turns a `Function` into `Block { at, succ }`s, one per block in
layout order, the entry first, so the walks of [`crate::graph::loops`](graph.md) run
on the rich MIR. `successors_of` and `predecessors_of` answer for one block
without building the whole graph, and the `Around` trait does the same for
a `Loop`: `entering` (outside blocks that name the header), `exits` (edges
leaving the body) and `preheader` (the single entering block whose only
successor is the header). Building the graph per loop made a function of N
loops cost N^2.

`Shape` bundles what the passes ask of a body:

```text
 Shape
  +- dominance: Dominance   over llrm_mir Dominators; reachable, dominates,
  |                         immediate, depths, frontiers, irreducible
  +- loops: Vec<Loop>       from llrm_mir Loops, ids not BlockIds,
  |                         inner loops first
  +- reaching: Reaching     blocks with a path to a block, found on ask
                            and cached
```

`Dominance` treats an unreachable block as dominated by nothing and
dominating nothing. `dominates` and `depths` answer without building each
block's full dominator set (`dominators`, quadratic in a chain of
diamonds). `natural` orders loops by body size, inner first.
`Reaching::to` walks predecessors from the target and returns the cached
set on a second ask.

`_unreachable` deletes blocks the entry does not reach, trims phi inputs of
removed edges, and replaces uses of values defined in dead blocks with
poison before erasing them.

## Provides
`Shape` (a `Analysis` named `shape`), the
`Dominance` and `Reaching` types, and the id/graph helpers. Consumers:
nearly every loop pass and analysis in `llrm-transforms` and this crate,
and the backend driver and isel (grep `cfg::Shape`, `cfg::id`).

## Needs
llrm-mir's `Dominators` and `Loops`.

## Keeps
`Shape` is preserved when it is kept, or when both `Dominators` and `Loops`
are. It does not read the outer (module) analyses.
