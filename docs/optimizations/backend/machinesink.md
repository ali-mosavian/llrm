> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinesink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinesink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinesink.rs sha256:1d62c3cc3eb3f965d8f53d7bc04f8cf854c5e3809170667db151ac7f725bf5c1 -->

# machinesink

Moves a computation into the one successor block that uses it, breaking a
critical edge where that is what lets it go. Translated from
[MachineSink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp)
(`MachineSinking`, before PHI elimination, on SSA).

## How it works
Each block with several successors is walked bottom-up. An instruction that
is safe to move (`safe_to_move`: no store, call or volatile access, a
load only above every store of its block, no register but virtual ones)
goes to the successor that dominates every use of the values it defines
(`FindSuccToSinkTo`): a use in a phi counts at the end of the phi's
predecessor. Successors are tried by block frequency, then by loop depth,
least first. A sink that leaves the block it started in on every path is
kept only where `isProfitableToSinkTo` says it shortens the execution of
some path.

Where the successor has other predecessors the instruction would run on
paths that never used it, so the edge is marked to be split
(`PostponeSplitCriticalEdge`). After the walk the marked edges get a block
of their own and the walk runs again: the instruction now sinks into the new
block, which is its successor's only predecessor.

```text
 before                          after
 b1: v = mov 0                   b1: cmp; je e
     cmp; je b4                  b2: ...        e: v = mov 0; jmp b4
 b2: ...; jmp b4                 b4: r = phi [b2: x] [e: v]
 b4: r = phi [b1: v] [b2: x]
```

DEVIATE M1 (no number): a load is sunk only into a successor with no other
predecessor, or a block made for the edge. LLVM also sinks it along a
critical edge when `hasStoreBetween` finds no store on any path between.
Exit: that function translated.

DEVIATE M2 (no number): `isProfitableToSinkTo` ends by asking the register
pressure of the target (`registerPressureSetExceedsLimit`) for an
instruction inside a cycle; no instruction here sinks on that ground.
Exit: the target description's pressure limits.

DEVIATE M4 (no number): only an instruction that reads no value moves (a
constant). LLVM moves any, and its allocator splits the operands' ranges
around the stretch; ours spills. Rows with every instruction movable and
each operand read again below the destination: `bench/grep` -O2 m16 +718
executed instructions and 601 spill stores (two sunk `movzx`),
`bench/quicksort` -Os m16 +22 bytes. Exit: the greedy allocator with
SplitKit as the default allocator.

DEVIATE M5 (no number): in a build for size no edge is broken. The block
costs a jump (`bench/recpow` -Os +5 bytes with the edge split) and nothing
in a size build pays it back. Exit: block placement that falls through
every split edge.

DEVIATE M3 (no number): no `PerformSinkAndFold`, no sinking into cycles
(`SinkInstsIntoCycle`, off by default), no debug-value handling (an
instruction has none).

## Example
`b1: v = mov 0; cmp x, 0; je b4` and `b4: r = phi [b1: v] [b2: w]` leave
`v = mov 0` in a block on the edge b1 to b4: the path through b2 no longer
runs it.

## Runs
A machine phase before `phielim` (`flow::machine_with`), in every machine
at -O1 and above.
