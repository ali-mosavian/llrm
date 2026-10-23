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
  owner w-8, 11-09.

The two-address pass makes the LEAs now: `convertInstTo3Addr` for a dying source whose sum ends in another register (`isProfitableToConv3Addr`, TA-5 closed) and `hasCommutePreference` (TA-2 closed) keep the LEA as the add's second source, so `optLEAALU` has its shape; `LiveRangeShrink` counts a delivered value as the pin's `COPY`, so it leaves the order alone.
