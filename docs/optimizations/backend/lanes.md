> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/lanes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/lanes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/lanes.rs sha256:86eb73c999e21536a938666a6f16d2970175cacaa99dc9c158c5af1cbc0e03d0 -->

# lanes

Python's `set[tuple[Register_, int]]` of register and flag lanes, as a
bitmask.

Every lane a body can name is a byte of a general register root, a byte of
a segment register, or a flag bit against `RegId::None`: 72 in all. Bit
`i` is the `i`-th lane in `(RegId, u32)` order, so iteration visits lanes
in the order a `BTreeSet<Lane>` would.
