> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/liveness.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/liveness.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/liveness.rs sha256:d99f9d687601644606008c451bc953ad79cd4e291438cb97de848da7dec4526d -->

# liveness

Which physical register and flag lanes are live into and dead out of each
block, and what one instruction reads and writes.

## How it works
A lane is one byte of a general or segment register, or one flag bit
([`crate::backend::lanes`](lanes.md)). `effect` gives an instruction's reads and
writes: as it decodes (`_register_effects`), else as its contract declares
(`_declared`: a call's `requires`, `clobbers` and `delivers`, a return's
explicit results and the architectural state its epilogue needs). An
instruction with neither is unknown and may read anything. The answer is
worked out once per instruction and kept on it.

A shift's effect also lists the bytes it moves, `(written, source)`: a
source is read only where its written byte, or a flag the shift sets, is
live after it. A parallel copy group is one effect at its last copy, since
it reads every source before it writes any destination.

Inside a block the walk goes backward:

```text
  live_before = (live_after - writes) | reads
```

Across blocks `live_into` finds the least fixed point with a worklist that
starts from the last block: a block is recomputed when a successor's live
set changed. A block with no successor leaves everything live, so a copy
written as the last instruction of an exit block survives, and so does a
parallel copy for a phi, which is written exactly at a block end.
`dead_at_exit` is the complement, per block: the lanes nothing reads again.

The answer is kept between asks and brought up to date from the blocks that
changed (see [`crate::backend::incremental`](incremental.md)), because the peephole
sub-passes ask in turn and each leaves most blocks as they were. A block's
step is the lanes it reads before writing them and those it writes, kept
while the block is the same. `LLRM_CHECK_LIVENESS=1` solves the whole body
from scratch after every answer and asserts they agree.

SI, DI and the frame register are not return inputs: their incoming values
live in the prologue's saves, so a dead final write to one stays removable.

## Example
No example: the lane sets it computes are printed by no dump, and the tests
assert them without printing. See `lanes.rs` for the lane model.

## Used by
`peephole` and the sub-passes it runs (`regthrash`, `copysink`,
`postrasink`, `exactaddress`, `upperzero`, `sharedstores`, `machinedce`),
`loopslots`, and the peephole rule walker (`peep/walk`).
