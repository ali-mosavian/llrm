# FP binary constant fold: plan (2026-10-22)

Owed item 7 of `constantfolding.rs` (#2246): `ConstantFoldFPInstOperands` (ConstantFolding.cpp L1574, d1106deb71cc). Today `fadd/fsub/fmul/fdiv/frem` of two constants never fold there.

## Evidence it matters
w-8's `client` repro (`12-pipeline.ll`, `_pl_death_selftest@3`): the pin's `print<scalar-evolution>` gives the float exit `i32 12`; ours `COULDNOTCOMPUTE`. `compute_exit_count_exhaustively` (scev/exit_value.rs) is a faithful translation and runs, but `evaluate_expression` reaches `fadd float 1.1, 0.1`, `constant_fold_inst_operands_at` answers none, and the exhaustive run stops.

## The rule (a numbered DEVIATE, narrower than the pin)
The pin folds `fadd float` in f32 with round-to-nearest. QB, Nib and C on an x87 target evaluate temporaries wider (80-bit, spilled at 80, D-RFP5), so an f32 or f64 fold can change a printed value.
Fold `a op b` in format F only when the operation is exact in F: the real result is representable in F. Then any evaluation width at least F's (f32, f64, x87) yields the same number, and the fold is right whatever width the target uses. Otherwise refuse (the pin would fold).
- add/sub/mul: error-free transformation in f64 (two-sum, fma) shows the f64 result exact; for f32 the f64 result must also convert to f32 exactly. div: `fma(q, b, -a) == 0`. rem: the IEEE remainder of the same-format operands is exact.
- refuse: a NaN result or operand (payload nondeterministic), an inexact result, an overflow to infinity (inexact), `x86_fp80` (held as a double), and an instruction with `nsz`, `reassoc`, `contract` or `arcp` (the pin's `AllowNonDeterministic = false`).
- denormals: MIR has no `denormal-fp-math` attribute; IEEE mode is assumed, as the pin does without the attribute.
- Exit: a "strict float width" fact in the profile, true only under the standard-precision flag (every assignment rounded), where the pin's f32 fold is faithful. None exists today; C/Nib f32 follow the fast mode (excess precision on x87) and QB evaluates at 80 bits, so the client loop (`next_frame += 0.1f`) stays open.

## Tests
fadd/fsub/fmul/fdiv/frem on f32 and f64 (exact folds); each refusal (flags, NaN, inexact, overflow, fp80); denormal operands and results; a pair where f32 rounding and x87 evaluation differ (`0.1f + 0.2f`): must not fold. SCEV: a float-controlled loop gets its exit count.

## Before ready
The 43-program float table against real BC (`bcprobe/run.py`), tests/run qb and nib groups, `qbrt_run --all`, the C float programs: 0 output differences; objsweep, rows, QCport, compile Ir.
