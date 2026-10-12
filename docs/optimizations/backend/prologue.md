> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/prologue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/prologue.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/prologue.rs sha256:0fea12da0e1eb1942771b4a0ae4690f05f3fe7fcc9a75b4408b60466c7b0d008 -->

# prologue

Port of `qbopt/backend/prologue.py`: reserving the frame the spiller
asked for, and giving it back.

For a runtime-framed procedure, reserve after B$ENRA establishes BP and
release before B$EXSA tears it down. Refused where the body's exits
cannot all be found.
