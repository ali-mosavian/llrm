> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ranges_scopes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ranges_scopes.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ranges_scopes.rs sha256:711cbf05e0980b3c07692fb58f73b64e5a3ed51bf993ce244836e533000c5c98 -->

# ranges_scopes

What `Bounds` knows at a block of a counted loop, made from the loop's
`known` by the edges that narrow it there.

## How it works
A loop's `known` holds what holds throughout it: its counters' trip
intervals, the boxes of its phis, and what its operations compute of them.
The scope at a block is `known` narrowed by the branch edges on the
dominator chain of the block, inside the loop and above its header, and by
the assumes above the block, then swept again for the operations those
reach (`LoopScope::scope_at`). The edge chains of the blocks of a loop share
their prefixes (`Prefixes`), and the swept `known` is kept (`Caches`).

The solve and the consumer that asks later make a scope with the same code:
the solve keeps the loop (`ranges::Lazy`) with its `Caches`, and
`Bounds::force` makes the scope of a block when it is first asked, from the
unit the consumer asks with.

## Example
A loop of three blocks, `b1` (header), `b2` and `b3`, where `b2` is reached
from `b1` on `i < 5` and `known` holds `i` in `[0, 9]`: the scope at `b2`
is `known` with `i` in `[0, 4]`, the operations of the loop worked again
from that; the scope at `b3` is `known`.

From unit test `a_scope_made_when_asked_is_the_scope_the_solve_makes` in
`manager_tests.rs`.

## Provides
`LoopScope::scope_at`, `Solve`, `Caches`.

## Used by
[`crate::ranges`](ranges.md): the solve of the loops (`bounded_solved`) and
`Bounds::force`.

## Needs
`crate::edge_delta` and [`crate::consts`](consts.md) for the
intervals an edge and an operation give.
