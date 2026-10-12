> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitsink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitsink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitsink.rs sha256:bc57a1c1504dbe642e47c7fa08b3793076797c699832f0b294099ae5b1565089 -->

# exitsink

An add, subtract or extension in a loop that only exit phis read, moved to
those exits: LLVM's LICM sinking an instruction only used outside the
loop, a copy to each exit that reads it. The last trip's update of a
pointer strength reduction carried is computed once, on the way out, and
so is the `sext` of a narrow counter whose value `work += i` reads after
the loop (mandel: `inc cx; movsx eax,cx; cmp cx,20h` every trip).

Adapted from llrm-core's `optimize/exitsink.rs`. Each operand the loop
defines is read through an exit phi, as LCSSA has it; an invariant one
directly. The old one moved one a call; this one repeats to a fixed
point.

Dropped, with no rich-MIR counterpart: the refusal of a body whose
opaque or barrier operations read registers their operands do not name,
flags live into the exit, other results of the moved operation read
elsewhere, and its stack and x87 effects.

The old test is skipped there: its fixture spied on `strength.reduced`.
These are new.
