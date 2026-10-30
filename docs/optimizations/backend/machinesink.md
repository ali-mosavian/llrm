> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/machinesink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/machinesink.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/machinesink.rs sha256:4c6407b4c8dff0d8f7d790edfcc0f9de55444d315e8ce5943f91cecd1d78c4b6 -->

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

Row M2 (was DEVIATE M2 #2903, retired 2026-10-25): inside a cycle an
instruction goes to a post-dominating block when no in-cycle operand it
reads would be held there past the block's pressure (the limit is the number
of allocatable registers, one for every set). x_bubble -O2 m32: the inner
loop's `inc` goes to the latch as the pin has it.

Row M4 (was DEVIATE M4, retired 2026-10-25 on the greedy allocator): a build
for size moves what the pin moves; the rule that kept an instruction in
place when its operand lives on, is read below the arm or dies in the block
is gone. Measured (matched pair, own dists, rows, qcport.py, objsweep):
x_collatz -Os clocks -7.2%, instructions -11.7%, code -4.4% (the pin's
shape: `lea` into the odd arm, `shr` into the even one), sieve -10.3%
clocks, matmul -2.9% clocks and -5.3% code; objects -Os: x_mergesort m16
-219 B, x_collatz m16 -105, matmul m32 -113, sieve m16 +82; QCport -Os
263235 -> 263448 (+213 B, +0.081%, 40 modules changed; -O1 to -O3
identical). The rule had bought -85 B of QCport -Os on the old allocator.

Row M5 (was DEVIATE M5 #2903, retired 2026-10-28): a build for size breaks
a critical edge as the pin does (`isWorthBreakingCriticalEdge` has no size
check). Measured (llrm-cc, bench and tests/run C programs, -Os, own
binaries a pair): m16 179 programs +42 B (8 smaller, 12 larger by at most
7, one by 24), every changed program within +9 B of main's figure; m32 182
programs +82 B (18 larger by 4 to 12, one by 24, none smaller): each
split edge costs a jump. QB `parity/branch` -Os bq32 kept the `shl`, `sub`
and `add` of one arm above the test (47 instructions against 30).

Row M6: an edge of a loop is broken in a build for size too (x_collatz -Os
m32 313 -> 309 B, m16 340 -> 334, no object grows): the
pin's `mov ecx, 32` of an exit phi runs once, in a block of its own, not
every trip in the latch. Measured (own dist pair, sizes.py, clocksweep, m16
-Os, 312 programs): 185933 -> 185910 B, 10 programs changed (6 smaller, 4
larger by 1 to 2 B), no program slower; mandel.nib 185 -> 178 B and
-3.96% clocks, mandel.c -2.2%, mandel.bas -1.5%.

DEVIATE M6 (no number): an instruction that reads or writes any physical
register stays, where `FindSuccToSinkTo` lets a physical def marked dead
through (and a use of a constant register). Effect: none seen, 0 sinks
missed over d_faces and the 820-object corpus at -O2 (counted: no
instruction that passes `safe_to_move` has a physical write and no read).
The flags are the pin's rule in full: a def moves only if dead (read by no
later instruction of its block and no successor), and not into a block that
reads them (`SinkInstruction`'s zombie check).
Exit: honor a dead implicit def when LIR marks one.

DEVIATE M7 (no number): an instruction whose pinned operands are all
sources (the count of `shld`/`shl r, cl`) stays, as the pin's `$cl = COPY`
makes it read a physical register; ours names the pin only in the register
class, so `touches_machine_state` asks the classes. One with a pinned result
(a divide: `ax`, `dx`) still sinks: the pin's does not, but holding a divide
in its block grew switch.c 16 B at m16 -Os. Exit: none.

DEVIATE M3 (#2903): no `PerformSinkAndFold`, no sinking into cycles
(`SinkInstsIntoCycle`, off by default), no debug-value handling (an
instruction has none).

## Example
`b1: v = mov 0; cmp x, 0; je b4` and `b4: r = phi [b1: v] [b2: w]` leave
`v = mov 0` in a block on the edge b1 to b4: the path through b2 no longer
runs it.

## Runs
A machine phase before `phielim` (`flow::machine_with`), in every machine
at -O1 and above.
