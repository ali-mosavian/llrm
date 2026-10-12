> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/target.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/target.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/target.rs sha256:eb12948849dc01f5792aa4ec141f1b2b318c4c4ad28fb543be9a1a56e61ea59c -->

# target

Port of `qbopt/backend/target.py`: what the machine has, and what each
instruction requires.

LLVM's `TargetRegisterInfo` and `TargetInstrInfo` in one module; the two
halves are the two sections below. A register class is the unit an
allocator works in: 16-bit addressing reaches memory through bx, bp, si
and di and nothing else.
