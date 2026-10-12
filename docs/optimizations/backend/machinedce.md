> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinedce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinedce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinedce.rs sha256:34930a38ee8634d4674d0dea9840a6afa0440166423a8bf343e04e6370e8dcf1 -->

# machinedce

Port of `qbopt/backend/machinedce.py`: eliminate pure allocated
instructions whose complete result is dead.

MIR dead-code elimination reasons about values before allocation. Lowering,
splitting and physical rewrites can leave a machine computation whose
virtual definition still exists but whose physical register and flag results
are all dead. This pass answers only that machine question. Source memory
reads, control flow, trapping arithmetic, x87 work and relocations are
deliberately outside it.
