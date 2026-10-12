> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/spares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/spares.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/spares.rs sha256:54291a1c2702d203b5a92ba8669359e51d8bc7dd2c41fcf275770fcadbe6cda0 -->

# spares

What the optimizer proved of a fixed-cell load, kept for selection: each
write that leaves its bytes as they were, as `!llrm.spares` on the load.
LLVM's `MachineMemOperand` keeps the IR value for the same use,
alias queries at the machine level (`MachineInstr::mayAlias`). Last, so the
instruction ids it names are the ones selection sees.
