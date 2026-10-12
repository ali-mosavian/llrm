> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinedce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinedce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinedce.rs sha256:09fcd67b869afd9daf4d1f212cd532e42c7cd0c8820a9136523e127c046fb520 -->

# machinedce

Port of `qbopt/backend/machinedce.py`: eliminate pure allocated
instructions whose complete result is dead.

MIR dead-code elimination reasons about values before allocation. Lowering,
splitting and physical rewrites can leave a machine computation whose
virtual definition still exists but whose physical register and flag results
are all dead. This pass answers only that machine question. Source memory
reads, control flow, trapping arithmetic, x87 work and relocations are
deliberately outside it.
