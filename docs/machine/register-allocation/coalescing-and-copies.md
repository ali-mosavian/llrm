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

[`coalesce::joined`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/coalesce.rs#L81) is LLVM's `RegisterCoalescer`, conservative: there is no undo, so a join must be safe. It joins by live range and builds no interference graph (`_interference` remains for `twoaddr` and the spiller's sibling pricing).

Copies are visited in the blocks of deepest loop first, and in a loop the blocks that run most. A copy of two values `x := y` is joined when:

1. they are not already one web, and their live ranges share no point, or share one run that the copy begins, of one width, with neither web written inside it (and the copy is not a member of a parallel copy);
2. they are not pinned to different registers, and the registers both may take have a common member, taking each value's class and the registers no point in the merged range destroys.

No colourability test is made: two ranges that share no point leave the live count of every point as it was, and what the allocator does with a range of many segments is its to split and evict. The merged web has the union of the two intervals, the wider width and the pin of whichever had one. A joined copy becomes an identity and is removed; every occurrence is renamed to the web's root.

## Hints

A copy the coalescer refused stays a `mov` between two values. `_copy_hints` records both as partners of each other, and the assignment loop votes for the register a placed partner holds ([assignment](assignment.md)). `_wanted` carries a fixed register along chains of copies, so a loop's sum is seated in the register its return copy will want. After the queue empties `_recolored_hints` moves a value into a partner's register when that joins more of its copies. A hint never overrides a requirement ([`test_copy_hint_cannot_override_a_register_requirement`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/allocate.rs#L5258)).

## After allocation

The parallel copies phi elimination made are still `group`s of moves between locations. [`ParallelCopy`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/parcopy.rs#L171) writes each in an order where no move reads what an earlier one wrote. A move of a place into itself is dropped. A cycle is broken by exchanges (`xchg`) where the target can exchange every adjacent pair, else through a register that is dead after the group and that the group neither reads nor writes, else through the stack (push the place the closing move reads, perform the others, pop). The cheapest of the three by the CPU's `xchg_rr`, `mov_rr`, `push_r` and `pop_r` prices is taken, ties going to exchanges, then register, then stack. Where the target is tuned for size there are no prices (`CyclePrices::of` is `None`), no register is borrowed, and a cycle is exchanged where it can be, else goes through the stack. A register is borrowed only if it is one the convention does not make the function save. Tests: [`test_a_cycle_of_two_widths_goes_through_a_dead_register_where_that_costs_less`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/parcopy.rs#L666), [`test_a_cycle_of_one_width_is_exchanged_only_where_that_costs_less_than_a_dead_register`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/parcopy.rs#L706).

The first peephole sub-pass is [`regthrash::thrashed`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/regthrash.rs#L32), Open Watcom's `RegThrash`, the inverse of coalescing: for `OP -> Y ... mov Z, Y` where Y dies at the move, Z is dead and Y is untouched between, the producer writes Z and the move becomes a self-copy. It removes copies the coalescer refused for colourability, and the instruction must still encode (`select::emit` is the check). The later passes `copyprop`, `copysink` and `postrasink` remove or move copies over physical registers ([backend](../../optimizations/backend/readme.md)).
