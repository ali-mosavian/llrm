> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/constrain.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/constrain.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/constrain.rs sha256:bbd8b71081d04d32aeeea2a626c7c8f4d49a4e4a71ca6ba845d9a9ee6c3ebbbd -->

# constrain

Port of `qbopt/backend/constrain.py`: giving a required register a value
of its own.

The value an instruction requires in one register is a value of its own,
live across that instruction and nothing else, copied in and out.
