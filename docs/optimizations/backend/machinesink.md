> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinesink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinesink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinesink.rs sha256:d31ead3b1b5a49c5359735c096c1e4bb2bdbad931a9fb4d0bc3b04af7e18a9e4 -->

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

DEVIATE M7 (row 44): `hasStoreBetween` counts every store, call and
ordered access as one that may alias the load: no alias analysis reaches
LIR (`mayAlias(AA, MI)` is always true). Retires M1, which sank no load
along a critical edge. Exit: memory operands with an alias query on LIR.

DEVIATE M2 (#2903): `isProfitableToSinkTo` ends by asking the register
pressure of the target (`registerPressureSetExceedsLimit`) for an
instruction inside a cycle; no instruction here sinks on that ground.
Exit: the target description's pressure limits.

DEVIATE M4 (#2903): in a build for size an instruction with operands moves
only if each operand is live out of its block already (nothing is
lengthened over the edge: an `fsub` whose operands died there made the x87
stackifier hold them in memory, qcport `pl_trace` +125 bytes) and is read
by nothing below in the arm (the two-address form needs no copy: `x > C ?
x - C : x`, the `add` in the arm is in place). The pin's MachineSink has no
optsize rule; it moves any and its allocator splits the ranges around the
stretch. The gain and the loss come from one shape: a `lea` sunk into an
arm whose result a phi takes becomes `add; mov` plus a copy block on the
join edge. Rows (head 9e3c893585, base 47978c1a46; tools/rows/qcport.py, no
flags): QCport -Os 265956 -> 265871 (-85 bytes, 13 modules; larger: `sc` +2
of 11041, 0.018%, `d_alias` +1 of 12363, 0.008%); objects -Os m16 5 changed
-327 bytes, larger `bench/jpeg_fixed` +3 of 2958 (0.10%) and `jpeg_single`
+3 of 2826 (0.11%), m32 8 changed -244, none larger; x_adler -10.8%,
x_crc32 -6.7%, x_fixmul -6.1% clocks. Otherwise (-O1 to -O3) every safe
instruction moves; `bench/quicksort` -O2 m16 costs +985 clocks (+0.3%, a
reload more per trip). Exit: a test of the sink that prices the phi copy
from the target description (general, not a shape rule), 2026-11-04;
review 2026-11-05 with the greedy allocator and SplitKit as the default.

DEVIATE M5 (#2903): in a build for size no edge is broken. The block
costs a jump (`bench/recpow` -Os +5 bytes with the edge split; over the
834-object sweep at head 1b78847ae, breaking it at -Os is 37 objects larger,
+228 bytes, and 26 smaller, -106 bytes). `bench/hanoi` -Os m32 keeps the
`xor` and a copy before its test, and a frame, where the pin sinks the
constant (195 bytes against 190 with the edge broken). Exit, decided
2026-10-22 with jumps' block-by-block diff: a placement rule jumps names,
or sinking with shrink-wrapping (lir-cleanup-port for the sinking half).

DEVIATE M6 (no number): an instruction that reads or writes any physical
register stays, where `FindSuccToSinkTo` lets a physical def marked dead
through (and a use of a constant register). Effect: none seen, 0 sinks
missed over d_faces and the 820-object corpus at -O2 (counted: no
instruction that passes `safe_to_move` has a physical write and no read).
The flags are the pin's rule in full: a def moves only if dead (read by no
later instruction of its block and no successor), and not into a block that
reads them (`SinkInstruction`'s zombie check).
Exit: honor a dead implicit def when LIR marks one.

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
