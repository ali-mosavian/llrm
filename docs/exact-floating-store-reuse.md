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
