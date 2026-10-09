# gvn: forward loads by MemorySSA walk, drop the availability dataflow

Status: proposal. Issue #1143.

## Where gvn's time goes

gvn is the largest row of the QCport -O2 compile: 8.6% of 308 G instructions
(dist build). Profile of host.c at -O2 (gvn is 10% of samples), by call tree:

| piece | share of gvn | share of compile |
|---|---|---|
| `transform::forwarded` (load forwarding) | 60% | 6.1% |
| of which `avail::holders`/`after` (per-block available-cells maps) | 34% | 3.4% |
| of which MemorySSA walks and build | 17% | 1.7% |
| `subexpressions` (scalar numbering) | 3.5% | 0.35% |
| `joined` (scalar PRE) | 3.1% | 0.31% |
| spill pricing (`motion_price`) | 8% | 0.8% |

Scalar numbering and PRE are cheap. The cost is load forwarding, which runs
two mechanisms on the same loads:

1. a forward dataflow, `avail::holders`: the map of cells held and their
   holders on entry to and exit from every block, cloned at joins,
   intersected at meets and killed per store through the overlap-bucket index;
2. `avail::memory_providers`, for the loads the dataflow did not serve
   ("recover dominating memory values the forward lattice lost at loops"):
   a MemorySSA clobber walk to a single exact store, or to an earlier load of
   the same bytes with an unchanged memory state (`unchanged`), whose value
   dominates.

GCC's FRE (tree-ssa-sccvn.cc) has only the second kind: a load is looked up
through the virtual-use chain with the alias oracle
(`vn_reference_lookup`, `walk_non_aliased_vuses`). It keeps no per-block map
of memory.

## Finding

Prototype: every wanted load goes to `memory_providers`, and `holders` is not
computed. Nothing else changes (about 15 lines).

| | before | after |
|---|---|---|
| host.c -O2, `mir gvn` own | 2456 M | 2085 M (-15%) |
| host.c -O2, whole compile | 34.26 G | 33.63 G (-1.9%) |
| QCport instructions geomean -O1 / -O2 / -Os | | 0.9989 / 0.9751 / 0.9746 |
| programs instructions geomean -O1 / -O2 / -Os | | 0.9983 / 0.9879 / 0.9878 |
| worst row, any level | | 1.0017 (noise) |
| scaling axes (30 axes, 529 steps) | | no rise |
| objects differing, 131 files x 4 levels | | 7 of 524 (mdl_ai, menu, pak) |

Where the object differs it is smaller or equal: mdl_ai -O2 28005 -> 28002
bytes, -Os 26987 -> 26916, menu -O2 7296 -> 7277, -Os 6601 -> 6593, pak equal.
This is before any work on `memory_providers` itself, whose own cost on host.c
is 0.5% of the compile, 73% of that in `memoryssa::covered` (a `placed` per
pair).

So the walk subsumes the dataflow on 517 of 524 objects, and is dearer than it
needs to be only in `covered` and a quadratic scan of load groups.

## Proposal

Not a new numbering engine. Make the walk the only load forwarder and delete
the dataflow.

1. **Switch.** `Options::gvn_dataflow` (default on), `-fgvn-dataflow`. Off: all
   wanted loads go to `memory_providers`, `holders` is not computed.
2. **Check mode.** `LLRM_CHECK_GVN=1` runs both on the same input and prints,
   per function, the loads each serves and the other does not, with the cell
   and the reason (loop-carried, several stores covering the load, join with
   differing holders). Soundness asserts are the walk's own (`available`,
   `serves`, `unchanged`), already on the production path for the missing
   loads. The 7 differing objects are the first corpus for it.
3. **Decide.** With the switch off, run the bench, the quality ratchet and the
   clocks. A loss is a load class the walk cannot see; fix the walk (not the
   dataflow back in) or accept it with numbers.
4. **Flip the default, then delete** `avail::holders`, `after`, `meet`,
   `forwardable_by`'s dataflow half, `Held`, `provider` and the
   `LLRM_CHECK_HOLDERS` mode (`avail.rs`, about 700 of 823 lines). The `CellMap`
   stays: `consts` uses it.
5. **Then speed the walk.** `covered`/`placed` per pair; the groups scan in
   `memory_providers` (`same_bytes` against every group, quadratic in distinct
   cells, counted by `same_runs`).

## What it keeps

`subexpressions` (nearest dominating leader, `avoid_store_crossing`), `joined`
and `loadjoins` (PRE), `propagated`, and the pricing of a load served across a
store (`motion_price`, skipped when the crossing numbering fits its registers,
#1132). Pipeline position and the fixed point are unchanged.

## Facts reused

`Accesses` and `MemorySSA` (`built`, `clobbers`, `unchanged`, the jump table),
`Reach` (#1145), `Shape`/dominators for `available`, `Registers`/`Known`,
`profit::spill_forecast`. New: none.

## What changes output

Loads the dataflow served and the walk does not, and the reverse. Prototype:
7 objects of 524, none larger. The flip waits on the bench and the ratchet.

## Predicted cost

Measured with the prototype: -2.5% of the QCport -O2/-Os compile (gvn 8.6% ->
about 6.2%). Steps 4 and 5 predict a further -0.3..-0.6%: the 0.5% of
`memory_providers` own cost is mostly `covered`. -O1 is unchanged (gvn is off).

## Why not sccvn's whole shape (scoped tables, one reverse-postorder walk)

Scalar numbering is 3.5% of gvn and already a dominance-checked hash table
(`subexpressions`). The cost sccvn avoids is the per-block memory map, and this
removes it. What remains in gvn after the switch is shared facts (Accesses,
MemorySSA, pricing) that a scoped-table rewrite would still need. A rewrite
would be justified only if `subexpressions` or `joined` grow.

## Risks

- Loop-carried values: `memory_providers` already serves them; the check mode
  lists any the dataflow served that it does not.
- Several stores covering a load: `covered`; this is the cost and the
  remaining difference to look at.
- Worst files and super-linear growth: the scaling axes show no rise; the
  groups scan is the candidate to watch.
