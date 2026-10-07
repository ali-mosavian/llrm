# Stable slot numbers for the allocator (#560): what was tried, and what a sound version needs

## Why

Allocator cost per LIR instruction is flat (~0.2 ms) up to ~200 instructions and 9x higher at 1800 (732 QCport
functions, log-log slope 1.25 below 300 instructions, 2.21 above; `part_frame` 435 -> 1647 instructions: 271 ms ->
11,385 ms). Every spill or split rewrite rebuilds intervals, facts, colour slots and interference over the whole body,
and the dense numbering (two slots per instruction that is no mark) renumbers every later slot, so nothing computed
for an untouched value survives a rewrite. LLVM's `SlotIndexes` leave a gap between instructions
(`SlotIndex::InstrDist = 4 * Slot_Count`), an inserted instruction takes a free number
(`SlotIndexes::renumberIndexes` only when a gap runs out), and `LiveIntervals::repairIntervalsInRange` and `SplitEditor`
repair only the intervals a rewrite touches.

## What was built (local branch `perf/slots1`, head b721facf, not pushed)

- `analysis/slots.rs`: a stable number per instruction, gaps of `PITCH = 64` between instructions, a block's slots in
  `[index * REGION, (index + 1) * REGION)` with `REGION = 2^24` so that a block's end is the next block's start,
  numbered by diffing against a thread-local memory of the last numbering of the same function by `Arc` identity (an
  instruction seen before in the same block and in increasing order keeps its slot, a new one takes the midpoint of its
  neighbours, a block whose gap is spent is numbered afresh).
- `Dense`: a view from a stable slot to the dense slot it stands for (block start + 2 + 2 x instructions before that take
  a window, a point in an instruction's window keeping its place in it, a point in the gap after it being where the next
  window begins).
- `Segment` carries its two ends in the dense numbering; `Interval::size()` is counted there, so every size and weight
  is what it was; joins keep the dense ends.
- `LLRM_CHECK_SLOTS=1`: every numbering compared with the dense numbering of the same body, every walked interval with the
  dense walk.
- Tests: an inserted instruction takes a free slot and no other moves; a spent gap numbers only its block afresh; the dense
  view equals the dense numbering after 400 random rewrites.
- Step 0 (merged, #798) made the allocator ask the numbering for points (`def_point`, `window_end`, `spill_size`) and a test
  keeps the slot constants inside `analysis/intervals.rs`. That, and the dense-carrying `Segment`/`Interval`, are sound and
  stay.

## What failed, and why

Gate on the branch: QCport 59 of 130 objects differ from main, bench 70 problems, 4 lib failures. `LLRM_CHECK_SLOTS` was
clean, because it compares each numbering with the dense one per body; nothing compared one body's numbering with another's.
Bisected by experiment on `pak.c`, `ls.c`, `vid.c`:

| numbering | result |
|---|---|
| dense everywhere | objects identical |
| stable in `indexed` only, or in `indexed_shared` only | objects differ |
| one numbering for every caller (`indexed` = a copy of `indexed_shared`) | pak, ls, vid identical; 22 QCport modules panic ("value cannot be spilled and no register it may take is free", `allocate.rs:1133`) |

The dense numbering is a pure function of position, so every consumer that numbers a body, a copy of it with some
instructions replaced (the spiller's `tracked` body in `_existing_colors_by`, which gives the instructions that name a slot
home a pseudo-value), a sparse body (`intervals_sparse`) or a remembered answer (`intervals_over`'s `RECENT`) gets the same
slot for the same position, and compares freely: live intervals against mask slots against the occupants of a slot home.
A numbering keyed by `Arc` identity is a function of the body's history: a clone gets new slots, asking again for an older
body after a newer one numbers it differently, caches keyed by body identity hold numbers from the old numbering, and the
allocator's best-candidate restore sees both. The same numbers then mean different places.

## What a sound version needs (A)

1. The numbering is a property of the body value: computed once per body object, immutable, held with the body (a lazily
   filled cell on `LirBody`), and validated by identity of the blocks' instructions as `Postings` is (a body edited in
   place cannot be asked for its numbering again; `tracked.blocks[b].insns[p] = made` is such an edit).
2. A body made from another by the one construction path (`LirBody::with_blocks` and the few struct updates) takes its
   numbering from the parent's by the diff, so the numbering of a body is a function of the body and its parent, not of
   what happened to be numbered last.
3. A body that replaces instructions in place (the `tracked` body, the sparse body) takes its parent's slots BY POSITION,
   through one function, `Indexes::derived(parent, body)`, never by diff.
4. Every cache holds the numbering it was made under (`intervals_over`'s memo, `Facts`), and a consumer that combines two
   things asserts they have the same numbering (`Rc::ptr_eq` of the numberings).
5. A block whose gap is spent is numbered afresh: a new numbering, so every cache made under the old is invalid by item 4;
   it must not renumber under a cache.
6. Instrument: `LLRM_CHECK_SLOTS` compares each numbering with the dense one per body and a SECOND check compares across
   bodies: for every pair of bodies a consumer combines, the same instruction (by identity) or the same position (by
   derivation) has the same slot. The test that guards it builds a body, numbers it, clones it with an instruction replaced,
   numbers the clone and asserts the replaced position has its parent's slot; it fails for the clone-gets-new-slots design.
7. Then per-value repair (`repairIntervalsInRange`): a rewrite names the values it touches (`Postings::replaced` already
   computes them); their intervals are dropped and worked out again from their occurrences by the backward walk from each
   use; an untouched value keeps its segments bit for bit, and only its dense size (hence weight) moves where a segment
   spans inserted or removed instructions (a stabbing query over the segments). The intervals map order is not read:
   reversing it changed no object over 43 QCport modules.

Status: (C) first, the safe constant-factor items; (A) only if big functions still matter after #791's cap and #806.
