# After allocation

Audience: an engineer asking which phase made a register or slot in the final code what it is. This page lists what runs once every value has a place, in order.

| Phase | Does |
|---|---|
| `ParallelCopy` | writes each phi copy group in an order that computes it ([coalescing-and-copies](coalescing-and-copies.md#after-allocation)) |
| `Prologue` | reserves the frame the spiller asked for. `assemble::phased_to` skips it: `masm` writes the prologue from the frame's reserve |
| `Peephole` | rewrites that depend on final registers, repeated to a fixed point: `regthrash`, `copyprop`, `copysink`, `spillforward`, `postrasink`, ... ([backend](../../optimizations/backend/readme.md)) |
| `LoopSlots` | [spill traffic in loops](#spill-traffic) |
| `Scheduler` | orders fully allocated instructions for the CPU's pipeline |
| `ControlFlow` | block order and redundant jumps: the physical order decides which explicit edge falls through |

Then, outside the phase list, `masm` builds the procedure: the prologue and epilogue and the registers saved, and `shrinkwrap` decides where each piece is set up.

## Spill traffic

[`LoopSlots`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/loopslots.rs#L1) runs once spill traffic is final. Allocation spills a value across its whole range, but inside one loop registers may still be free: untouched there, or used only to reload a slot for the next instruction. Each slot the loop reaches moves into one, loaded on entry and stored on exit where the loop writes it and the code after reads it.

* The frame register is one more when the loop reaches no other frame cell: it enters with `push bp; mov bp,[bp+d]` and leaves with `pop bp`. Such a loop may not call, trap or touch x87 state, because the runtime walks the BP chain when it raises an error.
* A register live through the loop but untouched in it is parked on the stack, pushed on entry and popped on exit.
* A spill slot's address is never taken, so no pointer reaches a slot this moves.

Tests: [`test_a_dword_slot_parked_in_the_frame_register_is_pushed_and_popped`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/loopslots.rs#L1299), [`test_a_far_pointer_load_keeps_its_slot_in_memory_when_a_loop_holds_the_low_word`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/loopslots.rs#L1425).

`spillforward` has already removed the reloads a register holds ([rematerialization-and-forwarding](rematerialization-and-forwarding.md#forwarding)).

## The frame

`Frame` is shared by the phases of one function: isel's slots (allocas, temporaries, homed arguments), the slots the allocator and the spiller take, and the reserve `masm` takes off SP. Spill slots sit below BP and above the allocas (`Frame::hole`). A frame cell is priced as a near one (a one-byte displacement) in the allocator and the cost model, since its final place is decided by the layout after the phases; if far cells appear with a spill area, the function is laid out again with the allocas below a hole the spill slots fill ([second-frame](../../optimizations/second-frame.md)).

## Saved registers and shrink-wrapping

The registers a convention keeps for the caller (`FrameRegisters::saved`) are pushed and popped by `masm` for those the body writes or a call in it disturbs. [`shrinkwrap`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/shrinkwrap.rs#L1) sets each piece (a saved register, the frame) up at the block that dominates every use of it, and takes it back at each return that block dominates. Where a return can be reached both through that block and around it, or the set-up would sit in a loop, the piece is set up at the entry. A function without a frame register (`hasFP` false) saves it like SI and DI when it uses it as a value ([inputs-and-classes](inputs-and-classes.md#the-register-file)). Tests: [`test_a_return_the_early_exit_shares_with_the_late_path_keeps_the_save_at_the_entry`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/shrinkwrap.rs#L462).
