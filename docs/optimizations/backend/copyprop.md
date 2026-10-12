> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/copyprop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/copyprop.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/copyprop.rs sha256:a9bf161ea430a2750d67b6850b1d9b9f7c37804d8a988cdde965598070cbc544 -->

# copyprop

Port of `qbopt/backend/copyprop.py`: forward and remove physical register
copies proven on every CFG path.

LLVM's MachineCopyPropagation tracks physical register units after
allocation; here a unit is one byte lane.  Undirected lane equality removes
redundant copies, while a second must-analysis retains the reaching copy's
direction for operand substitution.  Selection and decoded effects are both
checked again before a source is renamed.
