> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ranges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ranges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ranges.rs sha256:dc13702c5dfff08914df310b45eb200f5691191947123983ec8b97427915227d -->

# ranges

Bounds each integer value in each block by a non-wrapping interval, from the
branch edges that dominate the block and the counted loops that hold it. A
bounds check or overlap test that the interval decides can then go.

## How it works
An `Interval` is `[low, high]` over mathematical integers at a bit width,
signed. A value's interval is the intersection of what several sources give.

Operands. `_operand` reads a constant's, an entry in the scope's
`Intervals`, a parameter's `range` attribute or a `!range` on a load or call
(`declared`), or a single known value from [`crate::consts`](consts.md). `_computed`
gives what an instruction computes from its operands' intervals (`add`,
`sub`, `mul`, `shl`, `and`, `sext`, `zext`, fixed-point multiply), and no
interval where the result could wrap.

Edges. `dominated_edges` walks the dominator tree. A conditional `br` on an
`icmp` narrows the compared values on each edge (`on_edge`): signed
predicates move bounds, `eq` intersects, `ne` trims an end when the other
side is a point, unsigned ones only decide whether the edge is possible, and
`assume` calls add facts. `_refine_through` pushes a bound back through the
`add` or `sub` that made the value. An edge counts only when its destination
has that one predecessor, because a join is another way around the check; a
block starts from its sole predecessor's facts narrowed by the edge, or else
its immediate dominator's. An edge no execution takes makes the block
unreachable. `EdgeStates` keeps each block's state so a change re-solves
only the blocks it reaches (`edges_solved`).

Loops. `bounded_solved` handles each counted loop (`induction`'s trip
counts) outer first. A counter that steps by a constant from a known start
gets the interval its trip count allows (`_recurrence_span` refuses
wrapping), a header that tests before the trip also sees the exit value, and
a counter with an `nsw` step never goes back past its start. The loop's
operations are then swept to a fixed point in the scope of each block,
narrowed by the edges inside the loop and above its header. Header phis with
no counted proof that stay inside a box `[-2^k, 2^k)` on every entry and
latch are given the box, grown by powers of two until the latches stay in
it. Two loops whose facts contradict at a block make it `unreachable`. Where
a nest's work (blocks times instructions, summed over its loops) is above
`CAP_WORK`, the loops with two or more around them get no box search and the
facts of their header alone in their blocks (`capped_from`): a block's scope
was made once per loop around it, each walking the dominator chain to the
block. `Bounds` records the decision, and an update works from a solve made
under the same one. `LLRM_CHECK_CAP` holds a capped solve to containing the
uncapped facts.
`Bounds` holds the result: facts from the loops (`within`) over those from
the edges. A change re-solves only the loops it reaches (`loops_reached`). A
loop with no counted proof and no phi to box (`box_candidates`) is left
before its operations are listed: most loops of a large body are. The boxes
are found by asking one value at one block (`ranger::LoopView`, which keeps
an answer until a box it read changes), not by sweeping the loop for each
latch.

```text
  entry:  ...
  b1:     %c = icmp slt %i, 10         b2 (taken):    %i in [lo, 9]
          br %c, b2, b3                b3 (not taken): %i in [10, hi]
  loop:   %i = phi [0, pre], [%n, latch]; %n = add nsw %i, 1
          counted trips 10    ->  %i in [0, 9] in the body, [0, 10] at exit
```

Queries. `scoped` is every block's intervals (loops narrowed by edges);
`operand_at` is one operand's at one block without copying the rest.
`covering` widens an indexed access into its own object to the byte interval
it can touch; `inside_object` says that interval lies inside the object.
`exact_offsets` finds index values whose every access sums, at any wider
width, to the same bytes, because every partial sum is a non-negative
integer below `2**index_bits`; it is used where the object's own address is
the only root known to start at 0.

## Example
`for i = 0 while i < 10: if i < 4: off = i * 2`:

```mir
define void @f() {
b0:
  br label %b10

b10:
  %i = phi i16 [ 0, %b0 ], [ %next, %b40 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b20, label %b50

b20:
  %g = icmp slt i16 %i, 4
  br i1 %g, label %b30, label %b40

b30:
  %off = mul i16 %i, 2
  br label %b40

b40:
  %next = add i16 %i, 1
  br label %b10

b50:
  ret void
}
```

`bounded` gives:

| block | `%i`        | `%off`   |
|-------|-------------|----------|
| `b30` | [0, 3]      | [0, 6]   |
| `b40` | [0, 9]      |          |
| `b50` | [10, 32767] |          |

The guard bounds `%off` to 0..6 in `b30` without leaking into the join
`b40`.

From unit test `test_guard_refines_subscript_without_leaking_to_the_join` in
`ranges_tests.rs`.

## Provides
The interval facts above. In the manager they are `DominatedEdges` and
`Bounded` (both incremental) and `Ranges`, which bundles them with registers
for a pass; `Bounded` is held only when `Outer::loop_intervals` is on (-O2
and up). Consumers: [`crate::alias`](alias.md), [`crate::regions`](regions.md), [`crate::guards`](guards.md),
[`crate::memoryssa`](memoryssa.md), [`crate::avail`](avail.md), [`crate::parameter_ranges`](parameter_ranges.md), and in
llrm-transforms `algebraic`, `hoist`, `loopexit`, `indvars`, `decide`,
`fixednarrow`, `ports`, `interprocedural`.

## Needs
[`crate::consts`](consts.md) (`Known`, the registers), [`crate::induction`](induction.md) (counted
loops), [`crate::assumptions`](assumptions.md), [`crate::cfg`](cfg.md) (shape and dominance).

## See also
[`crate::ranger`](ranger.md), the edges' facts of one value at one block on request,
checked against `edges_solved` under `LLRM_CHECK_REPLAY`.
GCC's ranger (`gimple-range-cache.cc`, `ranger_cache::range_on_entry`);
LLVM's LazyValueInfo and ScalarEvolution value ranges.
