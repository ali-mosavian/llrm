# Takeover checkpoint — 2026-09-09

Goal: correct modern-compiler-quality output, machine-independent MIR, every documented target within 1.5x. **Not complete.** Branch `restore-through-lir`; checkpoint `dda3d46` and backend increment `abf9ab5` are committed. Stashes are untouched.

## Operand-width increment

Constant reads now mask and narrow a fact to the semantic operand's width. Previously a word read of 0x12350000 shifted right once folded to 0x8000 instead of 0. Three byte/word cases failed first, passed after the fix, failed with the mutation restored, then passed again. The constant suite had 985 passes and one existing comparison-materialization failure, reproduced with old read semantics. Ruff passes for both changed files. Sampled hotlop, lngmix and harr costs are unchanged; this is correctness progress, not a measured optimization gain.

The next increment teaches constant propagation MIR's existing step semantics, restoring increment/decrement folding without machine names. Six width/wrap cases and the existing cmpord regression fail when this change is removed; the complete constant-propagation file now passes 992 tests (15.75 seconds). PDS bools improves from 176 to 174 (1.38x); hotlop/lngmix are unchanged. Production stage files: `/tmp/qbopt-constant-steps-20260909`. No all-target or runtime-completion claim.

Committed as `c72f918`. The following alias increment requires equal segment origins as well as equal offset values before using displacement arithmetic. Distinct or unknown far segments can overlap despite disjoint offsets (1000:0020 equals 1001:0010). Three new checks fail first and under mutation; the alias/loop-motion checks pass 12 tests. Sampled harr, matrix and segld costs remain unchanged. This repairs a soundness prerequisite, not the remaining array-extent proof.

Alias fix committed as `ffb0269`. A strength-reduction experiment appeared to improve harr from 6.27x to 4.47x and passed harr's output check on all three compiler families. **Do not accept that as valid optimization evidence**: the dumps show the shifted full address being mistaken for both loop counters because their old variable numbers match. harr writes and immediately rereads an address, so its sum does not independently validate that address. Artifacts: `/tmp/qbopt-strength-harr.zYw8Ia`. Strength remains disabled. The next analysis fix keys recurrences and backedges by exact SSA identity; two focused checks fail first and under mutation, then pass. Fresh variable collisions in strength.py and proper SSA construction remain to fix before enabling it.

Exact identity committed as `01ebaaa`. The old raw-MIR matrix coverage assertion now fails (zero recognized counters); it relied on equality not proven by its input. The next increment follows only width-preserving value copies, finding three exact counters/two derived candidates in promoted matrix and one/one in segld. Raw-memory equivalence still requires promotion, not variable-name matching. Generic isolated SSA construction is extracted from promotion into `ssa.py` and used by strength reduction. New counters now use fresh variable IDs from the complete graph and carry an explicit loop phi; removing either fix fails the regression. An unavailable stride is refused, and duplicate transformations are skipped. Strength remains disabled: no verified performance gain from the repaired implementation yet, and multiple-entry/backedge recurrence validation and profitable selection remain outstanding.

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

Immediate-multiply checkpoint: single-result MIR products now propagate known
factors; lowering selects the existing three-source immediate representation,
avoiding a destination tie. Widening/multi-result products are unchanged. Three
regressions fail with the old propagation/lowering; six focused checks pass.
Strict LIR matrix/lngmix runtime passes on PDS /G2 with experimental reduction
(`/tmp/qbopt-immediate-multiply.jthvzG/p-g2`). No measured kernel gain yet:
experimental matrix stays 13,138 (2.12x); production stays 12,334 (1.99x).
Do not enable strength on these figures.

Constant-rematerialization checkpoint: the spiller recreates single-definition
immediate constants at uses instead of allocating a stack slot. Grouped parallel
copies and redefined values are excluded; rematerialization precedes in-place
updates of other spilled values. Thirteen spiller checks pass; the old spiller
fails the no-slot regression. Strict LIR-only runtime passes matrix, hotlop,
pressx and lngmix across PDS /G2, QB /O and VBDOS /G3 (12 cases), artifacts
`/tmp/qbopt-remat.YNcAMl`. Matrix modeled cost falls 12,500 -> 12,334 (1.99x).
The target remains 6,210; this is progress, not completion or a full gate.

Zero-fact checkpoint: identical, same-width Held operands of integer XOR/SUB
now establish zero without requiring an input fact. Six fail-first/mutation
regressions and all 998 constant checks pass. Strict LIR-only runtime transforms
(refusal raises instead of falling back) pass matrix, hotlop and pressx on PDS
/G2, QB /O and VBDOS /G3: `/tmp/qbopt-zero-verified.PNnu34`.
This is analysis capability, not a speedup claim: matrix production cost is
12,500 / 6,210 = 2.01x; strength-enabled is 13,224 = 2.13x. Harr stays 6.27x;
segld is 3.85x production / 3.91x with strength. Keep strength disabled. Constant
materialization and allocation now need improvement; do not suppress valid facts
merely to preserve old instruction selection.

Dead-byte ownership checkpoint: `_without` now checks adjacency using `covers`,
not the operation's old address. The former check made a survivor span bytes
still owned by a jump after transformations separated address and ownership.
The focused regression fails with the old function; 46 checks pass. With the
zero-fact and strength experiments enabled in memory, matrix now reports LIR
`rebuilt` and passes PDS /G2 (`/tmp/qbopt-zero-fold.ckjHrW/ownership-p-g2`).
Neither experimental production switch changed in this checkpoint.

CSE phi checkpoint: CSE now replaces phi inputs as well as operation uses when
deleting a repeated computation. A focused regression fails with the old pass;
45 related checks pass. The zero-fact experiment exposed this dangling definition
at matrix 0x79; artifacts: `/tmp/qbopt-zero-fold.ckjHrW`.
Zero folding is withdrawn, not enabled: after fixing the phi, matrix refuses LIR
emission with `0x0044: 9 bytes are claimed by more than one op`. Subsequent runtime
PASS results were fallback, not successful recompilation. Without the experiment,
matrix emission succeeds at 12,076 / 6,210 = 1.94x. Diagnose ownership before
reintroducing identical-operand XOR/SUB zero facts; 998 host checks missed it.

Two-address multiply checkpoint: the pass skipped all MULTIPLY operations even
though the single-result, two-source form reads its destination. It now inserts
the required first-factor copy, leaving widening/fixed forms alone. The old pass
fails the regression; 48 focused checks pass. Experimental reduction passes
matrix on PDS /G2 (`/tmp/qbopt-matrix-tied.CtbS1u/p-g2`) but still costs 13,144
against production 12,076 and target 6,210. Reduction remains disabled pending
profitability and broader correctness evidence; this is not a full-gate result.

Insertion-location checkpoint: setup/update operations now use their insertion
sites instead of the old product address. Tail updates retain the predecessor's
last operation address with zero-width ownership at its end, so a branch to the
next block skips the update. Stage evidence is in
`/tmp/qbopt-matrix-reducer.IRijKE/{baseline,reduced,anchored-tail}`.
Original experimental emission put setup inside loops; corrected entry jumps
now skip latch updates (0x4d -> 0x7d and 0x8f -> 0xa3).
44 focused checks pass; the old reducer fails the insertion-location regression.
Still not safe to enable: setup `zero * 20` emits `imul bx,bx` after loading 20;
trace two-address/coalescing next. The cost regression was not pure pressure.

Current-iteration checkpoint: reduction now replaces a product with a semantic
copy into its original result instead of deleting that definition and replacing
all consumers. Exit phis therefore retain the pre-increment result. This removes
the separate deletion/renaming path and leaves copy elimination to allocation.
The exit regression fails with the old reducer; 43 focused checks pass.
Experimental `matrix-p-g2` modeled cost is 14,164 versus production 12,076
(2.28x versus 1.94x, target 6,210). No enablement: inspect production-stage dumps
to explain the added cost and condition preservation before any runtime batch.

Phi-edge SSA checkpoint: isolated construction now resolves existing phi inputs
at their predecessor ends using analysis-only reads, removed from the returned
body. The regression fails with the old constructor; 42 focused checks pass.
This does not yet substitute strength-reduced exit phis: the outgoing counter
may already be incremented, whereas the original product denotes the current
iteration. Preserve that pre-increment value explicitly before wiring exit uses.

Address-use checkpoint: strength reduction and isolated SSA construction now use
one semantic substitution utility in `ssa.py`, shared with existing transforms.
Memory operands, access lists and merge inputs stay aligned with the renamed SSA
uses. The indexed-use regression fails independently with either old renamer;
41 focused checks plus two existing substitution checks pass. Existing phi-edge
replacement and lowering condition preservation still prevent enablement.

Loop-entry checkpoint: reduced-counter setup now requires a dedicated preheader;
an entry block with a bypass successor could previously introduce a memory read
on the bypass path. The focused regression fails with the old reducer and passes
with the guard; 40 related checks pass. Still outstanding before enablement:
condition preservation in lowering and complete replacement of address/phi uses.
Do not solve condition preservation by teaching MIR optimization machine flags.

Product-result checkpoint: loop reduction now requires the first semantic result
to be the only live result, following phi dependencies transitively. Previously
a high-only result could be replaced by the low recurrence, and a second live
result behind two phis was missed. Both regressions fail with the old function;
38 focused checks pass with the fix. No production switch changed.

Loop recurrence guard checkpoint: all entry values and backedge steps must agree;
one valid backedge no longer certifies the others. Shift reduction now requires a
constant, in-range count and the counter in the value operand, not the count.
Focused checks: 35 passing across induction identity, promotion and MIR-boundary
tests. Restoring the old functions independently reproduces the rejected-path and
shift regressions. Strength reduction remains disabled; this is prerequisite
correctness work, not a new speedup or a full-gate result.

1. Whole-value recognition in raise: lngmix still carries split long accumulator halves, joins and spills. Do not move register-aware widening into MIR optimization. Existing JOIN forms are not yet uniformly explicit semantic operands.
2. Sound array objects/extents, enabling scalar promotion and loop-address reuse. FAR accesses may alias DGROUP; the next named displacement is not proof of an array boundary. Runtime descriptors distinguish near, far and huge storage. Use those facts, not a blanket no-alias rule.
3. Close the measured target gaps, then run integration/commit gates. Keep fail-first symptom regressions per fix and dump adjacent stages when debugging.

Generic copy propagation plus trimming merge dependencies was tried and **reverted**: pressx printed 0 instead of 7500 on all three compilers, and some QuickBASIC runs failed to complete. Do not resurrect that shortcut. A suspected moved-store relocation defect was disproved by the emitted-object check; removing its unnecessary symbol override did not fix a runtime bug.

Runtime scope explicitly excludes /V, /W event trapping and /X resumable errors; do not spend the next round inventing event interfaces. VBDOS B$ENRA remains unestablished. LLVM LICM dedicated-exit/store-dominance rules informed store sinking; LLVM/GCC references are under /Users/alim/work/other.
