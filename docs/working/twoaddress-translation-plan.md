# Translating TwoAddressInstructionPass

Pin d1106deb71cc, [`TwoAddressInstructionPass.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp). Plan only; the table of what differs is in [phi-pre-ra-translation](phi-pre-ra-translation.md). `twoaddr.rs` today is its own pass (update in place, commuting, a memory destination in a fresh register).

## The premise: a count, not a story

`render/dl.c` -O2, function `_dl_muzzle@3`, the same MIR (the `addresssink` stage of our build, parity 3b70c16822) through the pin's `llc` (`-print-after=`, copies counted as `COPY`) and through ours (`ISEL_DUMP=1`, copies counted as `vN := mov vN`; the two counts are not the same measure):

| after | pin | ours |
|---|---|---|
| PHIElimination | 34 | 33 |
| TwoAddressInstruction | 47 (+13) | 58 (+25) |
| RegisterCoalescer | 3 | 10 |

So ours adds twice the copies at this pass and leaves more after coalescing. That is what the translation has to move, and the first thing to find is how much of the 25 vs 13 is the pass and how much is the tied-operand set (four operations here, the descriptors' tied operands there).

## Data the pass needs that LIR does not have

- Kill flags: the pin reads `LiveVariables` or `LiveIntervals` (`isKilled`, `isPlainlyKilled`, L394-L483). LIR has none; use the intervals (`allocate::LiveAt`, `analysis::intervals`) and say so as a DEVIATE.
- Instruction distances (`DistanceMap`, `Dist`) and the bounded data-flow walk (`MaxDataFlowEdge`, L89): numbered by position in the block.
- Tied operands from the descriptors (`collectTiedOperands`, L1554): the x86 table says which source is tied to which destination; today `ties` lists four operations.
- No statepoints, no `REG_SEQUENCE`, no subregisters: `processStatepoint` (L1792) and `eliminateRegSequence` (L2035) are n/a (DEVIATE C1 of the coalescer says why).

## Pieces, in the pin's order of functions (one PR each, each priced on all rows and QCport, no tuning)

1. The helpers and maps: `getTiedUse`, `isRevCopyChain`, `noUseAfterLastDef`, `isCopyToReg`, `isKilled`, `isTwoAddrUse`, `findOnlyInterestingUse`, `getMappedReg`, `regsAreCompatible`, `removeMapRegEntry`, `removeClobberedSrcRegMap`, `regOverlapsSet` (L305-L645). No behaviour change by themselves: objects identical.
2. `isProfitableToCommute`, `commuteInstruction` (L648-L783), with `_commuted`/`_less` retired.
3. `isProfitableToConv3Addr`, `convertInstTo3Addr` (L784-L849): x86 `add` to `lea` at this stage; today selection makes the LEAs.
4. `scanUses`, `processCopy` (L850-L927): the hint maps, replacing `_copy_destinations`.
5. `rescheduleMIBelowKill`, `isDefTooClose`, `rescheduleKillAboveMI` (L928-L1284): the two reschedulers.
6. `tryInstructionCommute`, `tryInstructionTransform` (L1285-L1553).
7. `collectTiedOperands`, `processTiedPairs`, `run` (L1554-L2034): the driver; `ties` and `_untied` go.

Each piece has a numbered DEVIATE for what stays (kill flags, distances, the tied table), with an exit and a review date. The old pass stays in place until the piece that replaces its part is in; objects must be identical or each change explained row by row.

## First fail-first test

The far-pointer test `test_a_far_pointers_phi_is_not_copied_through_ax` is the premise for the coalescer side only (a phi temporary's class). For this pass the first test is a minimum of the count above: a function whose pin `llc` output after TwoAddressInstruction has fewer copies than ours, reduced from `_dl_muzzle@3` by `tools/stages.py`-style stage dumps. It is not chosen yet; picking it is the first step of piece 1's PR.

## Not done

No measurement of what any piece changes; the counts above are one function. The vreg class field is a data-model change and is not part of this plan (decided elsewhere).
