> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/machinelicm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/machinelicm.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/machinelicm.rs sha256:1556dc8ee472668759f3be2cd6ea20fd33650d8ca8278d901f837ad48c5b30c8 -->

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
  (`InitializeLoadsHoistableLoops` L1472, `IsGuaranteedToExecute`); DEVIATE
  ML2: an `fld1`/`fldz` and its `fchs` also move where the function writes
  the control word, when the loop is innermost and only arithmetic reads
  them (below);
- every value it reads is defined outside the loop or was hoisted
  (`IsLoopInvariantInst`);
- it is not as cheap as a move, unless it is a register copy that is the
  only reader of its source (`calcRegisterCost` is nil: bintree -O1
  `insert`'s zero-extended `lea` index, then the load it feeds)
  (`IsProfitableToHoist`: a cheap instruction is hoisted only if it does not
  raise the pressure, and a constant cannot be made again where the
  allocator wants it, DEVIATE E3 of `precse`);
- it sets no flags a later instruction of its block reads;
- the loop does not already use as many values of its register class as the
  target has registers (`CanCauseHighRegPressure`, with the values named in
  the loop standing for the pressure: DEVIATE ML1, the pin tracks the live
  registers of each block of the loop; a block that does not run on every
  trip also needs the loop's peak of live registers below that,
  `AvoidSpeculation`).

Not ported, priced at no yield (release/next 34d21c9f34, every C, nib and
BASIC program at -Os and -O2, counters in a throwaway build):
`getOrCreatePreheader` splitting an edge: the 6 to 32 loops per language
and level without a preheader hold 0 hoistable instructions;
`ExtractHoistableLoad`'s pricing held for the C, nib and BASIC corpus (no
scalar SSE) and is wrong for SSE code: nbody_single -O3 m64 left
`mulss xmm, [rip+K]` in a loop where the pin has the constant in a register
before it. It is ported (`Policy::unfold`): scalar SSE arithmetic that reads
a constant-pool cell is split, and the load moves if it can.

`MayCSE`: only reached in the pin's high-pressure branch for an instruction
outside a block that runs every trip, where the `constant` path above
already hoists what the allocator can make again.

DEVIATE ML2 (owner x87, exit 2026-12-08), 11-08 re-check: the pin keeps
`fld1; fchs` inside the loops of x_ld_abs -O2 (`-march=i486`, the pinned
clang's listing) and gcc carries the constant on the stack (`fsub st,
st(1)`), so neither exit holds. The pin's `FLD1` and `FLDZ` read `$fpcw`,
which the function's `FLDCW` (for an `fptosi`) writes, so nothing moves and
`fld1; fchs; faddp` (18 clk) is made on every trip of a loop that adds
`-1.0`. Both and `fchs` are exact under any rounding control. A constant
that an `fchs` flips moves, with the flip, when the loop is innermost,
stores no x87 value, every reader is an arithmetic instruction beside a
register (a compare wants the constant on top, a memory operand makes the
held value be copied: `fxch`es and `fld st(i)` cost what the hoist saves)
and the stack has room (`CARRY_RESERVE`); not at -Os, where the `fstp` that
ends the carried value costs 2 B (as X1). A plain `fld1`/`fldz` stays: it
saves 4 clk and the loop's back edge pays `fxch`es (recfloat, +8%).
x_ld_abs -O2 13,320 -> 12,693 clk (gcc 13,113) at 80 bits, 12,684 ->
12,057 at 64; double x_horner -O2 92,084 -> 83,304. The exit: does the
pin's `isConstantPhysReg` stop counting `FLD1`/`FLDZ`, or gcc's reg-stack
keep the same cells.

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
