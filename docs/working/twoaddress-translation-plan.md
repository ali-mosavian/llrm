# Translating TwoAddressInstructionPass

Pin d1106deb71cc, [`TwoAddressInstructionPass.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp). Plan only; the table of what differs is in [phi-pre-ra-translation](phi-pre-ra-translation.md). `twoaddr.rs` today is its own pass (update in place, commuting, a memory destination in a fresh register).

## The premise: a count, taken the same way on both sides

`render/dl.c` -O2 at parity 3b70c16822. The pin: `llc -mtriple=i386-pc-linux-code16 -frame-pointer=all -O2 -print-after=phi-node-elimination,twoaddressinstruction,register-coalescer` on the build's own `37-addresssink.ll`, counting `COPY` opcodes in the pin's printed MIR per function. Ours: `ISEL_DUMP=1` of the same compile, counting `mov` with one register operand (loads, stores and immediates excluded). The two opcodes are not the same measure: the pin's `COPY` also carries the ABI argument and return moves, ours has those as pseudo-ops.

| function | PHIElimination pin/ours | TwoAddress pin/ours | Coalescer pin/ours |
|---|---|---|---|
| `_dl_muzzle@3` | 34/36 | 47/61 | 3/13 |
| `_dl_decay@3` | 27/18 | 34/24 | 10/3 |
| `_dl_explosion@3` | 23/25 | 30/34 | 3/8 |
| `_dl_live@3` | 34/30 | 38/36 | 7/3 |
| `_dl_mark@3` | 53/42 | 59/51 | 15/8 |
| `_dl_relink@3` | 72/66 | 101/107 | 12/28 |

Copies added by the pass, pin/ours: muzzle 13/25, decay 7/6, explosion 7/9, live 4/6, mark 6/9, relink 29/41. Ours adds more in every function but decay. After the coalescer the sign flips in decay, live and mark (ours fewer), so the final count is not one-directional and the opcode difference may be most of it. What the count supports: the pass adds more copies than the pin's in five of six functions, most in muzzle and relink. It does not support a claim about the final copies.

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

The far-pointer test `test_a_far_pointers_phi_is_not_copied_through_ax` is the premise for the coalescer side only (a phi temporary's class). For this pass the first test is a reduction of `_dl_muzzle@3` (13 added by the pin, 25 by ours) to the smallest function where ours adds more copies than the pin at TwoAddressInstruction. It is not chosen yet; picking it is the first step of piece 1's PR.

## Not done

No measurement of what any piece changes; the counts above are six functions of one file. The vreg class field is a data-model change and is not part of this plan (decided elsewhere).
