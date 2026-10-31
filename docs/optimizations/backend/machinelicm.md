> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/machinelicm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/machinelicm.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/machinelicm.rs sha256:80f77d179d1ab5620248331a11a5efec70e042766a114d2b51e7179c59b0a09d -->

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
  pinned operand, no call, no x87 value (`candidate`), an `fld1`/`fldz` in a
  loop with no call and a free stack slot, in a function that never writes
  the control word (`constant_load`), or a load in a block that runs on
  every trip of a loop with no store, call or volatile access
  (`InitializeLoadsHoistableLoops` L1472, `IsGuaranteedToExecute`);
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

Not ported, priced at no yield (release/next 34d21c9f34, every C, nib and
BASIC program at -Os and -O2, counters in a throwaway build):
`getOrCreatePreheader` splitting an edge: the 6 to 32 loops per language
and level without a preheader hold 0 hoistable instructions;
`ExtractHoistableLoad`: 0 instructions, none of the 158 loop instructions
flagged invariant (`isDereferenceableInvariantLoad`) is a folded load;
`MayCSE`: only reached in the pin's high-pressure branch for an instruction
outside a block that runs every trip, where the `constant` path above
already hoists what the allocator can make again.

## Example
A loop that adds `a[i] * k3` where `k3 := k + 3` is made inside it (the
sum is made by instruction selection, after the IR's LICM has run). The
sum is made once, ahead of the loop:

```text
  pre:                              pre:
    jmp loop                          v9 := add k, 0x3
  loop:                       =>      jmp loop
    v9 := add k, 0x3                loop:
    v10 := imul v8, v9                v10 := imul v8, v9
```

## Runs
A machine phase before `precse` (`flow::machine_with`), at every -O level
but -O0.
