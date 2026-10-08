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

## (b) What holds positions here

| holder | file | shape today | what a spill costs |
|---|---|---|---|
| numbering | `analysis/intervals.rs` `Indexes` | `at: IndexMap<insn address, slot>`, dense, 2 slots per instruction, rebuilt by `indexed` / `indexed_shared` (validated by `Arc::ptr_eq` over the body) | O(body), hashing |
| intervals | same, `Interval`/`Segment`, `Remembered`, `updated` | segments in dense slots; `updated` maps every untouched interval through the old-to-new slot map and recomputes the touched | O(values x segments) per rewrite |
| weights | same | `weight = total / (size + GRACE)`, `size` = slots covered | follows the remap |
| postings | `backend/postings.rs` | `At = (block, index)`; `replaced` redoes a changed block | O(block) per spill |
| homes' colours | `spiller.rs` `_existing_colors_by`, `slots.rs` | homes as pseudo-values, liveness over a sparse body of every block, `starts` slots | O(blocks) per spill |
| facts | `allocate.rs` `Facts::of` | live, masks, widths, hints, slots, sibling prices, classes: all values, every rewrite | O(values) |
| holders | `liveunion.rs` | segments per register; `refresh()` drops the indexes | rebuilt lazily |
| frequency | `analysis/frequency.rs` | `Frequency::of(body)` per rewrite | O(body) |

An instruction already has an identity: the address of its `Arc<Insn>` (`key(insn)`), kept alive by `Remembered`/`Followed`. What is
missing is a stable *number* and an *order structure* over those identities.

## Design

1. **`Slots`**: one persistent numbering per allocation, owned by the allocator state (not a thread-local keyed on a body scan). Key:
   instruction identity. Value: a gapped slot (spacing `DIST`, midpoint insertion, local renumber, repack at 20%, as LLVM).
   `Slots::apply(old_body -> new_body)` takes the changed blocks (the same `Arc::ptr_eq` diff `postings` does, O(changed blocks) when
   the caller says which blocks it rewrote) and inserts/removes entries. Block spans live beside it.
2. **Rank, for byte-identical output.** Today `Interval::size` counts dense slots, and weights, spill choice and queue order read it.
   With gaps, `end - start` is no longer a count. So `Slots` also answers `rank(slot)` (instructions before it) in O(log n) (a Fenwick
   tree over entry order, updated on insert/remove), and `size`/`weight` are derived from ranks. Without this the series is not
   byte-identical; it is the part most likely to be wrong and the first thing to build and check.
3. **Intervals** keep their segments across a rewrite. Only values the rewrite names (and values live across a changed block's
   boundary) are recomputed, as `updated` already decides; the remap disappears. Weights are computed when read (the queue reads
   them for values it holds), not stored for all.
4. **Postings** hold `InsnId` (the entry), not `(block, index)`; `block_of`/`index_of` come from `Slots`.
5. **Homes (LiveStacks analogue)**: `Frame` keeps, per home, its interval in `Slots` numbers, extended at the point the spiller emits
   an access. `_existing_colors_by` reads it. The sparse walk stays as the check.
6. **Facts** keep their per-value entries; `Facts::of` becomes `Facts::updated(previous, touched)`.
7. **LiveUnion** is not refreshed: only the touched values leave and re-enter.

Each of 3-7 compares against the old whole computation under its own `LLRM_CHECK_*` switch, as `LLRM_CHECK_POSTINGS` and
`LLRM_CHECK_SIBLINGS` do, over the 393-object corpus (66 programs + 65 QCport modules x 3 levels) and the unit tests.

## (c) PR series (each byte-identical, own check, own fail-first test, `measure` step shows the drop)

1. `Slots` + rank behind `indexed()`: gapped numbering and rank, same answers. Check: every instruction's rank equals its dense
   slot / 2, and ordering is isomorphic, over the corpus; unit tests for insertion, local renumber and repack. No consumer changes.
2. Intervals without the remap (`updated` keeps segments; weights on read). Check: `updated` == `worked_out`.
3. Postings by `InsnId`. Check: `LLRM_CHECK_POSTINGS` as now, plus ids.
4. Homes' intervals kept (`_existing_colors_by` reads them). Check: == sparse walk (`LLRM_CHECK_COLORS`, `LLRM_CHECK_RANGES`).
5. `Facts::updated`. Check: == `Facts::of`.
6. `LiveUnion` without `refresh()`, `Frequency` carried. Check: == rebuilt.

## (d) Expected drop (d_faces share of the compile, from the profile; to be re-measured per PR)

| PR | removes | d_faces | QCport total |
|---|---|---|---|
| 1 | infrastructure; the `indexed` rebuild (~1-2) | -1 | -0.3 |
| 2 | `intervals_over` + `updated` 9.4 -> ~3 | -6 | -2 |
| 3 | `postings::following`/`replaced` (2.0 + 0.8 + part of 3) | -3 | -1 |
| 4 | `_existing_colors_by` 9.0 -> ~1 | -8 | -2 |
| 5 | rest of `Facts::of` (17.7 - 9.4) -> ~3 | -5 | -2 |
| 6 | `refresh`, `_overlapping`, `Frequency::of` (~3) -> ~1 | -2 | -0.7 |
| | | about -25% | about -8% |

The `live` axis should fall from 3.5 toward ~2.2, the point of #981. These are estimates from a flat profile; each PR reports its
measured drop, and the series stops where one comes in under half its estimate.

## Risks

- Rank/size parity (2 above). If it cannot be made exact, the output changes: the series would then be a quality change and must be
  measured as one (geomean and worst row), not claimed byte-identical.
- Instruction identity by `Arc` address assumes bodies keep `Arc<Insn>` for unchanged instructions. True for the spiller, split and
  the rewrite helpers today (`postings` relies on it); a pass that rebuilds every `Arc` forces a full renumber (cost as now).
- Blocks that change order between phases get a fresh `Slots`: the persistent numbering is per allocation.
