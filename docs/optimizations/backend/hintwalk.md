> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/hintwalk.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/hintwalk.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/hintwalk.rs sha256:a96f92f9c58b9fa02bf2bc0cff0e7da82422b36477432cda3d53f02a26be0a00 -->

# hintwalk

The register each value is wanted in because of the copies that lead to it
from a pinned value, kept up to date by what a rewrite names.

## How it works
`hints` says, for a value, the values it is copied to or from; a pinned
value (an argument, a return) has its register fixed. A value wants the
register of the pinned value that reaches it through the fewest copies, up
to `depth` of them, and where two reach it equally far the one of the
smaller value number: a property of the graph, not of the order a walk
happened to take, so the walk made afresh and the walk kept up to date are
the same map.

A rewrite names the values whose instructions it changed (`touched`); only
those have a hint list that can differ. The walk keeps for each value it
has reached the distance, the value it came through (`via`) and the values
that came through it, and for each value that is walked its hint list and
the walked values that name it. A changed list gives the values it names a
new set of ways. A value whose way is gone is marked, with what came
through it, and found again from the ways that are held, nearest first and
the smaller value among equals (Ramalingam and Reps's update of a shortest
path tree); a shorter way offers itself to what it names. A value found
again keeps its place among what came through it, so a chain of copies that
a rewrite lengthens by one is relabelled in place, not walked and entered
again. What is not reached after that is taken out.

```text
  pin A ─ a1 ─ a2 ─ a3 ─ a4        a rewrite names a2, giving it a copy t:
  pin A ─ a1 ─ a2 ─ t ─ a3 ─ a4    a3 and a4 are marked, found again through t
```

`LLRM_CHECK_WANTED=1` makes the walk afresh after every update and fails
where the two differ.

## Example
Pinned `10` is `ax`; `10` hints `1`, and `1` hints `2` and `3`; `2` hints
`4`:

| value | way | wants |
|---|---|---|
| 1 | 10 | ax |
| 2, 3 | 1 | ax |
| 4 | 2 | ax |

A rewrite that names `2` and gives it no hint leaves `4` without a way, and
`1`, `2` and `3` as they were.

From unit test `a_rewrite_that_names_a_value_changes_what_comes_through_it`
in `hintwalk_tests.rs`.

## Provides
`HintWalk`: `fresh`, `update`, `wanted`.

## See also
[`crate::backend::allocate`](allocate.md), which asks `wanted` of each value it places.
