# Modern code-generation plan status

This is the live state of the GCC/LLVM-quality plan.  A checked item means
the mechanism is in the production pipeline with a focused regression; it
does **not** mean the final quality gate has passed.  Each implementation
iteration updates this file in the same commit.

## Acceptance state

- [ ] Every supported CPU has hand-audited, candidate-ABI targets and meets
  the 1.10 structural/dynamic limits.  `bench/c/targets.json` is intentionally
  still empty, so this gate is not claimed.
- [ ] The complete BASIC/C matrix and QCport integration have passed at the
  final acceptance revision.
- [ ] QCport's C loops have a reproducible, source-line-mapped BCC/WC
  comparison.  The existing executable-only audit is advisory because its
  source revision cannot be proven.

## Implementation state

| Phase | State | Current boundary |
|---|---|---|
| Per-CPU measurement | in progress | CPU profiles, C corpus, static/dynamic metrics and reference listings exist; audited targets remain. |
| MIR/LIR provenance and fresh OMF | largely complete | allocated LIR emits directly with external source maps/allocation hints; legacy object-rewrite compatibility remains. |
| SROA and scalar promotion | partial | fixed/disjoint leaves and some indexed leaves promote; general aggregate/copy decomposition remains. |
| Pressure-aware allocation | partial | spilling, slot colouring, byte RMW selection and local rematerialization exist; global splitting/rematerialization and x87 allocation remain. |
| Loop optimization | partial | exact recurrences, formula costing, specialization, peeling and exact unrolling exist; versioning, rotation and broad pressure forecasting remain. |
| Whole-module optimization | partial | summaries, constant returns, private inlining and private procedure DCE exist; full IPSCCP/cloning and private-data DCE remain. |
| Post-allocation quality | partial | copy propagation, machine CSE/DCE and C-path tail sharing exist; source-map-aware BC tail sharing and CPU scheduling remain. |

## Iteration log

### 1. Best-case GCC/LLVM listing contract — 2026-09-18

`tools/quality.py --references` now records a formal
`best-case-flat-i386-structural-reference` contract in its top-level report,
each compiler listing, and every structural comparison.  It says explicitly
that flat 32-bit GCC/LLVM output is an advisory reference for algorithmic loop
shape, expression count and memory traffic—not an ABI-equivalent target for
the segmented 16-bit medium model.  The focused report regression and a real
GCC/Clang sieve listing generation both pass.

Next: retain source-line-address evidence in a paired clean QCport BCC/WC
build, then use it to validate or reject the `r_recursive_world_node` loop
regression before changing allocation or aliasing for that case.

### 2. Reproducible QCport compiler listings — 2026-09-18

`tools/qcport_listings.py` creates matched BCC `-S` and qbopt assembly listings
from a clean QCport worktree, plus a hash manifest covering the QCport and
qbopt commits, every input source, CPU profile, output listing and stage dump.
It rejects a dirty source tree rather than comparing stale binaries to edited
loops.  The next run must use a clean QCport worktree and retain the manifest
with any loop-level regression or performance claim.

### 3. QCport `r_walk` evidence — 2026-09-18

The harness was run on a detached, clean QCport worktree at `18f5e1f`, for
`src/render/r_walk.c` SHA-256
`e5abf5f8fb67c3cac011579fcb980bff446ab068c130dd2c196652bbf802fef0`.
BCC's CodeView listing maps the marked-face loop directly to line 50.  Its
complete loop body is 16 instructions: it retains `world` in DI and `rdr` in
SI, then uses `les` and a byte RMW.  qbopt emits 24 instructions for the same
iteration: it reloads both incoming pointer bases from the frame, reloads their
selectors, and widens the byte RMW through word temporaries.

The first excess is present in MIR, where the loop still loads `world->lfc`
and `rdr->pflag`; allocation makes the incoming pointer values cheap frame
rematerializations instead of splitting their live ranges around the call-free
loop.  This confirms a general phase-4 region-splitting and pressure issue.
Do not add a `r_walk` special case: the next implementation must preserve
profitable invariant values across any call-free loop and include a
source-independent fail-first regression.

### 4. Allocator split trace — 2026-09-18

The initial explanation above was refined with a trace of the production
allocator.  `splitkit` does propose its normal hottest-region split for both
incoming pointer values (block 77, the loop body), but rejects both because
no legal general-purpose register is free in that region.  The blocker is not
a missing split category.  qbopt widens the byte load/OR/store into several
word temporaries before allocation, while BCC keeps the operation as one byte
read-modify-write instruction.  Those temporary live ranges consume the
capacity that the existing splitter needs.

Next implementation: select a general byte memory read-modify-write form in
lowering when its address, load and store are identical and no intervening
memory effect can be crossed.  The regression must use a standalone C input;
after it lowers pressure, rerun the paired `r_walk` manifest to establish
whether the generic splitter keeps the pointer bases.

### 5. Exact byte RMW selection — 2026-09-18

`fixtures/c/rmwbyte.c` first failed its emitted-assembly regression: its far
byte compound assignment was expanded into a byte load, two zero extensions,
a word OR, a truncation and a byte store.  `backend/rmw.py` now makes lowering
select `or byte ptr es:[base+index],reg8` when the load/OR/truncation/store
values are private, the byte cells are identical, and only integer-only mask
preparation lies between the original read and write.  The selected operation
still performs one byte read and one byte write; no arbitrary intervening
memory read, write, call, branch, or provenance boundary is crossed.

This is deliberately instruction selection inside `lowered()`, before
allocation—not a MIR pass and not an LIR optimization phase.  MIR keeps its
language-level promotion semantics and has no target form or register fact.
The fail-first standalone C regression now emits the byte RMW.  The immediate
QCport listing visibly replaces the former promoted sequence with `or byte
ptr es:[bx+di],dl`, but the two pointer bases still reload from their frames:
this removes pressure, yet does not by itself make the existing regional split
fit.  The next allocator iteration must explain the remaining occupied
ranges from a fresh reproducible listing rather than assume the split should
now succeed.

### 6. Listing provenance guard — 2026-09-18

The first RMW QCport listing exposed a measurement defect: it contained
uncommitted qbopt code while its manifest named the previous qbopt commit.
`tools/qcport_listings.py` now rejects a dirty qbopt checkout just as it
already rejects a dirty QCport worktree.  The new fail-first regression names
the stale-lowering symptom and verifies the `qbopt checkout is dirty` refusal.
The next paired `r_walk` run must be made after this iteration is committed;
the pre-commit manifest is retained only as a visually inspected diagnostic,
not performance or revision evidence.
