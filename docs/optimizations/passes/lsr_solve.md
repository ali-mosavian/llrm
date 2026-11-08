> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_solve.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_solve.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_solve.rs sha256:dadb5691a1b5f1f135445e61dbffc66af7336d7ccb7ba3ca7f76c53f73e924dc -->

# lsr_solve

The solver of LLVM's loop strength reduction: one formula per use, by a
depth-first search pruned by the best cost found (`SolveRecurse` and
`Solve`; LoopStrengthReduce.cpp at `d1106deb71cc`, l.5573-5710).

## How it works
Uses are decided in order. A formula of a use must reference every register
already chosen that the use could also reference (up to its register count),
which prunes most of the search; its cost is rated on top of the cost so
far, and the branch is dropped when it is no better than the best solution.
A formula of the first use that is a single register is remembered as
visited: formulae later in the search that name it lose.

## Example
The array walk and its exit test: the store keeps `{%p,+,4}`, the compare
keeps `{(-1 + %n),+,-1}`, one register each, as `opt -passes=loop-reduce`
has them.

## Runs
Nothing runs it yet: stage 5 of the translation of LSR.

## Limits
DEVIATE: `AMK_PreIndexed` is not ported, and so a loop that wants `-(An)`
(a pointer stepped down before each access) stays a step and an access.
The pin sites: `RateRegister`'s `CanPreIndex`
([L1468](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1468))
and the offset-minus-step formulae of `GenerateConstantOffsetsImpl`
([L4230](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L4230)).
No target of ours prefers it (`getPreferredAddressingMode` is `PostIndexed`
or `None`), and the overlay has no `-(An)` fold either. Exit: with the first
target that prefers pre-indexed addressing. The
`-lsr-preferred-addressing-mode` option (L170) is not either: the target's
hook decides. Ported, for `AMK_PostIndexed`: L1445, L1461-1471, L3780,
L3988, L4735, L5196, L5609 and `getFixupInsertPos` (L6190,
`lsr_implement.rs`; a store keeps the default position, as the pin's
`I->getType()` is `void` for it). DEVIATE LU3: see `solve`, the drop of a
rewrite that spills (real mode).

DEVIATE LS1: real mode (m16) skips the pin's `VisitedRegs` rule (a
one-register formula of the first use makes its register a loser for the
later uses; `Tti::lsr_visited_registers_lose`). With the rule on, m16 is
+1..+8 B in 41 of 192 bench cells and better in none (histo's sum loop
keeps a pointer beside the counter and a `cmp`). Every other target runs the
pin's rule. owner: w12, exit: 2026-11-30.

DEVIATE LU3b: the registers the loop keeps live beside its recurrences (the
values its body defines that SCEV does not make a recurrence, in registers
of the target's width, `Tti::gpr_bits`) are taken off the budget the solver
and the spill check count against; the pin counts none. Without it shellsort
bas -Os spent a third formula register to save an instruction and the
allocator spilled the `i32` chain (+6 B). No pin run can decide it: the pin
has no m16 TTI. Where no solution fits the registers left, the cut is
dropped and the search runs again without it: every one spills and the count
no longer tells them apart (stft_fixed c -Os, +11 B). Exit: a pressure model
on the pin's MachineLICM/RegisterPressure terms, by 2026-11-20, or when the
allocator's spill cost is priced in the comparator.

DEVIATE LU3e (real mode, `Tti::lsr_prices_spills`): a register past the
loop's real budget costs the references per register so far
(`Cost::spill_extra`), not the pin's one instruction, and the loop's real
pressure counts there even when LU3b's retry drops the cut. It ranks
solutions; the test against the loop as it stands does not read it. m16 grep
-11/-6/-6 B at -O1/-O2/-O3, no m16 cell worse. m32 (12 better, 6 worse) and
m64 keep the pin's one. owner: w12, exit: 2026-11-20, with LU3b.
