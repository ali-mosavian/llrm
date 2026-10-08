# Instruction identity that survives insertion

Status: design, no code. Owner: tiers. Review before the first PR.

## Why

A spill inserts reloads and stores. Every fact the allocator keeps about the body is written in positions (slot numbers,
`(block, index)` pairs), and an insertion renumbers every later position, so after each of the ~1100 spills of `d_faces` the
backend rebuilds or remaps each fact over the whole body. Measured (frame-pointer profile, -O2, 4222 samples): that
per-spill work is 42% of `d_faces`, 33% of `savegame`, 26% of `d_alias`, 12% of QCport's instructions; no leaf is above 2.3%, so no
local fix helps (#999 took siblings, 6%, by reading occurrences; the rest has the same cause). The missing mechanism is a numbering
that insertion does not disturb, and facts that are updated for what changed.

## (a) LLVM, from the source (llvm-project 084a4e6)

- **SlotIndexes** (`include/llvm/CodeGen/SlotIndexes.h`, `lib/CodeGen/SlotIndexes.cpp`). Every instruction owns a list entry
  numbered `InstrDist = 4 * Slot_Count = 16` apart. A `SlotIndex` points at its entry, so it names the instruction for as long as the
  instruction lives, and its number only orders. `insertMachineInstrInMaps` takes the midpoint `((next - prev) / 2) & ~3`; when
  there is no room (`dist == 0`) `renumberIndexes` renumbers forward with spacing `InstrDist / 2` until it catches up, and
  `packIndexes` respaces everything when a renumbering touched more than 20% of the function. Existing segments hold
  `SlotIndex`es, so an insertion leaves every `LiveInterval` valid.
- **LiveRangeEdit** (`lib/CodeGen/LiveRangeEdit.cpp`). One edit owns the registers a spill or split creates (`createFrom`), reports
  to a `Delegate`, and updates only what it touched: `eliminateDeadDefs` shrinks the intervals of the registers whose defs it
  removed, new instructions enter the maps with `LiveIntervals::InsertMachineInstrInMaps`.
- **LiveStacks** (`include/llvm/CodeGen/LiveStacks.h`). `slot -> LiveInterval`, made when `InlineSpiller::spillAll` commits to a
  spill (`InlineSpiller.cpp:1412-1426`: `getOrCreateInterval`, `MergeSegmentsInAsValue(LIS.getInterval(Reg))`), read by
  `StackSlotColoring` (`StackSlotColoring.cpp:226-320`: `LS->getInterval(FI)`, `overlaps` against each colour). Nothing is walked again.
  (LLVM's stack interval is the union of the spilled registers' live ranges; ours is access-based, below. We keep ours: output must
  not change.)

## (b) What holds positions here (checked against the code by the review, 2026-10-09)

| holder | file | shape today | what a spill costs |
|---|---|---|---|
| change discovery | `postings.rs` `following`/`current`, `intervals.rs` `indexed_shared`, `Remembered::is_of` (up to 6 per ask), `updated` (two hashed passes) | every cache re-derives "what changed" by comparing the whole body by `Arc::ptr_eq`; the rewriters already know (`materialized`'s `named`, splitkit's `Moved`) and throw it away | O(body) per cache per ask: the floor under everything else |
| numbering | `intervals.rs` `Indexes` | `at: IndexMap<insn address, slot>`; dense: 2 slots per instruction, +2 per block head (phi), meta instructions take none; rebuilt by `indexed` | O(body), hashing |
| intervals | same, `Interval`/`Segment`, `Remembered`, `updated` | segments in dense slots; `updated` maps every untouched interval old to new; `Remembered::of` deep-clones all intervals per ask; gives up on any CFG change | O(values x segments) |
| weights | `allocate.rs` 808-821, 2137, 2337 | four stages: base `total/(size+GRACE)`, sibling discount, fold discount, INF; `_totals` and the sibling sums add frequencies in body order (floats) | follows the remap |
| postings | `postings.rs` | `At = (block, index)`; `replaced` redoes a changed block | O(body) compare + O(block) |
| homes | `spiller.rs` `_existing_colors_by`, `_short_update_runs_whole`, `_local_updates_whole`, `slots.rs`; `Frame` | homes as pseudo-values, liveness over a sparse body of every block; the helpers number and interval their own intermediate bodies | O(blocks) per spill |
| facts | `allocate.rs` `Facts::of` | live, masks (clobber slots), widths, hints, sibling prices, fold prices, classes: all values, every rewrite; plus `constrain::required`, the after-queue loop and `_overlapping` over all values | O(values) |
| splitkit | `splitkit.rs` | `Region` spans, `index.slot`, `window_end == slot(next)`; split appends bridge blocks and rewrites `succ`/`phis` inside one allocation | positions + CFG change |
| holders | `liveunion.rs` | segments per register; `refresh()` drops the indexes | rebuilt lazily |
| frequency | `analysis/frequency.rs` | `Frequency::of(body)` per rewrite | O(body) |

Not affected: coalesce, ssaspill and peephole number their own bodies outside the allocation loop.

An instruction already has an identity: the address of its `Arc<Insn>` (`key`), kept alive by `Remembered`/`Followed`. The invariant
the design relies on, which the spiller and splitkit obey (`_renamed` -> `_with` makes a new `Arc`): a kept `Arc` keeps its order
relative to the other kept ones, an `Arc` appears once, a replacement is a new `Arc`. `Slots` holds the `Arc`s (so an address is not
reused) and asserts the invariant under the check switch.

## Design

0. **`Edit`** (LiveRangeEdit): what a rewrite did to the body, returned by every rewriter and consumed by every fact: blocks
   changed, instructions removed / inserted / replaced (with their neighbours), blocks appended, values touched. The `Arc::ptr_eq` diff
   stays only as the check (`LLRM_CHECK_EDIT`: the edit equals the diff). One type and one check switch, not one per fact. A rewrite
   that cannot say what it did returns `Edit::whole()`, and every consumer falls back to what it does now.
1. **`Slots`**: one persistent numbering per allocation, owned by the allocation state (the thread-local caches keyed on a body scan
   go). Key: instruction identity. Value: a gapped number (spacing `DIST`, midpoint insertion, local renumber, repack at 20%, as
   LLVM). `Slots::apply(&Edit)` inserts, removes and appends blocks; cost O(edit).
2. **A written point contract**: each instruction owns a *read* point and a *def* point; a block owns a head point (phi results) and an
   end; a derived `end` of a dead def is the next entry's read point. A meta instruction owns none (and an operand added by a rewrite
   can flip meta-ness: that is an insert or a remove in `Slots`, not a no-op). One function `dense(point)` is the only bridge to
   the old numbering; nothing else does arithmetic on points (the existing test `test_nothing_outside_the_numbering_does_arithmetic_on_slots`
   is extended to cover it).
3. **Rank, for byte-identical output.** `Interval::size` counts dense slots, and weights, spill choice and queue order read it. With
   gaps `end - start` is not a count, so `Slots` answers `rank(point)` (dense number) in O(log n) with an order-statistic tree (a
   balanced tree with subtree counts; a Fenwick tree cannot take a middle insertion). `size` and every position-derived length go
   through it.
4. **Weight has one owner**: the four-stage composition (base, sibling, fold, INF) moves into one accessor over `Slots`; "weights on
   read" never leaves two places that compose them. Sums are recomputed for each touched entry from its occurrences **in body
   order**, never by adding or subtracting deltas (floating-point sums; otherwise not byte-identical).
5. **Intervals** keep their segments across an edit; only touched values and values live across an edited block are recomputed. A CFG
   change (split bridge blocks) is an `Edit` with appended blocks; if the interval update cannot handle it, it falls back to a rebuild
   for that rewrite. How often splits do that is measured before PR 4 (`updated` gives up on any block change today).
6. **Postings** hold `InsnId` (the entry); block and index come from `Slots`.
7. **Homes (LiveStacks analogue)** belong to the allocation's `Slots` state, not to `Frame` (shared across the base and trial
   allocations and restored by snapshot). A home's interval is not "extended at each access": it is live through the blocks between a
   store and a load and cleanup removes accesses, so it is recomputed for the homes the edit touches. The spiller helpers that number
   their own intermediate bodies (`_short_update_runs_whole`, `_local_updates_whole`, `_existing_colors_by`) take `&Slots`/`&Edit`; until
   they do, the old caches survive beside the new owner, so this is part of the PR, not a follow-up.
8. **Facts** keep their per-value entries (`Facts::updated(previous, &Edit)`); `Masks`, `constrain::required`, the after-queue loop and
   `_overlapping` work on the touched values.
9. **LiveUnion** is not refreshed: touched values leave and re-enter.

Each step compares against the old whole computation under `LLRM_CHECK_*`, over the 393-object corpus (66 programs + 65 QCport modules
x 3 levels) and the unit tests.

## (c) PR series (byte-identical; each PR's own check, fail-first test, and `measure` drop)

0. `Edit`, produced by the spiller and splitkit rewriters; consumed by `postings`, `Remembered`, `indexed_shared` (O(edit) change
   discovery instead of O(body) `Arc` comparisons). Byte-identical by construction. Pays on its own.
1. `Facts::updated` for the fields that carry no position (widths, hints, sibling and fold prices, classes), fed by `Edit`.
2. `Slots` + rank + the point contract behind `indexed()`: same answers. Check: `dense(p)` equals the old numbering for every point
   kind (read, def, block head, derived end, meta) over the corpus; unit tests for insertion, local renumber, repack, meta flip.
3. Intervals without the remap, weights through the single accessor.
4. Masks, `LiveUnion` without `refresh()`, `constrain::required` and the queue loops on touched values.
5. Homes in `Slots`, the spiller helpers threaded.

## (d) Expected drop (d_faces share of the compile; profile: `perf record -F 400 --call-graph fp` of `llrm-c -O2` on QCport
`render/d_faces.c`, binary built with `-C force-frame-pointers=yes` at main d78fb2434; flat, top leaf 2.3%; re-measured per PR)

| PR | removes | d_faces | QCport total |
|---|---|---|---|
| 0 | O(body) change discovery: `postings::following` 2.0, `indexed_shared` 0.8, `Remembered::is_of`, `updated`'s passes (~3) | -4 | -1.3 |
| 1 | non-positional part of `Facts::of` (widths 1.2 + sibling/fold prices + classes + hints, ~5 of 8.3) | -4 | -1.3 |
| 2 | infrastructure | -0 | 0 |
| 3 | `intervals_over` + `updated` remap + `Remembered::of` clones (9.4 -> ~3) | -6 | -2 |
| 4 | masks, `refresh`, `_overlapping`, `constrain::required`, queue loops (~4 -> ~1) | -3 | -1 |
| 5 | `_existing_colors_by` 9.0 -> ~3 (home liveness is recomputed for touched homes, not free) | -6 | -2 |
| | | about -23% | about -8% |

Stop rule: PR 2 is not judged alone (it enables 3-5); PRs 0, 1, 3, 4, 5 are judged against their own row, and the series stops where a
measured drop is under half its estimate. The `live` axis should fall from 3.5 toward ~2.2 (#981).

## Alternatives considered

- Edit log + `Arc`-shared blocks (`Arc<[Arc<Insn>]>`): identity checks and body clones O(blocks). This is PR 0 plus a cheaper body
  clone; it does not remove the interval remap, so it is a step of the series, not a replacement.
- Batching spills: changes the allocation order, not byte-identical. Rejected.
- Ids stored in `Insn`: copied by `_with`/clone, so a replacement would keep the old id. Rejected; `Arc` address is the key.
- An instruction arena: too invasive for the gain.
- Per-block dense numbering with a tree over block bases: parity by construction, but breaks `LiveUnion`'s global keys. Gapped
  numbering with rank is the choice for intervals and the union.

## Risks

- Rank/size parity (3): if it cannot be made exact the output changes, and the series is a quality change to be measured as one.
- A pass that rebuilds every `Arc` forces a full renumber (cost as now).
- Blocks that change order between phases get a fresh `Slots`: the numbering is per allocation.
