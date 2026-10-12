> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/window.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/window.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/window.rs sha256:c50db86c513ed34a3c22e8e8bf114a4e2f35abe59a4b1a1490397bad02d614a5 -->

# window

A counted loop's huge pointers split into windows: LLVM's IRCE, with the
carry into the selector as the check that cannot fire inside one.

An outer loop walks the windows. At each one every huge recurrence is
normalized (`Intrinsic::Window`), so its offset starts below one selector
step, and the loop runs as many whole trips as the target's window
(`Machine::huge_window`) holds for every recurrence, or the trips left,
on far pointers that never carry. Where the trips provably fit one
window there is no outer loop: the start is normalized once.

The trips are `induction`'s proof, the recurrences its `pointers`. The
split is priced: a window's setup against the carries it saves.
