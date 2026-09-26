# Semantic switches

Implemented infrastructure:

- `Kind.SWITCH`: one integer selector, ordered `(value, block)` cases,
  `target` as the explicit default. Repeated destinations are legal;
  duplicate values after width normalization are not.
- SCCP selects constant cases/defaults; dead-code elimination retains
  unknown switches. Branch threading conservatively leaves their edges alone.
- Loop cloning remaps both cases and default, retaining successor phis.
- Lowering expands to comparisons/branches and repairs phi predecessors.
  Cases equal to default need no comparison. Unsupported forms fail atomically;
  expansion across a live condition is refused.

Focused checks: 38 passed across switch lowering/cloning, existing CFG
cloning and constant conditions. Restoring stale case destinations made the
cloning regression fail. New-file ruff/ty checks pass.

Backend witness (selector allocated to AX):

```text
before: switch selector {1: A, 2: A, 3: default}, default
```

```asm
; after lowering: compare bytes checked through the encoder
cmp ax,1           ; 83 f8 01
je A
cmp ax,2           ; 83 f8 02
je A
; default successor; layout supplies a jump if it is not fall-through
```

## Runtime recognition checkpoint

Raise now recognizes audited QB/PDS/VBDOS dispatch with unobserved outgoing
values. It prunes dead phis, refuses opaque bodies and segment-valued memory
references, and keeps the original call behind an unsigned `selector > 255`
guard. Normal values use SWITCH; the table remains attached to the retained
runtime call. User-defined helpers are excluded.

PDS JUMPS emits through LIR and runs: all six rows and DONE match baseline
byte-for-byte, both return to DOS, and both link without errors. Evidence:
`/tmp/qbopt-dispatch-runtime.a5980U/result.png`, BASE.TXT and OPT.TXT.
All-stage dumps: `/tmp/qbopt-dispatch-raised-jumps-20260911`.
Object size grows 1140 -> 1176 bytes before loop simplification; no speedup
or target improvement is claimed. FPS does not apply to this console test.

```asm
; before
mov bx,[bp-2]
call far B$OGTA

; after (labels name the destinations in the emitted dump)
mov ax,[bp-2]
cmp ax,255
ja errorPath
jmp normalPath
errorPath:
mov bx,[bp-2]
call far B$OGTA
; original inline table follows
normalPath:
mov ax,[bp-2]
cmp ax,2
je case2
mov ax,[bp-2]
cmp ax,3
je case3
jmp case1
```

Five recognition/emission checks pass over QB/PDS/VBDOS. Changing the guard
limit to 65535 fails all four compiler guard checks; the mutation is restored.
See `../measurement/targets.md` for the audited `B$OGTA` contract and LLVM reference.

### Boundary gate

The PDS `/O /G2` READ/DATA witness now checks eight selectors with native-FPU
emission (no fallback), successful links and observed return to DOS:

| Selector | Baseline and candidate |
| --- | --- |
| 0, 4, 255 | DEFAULT, DONE |
| 1, 2, 3 | FIRST/SECOND/THIRD, DONE |
| 256, -1 | Illegal function call; no DONE |

Six normal outputs are byte-identical. Error diagnostics retain their class
and module, but their reported address changes `0825:0045 -> 0825:0085`
because the runtime call moved. They are not byte-identical outputs.
Evidence: `/tmp/qbopt-dispatch-boundary.9ap9IY/result-final.png`; per-case
compiler/linker logs, original/emitted objects, output logs and replay scripts
are beside it. Every compile reports zero severe errors. FPS is inapplicable.

`tests/fixtures/regressions/dispatch.bas` (CRLF) and `dispatch-p-g2.obj` retain the
256 witness, compiled as D6.BAS. The initial INPUT version refused an
unestablished helper interface; it was not accepted as an optimized run.
Seventeen focused tests pass, including live-output/unknown-input refusal
and semantic boundary paths. A mutated guard accepting 65535 instead of 255
fails the 256 and -1 behavioral tests; restored code passes.

The boundary gate is closed for PDS, not all compilers/error handlers.

## Range-driven branch simplification

JUMPS's promoted counter already has the loop-body interval 1..3. Branch
simplification now consumes that proof: unsigned `counter > 255` is impossible.
Signed intervals crossing zero are conservatively interpreted as the full
unsigned span; negative selectors are not mistaken for small positive values.
Layout drops a dispatch table and its relocations when its owning call is dead.
Unrelated VBDOS statement tables remain intact.

This also removes the conservative call clobber that forced counter spills.
Actual PDS emission, with destinations named for readability:

```asm
; before: guarded candidate, inside the loop
mov ax,[bp-2]
cmp ax,255
ja errorPath                 ; retained B$OGTA call and inline table
; normal dispatch
mov ax,[bp-2]
cmp ax,2
je case2
mov ax,[bp-2]
cmp ax,3
je case3
jmp case1
; latch
inc word [bp-2]

; after: counter stays in SI across print calls
cmp si,2
je case2
cmp si,3
je case3
jmp case1
; latch
inc si
```

Object size **1176 -> 1095 bytes**; modeled cost **1366 -> 1058**, or
**1.84x -> 1.43x** of the unchanged 742 target. No runtime speedup claim.
PDS native-FPU baseline/candidate link successfully and print byte-identical
six rows plus DONE. Both return to DOS; FPS is inapplicable.
Evidence: `/tmp/qbopt-dispatch-range-run.3LhAlM/result.png`, output/link logs
and replay scripts alongside it. Every stage is dumped in
`/tmp/qbopt-dispatch-range-fixed-20260911`.

Focused switch/range tests: 92 pass, including emitted QB/PDS/VBDOS objects,
live error-table retention, and positive/negative unsigned boundaries.
Disabling range consumption restores B$OGTA and fails the emitted regression;
retaining the dead table makes that same regression reject the malformed code
map. Both mutations were observed failing and restored.
Bounded branched-loop unrolling remains unimplemented; it is not required to
put this PDS witness below 1.5x.

The expanded unsigned-edge check passes 80 cases across 16/32-bit widths.
Layout and constant-condition checks finish with 2955 passed and one failure:
`test_a_fold_does_not_keep_the_fixup_of_the_read_it_replaced` no longer finds
a folded memory read in its BOOLS witness. It fails identically in an isolated
HEAD archive (`/tmp/qbopt-head-layout-check.jqp2wg`); its assertion is retained.
The full commit gate is not green, and this batch remains uncommitted.
