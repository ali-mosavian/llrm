> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/parcopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/parcopy.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/parcopy.rs sha256:fbab6bd48bbbc0b314790a9e80c2c1ff0e6b8252c7fbfafe1e59879197157e1c -->

# parcopy

Port of `qbopt/backend/parcopy.py`: scheduling a parallel copy, moves
that happen at once, written in an order.

After allocation: which moves conflict is a question about locations. A
move may go once nothing left in the group reads what it writes; a cycle
is exchanged or rotated through the machine stack: every cycle can be.
