> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/constrain.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/constrain.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/constrain.rs sha256:94a93d941c9ca770d84684f1c9f1d1958435872ddee77a27ce559dae20c9310f -->

# constrain

Port of `qbopt/backend/constrain.py`: giving a required register a value
of its own.

The value an instruction requires in one register is a value of its own,
live across that instruction and nothing else, copied in and out.
