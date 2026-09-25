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

## Arithmetic operands reuse exact stores

Forwarding now also replaces binary32/binary64 arithmetic memory inputs
with live extended values from exact stores. It preserves arithmetic order,
precision and rounding. Aliasing writes invalidate providers; unknown or
potentially exceptional intervening operations clear them. Lowering accepts
this explicit input-format change, while retaining its other semantic guards.
Allocation consumes a last-use register operand with popping arithmetic,
including the reversed forms required to preserve subtraction and division.

FPCSE initially cannot use this: the loop accumulator is unknown. After the
existing exact loop specialization, its accumulator is 438.75, and the next
forwarding round removes both product memory reads. The actual PDS sequence
changes as follows (symbolic names replace relocated operands):

```asm
; Before                         ; After
fstp dword [p]                   fst dword [p]
                                 fxch
fdiv dword [c]                   fdiv dword [c]
fstp dword [q]                   fst dword [q]
fld dword [s]                    fld dword [s]
fadd dword [p]                   faddp st(2),st(0)
                                 fxch
fadd dword [q]                   faddp st(1),st(0)
fstp dword [s]                   fstp dword [s]
```

Two memory reads disappear but two exchanges are added. Reachable code grows
two bytes; the complete object shrinks from 933 to 925 bytes because fewer
relocations are needed. This is value reuse, not a measured timing win.
FPCSE prints the expected 487.5 on QB /O, PDS /G2 and VBDOS /G3. All three
compile logs report zero severe errors. 98 focused checks pass; the three
fixture regressions and four arithmetic-order cases fail with their respective
changes disabled. Dumps: `/tmp/qbopt-float-arithmetic-False` (before) and
`/tmp/qbopt-float-arithmetic-final` (after).

### Stack-order selection

Allocation now chooses the normal or reversed popping form from the current
operand positions. If the right operand is already on top, it need not be
exchanged with the left before arithmetic. Subtraction and division select
their corresponding non-reversed forms to retain the original operand order.

FPCSE's final `faddp st(2),st(0); fxch; faddp` is now simply
`faddp st(2),st(0); faddp`. The earlier exchange before division remains.
The PDS object shrinks another three bytes, 925 to 922, with the software-FP
protocol intact. All three fixture configurations assert one exchange, not
two. Eight allocator cases cover both stack orders and all four arithmetic
operations; the four right-on-top cases failed before this change.
61 focused checks pass, and VBDOS FPCSE still prints the expected answer.
Stage dumps: `/tmp/qbopt-float-stack-order`.
