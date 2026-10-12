> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/copysink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/copysink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/copysink.rs sha256:5e1e1a8ad9b26e9fc9c47fba7b7f878c945cd456ea962beea94ea01e5753ef81 -->

# copysink

Port of `qbopt/backend/copysink.py`: move a copy out of a loop when only
what is after the loop reads it.

A phi whose value is computed in a loop and read after it becomes a copy on
the edge back to the header, so it runs every iteration to hand over a value
nothing inside the loop looks at. The copy belongs on the exit edge. That is
sound while every way out runs the copy first (its block dominates the
exiting one: a loop left from its header before any trip never ran it, and
the destination must keep what it held), the destination is read nowhere
in the loop, and the source is not written between the copy and the exit,
which is what this checks.
