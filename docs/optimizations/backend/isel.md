> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/isel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/isel.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/isel.rs sha256:462e79a032331263637a506e01243ea3822bb412cbac5f622917f01402b6e895 -->

# isel

Instruction selection from MIR: each function's instructions as LIR over
virtual registers, as LLVM's SelectionDAG makes MachineInstrs. It chooses
instructions and addressing only; two-address form, registers and the
frame's size are the machine phases'.
