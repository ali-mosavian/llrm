> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/homes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/homes.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/homes.rs sha256:062254ac3ab7e15ab00ec93093c7bb91a7265caacd95868210dfbb1cf3e92804 -->

# homes

A phi whose value is a cell's: what the optimizer proved of a loop-carried
value that the program also stores, kept for selection as `!llrm.home` on
the phi, naming one store of the cell. Where such a value is spilled the
backend reads the cell: no store of its own, and a restore that is the load
promotion removed. GCC's `REG_EQUIV` by store, which LLVM has no counterpart
of. Last, as `spares`, so the ids it names are the ones selection sees.

A phi `p` is in cell `c` where each of its inputs `v` was stored to `c` and
nothing has written `c` since, at the end of the block `v` comes from, and
nothing writes `c` while `p` is live: `p` is then what `c` holds at
every point it is read. A store of `p` itself writes what is there.
