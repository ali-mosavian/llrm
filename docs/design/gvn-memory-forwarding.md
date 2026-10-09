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

The saving is net. Profile of the same compile after the switch (samples):
`forwarded` falls from 6.1% to 2.9% of the compile, the walks are 2.1%,
`memory_providers` 0.65%, `covered` 0.8% (dse also calls it), MemorySSA build
0.8%. Samples fall more than instructions (-1.9%): the dataflow's maps were
cache-missing allocation. The -O1 row (0.9989) is inside the method's spread;
gvn is off there.

With the prototype and `LLRM_CHECK_JUMPS=1 LLRM_CHECK_CLOBBERS=1`, the gate's
`build`, `integration`, `bench`, `run` (the programs executed), `qcport` and
`pytest-programs` steps pass.

The walk subsumes the dataflow on 517 of 524 objects by bytes, which is not
yet a statement about loads (see Check mode).

## Loads the dataflow serves and the walk does not

Missed optimisations, no miscompile found (review of avail.rs and
memoryssa.rs):

- (a) A join where both arms store the same operand, and the load is below the
  join: `meet` keeps the cell; the walk finds two clobbers, and the store path
  needs one. `loadjoins` rewrites only a load in the join block.
- (b) Alias precision from register ranges: the dataflow asks `may_clobber`
  with `known` (`ranges::constants`), the walk with `None`. Stores to `a[i]`
  with `i` known to miss the cell kill under the walk and not the dataflow.
  Fix: give the walk the same `known`.
- (c) Invariant loads after an initialising store: the walk skips every def for
  an invariant load, so it never returns the store. The dataflow forwards the
  stored value.

Each gets a regression test before the flip.

## Proposal

Not a new numbering engine. Make the walk the only load forwarder and delete
the dataflow.

1. **Switch.** `Options::gvn_dataflow` (default on), `-fgvn-dataflow`. Off: all
   wanted loads go to `memory_providers`, `holders` is not computed. The walk's
   result does not depend on `avoid_store_crossing` (`Crossings` filters after
   it), so `forwarded` caches the `Vec<Forward>` in the `OnceCell` that holds
   `Held` today, and the two numberings of a crossing function walk once.
2. **Check mode.** `LLRM_CHECK_GVN=1` runs both on the same input and counts,
   per load: served by both with the same provider, by both with different
   providers (after `ssa::provider` resolution), by the dataflow only, by the
   walk only, with the class of each dataflow-only load ((a)-(c) or other).
   Corpus totals are per load, not per object: a lost forward can hide behind
   a gained one. Soundness is not checked by comparison; it rests on
   `LLRM_CHECK_JUMPS=1 LLRM_CHECK_CLOBBERS=1` over the corpus with the switch
   off, and on executing the programs and QCport modules (the gate's `run`
   and `qcport` steps) with it off; mdl_ai, menu and pak are run explicitly.
3. **Speed the walk, before the flip.** The only super-linear risk the switch
   adds is `memory_providers`' scan of every load group with `same_bytes` per
   load (quadratic in distinct cells, counted by `same_runs`), each pair two
   `covered` calls. Key the groups by a canonical (base, offset, width) as
   sccvn's reference table does ((vuse, ref) pairs hashed, one lookup each),
   keep `covered` for the partial cases only, and add a per-load query budget
   (GCC: `sccvn-max-alias-queries-per-access`). Add a scaling axis that grows
   distinct cells per function.
4. **Decide.** With the switch off, run the bench, the quality ratchet and the
   clocks, plus the check-mode totals. A loss is a load class the walk cannot
   see: fix the walk (never put the dataflow back) or accept it with numbers.
5. **Flip the default, then delete** `Holders`, `Held`, `holders`, `after`,
   `meet`, `provider`, `forwardable_by`'s dataflow half and the
   `LLRM_CHECK_HOLDERS` mode (`avail.rs`, about 290 of its 823 lines; the
   dead-store solve stays), and their tests (`avail_tests.rs` uses `provider`
   and `holders`; port the cases worth keeping onto the walk). `CellMap` stays:
   `consts` uses it. `gvn_tests.rs`' `avail::solved()` test becomes "the walk
   runs once per numbering pair".

## What it keeps

`subexpressions` (nearest dominating leader, `avoid_store_crossing`), `joined`
and `loadjoins` (PRE), `propagated`, and the pricing of a load served across a
store (`motion_price`, skipped when the crossing numbering fits its registers,
#1132). Pipeline position and the fixed point are unchanged.

## Facts reused

`Accesses` and `MemorySSA` (`built`, `clobbers`, `unchanged`, the jump table),
`Reach` (#1145), `Shape`/dominators for `available`, `Registers`/`Known`,
`profit::spill_forecast`. New: the canonical group key.

## What changes output

Dataflow-only and walk-only loads, and loads with a different provider (the
walk takes the store or the earliest dominating load, the dataflow the first
cell held), so `Crossings` decisions can differ. Prototype: 7 objects of 524,
none larger. The flip waits on the check-mode totals, the bench and the
ratchet.

## Predicted cost

Measured with the prototype: -2.5% of the QCport -O2/-Os compile in
instructions (gvn 8.6% -> about 6.2%), more in time. Step 3 predicts a further
-0.3..-0.6%, sized on the post-switch profile above. -O1 is unchanged.

## Why not sccvn's whole shape (scoped tables, one reverse-postorder walk)

Scalar numbering is 3.5% of gvn and already a dominance-checked hash table
(`subexpressions`). The cost sccvn avoids is the per-block memory map, and this
removes it; GCC's PRE sets (AVAIL_OUT, ANTIC_IN) are per-block sets of value
numbers with memory only through vuse-keyed numbers, no kill sets, so they
correspond to `subexpressions` and `loadjoins`, not to `holders`. What remains
in gvn after the switch is shared facts (Accesses, MemorySSA, pricing) that a
rewrite would still need. Step 3 converges on sccvn's reference table.

## Risks

- A latent bug in the walk becomes much more exposed: it goes from a fallback
  for a few loads to serving every load. The check envs and execution runs in
  step 2 are for this.
- Loop-carried values: `memory_providers` already serves them (the dataflow
  starts every block empty and loses them at a loop header).
- Several stores covering one load: served by neither mechanism today (both
  need `same_bytes`); sccvn combines partial defs. Out of scope.
