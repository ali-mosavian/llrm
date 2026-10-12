> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dse.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dse.rs sha256:205b251293bcfdff79dd803f08b0ee932dc96098ea85e6b83da83f492f73bbe3 -->

# dse

LLVM's DSE: llrm-core's `DropStores` and `without_dead_stores`, from
`optimize/transform.rs`. A store is dead when every path overwrites its
cell before reading it, or when nothing after it can read the cell:
`avail::dead_stores` finds them, `observers::private` saying which cells
no call and no exit observes. `Promote` leaves every store; this takes
those to cells nothing reads any more.

Its calls' footprints are `Summaries`', where required; else each
callee is unknown.
Dropped with the BC frontend: the data-segment bounds and the error
handlers' contracts.
