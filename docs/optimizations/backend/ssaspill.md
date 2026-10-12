> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/ssaspill.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/ssaspill.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/ssaspill.rs sha256:44608f91eb6dbeff830f5f18e0f17af25f02fb5ea43e331c8676e2060b4c40fa -->

# ssaspill

Braun & Hack spilling on the SSA LIR ("Register Spilling and Live-Range
Splitting for SSA-Form Programs", CC 2009), run before `PhiElimination`.

Per block, Belady's rule over the values a register holds (furthest next
use leaves, next uses seen across blocks with a loop-exit distance); a use
the set lacks is reloaded; where a successor expects a value in a register
that a predecessor does not end with, the edge reloads it. Whether a value
is kept into a loop, and where its store goes, follow block frequencies.
Reloads redefine the value, and `ssarepair` restores SSA.

What a later phase will take (x87 compares' AX, constrained registers) is
stated on the instruction as `requires`, `delivers` and `clobbers`; this
reserves what the instruction states and nothing else.
