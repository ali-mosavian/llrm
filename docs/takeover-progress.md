# Takeover checkpoint — 2026-09-09

Goal: correct modern-compiler-quality output, machine-independent MIR, every documented target within 1.5x. **Not complete.** Branch `restore-through-lir`; checkpoint `dda3d46` and backend increment `abf9ab5` are committed. Stashes are untouched.

## Operand-width increment

Constant reads now mask and narrow a fact to the semantic operand's width. Previously a word read of 0x12350000 shifted right once folded to 0x8000 instead of 0. Three byte/word cases failed first, passed after the fix, failed with the mutation restored, then passed again. The constant suite had 985 passes and one existing comparison-materialization failure, reproduced with old read semantics. Ruff passes for both changed files. Sampled hotlop, lngmix and harr costs are unchanged; this is correctness progress, not a measured optimization gain.

The next increment teaches constant propagation MIR's existing step semantics, restoring increment/decrement folding without machine names. Six width/wrap cases and the existing cmpord regression fail when this change is removed; the complete constant-propagation file now passes 992 tests (15.75 seconds). PDS bools improves from 176 to 174 (1.38x); hotlop/lngmix are unchanged. Production stage files: `/tmp/qbopt-constant-steps-20260909`. No all-target or runtime-completion claim.

Committed as `c72f918`. The following alias increment requires equal segment origins as well as equal offset values before using displacement arithmetic. Distinct or unknown far segments can overlap despite disjoint offsets (1000:0020 equals 1001:0010). Three new checks fail first and under mutation; the alias/loop-motion checks pass 12 tests. Sampled harr, matrix and segld costs remain unchanged. This repairs a soundness prerequisite, not the remaining array-extent proof.

## Current implementation

The checkpoint introduced write-through memory promotion, allocation/spill corrections, MIR fixed-point iteration, truthful measurements and dumps of actual production passes. No re-raising of emitted bytes.

Since the checkpoint:
- Sink unobserved fixed-cell loop stores into a dedicated single exit. The store must execute in the exiting block; observers and unknown effects prevent motion.
- Coalesce equal copies using def/live-out interference, preserving unequal live-in values and partial-width distinctions. Normalize register classes and rename nested memory operands.
- Preserve block labels when their first instruction disappears, and transfer removed leading-copy byte ownership across inserted zero-span copies.
- Allocate constraint IDs above retained pins, fixing QuickBASIC/PDS division refusals.
- Remove the production MIR-emitter fallback. Backend refusal returns the byte-identical input and original diagnostic, never another emitter's output. Historical emission enum/field remain for compatibility.

## Measurements

Selected PDS `/G2` modeled costs, not hardware timings:

| Program | Checkpoint | Current | Target | Current ratio |
| --- | ---: | ---: | ---: | ---: |
| press | 420 | 326 | 308 | 1.06x |
| hotlop | 646 | 452 | 312 | 1.45x |
| lngmix | 921 | 867 | 210 | 4.13x |
| harr | 11694 | 11494 | 1834 | 6.27x |
| matrix | 14708 | 12076 | 6210 | 1.94x |
| segld | 26602 | 25802 | 6704 | 3.85x |

Checkpoint bools was 176/126 (1.40x); not remeasured in the latest selected run. No final all-target claim. The full opportunity command completed but its output was lost to truncation, so it supplies no recorded evidence.

The last full emission scan was 410 LIR / 77 MIR fallback before fixing seven jumps refusals and removing the fallback. Do not treat that as the current count.

## Validation

- Changed backend modules: 1,519 passed, two failures in historical fallback expectations (71.65 seconds).
- Those failures exposed a second emitter hiding injected backend diagnostics. Updated tests require refusal, byte-identical input, and the original reason; three failed before removing fallback. All five selected refusal checks now pass.
- Final focused check: 43 passed in 1.09 seconds. Rechecked hotlop 1.45x, press 1.06x and matrix 1.94x after fallback removal; matrix still fails the target. Ruff passed for the new loop-motion/coalescer work and edited whole-segment emitter; this is not a whole-project lint claim.
- Focused runtime programs passed on PDS, QuickBASIC and VBDOS: hotlop, hotlpx, press, pressx, matrix, harr, lngmix, flags. After address-class changes: jumps, arridx, arrprm, fpdeep, procs, harr, hotlop and pressx passed all three. VBDOS procs used unchanged input, not LIR success.
- Latest runtime artifacts: `/tmp/qbopt-coalesced-addresses-runtime-20260909`. Stage evidence: `/tmp/qbopt-lngmix-next-20260909`, `/tmp/qbopt-harr-next-20260909`, `/tmp/qbopt-jumps-coalesce-20260909`.
- Full commit gates have not passed. The checkpoint had lint failures and 787 type diagnostics; pytest was interrupted once its gate was already blocked. User authorized that unsigned checkpoint with hooks bypassed, not future commits.
- Subsequent authorization: use focused checks for progress commits and reserve the full gate for milestones. Current full type check reports 802 diagnostics; the milestone gate remains outstanding. Progress commits may bypass hooks under this explicit authorization, without claiming full validation.
- Required independent review was unavailable (expired Claude OAuth; Fable unavailable); user authorized proceeding. No Claude Desktop session was resumed.

## Next implementation priorities

1. Whole-value recognition in raise: lngmix still carries split long accumulator halves, joins and spills. Do not move register-aware widening into MIR optimization. Existing JOIN forms are not yet uniformly explicit semantic operands.
2. Sound array objects/extents, enabling scalar promotion and loop-address reuse. FAR accesses may alias DGROUP; the next named displacement is not proof of an array boundary. Runtime descriptors distinguish near, far and huge storage. Use those facts, not a blanket no-alias rule.
3. Close the measured target gaps, then run integration/commit gates. Keep fail-first symptom regressions per fix and dump adjacent stages when debugging.

Generic copy propagation plus trimming merge dependencies was tried and **reverted**: pressx printed 0 instead of 7500 on all three compilers, and some QuickBASIC runs failed to complete. Do not resurrect that shortcut. A suspected moved-store relocation defect was disproved by the emitted-object check; removing its unnecessary symbol override did not fix a runtime bug.

Runtime scope explicitly excludes /V, /W event trapping and /X resumable errors; do not spend the next round inventing event interfaces. VBDOS B$ENRA remains unestablished. LLVM LICM dedicated-exit/store-dominance rules informed store sinking; LLVM/GCC references are under /Users/alim/work/other.
