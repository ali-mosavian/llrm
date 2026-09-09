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
