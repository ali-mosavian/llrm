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
| Pressure-aware allocation | partial | spilling, slot colouring, selected folds and local rematerialization exist; global splitting/rematerialization and x87 allocation remain. |
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
