> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/copyprop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/copyprop.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/copyprop.rs sha256:5f31f7cf79e743ffe81c5f421cf27cf1727264f857df1935658a73905273f50d -->

# copyprop

Port of `qbopt/backend/copyprop.py`: forward and remove physical register
copies proven on every CFG path.

LLVM's MachineCopyPropagation tracks physical register units after
allocation; here a unit is one byte lane.  Undirected lane equality removes
redundant copies, while a second must-analysis retains the reaching copy's
direction for operand substitution.  Selection and decoded effects are both
checked again before a source is renamed.
