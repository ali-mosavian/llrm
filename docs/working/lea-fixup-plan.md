# X86 LEA Fixup (row 100): plan

Owner: w-6. Pin: [X86FixupLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp) at d1106deb71cc.

## What the pin does on i486

`runOnMachineFunction` ([L232](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L232)) reads `slowLEA`, `slow3OpsLEA`,
`leaUsesAG`; i486 sets none (X86.td: `Proc<"i486", [FeatureX87]>`), so only `optTwoAddrLEA`
([L564](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L564)) runs on each LEA, per block, in order:

1. `lea r1,[r1+r2]` (scale 1, no disp) -> `add r1,r2` (case 1).
2. `lea r,[r+d]` -> `inc`/`dec` for d = ±1, else `add r,d` (case 2).
3. `lea r3,[r1+r2]` before a `sub r3,r4` -> `optLEAALU` ([L500](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L500)).

All three need EFLAGS dead at the LEA and no segment, scale > 1, or symbol disp.

## Ours

Cases 1 and 2: group `lea_fixup` of `peephole.peep` (`lea_fixed`, run before
`increments`, which makes a step of one an INC or DEC as the pin's case 2).
Guards `sums_into`, `steps_in_place`; makers `other_term`, `step`. EFLAGS dead
is `flags_dead`; the pin's segment and symbol exclusions are `addr` unset.

The pin makes these LEAs before RA; ours makes an LEA only in
`peephole::addresses`. Probe: 115 programs of tests/run/c at -O2 -m32 sysv
i486, 3855 LEAs, **0** scale-1 LEAs whose destination is a source, so the
rule fires nowhere there; the unit tests build the shape.

## Rows

- LF-2 DEVIATE CG6: case 3 (`optLEAALU`, [L500](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L500)) is translated (peephole.peep `lea_alu`) for the add or sub right after the LEA; the pin looks five instructions ahead (`searchALUInst`, `checkRegUsage`). A window rewrite is slot for slot: a gap would put the two steps where only one of the pin's two placements allows.
  owner w-8, review 11-30. Measured 2026-10-23: 79 two-register LEAs over the 71 bench programs at -O2 and -Os, none with its add or sub within five instructions; no site, no cost.

The two-address pass makes the LEAs now: `convertInstTo3Addr` for a dying source whose sum ends in another register (`isProfitableToConv3Addr`, TA-5 closed) and `hasCommutePreference` (TA-2 closed) keep the LEA as the add's second source, so `optLEAALU` has its shape; `LiveRangeShrink` counts a delivered value as the pin's `COPY`, so it leaves the order alone.


## Translation in the pin's file order (w-8, from 2026-10-24)

The pin's file, function by function, with whether any `-march` llrm supports reaches it. `of_cpu` takes i386, i486, pentium, pentiumpro, k5, k6, athlon, core2 and x86-64. `slow-lea`
(`TuningSlowLEA`, X86.td L1350, L1375, L1390) and `lea-uses-ag` (L1336) belong to Atom, Silvermont and kin: none of the nine. `slow-3ops-lea` is in `X86_64V1Tuning` (L971), so `x86-64` reaches the
3-operand path; `TuningSlowIncDec` (L973) makes `OptIncDec` false there unless the block is optimised for size.

| # | Pin function (L) | Reached by | Ours now | PR | Date |
| --- | --- | --- | --- | --- | --- |
| 1 | `isLEA` (227), `runOnMachineFunction` (232), `getPreviousInstr` (295), `searchBackwards` (308) | all | none: two peephole groups (`lea_fixup`, `lea_alu`) run in the peephole walk, not per block in the pin's order | the pass `fixupleas.rs` with the block loop, the predicate and the two searches; slot registered, rules still in the peephole | 11-02 |
| 2 | `getADDrrFromLEA` .. `getINCDECFromLEA` (351-400), `checkRegUsage` (466) | all | `other_term`, `step`, the guards in `guards.rs` | the opcode maps and the register-usage check as functions, tested one by one | 11-04 |
| 3 | `optLEAALU` (500) | all | peephole `lea_alu` (LF-2: window of five at the pin's, no site on the bench) | moved into the pass, the group removed from peephole.peep | 11-06 |
| 4 | `optTwoAddrLEA` (564) | all | peephole `lea_fixup` (cases 1 and 2) | moved into the pass, the group removed | 11-08 |
| 5 | `processInstrForSlow3OpLEA` (752), `isInefficientLEAReg` (331), `hasInefficientLEABaseReg` (340), `mayHaveOffset` (347) | `x86-64` only | none | translated, tested at `-march=x86-64` | 11-10 |
| 6 | `processInstruction` (664), `seekLEAFixup` (681), `processInstructionForSlowLEA` (701) | none of the nine | none | not translated: no supported CPU sets `slow-lea` or `lea-uses-ag`. Exit: the first PR that adds such a CPU translates them | n/a |

Each row is one PR with its tests and the byte-identity check on the bench programs (objects must not change where the pin's pass has no site). A row that changes bytes states the site.
