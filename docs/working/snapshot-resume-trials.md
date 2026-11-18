# Snapshot-resume for trial passes (design note, 2026-11-11)

Owner x86-m32-c. Status: **closed 2026-11-11 (llrm-oversight ruling)**; S1 was built and falsified, S2/S3 not pursued. For llrm-oversight and llrm-coordinator.

## Outcome
- S1 (resume at the Late list) was built: `PassManager` hold/resume, shallow `ModuleAnalyses` fork. A replay with no trial equals the kept run (x_life, x_adler, x_jpeg_fixed at -O1), so the fork carries the stale GlobalsAA/Summaries faithfully.
- The premise was false: the unroll trial acts inside the walk too. `unroll@753` runs fused in the LPM2 slot of the walk's Simplify list at -O1, and the trial unrolls there (x_life: body 110 -> 129 instructions) before the Late list. A trial resumed at the Late list gives different code: x_life -O1 md5 83954c00 (today) vs bcf347e0, work saved +1533175 vs +1518242.
- The earliest affected pass is the walk for both trials, so the only output-identical resume point is before it: it saves the head, 5.9-16.1% of the mid-end (S2 table), on the trial compile only. Not worth it (ruling).
- S3 bound (`-Os`, 3x/2x counters, wall under load 15): second mid-end share of total compile x_jpeg_fixed 33%, x_idioms 39%, huge_runs.c 34%; known_loads_walks.c has no trial. Per-body resume can skip at most the bodies before the first noted one (35 of 60, 39 of 66, 30 of 62 body runs): about 17-19% of total, before the callers' re-runs. Not pursued.
- Work kept unpushed on branch `alim/snapshot-resume-s1` (local WIP commit).

## Scope
- LLVM has no trial: the pin makes each decision once. These are DEVIATEs: UNROLL-O1-TRIAL (#9101) and ROTATION-TRIAL (#9141) in `codegen/driver/trial.rs`; the 6502's UNROLL-TRIAL / UP1 (`llrm-6502/src/driver.rs`) share the noting state (`transforms::trial`).
- UNROLL-OS-TRIAL-STRICT (`Tuning::unroll_trial_bytes_strict`, on at m64; owner llrm-target-x86-m64, exit 2026-11-24): the -Os unroll is kept only where the code bytes shrink; m32's keep rule also takes a few bytes a loop for fewer clocks.
- Out: #9145 (a price in placement, no second compile); m68k's trial (`llrm-m68k/trial.rs`, private, m68k-matrix owns it).
- The default path (no trial noted, or a target with none) is untouched: it is today's single compile.

## Today
`compiled_module_trying`: `optimized_program(module.clone())`; if a candidate was noted, `optimized_program(module)` again with the decision reversed, both through the whole mid-end; the backend runs on the differing functions only for rotation (already cut by `only`), on the whole module for unroll.

Where the time sits (`LLRM_DEBUG=time`, x_jpeg_fixed -Os, llrm-cc): `mir walk` 105 ms of the mid-end (the CGSCC inline walk), `mir unroll@1476` 8 runs (one per body, in the Late list), `mir rotate@728` 18 runs (inside the walk's Simplify list, once per simplification of a body).

## Where each trial pass sits (`pipeline::recorded`)
| pass | slot | runs | what comes after it |
|---|---|---|---|
| unroll@1476 (Phase::Late) | after `Interprocedural`, `WriteOnly`, `Dead`, `LoopDeletion`, `GlobalDce`, walk2, `ReversePostOrderFunctionAttrs`, `CalleePop` | once per body | GlobalsAA recompute, LSR and the codegen-prep list, the post-pipeline steps of `driver::optimized`, the backend |
| rotate@728 (Phase::Simplify) | inside `walk::Walk` (callee first) and the `again`/`cleaned` lists | per simplification | the rest of the walk, `Interprocedural`, everything above |

The two differ: unroll is late and body-local, so a resume point is cheap and clean; rotate is inside the walk, whose decisions (inlining) read the simplified callee.

## Design
**S1, unroll (first PR).** `PassManager` runs `passes: Vec<Pass>` in order. Add `PassManager::run_from(program, index)` and let `recorded` return the index of the Late stage. The kept compile runs `0..index`, clones the `Program` (modules, target), then runs `index..`. If the trial noted candidates, a second manager is built by the same builder, the clone restored, and `index..` run with the trial on. Cost of a trial: the Late list + codegen prep + post steps + backend (as today), not the walk and IPO.
**S2, rotation (same PR if it holds, else second).** Snapshot at the `Walk` pass's index: saves the head (InferFunctionAttrs, GlobalOpt, Ports, Stamp, Early, ModuleIpo, Cleanup), not the walk. The saving is small; stated so the number is not oversold.
**S3, rotation inside the walk (only if S2 under-delivers).** Per-body: snapshot at the first body whose simplification notes a rotation, resume the walk for the bodies after it (callers re-run: inline decisions read the changed callee). Needs `Walk`'s worklist and `rerun` state made clonable. A larger change; not started without a number from S2.

## Captured state
- The `Program`: modules, `Context`, target. Cloned once at the snapshot point.
- Analyses, NOT simply recomputed: the pipeline keeps some module-level results stale on purpose (`GlobalsAA` and `GlobalsModRefAnalysis` are made at `compute`/`recompute` and read cached in between; `freeze::<Summaries>`). A tail that rebuilt them fresh would see fresher facts than the continuing run and change the output. So the snapshot forks `ModuleAnalyses` shallowly: the module-level `results`/`dropped`/`stateless`/`required` entries are `Rc` handles shared by the fork (results are read-only), the per-function caches are dropped (they are kept only when equal to a fresh computation: `check_kept`). `ProgramAnalyses` is forked the same way. This is the riskiest part; the md5 identity and the negative control test it.
- Pass-object state: the Late list is `Fixed::linear(applied, Phase::Late)` built from `Applied` alone; it holds no `Rc` shared with the head (`cloning`, `fused` belong to `ModuleIpo`/`Interprocedural`/`PerLoop`). To check for each pass in the tail by reading, not assuming; the check goes in the PR.
- Thread-locals: ~280 exist, nearly all counters for `LLRM_DEBUG=time`. State that reads: `TRIGGER` (cleared between passes), the trial notes (`take`/`with`), `FACTS`, `SEEN`, and the id counters (`NEXT_LINEAGE`, `NAMES_MADE`). Today's second compile runs after the first in the same thread with whatever the first left, so no output may depend on them; the md5 identity run is the proof, and the PR lists the readers found.
- Dump directory: off for the second run, as today (`again.dump = None`).

## Callee-fact ripple
- Unroll changes one body. Facts the tail reads from other bodies (attrs, `CalleePop`, nounwind, argmemonly) were fixed before the snapshot and the trial does not change them: `ReversePostOrderFunctionAttrs` and `CalleePop` run before Late. A body the trial changes can change its own derived facts only after the snapshot point, which the tail recomputes for it; whether callers in the tail read them other than through recomputed summaries is unverified: open question 2 covers it.
- Rotation inside the walk (S3) does ripple: an inlined callee carries the rotation. Handled by S3's re-run of later bodies; before S3 exists, rotation uses S2 and keeps today's full second mid-end from the walk on, so there is no ripple to handle.
- Fallback: if a tail pass reports that it read a fact the snapshot did not hold (a debug assertion on `Applied` version counters), or the tail errors, the compile falls back to today's two full compiles. The fallback is counted in `LLRM_DEBUG=trial`.

## PR bar (as ordered)
- No output change on any target: md5 identity of x86_16/32/64, arm64, m68k, z80, 6502, wasm32 at O1/O2/O3/Os on tests/run/c, and x86-m32 vsgcc x_*, counts per level (compiled/total), both llc md5s stated, base vs candidate from the same tree.
- Positive control: a module where a trial is kept (x_life -O1) and one where it is put back (matmul -O1), both md5-equal to today's; a negative control showing a deliberately wrong tail (snapshot taken one pass late) changes the md5.
- Compile time down on x_jpeg_fixed, x_idioms, huge_runs.c, known_loads_walks (`instructions:u`, before/after, per level where a trial fires); no kernel up.
- Tests (fail-first): a `PassManager::run_from` test (tail from index equals the full run's result on a snapshot); a driver test that a noted trial compiles the head once (counter of head runs 1, tail runs 2).
- PR body: Pass:/Translation:/General solution: at the top; Translation: none, LLVM has no trial; General solution: resume of a function-pass pipeline from a state snapshot, for any pass that wants a trial.

## S2 measurement (what the head before the walk is, `mir pipeline` total ms, Os, llrm-cc, `LLRM_BISECT` stopped before the walk vs full)
| kernel | head | full | head share |
|---|---|---|---|
| x_jpeg_fixed | 10.4 | 88.5 | 11.8% |
| x_idioms | 5.5 | 93.2 | 5.9% |
| huge_runs.c | 4.0 | 37.6 | 10.7% |
| known_loads_walks.c | 8.6 | 53.5 | 16.1% |
The head is an upper bound on what S2 saves (the first `rotate@728` runs inside the walk, after it). Around the 10% line: two kernels above, one at it, one well below. Method: a temporary `manager.bisect` from `LLRM_BISECT` (not committed); the walk is the pass run numbered by the `BISECT:` lines.

## Open questions
1. S1 first or S1+S2 in one PR? S2's saving is small; recommendation S1 alone, S2 if its measured saving is above 10% of the mid-end on the trial kernels.
2. Does any tail pass read `Applied`-time state set by the head (`TRIGGER`, `fused`)? To be answered by a read of the Late and codegen-prep lists before coding; the note is updated with the result.
