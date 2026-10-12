> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/spares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/spares.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/spares.rs sha256:59e5b826da736a97a28db663ef9f8be1df2ac583c35be44d27a6075da96a0d65 -->

# spares

What the optimizer proved of a fixed-cell load, kept for selection: each
write that leaves its bytes as they were, as `!llrm.spares` on the load.
LLVM's `MachineMemOperand` keeps the IR value for the same use,
alias queries at the machine level (`MachineInstr::mayAlias`). Last, so the
instruction ids it names are the ones selection sees.
