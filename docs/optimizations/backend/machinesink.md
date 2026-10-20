> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinesink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinesink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinesink.rs sha256:39eb673188f57e687e0833b5d0bda499978299c0782b48651b87e7f2e167be70 -->

# machinesink

Moves a computation into the one successor block that uses it, breaking a
critical edge where that is what lets it go. Translated from
[MachineSink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp)
(`MachineSinking`, before PHI elimination, on SSA).

## How it works
Each block with several successors is walked bottom-up. An instruction that
is safe to move (`safe_to_move`: no store, call or volatile access, a
load only above every store of its block, no register but virtual ones; the flags it writes dead: read by no later instruction of its block and live into no successor, and not live into the block it moves to)
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

DEVIATE M1 (#2903): a load is sunk only into a successor with no other
predecessor, or a block made for the edge. LLVM also sinks it along a
critical edge when `hasStoreBetween` finds no store on any path between.
Exit: that function translated.

DEVIATE M2 (#2903): `isProfitableToSinkTo` ends by asking the register
pressure of the target (`registerPressureSetExceedsLimit`) for an
instruction inside a cycle; no instruction here sinks on that ground.
Exit: the target description's pressure limits.

DEVIATE M4 (#2903): in a build for size only an instruction that reads no
value moves (a constant). LLVM moves any, and its allocator splits the
operands' ranges around the stretch. Rows with every instruction movable
at -Os: `bench/jpeg_single` m16 +61 bytes, `jpeg_fixed` m16 +50, `huge_runs`
+29 (all 36 -Os objects that differ: 8 larger, +163, 21 smaller, -998).
Otherwise (-O1 to -O3) every safe instruction moves; `bench/quicksort`
-O2 m16 costs +985 clocks (+0.3%, a reload more per trip). Exit: the
greedy allocator with SplitKit as the default allocator (target 2026-11-05).

DEVIATE M5 (#2903): in a build for size no edge is broken. The block
costs a jump (`bench/recpow` -Os +5 bytes with the edge split; over the
834-object sweep at head 1b78847ae, breaking it at -Os is 37 objects larger,
+228 bytes, and 26 smaller, -106 bytes). `bench/hanoi` -Os m32 keeps the
`xor` and a copy before its test, and a frame, where the pin sinks the
constant (195 bytes against 190 with the edge broken). Exit, decided
2026-10-22 with jumps' block-by-block diff: a placement rule jumps names,
or sinking with shrink-wrapping (lir-cleanup-port for the sinking half).

DEVIATE M3 (#2903): no `PerformSinkAndFold`, no sinking into cycles
(`SinkInstsIntoCycle`, off by default), no debug-value handling (an
instruction has none).

`pressuresink` (a load made where its reader is, within a block) has no
pin counterpart: LLVM orders loads by the scheduler. It is deleted when the
pre-RA machine scheduler is translated (target 2026-11-20).

## Example
`b1: v = mov 0; cmp x, 0; je b4` and `b4: r = phi [b1: v] [b2: w]` leave
`v = mov 0` in a block on the edge b1 to b4: the path through b2 no longer
runs it.

## Runs
A machine phase before `phielim` (`flow::machine_with`), in every machine
at -O1 and above.
