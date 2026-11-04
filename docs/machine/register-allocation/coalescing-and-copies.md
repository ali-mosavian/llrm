# Copies: phi elimination, ties, coalescing, hints

Audience: an engineer asking where a `mov` between registers came from, or why one was not removed. This page follows a copy from its creation to its removal.

Copies are made by [phi elimination](#phi-elimination), by [two-address rewriting](#two-address) and by [constraints](inputs-and-classes.md#giving-a-requirement-a-value-of-its-own). They are removed by the [coalescer](#coalescing) before allocation, by [hints](#hints) during it, and by [thrashing](#after-allocation) after it.

## Phi elimination

[`PhiElimination`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/phielim.rs#L94) takes LIR out of SSA. Each phi becomes a move at the end of each predecessor it names, ahead of the terminator. All the moves of one edge carry one `group` number: they are a parallel copy, read before any is written.

| Case | Result |
|---|---|
| a one-input phi (a loop exit's) | the value renamed; at a loop exit it is kept as a copy opening the exit, so the loop's register is not confined by a call after it |
| a critical edge (predecessor has several successors) whose incoming values are defined in their predecessors, read by nothing else and not live past the phi | each definition defines the phi's result outright: no copy |
| a critical edge otherwise | the copies stay in the predecessor, or go in a block of their own on the edge when `EdgePrices::cheaper_on_edge` says the edge's chance times the copies and a `jmp` costs less than the copies every time, or when a value is read on the predecessor's other paths |

Prices are the CPU's `mov_rr` and `jmp_short`. An edge block whose copies all went away is the edge again (`unsplit`, run first in the peephole).

## Two-address

x86 instructions write one of their sources. [`TwoAddress`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/twoaddr.rs#L44) rewrites each tied instruction `c := a op b` as `c := a` then `c := c op b`. First, a commutative instruction whose second source dies and does not interfere with the destination is swapped, so the destination reuses the dying source and no copy is needed; an update copied back to its source (`c := a + b; a := c`, `a` dead between) becomes `a := a + b`. A source that is a constant is materialized first. Tests: [`test_commutative_instruction_reuses_the_dying_operand`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/twoaddr.rs#L534), [`test_a_source_that_interferes_with_the_destination_is_not_the_one_tied`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/twoaddr.rs#L731). [examples](examples.md#constraints-ties-and-copies) shows the copies it makes.

## Coalescing

`coalescer` (`register_coalescer.rs`, `joinvals.rs`, `driver.rs`) is LLVM's `RegisterCoalescer`; `backend/coalesce.rs` hands it the live intervals and the target's answers (which values may share a register). A join has no undo, so it must be safe.

Copies are visited in block priority order (deepest loops first) by `copy_coalesce_in_mbb`; the ones that fail go on a work list and are retried until a round joins nothing. For a copy `x := y` `join_copy`:

1. refuses it if the two registers have no common member (classes, pins) or are pinned to different registers;
2. computes how each value of one interval relates to the other (`JoinVals::compute_assignment`: same value, erasable, conflicting) and refuses a conflict it cannot prune;
3. joins the smaller interval into the larger (`LiveRange::join`), erases the copy and renames every occurrence.

The result keeps the wider width and the pin of whichever side had one. `LLRM_COALESCE_WHY=1` prints the reason each remaining copy was left; `LLRM_CHECK_LIVEINTERVALS=1` recomputes the intervals afresh after a join and compares.

## Hints

A copy the coalescer refused stays a `mov` between two values. The allocator takes its other end as a hint: `calculate_spill_weights_and_hints` records it, `try_assign` tries the partner's register first, and after the queue empties `try_hints_recoloring` moves a range into a partner's register when that removes more broken hints than it costs. A hint never overrides a requirement.

## After allocation

The parallel copies phi elimination made are still `group`s of moves between locations. [`ParallelCopy`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/parcopy.rs#L171) writes each in an order where no move reads what an earlier one wrote. A move of a place into itself is dropped. A cycle is broken by exchanges (`xchg`) where the target can exchange every adjacent pair, else through a register that is dead after the group and that the group neither reads nor writes, else through the stack (push the place the closing move reads, perform the others, pop). The cheapest of the three by the CPU's `xchg_rr`, `mov_rr`, `push_r` and `pop_r` prices is taken, ties going to exchanges, then register, then stack. Where the target is tuned for size there are no prices (`CyclePrices::of` is `None`), no register is borrowed, and a cycle is exchanged where it can be, else goes through the stack. A register is borrowed only if it is one the convention does not make the function save. Tests: [`test_a_cycle_of_two_widths_goes_through_a_dead_register_where_that_costs_less`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/parcopy.rs#L666), [`test_a_cycle_of_one_width_is_exchanged_only_where_that_costs_less_than_a_dead_register`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/parcopy.rs#L706).

The first peephole sub-pass is [`regthrash::thrashed`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/regthrash.rs#L32), Open Watcom's `RegThrash`, the inverse of coalescing: for `OP -> Y ... mov Z, Y` where Y dies at the move, Z is dead and Y is untouched between, the producer writes Z and the move becomes a self-copy. It removes copies the coalescer refused for colourability, and the instruction must still encode (`select::emit` is the check). The later passes `copyprop`, `copysink` and `postrasink` remove or move copies over physical registers ([backend](../../optimizations/backend/readme.md)).
