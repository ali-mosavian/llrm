> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ranges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ranges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ranges.rs sha256:9619b2fffbdd3ff7d673ffc960303348c863cefc39ca7d3c19b40a8bd509ef82 -->

# ranges

Non-wrapping integer intervals per block, from the branch edges that
dominate a block and the counted loops that hold it.

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
it. Two loops whose facts contradict at a block make it `unreachable`.
`Bounds` holds the result: facts from the loops (`within`) over those from
the edges. A change re-solves only the loops it reaches (`loops_reached`).

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
GCC's ranger (`gimple-range-cache.cc`, `ranger_cache::range_on_entry`);
LLVM's LazyValueInfo and ScalarEvolution value ranges.
