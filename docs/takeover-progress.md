# Takeover checkpoint — 2026-09-09

## Promote read-modify-write accumulators

Promotion previously excluded arithmetic with both a memory read and write.
It now separates eligible fixed-cell updates into a value computation and an
observable store, then uses the existing availability and SSA construction.
If a separated read cannot be served from a proven available value, the
original body is retained. NESTED's sum now flows through the nested loop
phis. Its write remains: this enables subsequent store-motion work, not a
speedup by itself (2156 -> 2172, **2.81x -> 2.83x**).

The first runtime trial printed T=0 instead of 675: lowering discarded the
inserted store's relocation ownership. STORE now selects its own machine
operation, and lowering carries its symbol marker into LIR. The computation
owns the original bytes; the inserted store owns the relocated operand.
Both the promotion and backend ownership regressions failed with their old
implementations. Fourteen focused checks pass; NESTED/HOTLOP/FLAGS pass on
p-g2, q-O and v-g3 (nine runtime comparisons).
Dumps: `/tmp/qbopt-rmw-promote-verified`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-rmw-final-rto51u8w`.

## One reload per spilled operand

The NESTED allocation dump contained two identical frame reloads before each
outer-loop multiply. The LIR instruction names the same value twice; the
spiller made two fresh reloads, overwrote the first rename, and used only the
second. Reusing the first rename removes the unused reload without changing
operand multiplicity or any MIR pass. NESTED cost falls 2228 -> 2156,
**2.90x -> 2.81x**. Adjacent backend dumps show exactly two removed loads:
`/tmp/qbopt-single-spill-reload`.

The repeated-operand regression failed before the change; all 15 spill tests
pass. NESTED and SPILL pass strict-LIR runtime checks on p-g2, q-O and v-g3
(six comparisons), artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-single-reload-k5nmekdv`.

## Frame-to-frame phi copies

NESTED-p-g2 refused LIR at 0x004b because both ends of a parallel copy
were spilled. The spiller now keeps that copy grouped with both frame slots
explicit. After dependency ordering, parcopy expands it to a balanced memory
PUSH/POP pair, preserving flags without another scratch register. Selection
now supports 16/32-bit memory POP. Cyclic copies remain explicitly refused.

NESTED now emits LIR at 2228/768 = **2.90x**, not yet the 1.5x goal.
Stage dumps: `/tmp/qbopt-memory-parcopy-final`. Seven focused checks pass;
the spill regression failed against the old spiller, and both encoded-width
checks failed against the old selector. Nine saved runtime results pass:
NESTED, PRESSX, SPILL across p-g2/q-O/v-g3, checked against golden output and
DONE markers, not merely matching two empty files. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-mem-parcopy-c2493ga4`.
The earlier wider focused run had two unrelated MIR-versus-REFUSED expectation
failures; this is not a claim that the full test suite is green.

## Constant-divmod experiment — withdrawn

`db942bd` exposes LNGMIX's 100000 dividend. Replacing its constant DIVMOD
with quotient/remainder copies (14285 and 5) was prototyped but not retained.
Production remains unchanged: the prototype refused emission with
`0x005f: 12 bytes between the ops are not instructions`.

More importantly, adjacent MIR dumps pinpoint an SSA defect in round-three
hoist: the accumulator phi `v11_1 := phi entry:v11_2, latch:v1_6` disappears,
and its loop use becomes a use of the entry zero. Round-four fold then correctly
folds that *incorrect input* to `0 + 14290`. Fix hoist's handling of existing
cross-variable phis before reintroducing the divmod fold; do not blame the
constant evaluator for the phi already lost in the previous stage.

Evidence: `/tmp/qbopt-lngmix-divfold-20260909`, especially
`s29-mir-r03-fold.txt` versus `s32-mir-r03-hoist.txt`, then
`s42-mir-r03-place.txt` versus `s43-mir-r04-fold.txt`.
The prototype copied both semantic results, retained original byte ownership on
the first copy and used zero-width ownership on the second. Ownership remains
another blocker. No speedup or runtime success is claimed for this experiment.

## VBDOS procedure-exit interface

PROCS `/G3` now emits through LIR rather than refusing B$EXSA at 0x113.
The exit retains all six allocatable general-register inputs, including DX:AX
exported to the BASIC caller. Disassembly of VBDCL10E.LIB rtenexit.asm at 0x68
establishes that the incoming arithmetic flags are overwritten; the normal
frame-restoring return removes no caller arguments. Helper effects remain
unknown: no memory, clobber, or control guarantee is relaxed. Full-library
contract analysis is evidence for unresolved dependencies, not an automatic ABI.

The emission regression failed first; five focused checks pass. Strict-LIR
runtime checks pass PROCS on PDS `/G2`, QB `/O`, and VBDOS `/G3` (three cases
each). Stages: `/tmp/qbopt-procs-exit-20260909`. This removes a coverage blocker,
not a claim of target completion or a full integration gate.

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

VBDOS entry-contract checkpoint: `B$ENRA` can now be lowered at sites with
an unrelocated immediate `MOV BX,0` immediately before the call in the same
basic block. `tools/libdump.py B$ENRA` shows VBDCL10E.LIB's rtenexit.asm:
entry 0x17 builds the frame from CX; `or bx,bx` at 0x4b skips the unresolved
helper call at 0x5b when zero. Both BX and CX remain fixed call inputs,
so the selector's zero value must survive allocation. All other effects use
the worst-case contract. Nonzero, relocated, unknown or separately entered
call sites remain unestablished. Four focused checks pass; removing the
site specialization fails the positive case. PROCS is **still refused**:
the next exposed blocker is VBDOS `B$EXSA` at 0x113, not B$ENRA. No claim
of completed procedure support or runtime success is made for this increment.

ADDRM ownership checkpoint: VBDOS/G3's three-byte emission refusal at 0x80
is fixed. CSE had correctly transferred the deleted index reload's bytes to
the preceding high-word store. Widening recomputed its chain end from the
original instruction nodes, discarding that transferred ownership. Pair and
chain endpoints now honor current `covers`, with original node spans only as
fallback. The real-object emission regression failed before the fix and now
requires strict LIR output. ADDRM, ARITH and NEGNOT pass runtime checks on
PDS/G2, QB/O and VBDOS/G3 (nine program/configuration runs). Before/after
stage dumps: `/tmp/qbopt-addrm-ownership-20260909` and
`/tmp/qbopt-addrm-ownership-fixed-20260909`. The known PROCS/TWICE runtime
contract refusal remains, and broader full-goal validation is outstanding.
Five existing widening ownership/restore checks also pass (75.93 seconds;
these five checks internally traverse the fixture corpus, so they were run
once, not as a repeated test loop).

**Strength reduction is now enabled by default**, restricted to multiplication
chains in innermost loops. ADDRM's cheap shift chains regressed 2,308 -> 2,578;
excluding shift-only formulas keeps it at 2,308 and avoids an unnecessary
counter in ARRIDX as well. This policy deliberately forgoes the earlier
shift-only SEGLD/stride gains until pressure-aware selection exists.
Lowering rejects an inserted ADD/MUL crossing a live condition when comparison
scheduling cannot preserve it, including conditions live across block edges.
This is a correctness guard, not successful optimization of those cases.

Current PDS/G2 modeled costs, strength off -> on:
matrix 11,916 -> 11,418; arridx 1,138 -> 742; split 904 -> 508;
ivchan 1,399 -> 1,023. ADDRM 2,308, SEGLD 25,802 and stride 1,882 are unchanged.
The three-family emission check is 94/96 LIR: all 32 PDS/G2 and 32 QB/O;
30 VBDOS/G3. Its two pre-existing refusals remain ADDRM's unowned bytes at
0x80 and PROCS/TWICE's unestablished B$ENRA interface. Seventeen of eighteen
focused runtime program/configuration runs pass; VBDOS ADDRM explicitly
reports REWRITEFAIL, not PASS. Seventy-four focused host checks pass.
Removing either the cheap-work filter or the condition guard fails its
regressions. Production stages: `/tmp/qbopt-strength-production-arridx-20260909`.
Full milestone validation remains incomplete; the overall goal is not met.

Condition-selection checkpoint: lowering now schedules a pure, single-use
comparison immediately before its terminal branch. A synthesized stride add
between them previously left the branch reading the add's machine flags,
despite MIR naming the comparison's condition value. The change is confined
to lowering, following the adjacency role of LLVM SelectionDAG glue; MIR
passes retain their semantic ordering. Memory/effectful comparisons, additional
results and multiple consumers are not moved. Those general cases still need
condition materialization or flag-aware scheduling before unrestricted use.
The new ordering regression fails with scheduling removed; 40 focused checks
and 12 strict LIR program/configuration runs pass (matrix, segld, bools and
flags across three compiler families). Dumps:
`/tmp/qbopt-conditions-matrix-20260909`. Strength remains disabled and full
milestone validation is outstanding.

Innermost selection checkpoint: reducing matrix's outer row counter created
a live range across its inner loop and a spill/reload on every inner iteration.
Restricting strength reduction to innermost natural loops avoids that loss:
experimental matrix cost is now 11,418 versus production 11,916; segld is
24,642 versus 25,802; HARR is unchanged at 11,094. The new real-output cost
regression fails at 12,974 when the restriction is removed. Thirty-six focused
checks and six strict LIR runtime cases pass. Full dumps and final assembly:
`/tmp/qbopt-inner-strength-matrix-20260909`. Strength is still disabled pending
backend condition/flag safety and a broader milestone validation. This is a
temporary selection policy, not a substitute for target-aware pressure costing.

Affine-address checkpoint: induction analysis composes word multiply,
same-counter add/subtract and shifts, including known SSA constants. Matrix's
diagonal `(i * 20 + i) << 1` is recognized as stride 42; strength reduction
chooses terminal candidates rather than introducing counters for every term.
Thirty-five focused checks pass; disabling composition makes the two new
coefficient/wrap regressions fail. Six strict LIR runtime cases pass for
matrix/segld across PDS /G2, QB /O and VBDOS /G3. Stage-by-stage evidence is
in `/tmp/qbopt-affine-matrix-20260909`.
Strength remains disabled: experimentally enabling it costs matrix 12,974
versus production 11,916, though segld improves 25,802 -> 24,642. This is
analysis infrastructure, not a production performance gain. Profitable
selection and latch flag safety remain prerequisites to enabling it. The
focused lint invocation reports existing annotation/zip diagnostics; no
full-gate claim.

Dead-phi checkpoint: dead-code elimination now removes unused phi cycles,
retaining externally demanded values and all real argument/address readers.
Dependencies propagate through live phis; obsolete preservation-only references
are removed with dead phis. Matrix's unused high-product phis no longer block
projection, and its final assembly contains immediate `imul` forms. PDS modeled
cost falls 12,334 -> 11,916 (1.92x); pressx falls 820 -> 782. HARR remains
11,094. Seventy-two focused checks and 15 strict LIR runtime cases pass
(matrix/pressx/HARR/nots/lngmix across PDS /G2, QB /O, VBDOS /G3). Disabling
phi pruning fails the real-matrix regression. Dumps and emitted assembly:
`/tmp/qbopt-matrix-dead-phis`. Full validation remains outstanding.

Algebraic checkpoint: a machine-independent pass now simplifies integer
identities and projects a two-result word multiply to its low result when
the high answer, flags and preserved upper bits are unobserved. Actual reads
and phi inputs prevent projection; discarded preservation references are
removed with the discarded definitions. This lets lowering choose ordinary
two-address/immediate multiply forms without accumulator-pair constraints.
HARR PDS now emits `imul bx,cx`; modeled cost falls 11,494 -> 11,094 (6.05x,
still far above 1.5x). pressx changes 824 -> 820; the sampled matrix, SEGLD,
lngmix and arridx costs remain unchanged. Forty-six focused/algebraic-boundary
checks pass; disabling projection fails the real-HARR regression. Strict LIR
HARR/matrix/nots/SEGLD/lngmix runtime passes across PDS /G2, QB /O and
VBDOS /G3 (15 cases). `/tmp/qbopt-low-product-harr-20260909` contains every
stage and the emitted assembly. Full validation remains outstanding.

Copy-propagation checkpoint: CSE now substitutes full-width copy values into
their uses and removes the copies. Low-word copies are also eligible when
the existing bit-demand analysis proves their preserved upper word unobserved;
wide readers and partial reads of a wider source still prevent substitution.
HARR's first CSE stage removes copies at 0x5c, 0x6b, 0x6d and 0x7b. Required
machine moves are reintroduced downstream: modeled PDS costs remain unchanged
for HARR, SEGLD, pressx, lngmix and matrix. This improves the MIR boundary,
not the target scoreboard yet. Thirty-five focused checks pass; restoring the
old copy-retention code fails the regression. Strict LIR runtime passes those
five programs on PDS /G2, QB /O and VBDOS /G3 (15 cases). Stage evidence is in
`/tmp/qbopt-copy-values-harr-20260909`.
Rejected experiment: simply allowing CSE to share partial-write copies lowered
HARR 11,494 -> 11,296 but worsened SEGLD 25,802 -> 26,604. That relaxation is
not enabled; direct source-value propagation avoids the measured regression.

Array-request checkpoint: raise recognizes DDIM/RDIM argument setup and attaches
`ArrayRequest` to the call: symbolic descriptor, element width, dimension bounds,
and whether it replaces an existing allocation. Recognition stays in
`raising_arrays.py`, outside MIR passes. The ABI is documented in QB 4.5
`runtime/rt/dynamic.asm`; real HARR and SEGLD objects confirm argument setup on
PDS /G2, QB /O and VBDOS /G3, including QB's register-fed pushes. ADIM is not
classified as allocation. Thirteen focused checks pass; annotation preserves
strict LIR emission byte-for-byte for HARR on those three configurations.
`/tmp/qbopt-array-request-q-20260909` shows the request at raise and after opt.
These are requested shapes, NOT proof of successful allocation, physical
disjointness, lifetime, or in-bounds access. Next use requires those proofs;
do not feed requests directly to no-alias. No target reduction claimed here.

CSE semantic-operands checkpoint: optimizer-created operations no longer need
an original decoded node to participate in value numbering. Symbolic operands
also participate by their complete identity, never by their encoded zero.
The originless and symbolic cases fail under the old computation key; 33
focused checks pass, including distinct-address negatives. Strict LIR
HARR/matrix/chain/segld runtime passes on PDS /G2, QB /O and VBDOS /G3
(12 program/configuration cases). HARR stage dumps are in
`/tmp/qbopt-cse-symbolic-harr-20260909`. No new target reduction is claimed:
partial-write preservation and the far-store/descriptor alias barrier remain.

CSE value-identity checkpoint: removed the obsolete same-variable restriction
on common-expression reuse. The dominator/value/width checks still apply;
register placement belongs to allocation, not this pass. The regression now
exercises both direct and phi uses across different variable identities and
fails with the old restriction restored in memory. All 25 induction/SSA checks
pass, and strict LIR chain/matrix/lngmix/pressx runtime passes on PDS /G2,
QB /O and VBDOS /G3 (12 program/configuration cases). PDS chain shrinks three
object bytes and modeled cost changes 1,632 -> 1,630. This is a removed
architectural restriction, not a claim of closing the remaining target gaps.
Stage evidence: `/tmp/qbopt-cse-chain-20260909`.

Symbolic-address checkpoint: HARR's descriptor move at 0x6f was raised as
literal zero although its immediate has a relocation to segment 5 + 6.
Raise now preserves a symbolic operand (target, offset, width and addend),
lowering retains its relocation identity, and spilling excludes these operands
from literal rematerialization. This corrects an unsound constant fact; no
runtime miscompile from that fact is claimed. Both regressions catch in-memory
restorations of the bugs; 1,015 consts/spiller checks and two focused LIR checks
pass. Strict LIR HARR/segld/matrix runtime passes across PDS /G2, QB /O and
VBDOS /G3 (nine cases). Stage dumps are in `/tmp/qbopt-symbol-harr-20260909`.
Full validation remains outstanding; touched files retain baseline lint errors.
This establishes symbolic descriptor identity, not allocation extents or no-alias.

Reassessment after immediate multiply: the whole-fixture target scan still shows
large array gaps; ordinary bools/subexp configurations meet 1.5x. HARR's actual
final MIR (`/tmp/qbopt-harr-reassess-20260909/s24-mir-r02-place.txt`) stores through
F[v3_7] at 0x78, then reloads descriptor offset D[v4_5] at 0x7d to form v3_9 and
reads F[v3_9] at 0x83. They are not yet proven equal: the far store may alias the
descriptor in today's memory model. Removing the reload without proving distinct
storage is unsound. Next substantial work is allocation/descriptor provenance in
raise, exposed as object identity to MIR alias analysis. Do not treat FAR as a
no-alias promise or next-symbol bounds as object extents. Strength tinkering alone
does not remove this barrier.

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
# Hoisting preserves existing SSA edges

Hoisting no longer reconstructs the entire body from variable numbers after
moving invariant definitions. Their existing uses and phi edges remain valid
when the definitions move to the dominating preheader; reconstruction discarded
cross-variable accumulator phis. A regression on real LNGMIX MIR gives a phi
its own variable and checks its incoming value identities survive an actual
hoist. It failed before the fix and passes after it.

Validation: nine focused hoist/fixed-point checks pass. Strict-LIR LNGMIX,
HOTLPX and PRESSX run correctly on PDS `/G2`, QB `/O` and VBDOS `/G3` (nine
runtime passes). Stage files: `/tmp/qbopt-hoist-ssa-20260909`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-hoist-ssa-6h90ojay`.
Constant DIVMOD folding remains withdrawn; its byte-ownership issue is still
open. No performance improvement is claimed for this correctness repair.
# Constant division now reaches the executable

LNGMIX's constant quotient/remainder fold to 14285 and 5. The production cost
falls from 867 to 566 against target 210 (4.13x to 2.70x, not yet complete).
The folder uses signed truncation toward zero and retains zero-divisor and
signed-overflow cases, unknown operands, and live non-result effects.

Two ownership/selection issues were exposed and repaired in the same slice:
noncontiguous push-byte ownership now travels with MIR operations through CSE
and deletion, rather than disappearing with an operation ID; replacement
constants clear the original runtime-call node. Keeping that node emitted a
bare runtime call after its arguments had disappeared and timed out. No result
from that failed run is counted as validation.

Validation: 24 focused checks pass, one existing xfail. The emitted-code
regression fails when constant folding is disabled and separately when CSE's
extra-range transfer is disabled (the original 12-byte refusal). LNGMIX and
HOTLPX pass strict LIR execution on all three compiler families (six runs).
Artifacts: `/tmp/qbopt-constant-division-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-divfold-verified-8f_6zsli`.
Remaining LNGMIX work includes redundant high-half reconstruction and memory
traffic in its accumulator; inspect these stage dumps before changing them.
# High-part extraction experiment — next boundary to repair

The remaining LNGMIX joins reconstruct the high halves of constant 14285 and
5. `mir._handing_back` currently gives them no semantic arguments or results,
so constant propagation cannot prove either zero. A trial explicit EXTRACT
operation made both facts provable, but is not retained:

- Folding extraction to a COPY left it inside the loop (LICM excludes an
  independent copy), pulling its dependent ADD/ADC chain back in as well.
  Cost rose from 566 to 616. `/tmp/qbopt-extract-trial` has every stage;
  `s58-lir-lowered.txt` shows the zero move and ADD/ADC in the loop.
- Leaving EXTRACT unfurled instead refused at `0x004b: restore is not one
  select.py can emit`. The legacy restore adapter cannot lower an operation
  with explicit semantic operands. Renaming JOIN alone is not a migration.

Next implementation needs an explicit bit-extraction lowering, preserving
flags and partial-write semantics, alongside the raise change. Then propagate
constant carry from the known ADD into ADC so the whole invariant chain folds,
instead of replacing only its first instruction with a non-hoistable copy.
The trial was removed; production remains at 566 and no runtime or speedup is
claimed for this experiment.
# Explicit extraction reaches lowering

Runtime high-part handbacks now raise as EXTRACT(source, bit offset) with an
explicit result width. The backend implements the current 32-to-high-16 shape
with a balanced push/pop/pop expansion over abstract values. It preserves
flags and leaves register assignment to allocation. Unsupported shapes refuse.
The object writer now accepts expansion instructions with no original MIR op;
these carry generated semantics and own no original bytes.

Validation: the lowering regression failed before expansion was implemented;
13 focused extraction/division/condition tests pass. LNGMIX and HOTLPX pass
strict LIR execution on PDS `/G2`, QB `/O`, and VBDOS `/G3` (six runs).
Artifacts: `/tmp/qbopt-extract-lowered` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-extract-lower-2duh528o`.

This is a boundary migration, not a speedup: LNGMIX currently costs 594 versus
566 before it. Next fold explicit extraction plus known carry-dependent
arithmetic together; replacing only extraction with a constant previously
stranded its dependent chain inside the loop.
# Constant extraction and carry propagation

Constant propagation now evaluates explicit bit extraction and ADD_CARRY when
the exact input condition's carry is proven by a constant ADD. Carry facts are
kept separate from value facts: knowing carry does not establish the other
condition bits. Unknown carry and insufficient source widths remain unknown.
Folding also refuses a constant fact narrower than the result it would replace.

LNGMIX's constant ADD/ADC becomes constants; propagation into the remaining ADC
reduces cost from 594 to 580 (still 2.76x target). Fifteen focused checks pass;
carry tests failed before implementation and the width guard test fails when
the guard is removed. LNGMIX/HOTLPX pass on PDS, QB and VBDOS through strict LIR.
Artifacts: `/tmp/qbopt-carry-final`,
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-carry-tmu2qjhw` (PDS/QB),
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-carry-v-ak5up4y6` (VBDOS).

Next blocker: dead-code deletion groups operations by original address. A live
operation therefore keeps dead sibling copies. A trial deleting by object
identity instead refused with `0x005f: 12 bytes are claimed by more than one
op`; it was withdrawn. Ownership transfer must handle these siblings before
the newly constant chain can be fully removed. Do not loosen layout's check.
# Inserted moves own no disjoint input ranges

The shared-address deletion experiment's duplicate 12-byte ownership was
introduced by `objwrite._carried`, not the MIR deletion itself. Stage-by-stage
range counting found no overlapping ownership through optimization. An inserted
allocator move then inherited its parent operation's `extra_covers`, despite
correctly clearing its ordinary span and ID. It now clears the extra ranges too.
The targeted regression fails before the fix; ten extraction/division checks
pass after it. Production LNGMIX still emits at cost 580.

Deleting dead siblings by object identity now emits, but costs 598 and still
retains dead copies whose bytes have no adjacent taker. That deletion change
was not retained. `/tmp/qbopt-dce-identity` records its stages. The next change
should unify ownership of removed operations instead of relying on an adjacent
instruction's single span, allowing true deletion rather than leftover moves.
# Dead computations emit no bytes

Dead-code removal now replaces each dead operation with an empty ownership
marker: no values, operands or original node, but the same input byte ranges.
Lowering and selection give that marker an empty encoding. A live sibling at
the same input address no longer keeps the dead computation alive, and no
adjacent survivor is needed to inherit its bytes. This retains layout's full
coverage check rather than bypassing it.

LNGMIX drops from 580 to 552 (2.63x target). MATRIX measures 11376/6210
(1.83x). The regression fails against the old DCE implementation; seventeen
focused tests and nine strict-LIR runtime runs pass: LNGMIX, HOTLPX, MATRIX
on PDS `/G2`, QB `/O`, VBDOS `/G3`. Artifacts:
`/tmp/qbopt-dead-marker-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-dead-marker-qhdaewbz`.
The target is still unmet; accumulator memory traffic remains in LNGMIX.
# Promote carry-arithmetic memory reads

ADD_CARRY was missing from promotion's supported read operations. It now reuses
available stored values under the same alias/width checks as ADD, preserving
its condition input. LNGMIX's high accumulator read becomes an SSA value and
the loop emits `adc di,0` rather than reading its high word from memory.
Cost drops 552 to 540 (2.57x target).

The real-fixture regression failed first. Ten promotion checks and nine strict
LIR runtime runs pass (LNGMIX, HOTLPX, MATRIX on PDS, QB, VBDOS). Artifacts:
`/tmp/qbopt-promote-carry-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-promote-carry-m80cp_0f`.

Both accumulator stores remain in the latch. Store sinking currently considers
only stores in the block that has the exit edge, which is the test/header in
this rotated loop, not the latch. Extending it requires selecting the exit
phi's value and proving zero-trip behavior, not moving the latch value directly
to a path where it may never have been defined. The two stack-local temporary
stores and counter-copy traffic also remain visible in the emitted dump.
# Rotated-loop accumulator stores sink to the exit

Store sinking now handles a two-block rotated loop with one latch and one
exit. The moved store reads the header phi, not the latch definition. Its
outside incoming value must match the last initializing store in the entry
predecessor; unknown/aliasing writes, observers, missing initialization and
alternate latch entries refuse the move. COPY tracing respects result widths.

LNGMIX falls from 540 to 428 (2.04x target): both accumulator stores are after
the loop. The two temporary stack stores and counter-copy traffic remain.
The positive real-fixture test failed before implementation; missing-zero-trip
initialization remains a negative case. Eleven focused checks pass. Nine
strict-LIR runtime runs pass (LNGMIX, HOTLOP, MATRIX on PDS, QB, VBDOS).
Artifacts: `/tmp/qbopt-rotated-store-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-rotated-store-36lii9ld`.
# Phi widths enable counter-copy propagation

The CSE/copy-propagation width map previously described only operation results,
never phi results. It now reaches a fixed point across phis whose incoming
definitions all have the same known width. Unknown or conflicting widths stay
unknown; demanded-high-half checks still apply before substitution.

LNGMIX falls from 428 to 386 (1.84x target). The emitted loop counter now stays
in one register through increment and comparison; its two per-iteration copies
are gone. The positive width test failed first, the conflicting-width negative
passes, and fifteen focused checks plus nine strict-LIR runtime runs pass
(LNGMIX, HOTLOP, MATRIX across PDS, QB, VBDOS).
Artifacts: `/tmp/qbopt-phi-width-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-phi-width-99x3f49l`.
Two temporary stack stores and the split-word accumulator arithmetic remain.
