# Target coverage and the next floating-point gap

Snapshot after event-handler correctness and statement-table discovery work.
The full report before the latest table fix had 487 configuration rows:
229 comparable (all within 1.5x), 169 without targets, 87 provisional,
and two unmeasured DIVMOD event builds. The latest focused check resolves
those two as measured but without targets. These are configuration rows,
not counts of independent programs. This is not a completion claim.

Programs without targets include FPDEEP, FPEMU, DIVMOD, JUMPS, CMPORD,
CHAIN, ARITH, FLAGS, PROCS, NOTS and NEGNOT. Small standalone fixtures also
lack references. Event builds need event-preserving references; FPCSE and
FPCSEX still have invalid inherited floating-point denominators.

## FPDEEP: verified duplicate floating load

Current ordinary-build costs: PDS 11603, QB45 11819, VBDOS 11533. No ratio
is valid without an independently derived reference. The source's DOUBLE
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
