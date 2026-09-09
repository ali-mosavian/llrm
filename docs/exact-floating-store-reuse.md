# Exact floating store reuse

CSE now records an exact binary32/binary64 store as a provider for a later
load of the same bytes. The store remains. Its source value is kept alive
by ordinary SSA uses, with no register names in the optimization.

The source must have a proven finite value exactly representable in the
storage format. The existing same-block, memory-alias and nonexceptional
intervening-operation checks still apply. Unknown or rounding stores do not
provide a value. This does not bypass the unresolved runtime memory effects
that currently prevent FPBENCH's initial literal facts from surviving.

Controlled witness made from FPCSE's real raised operations, changing its
second load to read its first stored product. Allocated instruction forms:

```asm
; Before
fstp dword [product]
wait
fld  dword [product]

; After
fld  st(0)
fstp dword [product]
wait
```

This replaces the memory reload with a stack-register duplicate; it does
not reduce the instruction count. A future backend choice could use a
non-popping store where appropriate, but this change does not implement it.

The positive regression failed before implementation. Guards cover unknown
values, rounding, an intervening unknown operation and a same-cell write.
98 focused floating and architecture checks pass. FPEMU, FPCSE and FPDEEP
PDS /G2 production instruction counts are unchanged. No runtime speedup is
claimed. Production stage dumps: `/tmp/qbopt-exact-store-stages`.

## Follow-up: retain live values with a non-popping store

Floating allocation now selects `fst` for a binary32/binary64 memory store
whose input remains live. It needs neither a duplicate nor an extra stack
slot. Extended80 stores still duplicate and pop because there is no matching
non-popping memory encoding. The last use still pops normally.

Real FPICSE, FPI2CS and FPCALC fixtures from QB, PDS and VBDOS now emit one
`fst` and one `fstp`, with a single integer conversion. Previously their
first store emitted `fld st(0); fstp [destination]`. Thus nine fixture
configurations each remove one duplicate instruction. MIR is unchanged;
the first stage difference is floating allocation.

115 focused checks pass, including fail-first non-popping-store cases and
the extended80 guard. VBDOS /G3 executions of the three programs pass all
three cases each against their expected answers. No timing claim is made.
Before/after stage dumps are `/tmp/qbopt-retained-store-before` and
`/tmp/qbopt-retained-store-after`.
