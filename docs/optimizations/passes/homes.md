> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/homes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/homes.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/homes.rs sha256:a8568e7d773e2a6160645e6bdfa30eae378d3e0e611834a4719943a3cffd98b5 -->

# homes

Marks each phi whose value also sits in a memory cell with `!llrm.home`,
naming a store of that cell, so a spill of the phi reads the cell instead of
paying for a slot. GCC's `REG_EQUIV` by store; LLVM has no counterpart.

## How it works
A phi `p` is in cell `c` when each input `v` was stored to `c` and nothing
has written `c` since, at the end of the block `v` comes from, and nothing
writes `c` while `p` is live. `p` is then what `c` holds at every point it
is read. A store of `p` itself writes what is there. Only stores of an exact
cell (an object's own bytes at a constant offset, non-volatile) are
candidates; liveness and the paths between a store and a block are computed
once per function.

```text
 loop:  %s = phi [0.0,%pre],[%t,%loop]      %s = phi ... !llrm.home !{store}
        %t = fadd %s, %d             =>     (same body)
        store %t, ptr %cell
```

The mark is a hint. The backend re-checks it against the body as selection
leaves it (`storedhomes`: nothing writes the cell while the value lives, and
every definition is a plain copy). Where such a value is spilled it has no
store of its own and its restore is the load promotion removed.

## Runs
Last in the module pipeline, after `spares`, so the instruction ids it names
are the ones selection sees. On whenever optimizing.

## Needs
`Summaries`, `memoryssa::Accesses`, liveness.

## Keeps
Adds metadata only; no instruction changes.

## See also
Consumers: the spillers, `floatassign` and `isel` in `llrm-core`. Tests:
`homes_tests.rs`.
