# Reusing finite array values

FPDEEP now shares the loaded p(i) value within each arithmetic expression.
Its initialized array bytes survive the raised call-memory effects; counted
loop bounds and the index shift prove which aligned elements are read.
All candidate elements must decode to finite integers, and interval arithmetic
must prove each intervening operation exact at every supported precision.
Missing bytes, unknown alignment, segment values, wrap, NaNs, infinities,
denormals and nonintegral elements cannot establish this proof.

The analysis enumerates at most 64 candidate offsets. A shift establishes
alignment; an interval alone does not. Unknown effects invalidate memory
facts. This is not a declaration that arrays or constant pools are immutable.
No register names or CPU timing choices enter the MIR optimization.

PDS ratio expression before:

```asm
fld  dword [si]
fmul dword [si]
fld  dword [si]
fadd dword [si]
fdivp
fstp dword [q]
```

After:

```asm
fld  dword [si]
fld  st(0)
fmul st,st(0)
fxch
fadd st,st(0)
fdivp
fstp dword [q]
```

The first load and its exception checkpoint remain. Arithmetic order,
division and SINGLE storage conversion remain. The second stack value is
materialized by allocation rather than deleting a required stack slot.

Across SQ, RATIO and MIX, indexed array reads fall from eight to three per
iteration. PDS object size falls from 1538 to 1509 bytes. The static score
is unchanged at 11829: its coarse operation weights offset the removed
memory touches against stack operations. No runtime speedup is claimed.
The DOUBLE copy and complete constant-loop elimination remain outstanding.

Verification: three production reuse regressions failed before the change;
128 focused floating/range tests pass afterward. The existing missing-proof
test now suppresses both constant and interval proofs, retaining its original
assertion. Only FPDEEP changed in a before/after ordinary PDS emission scan.
All 33 output cases pass across PDS, QB and VBDOS with LIR emission required.

Stages and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-array-reuse-final-75n2fcfg`.
The adjacent `s18-mir-r02-hoist.txt` / `s19-mir-r02-forward.txt` diff shows
the memory operands replaced by held values; subsequent CSE removes the
repeated loads.

## Allocate the next-used operand on top

The next backend change keeps `p` on top when it will be used before `p*p`.
After duplicating p, the square can overwrite ST(1), leaving ST(0) ready for
the addition. This changes no MIR operation or evaluation order:

```asm
; Before                         ; After
fld  dword [si]                  ; fld  dword [si]
fld  st(0)                       ; fld  st(0)
fmul st,st(0)                    ; fmul st(1),st
fxch                             ; removed
fadd st,st(0)                    ; fadd st,st(0)
fdivp                            ; fdivp
fstp dword [q]                   ; fstp dword [q]
```

This destination choice is limited to duplicated identical operands of
register-register addition/multiplication, when the retained input's next
use precedes the result's. Other operations keep their existing allocation.
PDS size falls a further 1509→1506 bytes. Static cost falls by 100 units:
PDS 11729/1086=10.80x, QB 11947/1086=11.00x, VBDOS 11659/1086=10.74x.
These remain ranking scores, not runtime measurements.

The allocation regression failed first; 109 focused allocation/selection/
floating-bound tests pass. FPDEEP alone changed in the ordinary PDS scan;
all 33 runtime output cases pass across the three compiler variants.
Artifacts and full stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-float-destination-2byduooy`.
