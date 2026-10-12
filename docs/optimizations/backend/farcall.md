> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/farcall.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/farcall.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/farcall.rs sha256:ada67ea46fc6636edbf0205945afd95091a3888d93985102e395d49d77ea34aa -->

# farcall

Port of `qbopt/backend/farcall.py`: give a far indirect call the memory
operand the 16-bit ISA requires.

A 16:16 far call has only an `m16:16` form, while MIR carries the code
pointer as one four-byte value. Lower that target through one reusable
owned frame cell before allocation; this is target form selection, not
spilling.
