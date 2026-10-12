> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/lanes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/lanes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/lanes.rs sha256:8439e99a863fbf7201d46ddf1cd25251c4bbe1f08ae4459d663df1b7873940e8 -->

# lanes

Python's `set[tuple[Register_, int]]` of register and flag lanes, as a
bitmask.

Every lane a body can name is a byte of a general register root, a byte of
a segment register, or a flag bit against `RegId::None`: 72 in all. Bit
`i` is the `i`-th lane in `(RegId, u32)` order, so iteration visits lanes
in the order a `BTreeSet<Lane>` would.
