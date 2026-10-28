# Live ranges, interference and spill weights

Audience: an engineer debugging why two values did or did not share a register, or why one was spilled. This page states how a value's range is built, how a register's holders are queried, and what a spill is priced at.

## Slots and segments

`RegAlloc` numbers every instruction with two slots: where it reads and, one later, where it writes ([`intervals.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/analysis/intervals.rs#L15-L16): `PER_INSN`, `DEF`). Slots are spaced `GAP` apart, so an instruction a rewrite adds takes a slot between its neighbours' and no other instruction moves (`Indexes::patched`). What the code counts as "instructions" is `Indexes::uniform`, the consecutive number.

An [`Interval`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/analysis/intervals.rs#L38) is a sorted list of half-open `Segment`s plus a weight. A use ends a segment at the instruction's write slot, a definition starts one there, so a source that dies at an instruction and the destination it defines do not overlap and may share a register. A value defined and never read still has a one-slot segment. A segment that crosses a block boundary stays one segment (`_merged`); a redefinition leaves a hole.

How segments are found: per block, backward from the values live out, a definition closes a segment and a use opens one at the block top if nothing defines it first. The walk is `_walked_at` in [`intervals.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/analysis/intervals.rs#L1562); `LLRM_CHECK_RANGES=1` holds it to a plainer reference. A phi's result is defined at the block top, its arguments are read at the end of the predecessor (`liverows::live`). A parallel copy (`Insn::group`) reads all its sources before any write, so its moves are one point.

Which values are live into and out of each block (`allocate::LiveRows`) is a bit-row fixed point, or, in a large body, is found by following each value up its predecessors from where it occurs ([`live_rows_by`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/allocate.rs#L593), `WebRows`). `LLRM_CHECK_LIVE=1` runs both and compares. [`liveness.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/liveness.rs#L289) is another thing: which physical register and flag lanes are dead after allocation, for the peephole.

## Weights

[`LiveRanges::weight`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/live.rs#L183) is what spilling a value costs, per slot it is live:

```
weight = references weighted by block frequency / (live slots + grace)
         less what its copies and folds would save, floored at 0
weight = infinity if the value may not be spilled
```

* references: every definition and use, each times `Frequency::block` of its block, which comes from the branch odds and proven trip counts ([frequency.rs](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/analysis/frequency.rs));
* grace: [`GRACE`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/analysis/intervals.rs#L1776), so a one-slot value is not infinitely dear;
* copies: a plain move to a value the range could share a slot with, if neither is used by anything but moves and updates in place and their intervals do not overlap, saves the block frequency (`_sibling_free`);
* folds: a read an instruction can take as a memory operand (`add`, `sub`, `and`, `or`, `xor`, `cmp`, a 32-bit `imul`) saves a share of a reload set by the CPU profile's prices (`FoldDiscounts`, `LLRM_CHECK_FOLDS`);
* infinity: values the spiller made for one use (`unspillable`) and values a candidate shape protects (`protected`).

The weight is computed when read, from what it is made of, because a rewrite changes an interval's size and with it the weight.

## No interference graph

`RegAlloc` builds none. Whether a value fits a register is asked of the register's holders.

[`LiveUnion`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/liveunion.rs#L155) keeps, per whole register, the values placed in it in the order they took it. A register's holders do not overlap, so "who meets this interval" is a walk over the segments that start before it ends back to the first that ends before it starts: logarithmic in the holders, linear in the overlaps. A register with few holders is scanned. From `BIG` holders and `ASKED` questions since the intervals last moved it gets an `Index`:

| Part | Holds |
|---|---|
| `kept` | segments that meet no other kept one, by start |
| `points` | zero-length segments |
| `apart` | segments that meet one already kept, looked at one by one: a rewrite moves every interval and leaves clashes for the allocator to find |

`meeting(register, interval)` returns the holders, `busy` whether there are any, `add` and `remove` place and unplace. After a rewrite `refresh_among` re-indexes only the values whose intervals moved; `LLRM_CHECK_HOLDERS=1` checks every index against the pairwise answer. `Overlaps` is the same idea as a static tree, used to find the registers other values want (`Claims`).

An interference graph of values, not registers, exists before allocation in one other place: the spiller's check that two copies may share a slot (`_interference_among`, over one copy web).

## Where a register is destroyed

A value cannot take a register if it is live across a point that destroys it. `Masks` holds the `Mask` points of a body (calls, instructions with `clobbers`, and an instruction that needs the data group, which destroys the data segment register before it reads) sorted by slot, and per register where it is destroyed. [`_clobbered`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/allocate.rs#L2767) asks by bisection: during a point (the value must end at or before the write slot), before its reads (the value must end before the read), or the upper half only (only values wider than 2 bytes lose). `LLRM_CHECK_CLOBBERED=1` compares with a look at every point. Unit tests: [`test_clobbered_agrees_with_a_look_at_every_point`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/regalloc_fuzz_tests.rs#L1831).

## Facts kept across rewrites

Splitting and spilling rewrite the body many times. `Facts` ([`allocate.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/allocate.rs#L1340)) holds slots, intervals, fold and sibling prices, masks, widths, classes and copy hints, and `Facts::of` takes the previous `Facts` and the instructions the rewrite added and removed, redoing only the values those name. `LLRM_CHECK_FACTS=1` recomputes everything and asserts equality. The spiller asks for the same intervals through `live::held`, not by recomputing them.
