> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ranges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ranges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ranges.rs sha256:02ca309b55be1bba2688f2817442b4e344f8c01f79474239b37ff5669a3ed284 -->

# ranges

Bounds each integer value in each block by a non-wrapping interval, from the
branch edges that dominate the block. A bounds check or overlap test that
the interval decides can then go.

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

```text
  b1:     %c = icmp slt %i, 10         b2 (taken):    %i in [lo, 9]
          br %c, b2, b3                b3 (not taken): %i in [10, hi]
```

Queries. `covering` widens an indexed access into its own object to the byte
interval it can touch; `inside_object` says that interval lies inside the
object.

## Example
`if x < 4 { if x >= 0 { y = x * 2 } }`:

```mir
define void @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 4
  br i1 %c, label %b1, label %b2

b1:
  %lo = icmp sge i16 %x, 0
  br i1 %lo, label %b3, label %b2

b3:
  %y = mul i16 %x, 2
  ret void

b2:
  ret void
}
```

`dominated_edges` gives `%x` in [0, 3] and `%y` in [0, 6] at `b3`, and
nothing at the join `b2`.

From unit test
`a_guarded_block_bounds_what_it_computes_from_the_guarded_value`
in `ranges_tests.rs`.

## Provides
The interval facts above. In the manager they are `DominatedEdges`
(incremental) and `Ranges`, which bundles them with registers for a pass.
Consumers: [`crate::alias`](alias.md), [`crate::regions`](regions.md), [`crate::guards`](guards.md),
[`crate::memoryssa`](memoryssa.md), [`crate::avail`](avail.md), and in llrm-transforms
`fixednarrow`.

## Needs
[`crate::consts`](consts.md) (`Known`, the registers), [`crate::assumptions`](assumptions.md),
[`crate::cfg`](cfg.md) (shape and dominance).

## See also
[`crate::ranger`](ranger.md), the edges' facts of one value at one block on request,
checked against `edges_solved` under `LLRM_CHECK_REPLAY`.
GCC's ranger (`gimple-range-cache.cc`, `ranger_cache::range_on_entry`);
LLVM's LazyValueInfo.
