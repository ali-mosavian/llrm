> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/loopslots.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/loopslots.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/loopslots.rs sha256:afb49c98ff228dd6d5af3a6f87a81746ddf30ef58741331e361923697d82d17e -->

# loopslots

A loop's spill slots in the registers the loop leaves free.

Allocation spills a value across its whole range; inside one loop,
registers may still be free: untouched there, or used only to reload a
slot for the next instruction. Each slot the loop reaches moves into one,
loaded on entry and stored on exit where it was written and is read after.

BP is one more when the loop reaches no other frame cell: it enters with
`push bp; mov bp,[bp+d]` and leaves with `pop bp`. Such a loop may not
call, trap or touch x87 state, since the runtime walks the BP chain when
it raises an error. A spill slot's address is never taken, so no pointer
reaches any slot this moves.

A register live through the loop but untouched in it is one more, parked
on the stack: pushed on entry and popped on exit.
