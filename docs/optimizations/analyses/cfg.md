> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/cfg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/cfg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/cfg.rs sha256:265753edf39b773029e30acee0e704c717b103c528b63114594bc137c22481ae -->

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

## Example
A `switch` names its default and a case target `%b2` twice, and `dead` is
not reachable:

```mir
define void @f(i16 %s) {
b1:
  switch i16 %s, label %b2 [ i16 1, label %b2
                            i16 2, label %b3 ]

b2:
  ret void

b3:
  ret void

dead:
  br label %b2
}
```

`graph(function)` lists the blocks in layout order, each target once, the
unreachable block kept:

| `at` | `succ` |
| --- | --- |
| `b1` | `b2`, `b3` |
| `b2` | none |
| `b3` | none |
| `dead` | `b2` |

From unit test
`a_switch_names_each_target_once_and_an_unreachable_block_stays_in_the_graph`
in `cfg.rs`.

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
