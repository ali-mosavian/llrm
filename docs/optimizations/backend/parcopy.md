> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/parcopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/parcopy.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/parcopy.rs sha256:c50ec02ee32ed7090ec6bcd979ae8e33eac5165a2b5de6237cd2f4a7b977608b -->

# parcopy

Port of `qbopt/backend/parcopy.py`: scheduling a parallel copy, moves
that happen at once, written in an order.

After allocation: which moves conflict is a question about locations. A
move may go once nothing left in the group reads what it writes; a cycle
is exchanged or rotated through the machine stack: every cycle can be.
