> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/isel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/isel.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/isel.rs sha256:4c05fc5462e3429a586213ea63af7f6b5db4eff1d7958119f0d5854479299d6b -->

# isel

Instruction selection from MIR: each function's instructions as LIR over
virtual registers, as LLVM's SelectionDAG makes MachineInstrs. It chooses
instructions and addressing only; two-address form, registers and the
frame's size are the machine phases'.
