> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/prologue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/prologue.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/prologue.rs sha256:7262ef81586a7e08bacbd10133a510095b2c28fed1442fb51fb1a07f76a626bf -->

# prologue

Port of `qbopt/backend/prologue.py`: reserving the frame the spiller
asked for, and giving it back.

For a runtime-framed procedure, reserve after B$ENRA establishes BP and
release before B$EXSA tears it down. Refused where the body's exits
cannot all be found.
