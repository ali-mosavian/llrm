# Native FPU synchronization

Native arithmetic must not inherit BASIC's statement-by-statement exception
checkpoints. Preserving those checkpoints belongs to `--basic-semantics`,
not to the default native arithmetic policy. This does not authorize changing
rounding, reassociating expressions, or assuming NaNs cannot occur.

## Evidence and implementation boundary

- `raising_floats.annotated` turns original WAITs into MIR `FCHECK` operations.
  `raising_numeric_policy.native` removes INTEGER `INTO`; its `checkpoints`
  policy removes `FCHECK` and marks floating exceptions as deferred.
- `backend/floatalloc._integer_stores` independently inserts WAIT before and
  after materialized `fistp` only when preserving BASIC semantics.
- Local GCC `gcc/config/i386/i386.md`, `x86_fnstsw_1`, emits `fnstsw` directly.
  `i386.cc`, `output_fix_trunc`, emits the conversion and any required
  rounding-control changes without an explicit WAIT.
- Intel SDM volume 1, sections 8.3.12 and 8.6, separates pending-exception
  delivery from arithmetic execution. Ordinary x87 arithmetic instructions
  already check pending exceptions. An explicit checkpoint before integer
  work preserves the fault context for a recovery handler; it is not a
  general requirement to serialize every arithmetic operation.

Source: [Intel SDM volume 1](https://www.intel.com/content/dam/support/us/en/documents/processors/pentium4/sb/25366521.pdf).

## Implemented

Source checkpoints are dropped at raise under native semantics. Exact float
folding uses the machine-independent deferred-exception property instead of
reintroducing `FCHECK`. Backend conversion materialization receives the same
semantic policy. Rounding, arithmetic order and floating formats are unchanged.
Compare condition export is separate from trapping; this change does not
rewrite the status-export instruction or its consumers.

## Loop-invariant floating arithmetic

Native LICM now moves invariant floating arithmetic out of proven nonempty
loops when the operation executes on every first-iteration path and the
floating environment stays unchanged. Calls, opaque operations, checkpoints,
bypass paths and intervening cycles prevent this motion. BASIC mode retains
the original exception checkpoints and arithmetic placement.

FPCSEX, after CSE and then after LICM (symbol names replace relocations):

```asm
; before: each iteration         ; after: once before the loop
fld  dword [a]                   fld  dword [a]
fadd dword [b]                   fadd dword [b]
fld  st0                         fld  st0
fmul dword [c]                   fmul dword [c]
fstp dword [p]                   fstp tword [bp-10]
fdiv dword [c]                   fdiv dword [c]
fstp dword [q]                   fstp tword [bp-20]
                                 ; each iteration
                                 fld  tword [bp-10]
                                 fstp dword [p]
                                 fld  tword [bp-20]
                                 fstp dword [q]
```

The ordered `s = s + p + q` remains inside the loop on both sides. SINGLE
stores retain their rounding; cross-block storage is extended precision.
For ten iterations, multiply and divide execute once each instead of ten
times. The object grows from 952 to 967 bytes; this is work reduction, not
code-size reduction. Two invariant reloads per iteration remain a backend
opportunity, not a completed register-residency optimization.

Evidence: emitted-loop checks cover QB, PDS and VBDOS in native and BASIC
modes. PDS matches the original in all 144 qualified QEMU state comparisons.
82 focused checks pass, including strict trap-reference validation. Disabling
floating hoisting fails the emitted-loop regression. Restoring the raw pin
lookup fails the typed-pin refusal regression.

The kernel harness now reserves the actual BP-relative spill extent instead
of letting spill stores overwrite its control table. The runtime regression
failed before that correction. Dumps: `/tmp/qbopt-native-licm-after/`.
With only floating hoisting disabled/enabled in the same worktree, FPCSEX's
ten-trip modeled cost is 4076 -> 2807 units (31.1% less). These are model
units, not measured processor cycles or a verified optimal-target ratio.

All 21 renderer objects rebuilt in `/tmp/qbopt-quake-licm.4H3w3p` are
SHA-256 identical to `/tmp/qbopt-quake-float.dgK0tu`; the link response is
also identical. No redundant renderer run was made. Its last measured
24.23 FPS remains the prior build's result, not a new LICM measurement.

The broader `test_float_values.py` check has 35 passes and six failures:
one missing-conversion expectation, four emitted instruction-count
expectations, and one strict-exactness expectation under native semantics.
Disabling floating LICM still emits constant stores instead of arithmetic
for PDS FPCSE, confirming that particular mismatch predates this change.
These failures remain open; the full gate is not green.

Emitted FPDEEP conversion before and after:

```asm
; before                     ; after
fistp dword [bp-4]            fistp dword [bp-4]
wait                         mov eax,[bp-4]
mov eax,[bp-4]
```

Validation: 89 focused float/checkpoint tests pass. Both source-checkpoint
and conversion-checkpoint mutations are detected by the emitted-code tests.
The actual FPCSEX optimized kernel matches the original in 144 QEMU masked
cases over precision, rounding, subnormal, overflow, zero-divide and NaN inputs.
This is not physical 387 timing evidence. The strict FPCSEX trap harness remains
a check of the strict reference, not a requirement for native exception timing.

The full PDS FPDEEP executable was also relinked against BCL71ENR and run:
all eleven printed values and `DONE` match the original byte for byte.
Artifacts: `/tmp/qbopt-native-waits.h3fujr/{BASE,DEEP}.{OBJ,EXE,TXT}`;
screen evidence: `deep.png` in that directory. The native object shrinks
1822 to 1650 bytes; FPCSEX shrinks 969 to 968. This validates the complete
FPDEEP conversion/printing path, not just the isolated FPCSEX kernel.

Native floating CSE now accepts acyclic paths with unchanged floating controls,
without requiring exact arithmetic. Calls, opaque operations and explicit
checkpoints block reuse; loads retain their independent memory-clobber checks.
Strict policy still requires the exact-path proof. This does not reassociate
expressions or remove storage rounding.

FPCSEX's native loop changes as follows:

```asm
; before                 ; after
fld  dword [a]           fld  dword [a]
fadd dword [b]           fadd dword [b]
fmul dword [c]           fld  st0
fstp dword [p]           fmul dword [c]
fld  dword [a]           fstp dword [p]
fadd dword [b]           fdiv dword [c]
fdiv dword [c]           fstp dword [q]
fstp dword [q]
```

PDS object: 968 to 952 bytes; code: six bytes smaller. The actual optimized
kernel passes 144 exact QEMU state comparisons, including rounding modes and
NaNs (`/tmp/qbopt-native-cse.AW8G8b/QEMU.BIN`). Reinstating the rejection of
non-exact paths makes the emitted-add-count regression fail.

The wider peephole check exposed an existing ADDRM assertion tied to a particular
register assignment (`lea si,[ebx+ebx]` versus `lea bx,[eax+eax]`); restoring the
pre-change checkpoint behavior reproduces it. It was not weakened here.
