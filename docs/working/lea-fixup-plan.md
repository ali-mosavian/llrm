# X86 LEA Fixup (row 100): plan

Owner: w-6. Pin: [X86FixupLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp) at d1106deb71cc.

## What the pin does on i486

`runOnMachineFunction` ([L232](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L232)) reads `slowLEA`, `slow3OpsLEA`,
`leaUsesAG`; i486 sets none ([X86.td]), so only `optTwoAddrLEA`
([L564](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L564)) runs on each LEA, per block, in order:

1. `lea r1,[r1+r2]` (scale 1, no disp) -> `add r1,r2` (case 1).
2. `lea r,[r+d]` -> `inc`/`dec` for d = ±1, else `add r,d` (case 2).
3. `lea r3,[r1+r2]` before a `sub r3,r4` -> `optLEAALU` ([L500](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L500)).

All three need EFLAGS dead at the LEA and no segment, scale > 1, or symbol disp.

## Ours

The pin makes these LEAs before RA (`convertToThreeAddress`), RA turns some
two-address. Ours makes an LEA only in `peephole::addresses`, priced against
the add it replaces, and `increments` makes INC/DEC. Probe: 115 programs of
tests/run/c at -O2 -m32 sysv i486, 3855 LEAs, **0** scale-1 LEAs whose
destination is a source. Cases 1 and 2 have no site; case 3 not probed.

## Rows

- LF-1 DEVIATE (exit: pre-RA LEA formation, row 62): cases 1-2 are not
  translated; nothing makes the input.
- LF-2 open (owner w-6, 11-09): case 3 sites via `optLEAALU`.
