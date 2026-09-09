# Target coverage and the next floating-point gap

Local dead-value update: FPDEEP now costs PDS 12287 (11.31x), QB 12555
(11.56x), VBDOS 12267 (11.30x). The target remains 1086. See
`docs/local-dead-values.md` for the removed return-half reconstruction,
runtime checks and pre-existing transform-test failures.

Latest FPDEEP update: explicit float-to-integer values and corrected helper
body accounting give PDS 13489 (12.42x), QB 12555 (11.56x), VBDOS 13419
(12.36x). Under the corrected accounting the preceding revision costs
14481, 14699 and 14411 respectively. See `docs/float-integer-values.md`;
the earlier scores below undercounted B$FIST and are historical.

FPDEEP update 2026-09-10: its complete ordinary-build reference is now
1086 units, independently derived from its exact source expressions and
printing sequence in `docs/targets.md`. After finite-array reuse and stack
destination selection, costs are PDS 11729 (10.80x), QB 11947 (11.00x),
VBDOS 11659 (10.74x). These are static ranking ratios, not runtime speedups.
See `docs/finite-array-reuse.md` for before/after assembly and verification.
Event-enabled configurations remain provisional.

Update 2026-09-10: ARITH has a complete ordinary-build reference of 592 units.
NOTS and NEGNOT references are corrected to 306 and 254: their earlier 378/290
targets unnecessarily retained dead stores and split argument pushes.

| Program | PDS cost / ratio | QB cost / ratio | VBDOS cost / ratio |
|---|---|---|---|
| ARITH | 658 / 1.11x | 680 / 1.15x | 652 / 1.10x |
| NOTS | 426 / 1.39x | 438 / 1.43x | 388 / 1.27x |
| NEGNOT | 282 / 1.11x | 292 / 1.15x | 282 / 1.11x |

Constant argument propagation closes QB NOTS's gap without increasing its target.
The scorer now prices decoded instructions rather than synthetic raised
operations. The historical totals below have not been rerun under that
change and must not be quoted as the current completion percentage.

Snapshot after event-handler correctness and statement-table discovery work.
The full report before the latest table fix had 487 configuration rows:
229 comparable (all within 1.5x), 169 without targets, 87 provisional,
and two unmeasured DIVMOD event builds. The latest focused check resolves
those two as measured but without targets. These are configuration rows,
not counts of independent programs. This is not a completion claim.

Programs without targets include FPEMU, DIVMOD, JUMPS, CMPORD,
CHAIN, FLAGS and PROCS. Small standalone fixtures also
lack references. Event builds need event-preserving references; FPCSE and
FPCSEX still have invalid inherited floating-point denominators.

## FPDEEP: verified duplicate floating load

The timed FPBENCH fixture exposed another literal-raising gap: BASE16 and
PTR16:16 relocations in its array descriptors invalidated the entire BC_CN
pool, including the unrelated 1.0 at offset zero. Known relocation shapes
now exclude their exact two- or four-byte fields; unknown shapes still
invalidate the pool. FPBENCH now has three entry literal facts instead of
zero, with boundary-overlap regressions. It still has zero exact floating
value facts at use sites: intervening memory effects kill the entry facts.
Emitted bytes are unchanged. Preserving a literal across those effects
requires a separate alias/escape proof, not declaring BC_CN immutable.

Earlier ordinary-build costs were PDS 11603, QB45 11819, VBDOS 11533;
the update above supersedes these. The source's DOUBLE
expression is `(d*d)/(d+d)` after assigning `d=12`.

The PDS emitted listing contains:

```asm
0168 fld qword [d]
016d fld qword [d]
0172 fadd qword [d]
0177 fdivp
017a fmul qword [d]
017f fstp qword [e]
```

Both loads resolve to BC_DATA+001a. The duplicate access is real, not merely
two zero displacements assumed to alias. Reusing a held value must still
push a second x87 slot; deleting the second load outright is the historical
miscompile described by this fixture. A candidate is a register-stack copy,
but its floating exceptions and precision semantics must be established
before implementation. Do not reassociate the arithmetic merely because
the particular constants happen to produce an exact answer.

No generated assembly changed during this inspection. Next implementation
work should connect proven floating-value reuse to stack allocation; LLVM's
strict-FP caveats in `docs/floating-environment.md` remain applicable.

### Located upstream blocker

Inspection of the current implementation changes the diagnosis: floating CSE
already exists in `optimize/transform.py`, and stack allocation already handles
shared floating values. CSE requires exact/nonexceptional facts; that guard
must stay. `analysis/floatfacts.py` can propagate exact constants through
typed floating operations and storage, but the assignment `d=12` is not
represented as such a store.

The initial PDS MIR at 014c loads destination BC_DATA+001a and source
BC_CN+0022, transfers DS to ES, then carries four opaque operations at
0154..0157. These are BC's four MOVSW instructions copying the DOUBLE
literal. The following FLOADs therefore have no known finite value, and CSE
correctly refuses to merge them under strict exception semantics.

The next implementation is frontend recognition of this copy: explicit
memory reads/writes plus the pointer updates, preserving direction and
segment requirements in the machine-facing layers. Supplying `d=12` as an
entry fact would be wrong: this assignment executes after the preceding
loop and runtime calls. Removing the second FLD without proving its effects
would also bypass the actual missing abstraction.

The literal reader now exposes bytes to integer as well as floating loads.
It also excludes the two bytes patched by each OFFSET16 fixup rather than
discarding its entire LEDATA record. This matters for BC_CN+0022: the DOUBLE
12 shares a record with relocated string descriptors. Other relocation
forms conservatively exclude the pool until their patch extent is supported.
The new scalar-load regressions fail on the previous implementation and
pass with the change; 56 focused literal/constant/floating checks pass.
Three existing SPILL immediate-encoding assertions fail unchanged on HEAD
and on this change; they were not altered or counted as passes.

All 96 primary configuration outputs are byte-identical before and after
this prerequisite. FPDEEP still emits the two FLDs shown above. Copy raising
and propagation across intervening effects remain unfinished; these entry
facts are deliberately not an immutability claim about the literal pool.

### Copy scalarization with explicit environment evidence

`frontend/raising_copies.py` now raises an individual word copy into an
ordinary MIR load/store and two narrow pointer definitions. It recognizes
explicit CLD/STD traversal and adjacent PUSH DS / POP ES selector setup,
requires known in-segment DGROUP addresses, and does not carry those
environment facts through a call or infer them at a non-entry block.
The pointer copies retain their upper-half merge inputs and emit MOVs,
not flag-clobbering additions. Overlap retains load-before-store ordering
for each element. Unknown traversal, selector state, or pointer wrap is
not scalarized. CLD/STD remain machine-state barriers but no longer claim
to modify arbitrary memory.

The focused witness uses FPDEEP's real copy and floating operations with
an explicitly established direction state. Existing strict CSE then changes:

```asm
; before                   ; after, in the explicit-state witness
fld qword [d]              fld qword [d]
fld qword [d]              fld st(0)
fadd qword [d]             fadd qword [d]
```

This is not yet a production FPDEEP speedup. Its copy occurs after a loop
and runtime calls, without a local CLD. Establishing the runtime direction
and selector contracts and propagating them through control flow remains
necessary. All 96 primary outputs are byte-identical with this raiser
enabled or disabled. Production stage dumps are in
`/tmp/qbopt-copy-raising-current`.

The 46 focused copy/literal/floating-value tests pass. Four copy proof
tests fail with scalarization disabled; the literal propagation test also
fails when CLD is restored to its old arbitrary-memory-clobber model.
The broader IR checks expose two pre-existing table/body-count assertions
that fail with scalarization disabled as well. The existing Rule 5 gate
also still reads nine obsolete flat-package paths; that gate needs repair,
not an assertion that architecture checks passed.

### Observable values, not copy-instruction bookkeeping

The raiser now discards unobserved copy-pointer definitions before returning
MIR. For the explicit-state initializer witness, that removes eight pointer
updates and the two address-setup definitions consumed only by the copies.
Byte-ownership markers remain for the removed input instructions. What
crosses the boundary is four ordinary typed loads and four stores, in the
original per-element order; it is not an eight-byte atomic load or a
memmove-style snapshot that would change an overlapping copy's behavior.

When a pointer result really is read, including on a successor phi edge,
its definition remains. The unchanged upper portion depends directly on
the value before the copy chain, not on three artificial intermediate
definitions. This retains observable value semantics without asking an
optimization pass to reason about SI/DI or the direction flag.

On this controlled witness, selection before ordinary optimization drops
from 18 MOV instructions / 54 MOV bytes to 8 / 24. These are comparisons
against the previous scalarizer, not against BC's original four MOVSWs and
not a production speedup. The 48 focused checks pass; the new dead/live
pointer assertions were observed failing before the cleanup. Production
FPDEEP still needs the previously described environment proofs.
