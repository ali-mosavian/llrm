> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinecse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinecse.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinecse.rs sha256:41cd1025f3885783b07a546900456a735ea4bd77d384ea6bb22052e908875c76 -->

# machinecse

Port of `qbopt/backend/machinecse.py`: eliminate repeated deterministic
computations after register allocation.

MIR GVN sees values, not the physical instructions lowering and allocation
create.  At this point the question is entirely physical: value-number the
byte lanes read and written by independently reproducible instructions, and
retain an anchor for the virtual/data-source ownership of a redundant
occurrence.
