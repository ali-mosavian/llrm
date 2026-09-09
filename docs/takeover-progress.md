# Takeover checkpoint — 2026-09-09

## Proven allocation identity removes the element reload

The bounded array proof establishes both extent and the descriptor-derived
segment for every marked access. Equal offset SSA values in that same proven
allocation therefore identify equal bytes without segment-register SSA.
Memory equality now uses that fact, while missing or different allocation
proofs still refuse equality. Existing forwarding removes the immediate reload
of HARR/SEGLD's just-stored element.

PDS costs: **HARR 5062 -> 4462 (2.43x)**;
**SEGLD 13456 -> 11056 (1.65x)**. Two new regressions fail against old memory
equality, including HARR's surviving element load; 20 focused checks and six
strict-LIR runtime comparisons pass across p-g2/q-O/v-g3. Segment setup remains
in the loop and remains the next placement opportunity.
Dumps: `/tmp/qbopt-allocation-equality`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-allocation-equality-hkp11p3v`.

## Induct the complete offset pointer, including invariant descriptor data

Affine composition now accepts a proven unchanged direct cell as an additive
offset. Strength reduction reads it in the preheader and advances the full
offset pointer thereafter. Symbolic descriptor references are canonicalized for
the inserted operand; no register names enter the induction analysis.
HARR's adjusted-offset read leaves the inner loop, which advances by 42 bytes.
The segment reload still remains in the loop.

PDS modeled cost: **HARR 7130 -> 5062, 3.89x -> 2.76x**;
**SEGLD 16896 -> 13456, 2.52x -> 2.01x**. Forty focused induction checks
pass; the real HARR preheader-read regression fails with old analysis, and a
descriptor-write variant keeps the read inside. Nine strict-LIR runtime
comparisons pass (HARR/SEGLD/NESTED across p-g2/q-O/v-g3).
Dumps: `/tmp/qbopt-invariant-pointer`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-invariant-pointer-p8ksw05l`.

## Reuse unchanged memory-dependent computations

CSE now admits explicit cell operands and checks intervening writes/barriers
before reusing them within one block. Far references without segment identity
remain excluded. This removes HARR's duplicate descriptor-adjusted address;
the existing induction-variable reduction still supplies the 42-byte step.
PDS costs: HARR **7930 -> 7130 (3.89x)**, SEGLD **20096 -> 16896 (2.52x)**.
The larger remaining goal is invariant descriptor/base/segment placement and
complete-pointer induction, not merely duplicate-expression removal.

Three fixture regressions fail against old CSE; corresponding unknown-write
cases retain both computations. Fourteen focused checks and nine strict-LIR
runtime comparisons pass (HARR/SEGLD/LNGMIX across p-g2/q-O/v-g3).
Dumps: `/tmp/qbopt-memory-cse`; runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-memory-cse-hslacsvl`.

## Bounded array paths unlock existing optimization

The raise now proves finite word-width array paths by exact scalar evaluation.
Each far access must use the descriptor's loaded segment and adjusted pointer,
and fit the allocation before its memory effect is interpreted. Unknown branches,
calls before later accesses, descriptor writes, unsupported operations, nonzero
lower bounds and traversal exhaustion discard the proof. No alias assumption is
used to establish the bounds. The resulting allocation metadata only excludes
direct accesses to the descriptor's program-data segment; other pointers remain
conservative. This is a bounded proof, not general symbolic range analysis.

Existing constant propagation and promotion now retain descriptor dimensions and
counter values across the element stores. PDS modeled costs improve:
**HARR 11294 -> 7930, 6.16x -> 4.32x**;
**SEGLD 25002 -> 20096, 3.73x -> 3.00x**.
Both still miss the 1.5x target. Segment-load optimization and further loop/codegen
work remain. No array-specific optimization pass was introduced.

Three real HARR dimension-fact regressions fail with the proof disabled; 35
focused checks pass, including rejected out-of-bounds/unknown-condition/changed-
descriptor/wrong-segment/exhausted proofs. Six strict-LIR runtime comparisons
pass for HARR/SEGLD across p-g2/q-O/v-g3. Stage dumps display allocation proofs:
`/tmp/qbopt-array-bounds-final`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-array-bounds-k2_79k98`.

## Memory identity requires the segment, not just the offset

HARR's descriptor-segment loads still produce Opaque results; element references
have no segment SSA identity. `same_bytes` previously accepted two such far
references as equal, even though an intervening segment reload can change the
address. It now requires known segment identity for far-reference equality.
Dead-store coverage reuses that equality rule after aligning displacements;
its old independent check also ignored relocated segment indices, allowing
equal offsets in different objects to appear to cover each other.

Three failing-before cases cover unknown segment, changed segment and different
relocated objects; a positive case preserves coverage for known same pointers.
3417 availability checks pass in 15 seconds; nine strict-LIR runtime comparisons
pass (HARR/SEGLD/NESTED across p-g2/q-O/v-g3). HARR's emitted stage is unchanged.
Dumps: `/tmp/qbopt-far-identity`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-far-identity-7nomd5wb`.

Next array proof must include segment provenance as well as offset bounds;
neither an unchanged offset nor a DIM request alone proves address identity.

## Numeric DIM postconditions reach constant propagation

The raise now emits generic normal-return memory values for numeric DDIM:
dimension count, element width, and each dimension's count/lower bound.
The descriptor layout stays in raising_arrays; constant propagation sees only
memory/value pairs, and stage dumps print them. Unknown stores still kill
the facts. This does not prove bounds or disjointness for element accesses.

Verified against shipped BCOM45/BCL71ENR/VBDCL10E dynamic.asm disassembly:
dimension count at +8, element width +12, counts/lower bounds +14/+16 with
stride 4. The stack is consumed backwards, so the last dimension is first.
QB's stores are at 0034/003a/004b/0052; PDS/VB at 0017/001d/0030/0037.
Only recognized compiler families and numeric allocation attributes use these
facts. Unknown families, strings, wrapping descriptors, RDIM and locally defined
runtime-name substitutes do not acquire these postconditions.

All three real HARR postcondition regressions fail with the old constant walk;
33 focused checks pass. Six HARR/SEGLD emission comparisons across the compiler
families are byte-identical with/without postconditions, so no new runtime loop
was needed. No speedup yet: loop element stores still conservatively invalidate
the facts. Dumps: `/tmp/qbopt-dim-postconditions-visible`.

## Constant memory facts retain pointer identity

The constant-cell walk recorded a constant pointer-relative store using only
its displacement, dropping the base/segment SSA value. A direct read could
then incorrectly acquire that constant. It now invalidates potentially aliased
facts but records a new direct fact only for a proven direct address. Proven
symbolic references are canonicalized on both reads and writes, making the
descriptor-address metadata usable by constant propagation.

Three new cases fail before the fix; 1030 focused constant/array checks pass.
Nine strict-LIR runtime comparisons pass (HARR/SEGLD/LNGMIX across p-g2/q-O/v-g3).
HARR's final MIR is identical to the previous stage dump; no speedup claimed.
Dumps: `/tmp/qbopt-constant-pointer-identity`; runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-constant-pointer-68wnorjb`.

Runtime source inspection also confirms that dynamic allocation can be near:
QB45 `dynamic.asm` uses the near allocator and DGROUP when FADF_FAR/HUGE are
clear. Array provenance therefore still requires an in-bounds proof; a DIM
request alone cannot establish disjointness from program data.

## Proven descriptor addresses without changing relocation operands

The raise now attaches a symbolic effective address to descriptor references
whose pointer is a known word-width symbol. Memory equality and alias checks
use that address; lowering retains the original address/base and relocation
operands. Thus HARR's descriptor fields at offsets 8 and 16 can be identified
without turning their unrelocated pointer-relative operands into invented
relocations. This proves field identity, not field contents or heap bounds.

Far accesses, wide pointers and effective offsets that wrap remain unknown.
Nineteen focused checks pass; all three compiler-family metadata regressions
fail against the old raise. HARR and SEGLD emit byte-identical objects with
and without the metadata on p-g2/q-O/v-g3 (six comparisons), so no additional
runtime loop was needed. Costs are unchanged. Array-element provenance and
in-bounds reasoning remain the next required part of the work.

## HARR: the next major gap is array provenance, not another arithmetic pass

Current stage evidence (`/tmp/qbopt-harr-current`, especially s43-mir-widen):
the raise recognizes DDIM's `(0..20, 0..20)` bounds and element width 2,
but that request is not connected to later element references. Those remain
`[es:bx]` with no allocation identity. Descriptor accesses likewise remain
`[abs+si+0xa]` and `[abs+si+0x2]`, even though their base comes from the
descriptor symbol. `raising_arrays.annotated` annotates allocation calls only.

Consequently a store through the element reference may alias the counters
and descriptor, so promotion, CSE and LICM correctly retain their loads.
The final loop still loads the dimension word at 0x5e, multiplies at 0x61,
recomputes the descriptor-based address at 0x72 and 0x7d, and reloads the
stored element at 0x83. The loop counter reload at 0x8a also blocks a simple
SSA recurrence proof. Adding another strength-reduction pattern cannot
resolve these missing memory facts.

Next implementation milestone: connect the versioned runtime allocation
contract to descriptor and element provenance in the raise, with an explicit
in-bounds proof before claiming disjointness from program data. Do not treat
every far access as heap memory or infer safety merely from a DIM request;
unknown indexing and descriptor mutation must remain conservative. Preserve
the original relocation-bearing operands in lowering while carrying semantic
provenance separately. This is needed to unlock the existing general passes,
not to add machine-aware special cases to them. No HARR speedup is claimed.

## Allocate sign extraction without fixed AX/DX when flags are dead

ADDRM's promoted accumulator added pressure around CWD's fixed-register
interface, producing a spill. Lowering now selects copy plus arithmetic
shift for word/dword sign extraction when flags are dead; the operands stay
abstract until allocation. Otherwise it retains the original flag-preserving
conversion. The decision uses backward flag liveness after branch scheduling,
including block live-outs, not a scan of the next instruction alone.

ADDRM cost **1794 -> 1320 (2.38x -> 1.75x)**, improving on the earlier 1492
baseline as well. The new dead-flags test fails before the change, and the
live-flags cross-block case retains CWD. Nine focused lowering checks and
nine strict-LIR runtime comparisons pass (ADDRM/LNGMIX/NEGNOT across the three
compiler configurations). Dumps: `/tmp/qbopt-sign-shift`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-sign-lower-2h24eknr`.

## Preserve independent promotion when an update cannot be split

The PDS scoreboard refresh caught SEGLD regressing 25002 -> 28202: an
unpromotable memory update caused promotion to return the original body,
discarding unrelated counter promotions too. Retry write-through promotion
without update splitting in that case; retain the unsafe update in memory.
SEGLD returns to **25002/6704 = 3.73x**. A real-fixture regression fails
before the fix. Twelve focused promotion checks and nine strict-LIR runtime
comparisons pass (SEGLD/NESTED/HARR across p-g2/q-O/v-g3). The older
promotion-store test now isolates store motion, whose separate tests verify
the store's new exit placement rather than its original instruction address.

Dumps: `/tmp/qbopt-segld-promotion-fixed`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-promote-independent-7qcme0hf`.

Current PDS measurements also show ARRIDX 0.89x, PRESS 0.82x, SPILL 1.37x,
SPLIT 1.30x; MATRIX 1.62x, ROTATE 1.54x and IVCHAN 1.51x remain above goal.
ADDRM regressed to 2.38x from the earlier 1.98x and is not fixed by this
change. HARR 6.16x and floating-point cases above 3x remain major gaps.

## Affine addresses with invariant offsets

Induction analysis now retains invariant additive terms while composing
word-width arithmetic. Strength reduction initializes these terms outside
the loop and advances only the scaled counter. This recognizes NESTED's
`(rowBase + column) * 2`: its address now advances by two instead of adding
row and column then shifting on every inner iteration. No machine register
names enter the analysis or transform.

NESTED cost **1962 -> 1900, 2.55x -> 2.47x**. The real-fixture regression
fails with the old implementation; nine runtime comparisons pass across
NESTED/MATRIX/ADDRM on p-g2/q-O/v-g3. Stage dumps:
`/tmp/qbopt-affine-offset`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-affine-offset-hx0x5oew`.

## Store motion through a nested loop body

A rotated loop need not have only two blocks: a unique latch with a direct
backedge still runs on every completed iteration. Store motion now uses
that property while retaining the unique header exit, zero-trip seed and
whole-loop observability checks. NESTED's accumulated sum is stored once
after the outer loop instead of once per row; initialization remains.
Cost **1990 -> 1962, 2.59x -> 2.55x**. It still misses the overall goal.

The real-fixture regression failed before this change. All 13 store-motion
checks pass, as do nine strict-LIR runtime comparisons of NESTED, MATRIX,
HOTLOP across p-g2/q-O/v-g3. Dumps: `/tmp/qbopt-outer-store-sink`.
Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-outer-sink-o3uxyvdc`.

## Sink the nested accumulator's inner-loop store

The zero-trip seed proof now follows predecessor edges and translates phi
inputs, instead of requiring initialization in the immediate preheader.
Every incoming path must establish the same cell/value pair; an unknown
write, barrier or uninitialized entry rejects the move. Backedges discharge
the same inductive obligation, while entry paths still need an actual store.
This recognizes NESTED's outer phi as the inner loop's memory seed.

NESTED now writes its sum at the inner-loop exit (five times instead of 30),
not on each inner iteration. Cost **2172 -> 1990; 2.83x -> 2.59x**. The
outer-loop exit store and remaining allocation costs are still opportunities.
The new real-fixture regression fails before the change; removing its seed
keeps the store in the loop. All 12 store-motion checks and nine strict-LIR
runtime comparisons pass (NESTED/HOTLOP/LNGMIX, p-g2/q-O/v-g3).
Dumps: `/tmp/qbopt-nested-store-sink`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-nested-sink-9g9z4h2i`.

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

# Nbody: correctness baseline and the next optimization boundary

At `051de97`, strict optimized LIR passes all 24 nbody outputs on PDS `/G2`,
QuickBASIC `/O`, and VBDOS `/G3`. This follows fixes for lost coalescing pins,
address-keyed dead-store deletion, duplicate widening, and undeclared restore
clobbers. NEGNOT and LNGMIX also pass on all three. This is a correctness
baseline, not evidence that the modern-backend performance goal is met.

The PDS stage dump at `/tmp/qbopt-nbody-pds-baseline` shows five runtime calls
in the interaction loop that `calls.sites()` recognizes with `consume`
arguments but an empty `pushed` classification. `mir._sites()` skips them:

- `0x1a0`: divide distance by 262144.
- `0x1b2`: divide 512 by the computed denominator.
- `0x1cd`: multiply deltaX by falloff.
- `0x1d4` and `0x204`: divide the respective products by 512.

The emitted loop spills `other` at `[bp-0x24]` and both halves of accX/accY
at `[bp-0x26]` through `[bp-0x2c]`. Array-index shifts and invariant position
reads remain too, but the arithmetic calls are an upstream constraint on
retaining those values.

Next: represent stack-fed runtime arithmetic as semantic MIR at the raise.
Resolve each argument's actual pushed value, including arguments pushed before
a nested call; combine high/low words with their exact width and ordering.
Do not merely inline stack pops into an opaque machine sequence: that hides
constant division and value lifetimes from the optimizer again. Preserve
stack balance and return-half consumers, then verify nbody and one focused
nested-call regression before measuring the resulting loop. No hand-derived
nbody target exists yet, so no modern-target ratio is established.

## Nbody: recovered arithmetic and constant division

`f921137` and `f2f5d57` recover stack-fed division and multiplication into
scalar MIR, including arguments pushed before nested calls. `3a8ac25`
propagates constants through word concatenation and removes stale machine
metadata from folded computations (CHAIN's MODMOD otherwise became 92344
instead of 13106).

Positive power-of-two scalar division now expands in algebraic MIR to a
sign-derived bias, addition and arithmetic shift; remainder is reconstructed
as dividend minus quotient times divisor and removed when unused. No register
or encoding is chosen by this transformation. Both negative inputs and the
minimum signed value preserve truncation toward zero.

PDS nbody's divisions at 0x1a0 and 0x1d4 become these sequences. With only
this transformation disabled, the weighted cost is 1,094,861; enabled it is
1,012,861 (7.5% lower). This is the opportunity model, not measured hardware
cycles or a ratio against a hand-derived target. Stage dumps:
`/tmp/qbopt-nbody-powdiv`. Strict LIR runtime checks pass nbody (24 outputs),
chain (7), and divmod (20), each on p-g2, q-O and v-g3. Focused regressions
were observed failing with the transformation disabled.

Remaining: loop-invariant current-body position loads, array-address
induction, live accumulators, the still-opaque arithmetic call, and a
hand-derived nbody target. The overall modern-compiler goal is not complete.

## Nbody: final inner call recovered, loop optimization unblocked

The last inner runtime division (PDS 0x204) was retained because dead phi
cycles mentioned its unused clobber results. Raising now follows phi inputs
only from actually read results and removes the unused phis. This exposes
the arithmetic and permits LICM to hoist the current-body scaled index;
accumulator promotion can also operate across the formerly opaque call.

Three integration defects surfaced and were fixed, not bypassed:

- Division relocation matching now permits a different index SSA value when
  the relocated address itself is unchanged; different addresses still fail.
- Widening cannot move a pair above an intervening definition it consumes.
  PDS otherwise stored DELTAY before computing it (PX0=6137536, expected 1258).
- A chain ending in stores restores the last computed high-half definition,
  not the empty definition list of its last store. Restore inputs and outputs
  are declared to allocation even though the operation is opaque. Otherwise
  PDS read an uninitialized spill and printed PX0=17758202 instead of 1258.

Each defect has a regression observed failing without its fix. Strict LIR
runtime checks pass nbody (24), negnot (4), and chain (7) on p-g2, q-O and
v-g3. PDS nbody's modeled cost is now 886,143, versus 1,012,861 before this
integration (12.5% lower). This remains a model, not a hardware benchmark.
Next: hoist invariant position reads themselves, strengthen address induction,
and derive the missing nbody target. No completion claim follows from these
nine focused runtime checks.

## Shift chains versus extra induction counters

Nbody's `other*4` is already recognized as an affine recurrence. Broadening
strength reduction to replace two-shift chains with extra counters increased
modeled cost from 886,143 to 940,015; that experiment was removed. Register
pressure matters, so recognition alone is not a profitability argument.

Algebraic simplification instead combines same-width left shifts whose final
flags are unused and whose summed count is below the value width. PDS's
0x119 shift disappears and 0x11b shifts by two. Cost becomes 880,313, with
nbody and HARR passing strict LIR on all three compilers. Stage dumps are in
`/tmp/qbopt-nbody-shift-combine`. Invariant position reads remain folded into
their subtracts and are the next larger opportunity.

## Invariant reads need an index-range proof first

The current-body index is invariant, but the indexed POSX read at 0x12d
has no allocation metadata. Alias analysis consequently considers it able to
overlap DELTAX, DELTAY, DIST2, FALLOFF, both accumulators and frame temporaries.
The existing dynamic-array path proof does not cover these fixed near arrays.
Extracting arithmetic memory operands alone therefore does not hoist the
position reads. That experiment was removed; extracting comparison operands
also produced wrong programs and is not part of the implementation.

The next prerequisite is now implemented: `_last_counter` accepts a
single-latch pretested loop with internal branches, provided its sole exit
is in the header. It handles either branch orientation and still proves the
update cannot wrap. On optimized PDS nbody it proves the inner counter's last
executed value is 5; a side-exit mutation is rejected. The regression failed
under the old two-block restriction. Nbody and stride pass strict LIR on all
three compilers.

Next, propagate this counter range through index scaling and use the resulting
byte intervals in alias analysis. Do not infer extents from neighboring symbols
or declare different-looking indexed operands disjoint. Load extraction and
LICM come after that proof, with branch-entry and relocation ownership intact.

## Scoped interval analysis

`qbopt/ranges.py` propagates signed, non-wrapping intervals through copies,
adds, subtracts, multiplication and left shifts. Loop counters seed the
analysis only in the taken loop body, never in its header or outside it.
Independent enclosing-loop proofs can intersect at an inner block.

For optimized PDS nbody, both the current-body scaled index and the other-body
scaled index are proven 0..20 bytes at 0x117. The latter bound is absent at
the inner loop header and exit. A real-fixture regression was observed failing
without the analysis; focused cases reject signed overflow and masked shift
counts. This is analysis only: emitted code and runtime behavior are unchanged.
Next is consuming these intervals in alias analysis, retaining conservative
behavior for unknown segments, wrapping addresses and unproven values.

## Range-aware alias queries

Alias analysis now projects a proven near 16-bit indexed access to its covering
byte interval. Unknown segments, width mismatches and address wrap remain
conservative. LICM supplies block-scoped read-side facts to these queries.
The real nbody POSX/DELTAX regression fails when the projection is disabled;
all twelve focused interval tests pass with it enabled. Saved runtime outputs
for nbody and HARR match BC on PDS, QB and VBDOS in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-range-alias-2382vm4p`.

This establishes an alias proof, not an nbody speedup. A separate experiment
extracting position reads from word arithmetic did hoist them, but increased
PDS nbody modeled cost from 880,313 to 905,513 and broke HARR on PDS and QB.
That experiment is removed. The next step is whole-value scalar MIR before
LICM, so two position values do not become four independently allocated words
and inhibit widening. There is still no hand-derived nbody target establishing
its distance from the 1.5x goal.

## Whole values survive split/rejoin boundaries

Before moving pair recognition, the dumps exposed an independent break in
value continuity: recovered scalar multiplication results were extracted into
two words and concatenated again before their next arithmetic operation.
Algebraic simplification now replaces an exact high/low extraction round trip
with the original 32-bit value. Different sources, offsets and widths do not
qualify; the rule has no machine dependencies.

Nbody's product at 0x1cd now feeds the signed /512 reduction at 0x1d4 directly.
The real-fixture assertion fails with this rule disabled. PDS modeled cost
falls from 880,313 to 820,313 (6.8%); this is not hardware timing or a target
ratio. Dumps: `/tmp/qbopt-recombined`. Nbody (24 cases), CHAIN (7) and HARR
(1) pass strict LIR on each of PDS, QB and VBDOS; runtime artifacts are in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-recombined-we7utdqf`.
Position arithmetic is still word-paired before late widening; early whole
values and invariant-load motion remain the larger unfinished step.

## Early scalar position arithmetic

`raising_longs.scalar` now recognizes adjacent load, memory-arithmetic and
store pairs at the raise boundary. It produces fresh whole values and explicit
extractions for surviving half consumers. Arithmetic requires an already
recognized whole input; memory references must match in SSA base, segment and
metadata as well as adjacent byte addresses. Live half flags and full-width
readers of a half prevent recognition. Legacy widening remains for other
idioms; this is not a claim that the migration is finished.

Nbody now enters optimization with whole position loads and subtracts at
0x11d/0x12d and 0x13c/0x144, and a whole DELTAX store at 0x135. PDS modeled
cost falls from 820,313 to 792,437. Dumps: `/tmp/qbopt-early-longs`.
The real-fixture regression fails with recognition disabled; 67 focused tests
pass. Nbody, CHAIN, HARR, negnot and arridx pass strict LIR on all three
compilers across two bounded runs. Latest artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-long-guards-ypyxvyud`.
Invariant position memory operands are still inside the scalar subtracts;
exposing those as whole loads is the next step toward LICM.

## Whole invariant position loads leave the inner loop

Early scalar arithmetic now separates its memory operand into a whole LOAD
and a pure value operation. The LOAD retains relocation identity and byte
ownership; the arithmetic has neither a machine node nor a relocated operand.
LICM moves both current-body position loads to the 0xf0 preheader in round
three, after the scaled-index range becomes available. The subtracts remain
inside the inner loop and consume the hoisted whole values.

The integration regression fails with load separation disabled. The range
test now follows source identities rather than requiring optimized operations
to retain their original addresses; its interval and alias assertions remain.
PDS nbody modeled cost is 788,837, down from 792,437. Dumps are in
`/tmp/qbopt-scalar-licm`; nbody, HARR and CHAIN pass strict LIR for PDS, QB and
VBDOS in `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-scalar-licm-z36i1_nd`.
This closes the specific invariant-position-load opportunity, not the overall
nbody target. Register pressure, accumulator halves and remaining arithmetic
round trips still limit the emitted loop.

## Scalar arithmetic across runtime results and loop edges

The raise now recognizes immediate arithmetic pairs and accepts an existing
scalar's exact extractions as a whole input, not only newly recognized load
pairs. The extraction proof is shared with algebraic recombination. Nbody's
post-division +1 and accumulator additions now remain 32-bit operations;
promotion can keep the whole accumulators across loop edges. PDS modeled cost
falls from 788,837 to 607,037 (23.0%). This remains a model, not hardware timing
or proof of the missing nbody target. Dumps: `/tmp/qbopt-scalar-immediates`.

VBDOS exposed three defects, each with a fail-first regression:

- A promoted symbolic load kept its relocation after becoming a register
  copy. Symbolic fixups now require a surviving operand just like other ones.
- Emission indexed original bytes before checking whether an instruction was
  synthetic. Original interrupt bytes are inspected only for original nodes.
- Phi elimination hardcoded word copies on ordinary and split edges. Nbody
  printed PX0=285219921 instead of 1258. Copies now preserve the width required
  across the phi's connected values, including implicit operand contracts.

The raised-only VBDOS program passed before the phi fix, isolating the defect
downstream of recognition. After the fix, nbody (24 cases), HARR (1), CHAIN
(7), and negnot (4) pass strict LIR on PDS, QB and VBDOS. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-scalar-phi-final-6yt_a4th`.
All 75 focused tests pass. The overall target and full architecture migration
remain unfinished; this result specifically restores whole-value continuity.

## Whole stores through exact half copies

Scalar recovery now follows width-preserving word copies back to the exact
high/low extractions, refusing width changes and cycles. The raise records its
new extraction definitions and uses the shared proof for stores as well as
arithmetic. DELTAY and FALLOFF are stored whole instead of split solely for
the stores. The real-fixture assertion failed before the change.

PDS nbody modeled cost falls from 607,037 to 517,793 (14.7%). Stage dumps:
`/tmp/qbopt-whole-stores`. Nbody, HARR, CHAIN and negnot pass strict LIR on all
three compilers; artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-stores-8ej78xls`.
79 focused tests pass. Memory forwarding and the remaining legacy multiply
sites still limit value continuity; a hand-derived nbody target remains due.

## Multiply sites use the scalar call-recovery path

Classified B$MUI4 sites no longer enter the frozen machine-sequence path.
They retain their original pushes through initial raising, then use the same
scalar argument-capture and result extraction as computed multiplies. Nbody's
real-fixture regression fails when the old route is restored.

Routing alone increased cost because paired memory pushes became separate word
loads and a concatenation. Adjacent high/low memory pushes with matching SSA
addresses now capture one whole load, retaining the low operand's relocation.
Separated pushes keep independent snapshots. DELTAX's full subtraction result
now feeds its later multiply directly; the older regression checks this full
dependency rather than insisting that a removed high-half concatenation exist.

PDS nbody modeled cost falls from 517,793 to 402,593 (22.2%). Dumps:
`/tmp/qbopt-whole-multiply`. Nbody, HARR, CHAIN and negnot pass strict LIR on
PDS, QB and VBDOS; artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-multiply-z2y54a7y`.
This removes the remaining frozen multiply sites in nbody, not every legacy
runtime idiom in the project. Target derivation and broader migration remain.

## Unsigned dword constants and the next forwarding experiment

The encoder now converts a dword immediate's bit pattern to the signed i32
representation required by iced. CHAIN's folded 0xbffffff9 previously refused
with `mov is not one select.py can emit`; a fail-first regression checks the
exact `66 be f9 ff ff bf` encoding. PDS CHAIN passes with the fix.

The next experiment is still uncommitted: value forwarding extends SSA
lifetimes instead of requiring a provider to be live already, and uses
operation identity rather than source address. It exposes legacy division
sites that cannot consume SSA operands, so classified divides are provisionally
routed through scalar recovery too. Nbody passes and models 388,393, but this
is **not an accepted milestone**: DIVMOD refuses an inserted multiply crossing
a live condition (PDS 0x20a). Retaining the multiply's own flag definition did
not resolve it, indicating another live condition, and that attempted change
was removed. Inspect the flag SSA/exceptional edges before committing this
broader migration. Dumps: `/tmp/qbopt-forward-divmod`; runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-forward-fixes-i44yb65c`.

## Call flag inputs follow established contracts

DIVMOD's refusal came from undefined condition values already present after
raising: scalar recovery removed arithmetic-call flags, while PRINT retained
reads of them. Error-handler edges propagated those phantom inputs around the
body. Raising unconditionally added FLAGS even to an established empty input
contract. It now uses the declared inputs, including FLAGS when explicitly
listed; unknown inputs remain conservative. A fail-first regression checks
the PRINT symptom and both explicit and unknown flag-input boundaries.

With the pending forwarding/divide experiment present, DIVMOD (20 cases),
nbody (24), CHAIN (7), and HARR (1) pass strict LIR on all three compilers:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-contract-flags-pziqn7pl`.
Stage dumps: `/tmp/qbopt-contract-flags`. Focused runtime/raising checks pass
259 tests, with one old divide-relocation regression still tied to the legacy
representation. This commit isolates the contract fix; the broader migration
and that regression's replacement remain pending.

## Forward values beyond BC's statement lifetimes

Memory forwarding now adds SSA uses even when the provider was not already
live. Allocation owns the resulting lifetime, including preservation across
calls. Forwarding identifies operations rather than source addresses, so two
captures at one address cannot substitute each other's operands. Classified
divide helpers now use scalar argument recovery alongside multiplies, allowing
them to consume the forwarded values instead of requiring frozen operands.

Both new regressions failed with the old behavior restored. The legacy
divide-relocation guard still checks index-value renaming and rejects a changed
address, using the real argument capture and an explicit original-operand
snapshot. Fourteen focused checks pass. The strict three-compiler runtime run
recorded above covers this implementation: DIVMOD, nbody, CHAIN, and HARR all
pass. PDS nbody now models 388,342 cycles, versus 402,593 at the prior scalar
multiply milestone; this is not hardware timing or a target-completion claim.

## Pending whole constant-store recognition

Current experiment combines adjacent constant word stores in raising, with
exact address/SSA equality and contiguous coverage. Nbody's ACCX/ACCY zero
initializers then have the same whole width as their updates, enabling
promotion and loop phis. The initializer regression failed before the change;
11 focused recognition tests pass, including base/address/gap exclusions.

Not accepted yet: nbody's modeled cost rises from 388,342 to 418,616. The
allocation diff shows both accumulator phis spilled, with additional edge
copies rather than register-resident accumulators. Compare
`/tmp/qbopt-accumulator-current` and `/tmp/qbopt-accumulator-whole`.
Nbody and HARR pass all three compilers, but CHAIN and DIVMOD refuse immediate
stores on PDS/QB (CHAIN 0x48; DIVMOD 0x9a/0x9c). VBDOS passes all four.
Runtime artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-initializers-x6ij0ddp`.
Next: inspect the immediate-store encoding refusal, then eliminate the
promoted-phi spill/edge-copy overhead. The source experiment is uncommitted.

## Dword constant stores accept their full bit patterns

CHAIN's refused initializer was `mov [seg:5+0xe],0xc1747c23`, not an
unsupported machine form. The encoder passed that positive bit pattern to
iced's signed-i32 constructor. Loads and stores now share the conversion to
the equivalent signed value. The exact-byte store regression failed before
the fix; both immediate load/store tests pass afterwards.

With whole initializers still experimental, CHAIN (7 cases) and DIVMOD (20)
pass strict LIR on PDS, QB and VBDOS:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-store-immediates-eqtnx5t9`.
The promotion cost regression remains: global accumulator stores survive
alongside private spill-slot loads/stores and phi edge transfers. This encoder
fix is committed independently; initializer recognition remains uncommitted.

## Pending conditional store sinking

Nbody's update stores are in a conditional body, not its latch, and POSX/POSY
reads appeared to alias ACCX/ACCY without scoped index intervals. The experiment
passes those intervals to alias queries and proves the exit value by tracing
the header phi's latch input back through conditional paths to matching stores,
including the initialized entry path. The invariant-only fallback remains
restricted to latch stores; a nonempty loop does not imply a conditional store
executes. The real Nbody sinking regression failed before this change.

Both accumulator writes now move to exit 0x227. Modeled cost improves from
418,616 to 409,016, still worse than committed 388,342 because private phi-spill
traffic remains. Dumps: `/tmp/qbopt-conditional-sink`. Nbody, HARR, LNGMXX and
NESTED pass strict LIR on all three compilers in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-conditional-sink-y6v0bvte`.
26 focused tests pass, but LNGMXX's nonempty invariant-temporary sinking check
still fails: its remaining whole temporary's provider is not classified
invariant. Do not weaken that check. Source changes remain uncommitted pending
that investigation, conditional-path boundary checks, and allocator improvement.

## Scalar divisor constants reach LICM

LNGMXX's remaining temporary came from a scalar divide by a value known to
be 7. Constant propagation excluded DIVMOD operands, so LICM could not prove
the operation nonfaulting. Divisor propagation now preserves operand order
and requires a width-complete fact. Lowering materializes a constant divisor
as an abstract temporary before the divide, leaving allocation to place it.

Three fail-first checks cover 7, zero, and -1, including insufficient-width
facts; the latter two divisors remain unsafe to speculate. All 16 loop-motion
checks pass, including the previously failing invariant-store check. The
dump in `/tmp/qbopt-divisor-constants` places the divide at preheader 0x4c.
LNGMXX, DIVMOD, and nbody pass strict LIR on all three compilers in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-divisor-constants-dbsfgqw2`.
This fix is separate from the uncommitted initializer/conditional-sinking
experiment, whose nbody cost remains 409,016 pending allocator work.

## Allocator evidence for the remaining promotion regression

Processing coalescing blocks by descending loop depth changed no modeled
cost (409,016), so that experiment was removed. Instrumenting the first
allocation, rather than guessing from final spills, identifies accumulator
intervals 556/557 with weights 14.04/11.42 and sizes 185/239 slots. Neither
has a fixed register or a clobber-mask conflict in any of the six registers.
They lose to shorter-lived intervals: ESI's sole overlapping assigned interval
501 weighs 46.67; EDI's 509 weighs 58.82. The allocation dump shows EDI
holding the constant 512 before the scalar divide.

The spiller already recognizes and rematerializes single-definition constants,
but allocation weights only count weighted references divided by interval
length, with no rematerialization discount. Next investigate that cost mismatch
and the competing ESI interval before changing allocation policy. Do not infer
that discounting constants alone will resolve the two accumulator spills.

## Whole accumulators and direct fixed-input rematerialization

LLVM's `CalcSpillWeights.cpp` halves rematerializable intervals' weights.
Trying that exact discount here changed no nbody cost, so it was removed.
The useful change is structural: constraint preparation now materializes a
proven constant directly into its required temporary instead of copying from
a separate live value. It uses the spiller's existing single-definition,
width-complete, nonrelocated constant proof. Its regression failed when the
old copy behavior was restored.

Together with whole constant-store recognition and conditional store sinking,
PDS nbody models 379,016, below the committed 388,342 baseline and the
409,016 intermediate. Dumps in `/tmp/qbopt-fixed-remat` show immediate 512
loads directly into EAX rather than preserving a separate constant in EDI.
Redundant preparations remain visible and are a later cleanup opportunity,
not hidden in the result. This is modeled cost, not hardware timing.

All 38 focused constraint, loop-motion, and whole-recognition checks pass.
Conditional sinking is refused without the matching initialization or without
either available alias proof. Nbody (24), HARR (1), CHAIN (7), DIVMOD (20),
LNGMXX (1), and NESTED (1) pass strict LIR on all three compilers:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fixed-remat-nkr3jy7h`.
The initializer/sinking experiment is now accepted with this allocation fix.
Nbody still needs its hand-derived target; the project-wide goal is not met.

## HARR descriptor addressing: symbolic analysis is not an emitted operand

The side-by-side assembly exposed `add di,[0]` at PDS output 0x66 with
no relocation. Induction composition converted a descriptor-relative read
to its alias-analysis symbolic address and handed that new operand to
strength reduction. The new instruction had no original fixup to carry.
HARR's printed sum did not detect this: forwarding already serves the sum
from the stored value, independently of where the array store lands.

Composition now retains the actual reference and requires its base to be
invariant. Generated operations include memory-address SSA uses; lowering
accepts literal displacements through those abstract bases. The read becomes
`add di,[bx+0Ah]`, preserving the descriptor pointer instead of reading DS:0.
The three emitted-code regressions failed before the fix. All 47 focused
induction tests pass; HARR and NESTED pass runtime on all three compilers.
Stage dumps: `/tmp/qbopt-harr-es` and `/tmp/qbopt-harr-address-fixed`.
Runtime artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-harr-address-y0z6zjgj`.
HARR costs remain 2920/2936/3132 for PDS/QB/VBDOS. The opaque ES load still
needs a machine-independent address representation; this fix does not hoist it.

The same focused runtime run found MATRIX prints T=190 instead of T=380 on
all three compilers. Replacing the changed functions in memory with HEAD's
pre-fix definitions reproduces the PDS failure independently, in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-matrix-baseline-xgc4dvts`.
Its earlier 1.31x score is therefore not evidence of goal completion. Fix
this existing correctness failure before further optimization.

## MATRIX recurrence multiplier: retain the load and its relocation

The next dump showed MATRIX's second loop initializing and advancing its
recurrence with unrelocated reads at DS:0. Unlike HARR's based descriptor
read, this multiplier was a direct relocated memory operand. Strength
reduction copied the Cell into newly invented arithmetic without moving the
original operation's relocation to either copy. With a zero stride the
diagonal became the first row, producing 190 instead of 380.

Strength reduction now materializes the invariant multiplier as one LOAD
in the preheader, retaining the original operand identity there, and uses
its SSA value for initialization and stride. Lowering explicitly maps an
invented LOAD to MOVE, independently of the originating multiply's machine
operation. The loop now advances with register arithmetic, not a repeated
memory read. Reductions whose stride cannot be constructed are rejected
before inserting any load.

All three new emitted-address regressions failed before the fix; all 50
focused induction tests pass afterwards. MATRIX prints 380 on PDS, QB and
VBDOS; HARR and NESTED also pass on all three. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-matrix-stride-adva5wll`.
Pass dumps: `/tmp/qbopt-matrix-failure` and `/tmp/qbopt-matrix-stride-fixed`.
MATRIX now models 8056/8060/8066 against 6210, about 1.30x on all three.
This restores this benchmark's correctness evidence, not project completion.

## Segment-value representation experiment

After the HARR and MATRIX address fixes, an emitted-code scan of all
available primary target fixtures (PDS /G2, QB /O, VBDOS /G3) found no
remaining unrelocated zero-word displacements. This checks that specific
failure shape only, not correctness of all relocations or runtime outputs.

A raising experiment separated `es := [descriptor+2]` into an ordinary
word LOAD and an opaque resource installation. CSE and LICM did hoist the
ordinary load out of both HARR loops, as intended. The emitted result also
kept the selector in AX for the entire nest, spilled the row counter and
descriptor pointer, and installed ES twice in the inner loop. Modeled PDS
cost rose from 2920 to 3072. Dumps remain in `/tmp/qbopt-harr-selector`.
The experiment was removed; no runtime correctness claim is made for it.

This is evidence against treating selector extraction alone as the completed
address migration. The next design must represent a far address's object
identity and offset in MIR and make the selector a lowering/allocation
choice, including liveness across clobbers. An opaque installation cannot
be optimized by ordinary value passes, and a permanently live GPR selector
is not a substitute for retaining the pointer in the appropriate machine
resources. Neither an ES-specific MIR hoist nor merely hiding ES's name
behind a new operation meets the boundary rule.

## Post-allocation constant-load peephole

The allocator's flexible classes still cover GPRs and addressing registers,
not address-space resources. A segment-value migration needs that capability
as well as abstract far-address operands; it is not complete.

The previously absent final peephole phase now exists after allocation,
parallel-copy expansion and frame insertion. Its first rule removes repeated
equal nonrelocated immediate MOVs into the same physical register at the
same width. Knowledge is local to a block and resets at any non-MOV or
unknown instruction. Writes invalidate all overlapping register aliases,
including AH versus EAX; memory reads are never removed. Dropped instructions
transfer their byte coverage through the existing LIR removal mechanism.
This is post-allocation encoding cleanup, not a new LIR optimization tier.

Nbody's emitted immediate-512 count drops from three to two; its regression
failed with three before the phase was connected. PDS cost drops from
379016 to 377016. All 10 focused peephole/prologue checks pass, including
partial writes, call/unknown barriers, clobbers, relocation and block edges.
Nbody's 24 cases, MATRIX and HARR pass strict LIR runtime on all three
compilers in `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-peephole-gc_sg3g2`.
Dumps: `/tmp/qbopt-nbody-peephole`, including the new final machine phase.

## Fixed-resource allocation and coalescing

A direct allocator probe corrects the earlier blanket statement that
non-GPR resources cannot be allocated: an explicitly pinned virtual value
already allocates to ES and the interference masks already honor ES
clobbers. Flexible classes remain GPR/addressing-only. The missing piece
found here was the coalescer's candidate domain: even two values pinned to
ES were intersected against the GPR set and could never coalesce.

Pinned non-GPR values without an incompatible addressing constraint now
have their explicit singleton domain. The Briggs test counts only neighbours
whose domains overlap the prospective merged class. A fail-first regression
proves an equal ES-bound copy coalesces while six GPR values are simultaneously
live, and allocation needs no spill. Additional checks retain different ES/FS
pins and reject keeping an ES value live through an ES clobber.

All 20 coalescer/peephole checks pass. Nbody (24 cases), MATRIX and HARR pass
strict LIR runtime on all three compilers in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-resource-coalesce-jwvqpoim`.
This establishes a backend prerequisite, not HARR's far-address migration:
MIR still needs to express the address-space value and each access's
dependency on it, with resource constraints supplied only by lowering.

## Target refresh and floating-point semantic audit

The target scoreboard at f8c9e96 puts PDS FPCSEX at 4508/1340 (3.36x),
FPCSE at 4386/1340 (3.27x), SPILL at 3206/1122 (2.86x), PRESSX at
710/308 (2.31x), and HARR at 2920/1834 (1.59x). These are modeled costs.
Numerous event-enabled PDS/VBDOS fixtures remain unmeasured because LIR
emission refuses them; direct checks of hotlop-p-evt and harr-p-evt report
an unestablished call interface, not an optimization success.

FPCSEX dumps in `/tmp/qbopt-fpcsex-current` show the repeated loads and
arithmetic still naming st0 rather than floating SSA identities. Its target
listing also reassociates the accumulator and bypasses SINGLE rounding.
An exact integer simulation of a 64-bit significand gives different results
for `(2^65 + -2^65) + 1` and `(-2^65 + 1) + 2^65` (1 versus 0).
The target document now flags that defect without changing the denominator
to make the score pass. Next FP work needs explicit value types and rounding
semantics before CSE/LICM, followed by a valid hand-derived target. Nbody's
target derivation and event-interface coverage remain separate open work.

### HARR: one address-space value across the loops

`raising_addresses` now names a near-memory ES selector load as an SSA
value and attaches that value to the following far-memory references.
Unknown clobbers end this local binding; block exits retain its observable
state. Lowering, not a MIR pass, constrains the live selector definitions
to ES. This is a bounded step, not a completed migration of all address
spaces or a cross-block resource-SSA construction.

Two integration defects surfaced in adjacent stage dumps. Complete narrow
copies were retained as though they preserved a high half, so equivalent
selectors prevented store-to-load forwarding. CSE now distinguishes a whole
copy from a partial write with `merges`. Lowering also collected deleted
origin entries as pins; a reused numeric ID pinned HARR's accumulator to ES.
Only current definitions now contribute selector pins.

HARR's primary PDS/QB/VBDOS modeled costs are **2326/2342/2538**, down from
2920/2936/3132: **1.27x/1.28x/1.38x** against the unchanged 1834 target.
The raw emitted loop contains a far store, accumulator add, and striding
induction variables, with no selector reload or array read. Stage dumps:
`/tmp/qbopt-harr-selector-live-pins`. These are modeled costs, not timings.

The regression verifies that the selector load lies outside every backward
branch interval and that the far read is eliminated on all three compilers;
disabling the raise makes all three cases fail. A clobber test and a
fail-first narrow-copy test cover the boundaries, including preserving a
genuine high-half merge. Focused tests: 27 passed. Seven runtime programs
(HARR, MATRIX, NBODY, LNGMIX, NESTED, PRESS, HOTLOP) pass on all three
compilers through strict LIR emission, including all 24 NBODY cases per
compiler. Runtime artifacts: `qbopt-selector-copies-n7sza3j1` under the
system temporary directory. The full transform test file still has nine
failures, reproduced with its pre-change CSE function; it is not a green
suite. HARR reaching its primary target does not complete the project goal.

### NESTED: combine integer address scales

The next raw loop inspection found `(row * 6) * 2` surviving as a multiply
and shift, with an extra temporary. Algebraic simplification now combines
single-use MUL/SHL scales at the same modular integer width. It does not
reassociate floating arithmetic, narrow a result, discard live flags or
high-half merges, or duplicate a shared producer. Lowering still chooses
the instruction. NESTED now emits `imul ...,12` instead of the multiply by
six followed by a shift; the previous spill slot disappears as well.

Primary modeled costs are **1202/1206/1212** (PDS/QB/VBDOS), against 768:
**1.57x/1.57x/1.58x**, still above goal. PDS was 1292 before this change.
Before/after stage directories are `/tmp/qbopt-nested-next` and
`/tmp/qbopt-nested-scales`. All three real-object regression cases failed
before the rewrite and passed after it. Algebraic and address-space tests:
67 passed, including modular overflow and refusal boundaries. Strict LIR
runtime runs of NESTED, MATRIX, HARR and all 24 NBODY cases passed on each
compiler; artifacts are `qbopt-scale-chain-brgqsuv7` in system temporary
storage. The remaining outer-loop multiplications are still visible; the
existing outer strength-reduction guard is pressure-related and has not
been removed on the strength of this result.

### Outer recurrences: remeasure the pressure guard

A fresh probe of the existing outer-loop ban contradicted its historical
rationale. With today's algebraic simplification and allocator, enabling
outer recurrences improves all three affected programs. NESTED spills only
outside its inner loop; its row multiplications disappear. The guard is
removed, leaving allocation responsible for splitting and spilling rather
than making every outer recurrence ineligible in MIR.

Primary modeled costs (PDS/QB/VBDOS):

| Program | Before | After | After / target |
| --- | --- | --- | --- |
| NESTED | 1202 / 1206 / 1212 | 1128 / 1132 / 1138 | 1.47 / 1.47 / 1.48 |
| MATRIX | 7996 / 8000 / 8006 | 7656 / 7660 / 7666 | 1.23 / 1.23 / 1.23 |
| HARR | 2326 / 2342 / 2508 | 2246 / 2262 / 2256 | 1.22 / 1.23 / 1.23 |

The preceding scale-chain change also improved MATRIX and VBDOS HARR;
the before column above is freshly measured, not copied from older rows.
The 63 available primary target fixtures show no other modeled change and
no new unmeasured result (`/tmp/qbopt-outer-target-audit.txt`). This is not a
correctness claim over those 63 fixtures. Strict LIR runtime validation is
NESTED, MATRIX, HARR and all 24 NBODY cases on each compiler, all passing;
artifacts: `qbopt-outer-recurrences-sh7l7689` under system temporary storage.
Induction, algebraic and address-space tests: 120 passed. The new emitted
NESTED regression failed on all three compilers with the guard present.

Dumps: `/tmp/qbopt-nested-outer-probe`,
`/tmp/qbopt-matrix-outer-recurrences`, `/tmp/qbopt-harr-outer-recurrences`.
The descriptor-hoist regression now requires its read to dominate the inner
preheader, allowing it to move farther out without weakening its memory
dependency checks. Product-width testing disables strength reduction to
inspect the multiply before recurrence formation eliminates it.

### PRESSX reference audit and completion-report correctness

PRESSX's emitted loop already has only add/inc/cmp/jle. Its apparent 2.31x
gap comes from comparing a runtime-input program to PRESS's folded-constant
reference. Instruction-by-instruction modeled decomposition is input 370,
computation 226, output 114, total 710. `docs/targets.md` records the audit
without inventing a more flattering denominator. A complete hand-derived
PRESSX reference remains required; this does not declare PRESSX finished.

The scoreboard now reports known invalid references (PRESSX, FPCSE, FPCSEX)
as PROVISIONAL and returns failure regardless of their numerical ratio.
Missing targets, including NBODY, also return failure with NO TARGET.
Previously an arbitrarily cheap result or absent target could return a
successful completion report. Four fail-first cases reproduce that defect;
all 13 scoreboard tests now pass. Valid targets still show their measured
ratio; HARR remains 2246/1834 in the focused CLI check. No emitted code or
target denominator was changed in this step.

### SPILL: preserve and consume partial memory constants

SPILL initialized neighboring h3/o1 with one dword store. Updating o1 then
discarded the whole fact, including the unchanged h3=7 bytes. Memory facts
are now canonical bytes, so partial stores invalidate only potentially
overlapping bytes and equivalent initializer widths agree at CFG joins.
Unknown writes remain conservative. Reads still require every byte known.

Operand folding now uses these width-proven memory facts as well as SSA
constants. It removes the replaced memory dependency and regenerates the
instruction from its MIR operation; ordered operands are not reversed.
The real SPILL fixtures now emit `add ...,7` instead of the invariant load
on each inner iteration. PDS/QB/VBDOS modeled costs are 2806/2812/2816
(about 2.50x of 1122), with PDS down from 3206. The accumulator remains in
memory; this is not completion of SPILL's optimization work.

Fail-first evidence covers the partial-write fact and all three emitted
fixtures. Constant propagation tests passed (1036 cases before operand
integration); the focused integration selection passed 91 cases. Six
runtime programs (SPILL's two checks, NESTED, MATRIX, HARR, NBODY's 24
checks, LNGMIX) passed through strict LIR on all three compilers. Artifacts:
`qbopt-memory-constants-o7e3tm7t` in system temporary storage. Stage dumps:
`/tmp/qbopt-spill-next`, `/tmp/qbopt-spill-byte-facts`,
`/tmp/qbopt-spill-constant-operands`.

### SPILL: promote fields initialized by a packed store

Promotion now chooses the width of each cell's reads and can capture a
fully covered field of a wider constant initializer. The original memory
store remains intact, including neighboring bytes. Exact-width stores still
capture their complete value, including when a narrower field is captured
beside them; unknown overlapping writes invalidate the narrow field's
availability. Unsupported partial updates remain in memory.

`consts.initialized` provides the same contained-value proof to promotion
and loop store sinking. The latter can therefore establish the accumulator's
entry value even when its zero was part of a wider initializer, and move
the write-back out of both loops. No machine register is selected by this
work. SPILL's inner loop now contains two immediate adds, increment,
compare and branch, with no memory reads or writes. Final global stores
remain before output calls.

SPILL's PDS/QB/VBDOS modeled costs fall from 2806/2812/2816 to
**1410/1416/1420**, or **1.26x/1.26x/1.27x** against 1122. All three primary
variants now meet the target. Stage directories:
`/tmp/qbopt-spill-promoted` (before the store-sinking proof) and
`/tmp/qbopt-spill-promoted-sunk` (after it).

The three real-fixture loop-memory regressions failed before the change.
Promotion, constant-cell and induction tests: 83 passed, including a shared
wide/narrow capture and an unknown overlapping write. The older HOTLOP
initializer-preservation test was corrected to find its packed initializer
and still requires that exact original store to remain after promotion.
Strict LIR runtime checks passed on all three compilers for SPILL, HOTLOP,
NESTED, MATRIX, HARR, NBODY and LNGMIX; artifacts are
`qbopt-packed-promotion-762zykcv` in system temporary storage. This verifies
those programs, not the full corpus or project goal.

### Split initialization and narrower output reads

ADDRM's long accumulator was excluded from promotion because output pushes
read its two words separately. Supported reads now determine the candidate
width; unsupported output reads remain in memory, backed by intact stores.
Promotion uses the existing byte-level constant memory analysis to capture
a full value after split initializers, only when every byte is established.
Calls and barriers conservatively invalidate these initializer facts.

ADDRM modeled costs: PDS 1578 -> 1460, QB 1584 -> 1466, VBDOS unchanged
at 1346. The accumulator reload is gone, but its loop write-back and the
long array store/reload remain opportunities; the 754 target is not met.
Stage evidence: `/tmp/qbopt-addrm-next` and `/tmp/qbopt-addrm-captured`.

The real PDS/QB regressions and complete split-initializer case failed with
the previous implementation. All 20 promotion tests pass, including missing
bytes, unknown overlapping writes and preservation of output memory reads.
Strict LIR runtime checks passed for ADDRM, SPILL, HARR, MATRIX, NESTED,
NBODY, HOTLOP and LNGMIX on all three compilers (33 cases each).
Artifacts: `qbopt-split-initializers-u9m6172r` in system temporary storage.

### Sink the split-initialized accumulator write-back

The loop exit proof now consults complete byte-level memory facts when a
single initializer cannot establish the expected entry value. This preserves
the zero-trip requirement: missing initialization or a clobber cannot justify
an exit store. The existing value/phi proof still establishes the backedge.
Facts are computed lazily only when the single-store proof is insufficient.

ADDRM PDS/QB costs fall again, 1460/1466 -> **1346/1352**; VBDOS remains
1346. The accumulator write-back is now after the loop. The target remains
754, so roughly 1.79x is still unfinished. The remaining long-array reload
is visible directly in the emitted loop, after its two word stores.
Before/after MIR dumps: `/tmp/qbopt-addrm-captured` and `/tmp/qbopt-addrm-sunk`.

47 promotion/store-motion tests pass. The new PDS/QB complete-initialization
cases fail before this change; missing and clobbered initializers retain
the loop store across all three compilers. Strict LIR runtime checks pass
for ADDRM, SPILL, NESTED, MATRIX, HARR, HOTLOP and LNGMIX (27 cases total).
Artifacts: `qbopt-split-exit-7s7gobdb` in system temporary storage.

### Raise signed word-pair stores as whole values

ADDRM stored an integer and its CWD-produced sign word into adjacent array
words, then loaded the same long. Raising now recognizes that complete
store as a machine-independent `sign_extend` value and a single long store.
Existing forwarding removes the reload. Recognition requires the matching
source, word widths and CWD definition, plus the existing identical-address
and adjacency proof for paired stores.

Lowering selects MOVSX; selection emits its explicit operands. Only CWD/CDQ
require AX/DX: applying that old blanket EXTEND constraint to MOVSX added two
unnecessary moves, so the target constraint now distinguishes those forms.
The resulting PDS/QB/VBDOS ADDRM costs are **1206/1212/1206**, down from
1346/1352/1346, still about 1.60x against 754. Address scales remain inside
the loop. Stage diff: `/tmp/qbopt-addrm-sunk` -> `/tmp/qbopt-addrm-whole-store`.

All three real-fixture whole-store regressions fail before the change;
the MOVSX encoding regression also fails before selection support. The
combined focused run reports 1118 passes and 12 existing selection failures;
all 12 also fail with the previous raising/lowering/selection/target code
loaded in isolation (`/tmp/qbopt-signed-stores-select-baseline.txt`). This is
not a claim of a green selection suite. Strict LIR runtime checks passed
99 cases across ADDRM, SPILL, NESTED, MATRIX, HARR, HOTLOP, LNGMIX and NBODY
on all three compilers. Artifacts: `qbopt-signed-stores-g026xtr_`.

### Address-shift recurrence probe: not shipped

Allowing plain SHL address recurrences would put ADDRM at 1096/1102/1096,
within 1.5x, and improve ARRIDX from 590/594/600 to 538/542/548. However,
STRIDE regresses by 25 modeled cycles and NBODY PDS regresses from 377016
to 410356. Runtime output still passes: correctness alone misses this loss.

NBODY's existing whole-position-load regression catches the important
structural difference: a position load stays in block 0x117 instead of
hoisting to 0xf0. Its inner loop also gains a second affine counter with
step 4 alongside the step-1 counter; existing range/alias proofs must remain
useful after this transformation. Unit-step and constant-start eligibility
gates did not prevent the NBODY regression. Do not repeat those probes or
ship a fixture-specific exception. Preserve the induction/range/alias
relationships before enabling these cheaper recurrences.

All experimental source/test changes were removed. The authoritative ADDRM
cost remains 1206/1212/1206, not the attractive probe number. Probe dumps:
`/tmp/qbopt-addrm-strides`, `/tmp/qbopt-nbody-shift-probe`; runtime artifacts
`qbopt-address-recurrences-adusqjrb` (96 passing cases across three compilers).

### Share the exit counter's proven iteration count

`ranges.bounded` previously bounded only the recurrence explicitly compared
at the loop exit. An independently advanced offset has no such comparison,
so replacing a scaled index with that recurrence discarded its interval.
The analysis now derives the number of advances from a proven canonical
exit counter and applies it to the other affine header recurrences.
Each start and step must be known; all taken values and the final latch
update must fit without wrapping. Bounds remain scoped to the taken body.

With the experimental shift policy injected only for diagnosis, this
restores NBODY's 0..20 offset intervals and position-load hoisting, reducing
the probe cost from 410356 to 401956. That is still worse than 377016,
so recurrence generation remains unchanged. The remaining probe regression
needs allocation/profitability analysis, not more alias speculation.
Production NBODY remains 377016 and ADDRM 1206 on PDS.
Stage diff: `/tmp/qbopt-nbody-shift-probe` -> `/tmp/qbopt-nbody-shared-ranges`.

73 focused range/induction tests pass. The injected-recurrence real-program
range regression fails with the previous analysis. Wraparound, final-latch
overflow, decreasing and constant recurrences have focused coverage.
Strict LIR runtime checks pass 93 cases across ADDRM, ARRIDX, STRIDE,
MATRIX, NESTED, HARR and NBODY on three compilers. Artifacts:
`qbopt-shared-trip-ranges-p3em9ms0`. The preceding probe's runtime total was
also 93, not the 96 recorded above.

### Pressure audit and copied-constant rematerialization

Final MIR peak live values in the shift-recurrence probe grow from 5 to 6
in ADDRM and from 12 to 15 in NBODY's interaction nest. The range proof is
repaired; additional loop-carried values still make allocation more costly.
Discounting literal spill weights did not change either NBODY result.

The spiller now follows full-width, uniquely defined copy chains when
proving a literal can be rematerialized. Previously a copy of a known
constant acquired a frame slot and reload even though the direct constant
did not. Redefinitions, grouped operands, symbolic addresses, width changes
and unseeded copy cycles remain excluded. Use widths are collected once,
not by rescanning the body per candidate.

23 focused spiller tests pass; the copied-constant no-frame-memory test
fails with the previous spiller. All 97 compared primary fixture objects,
including NBODY's regression fixture, are byte-identical before and after.
This is a backend capability improvement, not a benchmark improvement.
The NBODY pressure regression and ADDRM's 1.60x gap remain open; do not
claim copied-constant rematerialization resolved them.

### QB long arithmetic: setup instructions were not pushes

The QB LNGMIX/LNGMXX outlier was real retained runtime work. For classified
call sites, raising reconstructed the argument list from every instruction
between setup and call, including MOV/CWD. Push grouping then rejected it,
leaving both runtime helpers in the loop. The reconstructed list now contains
only actual pushes; setup computations remain separate MIR operations.
Classified literal arguments retain their proven whole constant at capture,
instead of becoming a concatenation of opaque sign-word computations.

QB LNGMIX cost falls **10592 -> 306**, within 1.5x of its 210 target.
QB LNGMXX falls **10634 -> 373**, still above target. PDS/VBDOS are unchanged:
LNGMIX 302, LNGMXX 371. LNGMIX folds both arithmetic operations; LNGMXX
retains one shared DIVMOD. These are model costs, not hardware speedups.
Raw/stage evidence: `/tmp/qbopt-lngmix-q-gap`, `/tmp/qbopt-lngmix-q-pushes`,
`/tmp/qbopt-lngmix-q-constants`.

18 focused raising tests pass. Both new QB real-fixture regressions fail
with the previous raiser; PDS/VBDOS cases already passed. Strict LIR runtime
checks pass 147 cases across LNGMIX, LNGMXX, DIVMOD, ADDRM, NBODY and HOTLOP
on all three compilers. Artifacts: `qbopt-runtime-arguments-cmdgbxab`.

### Choose the dying operand during two-address legalization

LNGMXX's accumulator addition had the invariant first, so two-address
legalization copied the invariant into a temporary, added the accumulator,
then copied the answer back at the phi. For commutative integer ADD/AND/OR/XOR,
legalization now prefers an already-tied second operand or a dying second
operand when the first remains live. Equal widths and explicit value operands
are required; grouped and fixed-interface instructions retain their order.
This is instruction legalization before allocation, not a new LIR pass tier.

PDS/QB/VBDOS LNGMXX costs fall 371/373/371 -> **331/333/331**. HOTLPX falls
532/536/542 -> **452/456/462**, meeting its 312 target in all three variants
(1.45x/1.46x/1.48x). The 63-target-fixture audit has only these six changes,
all improvements. NBODY PDS improves slightly, 377016 -> 376416, but the
shift-recurrence probe still regresses (401156), so its policy stays disabled.
LNGMXX remains above target. Stage evidence: `/tmp/qbopt-lngmxx-next` and
`/tmp/qbopt-lngmxx-commuted`.

12 focused legalization tests pass, including three real emitted-loop tests
that fail with the previous legalization. The wider LIR tests have 37 existing
failures, reproduced with the previous two-address code in
`/tmp/qbopt-twoaddr-lir-baseline.txt`; they are not presented as green.
Strict LIR runtime checks pass for LNGMIX, LNGMXX, DIVMOD, ADDRM, NBODY,
HOTLOP, SPILL, MATRIX, NESTED and HARR (162 cases), plus HOTLPX separately.
Artifacts: `qbopt-commuted-operands-hozw1s5m`, `qbopt-commuted-hotlpx-32srgv2g`.

### Target audit correction: HOTLPX and LNGMXX remain unverified

Both runtime-input twins inherited the original program's whole-program
denominator, without their own full reference listing. HOTLPX cannot use
HOTLOP's folded product 21, and LNGMXX cannot fold its runtime dividend to
100000. Input setup and arithmetic must be derived, not copied or added
to the denominator until the output happens to pass. Closed-form loop
evaluation is also a valid optimization the reference must consider.

Their legacy 312/210 values remain unchanged but are now PROVISIONAL.
The earlier statement that HOTLPX meets its target is withdrawn. Its real
80-cycle improvement remains; no optimization was removed. The two new
scoreboard cases fail before this change because a low cost incorrectly
certifies these unestablished references. `docs/targets.md` records the
source differences and requirements for replacement reference listings.

### Whole-width recurrence starts

Induction analysis used a nonexistent `Value.wide` attribute and therefore
reported every recurrence start as 16-bit. LNGMXX's 32-bit accumulator had
a word start and a long step; a full-width copy on the backedge could hide
the recurrence altogether. The analysis now reads the backedge definition's
explicit MIR result width, requires agreement across backedges, and carries
that width into the initial value. It uses no machine origin information.

Five new cases fail against the previous analysis: direct/copied long
recurrences and the real LNGMXX accumulator on all three compilers.
The focused induction/range checks pass 78/78. All 96 primary fixture
objects emit identical bytes versus the previous implementation. This is
analysis groundwork for evaluating loop exit values, not a claimed speedup.

### Evaluate affine loop exits instead of running the loop

`loopexit.evaluated`, within the MIR strength stage, now replaces a finite,
side-effect-free two-block loop with each header recurrence's final value:
`start + count * step`. This follows the exit-value evaluation idea used by
LLVM IndVarSimplify. The control recurrence must have a proven finite,
non-wrapping count; integer accumulators retain their own modular width.
Stores, unmodelled operations, escaping latch/flag values, incomplete
recurrence descriptions, and unproved termination prevent deletion.
Zero-trip loops are currently left to other simplification, not guessed.

The real emitted-backedge assertions failed first on all three LNGMXX
fixtures. Adjacent MIR dumps at `/tmp/qbopt-lngmxx-exit-before` and
`/tmp/qbopt-lngmxx-exit-fixed` show the recurrence becoming a multiply and
the loop disappearing. An intermediate dump caught retained original bytes
on removed operations; cleared operations now explicitly own no computation,
and the rewritten jump retains the provenance needed to lower its new target.

Modeled costs (PDS/QB/VBDOS):

| Program | Before | After |
| --- | --- | --- |
| LNGMXX | 331 / 333 / 331 | 269 / 271 / 269 |
| PRESS | 230 / 234 / 240 | 146 / 150 / 156 |
| PRESSX | 710 / 714 / 720 | 646 / 650 / 656 |

These nine objects are the only changes among 96 primary fixture objects;
all retain LIR emission. LNGMXX and PRESSX references remain provisional.
Focused loop-exit/induction/range checks pass 88/88. The earlier LNGMXX
width regression now disables exit evaluation so it still examines the
recurrence itself; its width assertion is unchanged. Strict runtime checks
pass 39 cases across all three compilers, covering the three changed programs
and LNGMIX, HOTLOP, HOTLPX, HARR, MATRIX, NESTED, ADDRM and SPILL.
Artifacts: `qbopt-loop-exit-34rk4ekd` and `qbopt-loop-exit-press-_bngjbd0`
under the system temporary directory. No full test suite was run.

### Sum affine increments in closed form

Loop-exit evaluation now handles an accumulator whose increment is a linear
expression of invariant values and basic recurrences. It expands same-width
COPY/ADD/SUB/INCREMENT/DECREMENT expressions, requires the accumulator's own
coefficient to be exactly one, and sums each changing term using the exact
integer coefficient `N*(N-1)/2` before reducing modulo the value width.
Repeated expression nodes are memoized. Nonlinear recurrences and mixed-width
expressions are not guessed. No machine origin or register is consulted.

HOTLPX becomes `20*(n*k)+210`, and constant HOTLOP folds to its answer.
Six real emitted-loop regressions failed first (both programs, all three
compilers). Additional cases cover descending counters, an overflowing
triangular sum, and refusal of a doubled accumulator. Stage dumps are in
`/tmp/qbopt-hotlpx-sum-before` and `/tmp/qbopt-hotlpx-sum-after`.

| Program | Before (PDS/QB/VBDOS) | After |
| --- | --- | --- |
| HOTLOP | 332 / 336 / 342 | 128 / 132 / 138 |
| HOTLPX | 452 / 456 / 462 | 272 / 276 / 282 |
| ROTATE | 452 / 456 / 462 | 296 / 300 / 306 |
| SPILL | 1410 / 1416 / 1420 | 470 / 476 / 480 |
| SPLIT | 352 / 356 / 362 | 152 / 156 / 162 |

These 15 are the only changed objects among 96 primary fixtures, all retaining
LIR emission. Focused loop/induction/range checks pass 98/98; strict runtime
checks pass 27 cases across the three compilers for these five programs plus
LNGMXX, PRESS and PRESSX. Runtime artifacts are `qbopt-triangular-sums-gxxko6e7`
and `qbopt-triangular-others-3q1oerpo` under the system temporary directory.
These are actual numerator improvements, not reference revisions; HOTLPX's
target remains provisional, and completion still requires valid modern-compiler
reference listings (including loop evaluation where legal).

### Coalesce with the neighbour's register palette

MATRIX's diagonal loop retained `mov temporary,pointer; add temporary,42;
mov pointer,temporary`. The coalescer rejected joining them because it
counted general-register neighbours as high-degree against the pointer's
three-register palette. In an unpinned neighbourhood, a neighbour with fewer
edges than its own available-register count can be coloured last. The test
now uses that count. Pinned neighbourhoods keep the established rule.

Prioritizing loop copies was tried first and did not remove the copies;
that ordering experiment was removed. Applying the new degree test around
pins also moved costs backwards in FPDEEP/NBODY; that expansion was removed.
The final change reduces MATRIX by 80 modeled cycles on every compiler:
7408/7412/7418 -> 7328/7332/7338. ARRIDX changes register assignment without
changing cost. The other 90 primary objects and NBODY's regression object
are byte-identical to the previous implementation (97-object comparison).

Three emitted-copy regressions failed before the change and pass afterwards;
all 16 focused coalescing tests pass. Stage dumps are at
`/tmp/qbopt-matrix-next` and `/tmp/qbopt-matrix-palette-after`. The broader
intermediate candidate passed 111 runtime cases; the final pinned-guard
version is separately checked on the six changed MATRIX/ARRIDX variants.

### Signed widening participates in value analysis

The scoreboard still does not certify completion: ADDRM remains at
1.60--1.62x on ordinary variants, event variants remain refused or expensive,
and several reference listings are missing/provisional. No gate was relaxed.

The explicit MIR SIGN_EXTEND introduced for whole long stores previously
had no constant or interval semantics. Constant propagation now interprets
the source's own sign bit and produces a full-width result; incomplete
source facts are insufficient. Range propagation retains the same signed
numeric interval at the wider width, rejecting intervals outside the
source's signed domain. No machine names or origin information are needed.

Eight constant/range cases failed first, as did the three real ADDRM
word-to-long counter-bound cases. ADDRM now retains 1..20 through the long
conversion feeding b(i). The constant cases also assert that the fold pass
actually replaces the conversion with a constant copy. The focused checks
pass 35/35, followed by the five augmented fold assertions. All 96 primary
fixture objects remain byte-identical, so this is analysis groundwork rather
than a measured execution improvement. No runtime suite was repeated for
unchanged emitted bytes.

### Retain constants across modeled sign extension

NBODY's interaction loop emitted `mov eax,512; cdq; mov eax,512; idiv ...`.
The post-allocation constant peephole forgot every fact at CDQ, although its
explicit destination is EDX. It now preserves unaffected constants across
register-only CWD/CDQ/MOVSX operations. Destination aliases and explicit
clobbers still invalidate facts; unknown operations, calls, and block edges
remain barriers. A conversion writing EAX is not treated like one merely
reading EAX.

The emitted constant-count assertion was tightened from two copies to one
and failed first, alongside the non-clobbering-extension case. All 11 focused
peephole checks pass. Dumps at `/tmp/qbopt-nbody-next` and
`/tmp/qbopt-nbody-constant-after` show the one deleted hot-loop materialization.
NBODY's modeled cost falls 376416 -> 374416. CHAIN also loses one materialization
per object: 921/933/881 -> 919/931/879. Those three CHAIN variants are the only
changed primary objects among 96. NBODY passes all 72 runtime cases across the
three compilers; CHAIN is separately checked on those same compiler variants.
Artifacts: `qbopt-nbody-constant-7k9vkbz8` and `qbopt-chain-constant-4j353rz8`
under the system temporary directory. No claim is made that NBODY's still
missing reference target has been reached.
