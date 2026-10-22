> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinelicm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinelicm.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinelicm.rs sha256:14448644d43b7dc2c95caf1cd038594eefaf6ec4a431071220c1b78386ee8cdc -->

# machinelicm

Early `MachineLICM`: a pure computation of values defined outside a loop,
made in the loop, is made once in its preheader. Translated from
[MachineLICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp)
(`HoistOutOfLoop` L827, `IsLICMCandidate` L1098, `IsLoopInvariantInst`
L1133, `IsProfitableToHoist` L1273, `CanCauseHighRegPressure` L1235,
`Hoist` L1615).

## How it works
The loops come inner first. A loop with one block outside it that enters
the header, and that block leaving only for the header (its preheader),
is walked in layout order, and again until nothing moves. An instruction
moves to the end of the preheader, before its jump, when:

- it is a pure operation on registers and constants with one result, no
  pinned operand, no memory, no call, no x87 value (`candidate`);
- every value it reads is defined outside the loop or was hoisted
  (`IsLoopInvariantInst`);
- it is not as cheap as a move (`IsProfitableToHoist`: a cheap instruction
  is hoisted only if it does not raise the pressure, and a constant cannot
  be made again where the allocator wants it, DEVIATE E3 of `precse`);
- it sets no flags a later instruction of its block reads;
- the loop does not already use as many values of its register class as the
  target has registers (`CanCauseHighRegPressure`, with the values named in
  the loop standing for the pressure: DEVIATE ML1, the pin tracks the live
  registers of each block of the loop).

DEVIATE ML2: no preheader is made where a loop has none; no load of an
invariant cell (`ExtractHoistableLoad`); no CSE of the hoisted value
(`MayCSE`).

## Runs
A machine phase before `precse` (`flow::machine_with`), at every -O level
but -O0.
