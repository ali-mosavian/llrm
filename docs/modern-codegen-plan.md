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
| Pressure-aware allocation | partial | spilling, slot colouring, byte RMW selection and local constant, frame, and relocatable-address rematerialization exist; global splitting/rematerialization and x87 allocation remain. |
| Loop optimization | partial | exact pre- and post-tested recurrences, composed pointer recurrences, formula costing, specialization, peeling and exact unrolling exist; versioning, rotation and broad pressure forecasting remain. |
| Whole-module optimization | partial | summaries, constant returns, private inlining and private procedure DCE exist; full IPSCCP/cloning and private-data DCE remain. |
| Post-allocation quality | partial | copy propagation, machine CSE/DCE, C-path tail sharing, conservative out-of-order latency scheduling, and a restricted P5 U/V scheduler exist; source-map-aware BC tail sharing, x87/segment scheduling, memory pairing, and full issue modelling remain. |

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

### 7. Committed RMW QCport evidence — 2026-09-18

The rerun is now reproducible: clean QCport `18f5e1f` at source hash
`e5abf5f8fb67c3cac011579fcb980bff446ab068c130dd2c196652bbf802fef0`,
against qbopt `4352de4`, CPU `386`.  The paired listing manifest records BCC
listing SHA-256 `ea48ccc713812effbdf57abb6e9b5e1ba00449ab01a00e6faadeb8fd2bbba73c`
and qbopt listing SHA-256
`ccbacccfc7ff5f46553d0e8b41dd78652f7bb7ad8ad7aa486dc629c92511badc`.
Raw assembly confirms qbopt now emits `or byte ptr es:[bx+di],dl` in the
marked-face loop.  It still reloads `world` and `rdr` from the frame at the
top of each iteration, whereas BCC retains them in DI and SI.

Therefore the next issue is not byte promotion or a missing split proposal:
the existing region split still cannot satisfy the residual interference and
must be traced from the allocated LIR/intervals.  Keep GCC/LLVM flat-i386
listings as best-case structural references only; this BCC medium-model pair
is the candidate-ABI evidence for any near-term allocation change.

### 8. Residual loop-pressure trace — 2026-09-18

The pre-allocation LIR pinpoints the remaining conflict.  The loop carries
both the source counter (`v347`) and a strength-reduced `i * 2` recurrence
(`v1028`), while an iteration also needs the face value, shifted bitmap index,
bit count and mask.  At the marked store those live demands already occupy the
six general registers available to the 16-bit addressing model, before either
incoming `world` (`v35`) or `rdr` (`v65`) can be retained.  The allocator's
frame reloads are consequently legal, deliberate spill/rematerialization—not
a failed alias proof.

BCC instead keeps `world` in DI and `rdr` in SI, retains one loop counter in
DX, and keeps the scaled leaf offset in a frame cell.  It trades an `add bx,
[bp-12]` for one long-lived recurrence, freeing capacity for the two invariant
bases.  The general next mechanism is phase-5 formula selection: price
recompute, stack-carried, and register recurrence forms against complete
per-iteration pressure *including invariant bases and address temporaries*.
Do not disable this particular recurrence or reserve registers by procedure
name; first add a source-independent pressure regression that demonstrates
the winning formula and then make `strength` choose it by target cost.

### 9. Medium-model address-form legality — 2026-09-18

The pressure investigation found that every CPU profile advertised flat-386
scaled address forms `{1,2,4,8}` to MIR.  This compiler emits 16-bit
medium-model effective addresses, whose legal indexed form is only
`[base+index]`; a 32-bit SIB scale would require a different address-size and
pointer model.  The profiles now expose `{1}` consistently.  The new
fail-first CPU-profile regression verifies that contract for every supported
CPU, and the focused profile plus C RMW tests pass.

This corrects the target interface rather than choosing a QCport formula.  It
does not itself remove `r_walk`'s word-offset recurrence: that recurrence is
the legal fallback for `i * 2` under 16-bit addressing.  The next iteration
still needs pressure-aware selection between that register recurrence and a
stack/recomputed offset, with BCC's medium-model listing as the ABI reference.

### 10. Address-form validation listing — 2026-09-18

The clean paired `r_walk` build at qbopt `a8eb96e` keeps the same qbopt listing
hash, `ccbacccfc7ff5f46553d0e8b41dd78652f7bb7ad8ad7aa486dc629c92511badc`.
That is the expected result for this loop: `i * 2` has no legal scaled
16-bit address form, so the existing strength reducer already chose its
register-recurrence fallback.  The profile correction prevents future MIR
formula selection from incorrectly pricing a SIB-scale option; it is not
claimed as a performance change here.

### 11. Pre-allocation far-pointer load selection — 2026-09-18

The next raw C loop regression, `fixtures/c/farloadloop.c`, reproduces the
relevant `r_walk` shape: near `World` and `Renderer` owners, each with a far
field used in an indexed hot loop.  It failed first with no `les` at all:
allocation had already rematerialized each owner for its offset word and its
selector word, so the final physical `far_loads()` peephole could no longer
prove a shared base.

`backend/farload.py` now performs this exact instruction selection within
lowering, before allocation.  It requires both the normal address proof
(adjacent words reached identically) and a semantic proof from the C raise:
the words must be consecutive, non-volatile `pointer4` provenance slices of
the same source object.  The latter is essential: the initial address-only
version visibly miscompiled the adjacent near `world` and `rdr` frame
arguments as `les cx, dword ptr [bp+6]`.  The regression now checks both
outcomes: two field `les` loads are present and no `les` reads that near
argument pair.

The corrected emitted loop is structurally closer to the BCC medium-model
listing: `mov bx,[bp+6]; les bx,[bx+38]` and the corresponding `rdr` field
load replace four independent owner/word moves.  This is a lowering-side
target form, not an MIR pass or a post-allocation optimization tier.  GCC and
LLVM listings remain best-case flat-i386 structural references; BCC remains
the ABI-constrained reference for this comparison.  No timing claim is made
until the committed qbopt revision can be used to produce a clean paired
QCport manifest.

### 12. Committed far-load QCport validation — 2026-09-18

The clean paired listing is now recorded at qbopt `b3bc47d7745a46cd806d007f45c39995d806d832`,
CPU `386`, against clean QCport `18f5e1f9e8d4ad54622da847dd5a412e6726ab50`
and source hash
`e5abf5f8fb67c3cac011579fcb980bff446ab068c130dd2c196652bbf802fef0`.
The manifest records BCC listing SHA-256
`3ad08c6af9b725812091677ea60022c629e0ff0c13abd7f82b09c7c0fcde38b7`
and qbopt listing SHA-256
`21d841130f6445e2c403b3e82fe995f6ba711f659c3bd244b702e08d8aa0204f`.

Raw assembly verifies both `world->lfc` and `rdr->pflag` now load through
`les bx,dword ptr [bx+38]` and `les bx,dword ptr [bx+1014]`, respectively.
This closes the former post-allocation address-identity loss.  The remaining
gap is now cleanly isolated to phase 4/5 pressure and formula choice: BCC
keeps the two near owners in DI/SI and carries its scaled offset in a frame
word, while qbopt still reloads both owners in the loop and carries both the
source counter and the scaled recurrence in registers.  That is the next
mechanism to compare with the GCC/LLVM best-case loop structures and BCC's
medium-model legal form; this iteration makes no timing claim.

### 13. Best-case reference audit — 2026-09-18

`tools/quality.py --references` produced fresh flat-i386 `clang -O3` and
`i686-elf-gcc -O3` listings for the compact far-load loop.  Both retain the
two owner pointers across their inner loop and each emits one byte RMW; neither
performs the repeated owner reload that qbopt still does.  They choose
different flat-model induction forms, and their pointer widths, stack ABI,
address sizes, and unbounded SIB addressing differ from the medium-model
target.  Their reported estimated instruction ratios (clang `1.99x`, GCC
`1.68x`) are therefore advisory structural signals, not quality gates or
timing results.

The BCC listing supplies the applicable target form: two retained near owners,
one source counter, and a frame-carried scaled offset.  This comparison gives
the next general implementation a concrete requirement: strength reduction's
register budget must account for loop-invariant address bases and actual
transient live pressure, not only a fixed two-register reserve.  The next
iteration will add that MIR-level pressure model with a fail-first regression;
it must not hard-code `r_walk` or any particular register assignment.

### 14. Formula-ablation result — 2026-09-18

Compiling the far-load loop with the entire `strength` transform suppressed
removes the carried `i * 2` recurrence, but still emits a frame reload of each
near owner before its `les`.  It therefore does **not** recover BCC's retained
owner shape.  This is direct evidence that changing the formula alone is not
the first fix: current allocation treats the invariant owner loads as cheap
to rematerialize and spills them despite their repeated address uses.

The next implementation target is phase 4's weighted spill policy.  It needs
to charge a loop-invariant address base for every dynamic reload and for the
far-field load it prevents, then compare that benefit against the best
available recurrence or transient spill.  Formula selection remains necessary
afterward, but the regression must first prove that an allocator can retain a
generic invariant address base rather than reserve DI/SI for this function.

### 15. Spill-price and split trace — 2026-09-18

The allocator trace confirms why the owners lose: its normalized interval
weight is used both to prioritize placement and to price spilling.  The two
long owner intervals consequently score about `0.08`, even though each has a
loop-weighted dynamic frame reload.  An experimental separation of raw spill
traffic from placement priority, including constant rematerialization and an
address-base reload charge, did not satisfy the fail-first retained-owner
regression.  The register file still needs a temporary register during the
face/mask sequence, so changing only the eviction victim cannot create a
legal assignment.

That experiment was deliberately discarded rather than committed: it leaves
the regression red and does not prove an improvement.  The evidence changes
the next implementation precisely: extend the existing `splitkit` ladder so
that a loop-invariant address base can be retained for its dynamic address
uses but cut around a disjoint high-pressure local region.  Accept a split
only when re-allocation lowers weighted memory traffic; that is the generic
phase-4 mechanism, not a function-specific register preference.  The
far-load regression remains the fail-first acceptance case for that work.

### 16. Assigned-range split experiment — 2026-09-18

The proposed broadening was tested directly, without committing it: after
the first failed allocation, `splitkit` was offered every assigned range as
well as the failed owners, then the body was reallocated and scored with the
existing loop-weighted traffic measure.  No individual assigned-range split
reduced the baseline traffic (`33`); several were neutral and the splits of
the far-field offset/value ranges increased it to `44`.  Splitting all
assigned ranges together was worse (`93` or `103`, depending on whether the
failed values were also offered).

This rejects the naïve "split more values" extension.  The current acceptance
criterion is correct, and the next phase-4 mechanism must choose a joint
pressure plan: retain the valuable invariant base *and* split/relocate a
specific short competing range as one candidate, then compare its complete
traffic and copy cost against the original assignment.  It cannot be an
after-the-fact split of whichever range happened to be allocated.

### 17. Fixed-register allocation invariant — 2026-09-18

Before evaluating a joint pressure plan, allocation now distinguishes a hard
register fact from an ordinary spill candidate.  A fail-first allocator
regression constructs two simultaneously live values both fixed to DX.  The
old greedy path allocated the first and returned the second in `spilled`;
spilling it would replace the hard requirement with a reload free to use any
register.  That is neither a valid allocation nor a valid recovery.

`allocate()` now reports that state as `Unplaced`.  The existing constrained
occurrence regression continues to prove that valid short fixed occurrences
are retained.  This is phase 4 correctness infrastructure, not a QCport
pressure solution: no whole-loop owner was pinned and the `r_walk` listing is
intentionally unchanged.  With impossible hard constraints no longer able to
masquerade as a spill plan, the next iteration can evaluate an explicit,
joint invariant-base/competing-range candidate honestly.

### 18. Fixed ranges are eviction-protected — 2026-09-18

The same audit uncovered a second half of the fixed-register invariant.  Even
after fixed values could not reach the ordinary spill result, a hotter flexible
range could evict one because eviction charged its normalized spill weight.
The requeued fixed interval then failed later.  A fail-first allocation
regression now proves that a protected hard range is never offered as an
eviction victim.

The allocator passes its fixed set to eviction and rejects a register whose
overlapping occupants include a protected value.  A controlled far-load
allocation trace consequently retains both provisional owner bases and spills
the competing derived index/recurrence instead.  The pins were diagnostics
only and are not part of production output: this establishes the feasible
joint-plan alternative, while the next iteration must choose it from
allocation costs and legal split/recompute alternatives rather than naming
those two owners or their registers.

### 19. Joint-plan feasibility audit — 2026-09-18

The allocated-LIR trace corrects the earlier location diagnosis: both near
owner loads are already hoisted by MIR LICM into the preheader.  Their values
are `v5` and `v13` at LIR entry; ordinary allocation spills them and the
spiller reloads each immediately before the corresponding `les`.  This is
the first stage at which the BCC shape is lost.

A controlled allocation with those two values protected retains them, but
the greedy alternative spills the carried word recurrence and a derived index
instead (`v114` and `v18`; the raw loop-weighted spill traffic rises from `44`
to `95`).  Carrying that diagnostic plan through the full spill/reload loop
also becomes unplaceable: the new reload has no legal register at its use.
It is therefore rejected, not promoted to an output regression or a tuning
claim.

The result narrows phase 4/5 precisely.  The required candidate is not
“protect invariant bases”: it must describe both the retained invariant and
the short value placed in memory or recomputed across the exact high-pressure
region, then price the resulting loads, stores, folds and copies as one plan.
In particular it must be able to consider the loaded face value's later bit
use, which BCC deliberately stores before reusing its register as the bitmap
index.  The next implementation is a general pressure-plan representation
and evaluator; it may not encode this loop, its values, DI/SI, or a source
procedure name.

### 20. Unplaceable reloads stop at allocation — 2026-09-18

Tracing the rejected joint plan through the spill/reload loop found an
independent allocator defect.  A short reload marked unspillable, with no
register free after its one eviction attempt, re-entered the assignment stage
forever because its infinite weight retried eviction on every visit.  On queue
budget exhaustion `allocate()` returned it in neither `where` nor `spilled`,
and final rewriting reported an unrelated missing-register error.

A fail-first allocator regression fills the register file with hard ranges
and asks for one unspillable reload.  Allocation now raises `Unplaced` at the
actual pressure boundary after its assignment attempt; it cannot return a
partial assignment.  This does not make the experimental retained-owner plan
profitable, but it makes all subsequent joint-plan candidates auditable and
prevents the same missing-value failure in any frontend.

### 21. Fold a spilled index into its dying address base — 2026-09-18

The medium-model register-pressure trace exposes one target-legal fold that
the ordinary spill path lacked.  A word held only as `[base+index]` cannot use
a frame slot as the index, but if the base dies at that exact access, the
spiller may emit `add base,[slot]` and read the unchanged cell through the
updated base.  This consumes the spill directly and eliminates the reload
register.  It applies equally to loads and read-modify-write cells.

The fail-first standalone regression verifies the fold and a paired safety
case verifies that it refuses when the base reaches a later access.  In the
controlled `r_walk` pressure experiment, the fold removes both formerly
unplaceable index reloads and yields retained near owners with the expected
`les; add base,[bp-slot]` structure.  No owner is automatically protected by
this commit: selection between the baseline and that full joint plan remains
the next phase-4 mechanism, to be priced rather than assumed.

### 22. Conservative joint invariant/index pressure plan — 2026-09-18

Allocation now evaluates a complete alternative when its baseline spills a
stable frame-loaded value that is a hot-loop memory base.  It protects those
invariant bases, re-runs allocation, and admits the alternative only if no
protected base spills and *every* displaced value has a direct, target-legal
recovery: existing rematerialization or the dead-base word-index fold from
iteration 21.  The candidate never names a source procedure or physical
register; normal register-class allocation chooses the actual locations.

Once admitted, the evaluated direct recoveries are applied as one plan rather
than mixed with speculative range splits or spill-web expansion.  That is
important: either rewrite would introduce a different set of pressure values
after the candidate was priced.  The end-to-end C regression first failed
with both `[bp+6]` and `[bp+8]` owner loads inside the loop; it now verifies
that they are preheader loads and that the loop uses retained owners plus
folded frame indexes.  This is phase-4 progress, not a claim of final
allocator quality; broader CPU and corpus evaluation remains a phase-boundary
gate.

### 23. QCport recursive-loop pressure audit — 2026-09-18

The compact acceptance loop is necessary but not sufficient.  A fresh clean
paired listing at qbopt `b99ecfe20e9b9c45f09657e910b3bdb3e3f7f24f`, CPU
`386`, against QCport `18f5e1f9e8d4ad54622da847dd5a412e6726ab50` has the
same source hash and the same qbopt listing hash
`21d841130f6445e2c403b3e82fe995f6ba711f659c3bd244b702e08d8aa0204f` as the
prior audited listing.  The real `_r_recursive_world_node` loop still emits
`mov bx,[bp+6]` before its first `les` on every marked face.  BCC's
medium-model listing instead keeps the near owners in DI/SI and uses BX only
for each transient far-field address.

The stage dumps locate the first loss exactly.  Optimized MIR block 49 has
already loaded `world` as `v35`; LIR block 77 uses that value as the base of
the `les`.  At `04-RegAlloc`, `v35` has been rematerialized to
`mov bx,[bp+6]`.  The value also crosses the later recursive/call path, so a
whole-function retained-base trial protects `v35` but forces unrelated,
non-recoverable far-address values out of registers and is correctly
rejected.  `rdr` is not even eligible for that trial: it is stable only over
this loop, not across the later call.

Forcing the current regional split for one or both owners does not improve
the generated loop: its fresh loop value is itself spilled under the
simultaneous face-value, shift-count, bitmap-index and far-address pressure.
This rejects both a looser split-feasibility gate and a whole-function
priority tweak.  The next phase-4 mechanism must construct and price a
**loop-scoped** invariant materialization/split together with the competing
short-lived values' legal recovery.  It must be generic over loop blocks and
stable source cells; it may not name `r_walk`, its arguments, or BCC's DI/SI
assignment.  The compact loop remains the positive regression; the clean
QCport listing is the real-world negative acceptance evidence for the next
iteration.

### 24. Loop-scoped pressure plan and exit-edge restoration — 2026-09-18

The missing phase-4 mechanism is now implemented in the allocator.  For a
spilled value that is both a memory base in a natural loop and used outside
it, `splitkit.loop_bases()` creates a loop-only value.  The copy-in is placed
on the preheader edge; a mixed latch/exit block restores the original through
a newly split exit edge, so restoration is paid once when leaving the loop
rather than once per trip.  The focused regression was fail-first: the old
split put the restore immediately before the loop branch, while the new test
requires the loop to keep the fresh value and the bridge to own the restore.

The allocator evaluates this scoped alternative as a whole.  It first folds
only dying word indexes belonging to a natural loop that actually uses one
of the fresh bases.  It also orders protected address owners after the
target's unique 16-bit word-base role where indexed memory needs that role:
SI/DI are tried before BX, but BX remains legal if necessary.  This is an
encoding-capacity rule from the target model, not an r_walk register hint.
The candidate is admitted only when its complete pre-folded allocation keeps
the fresh bases and strictly reduces loop-weighted spill traffic.

On the clean QCport probe, the baseline allocation's weighted spill traffic
is `79`; the scoped, pre-folded candidate is `59`.  The emitted marked-face
loop now begins `les bx,[si+38]`, then later `les bx,[di+1014]`: no
`[bp+6]`/`[bp+8]` owner reload remains in the loop.  It retains two remaining
differences from BCC that are intentionally not disguised as parity: the
loaded face value and one-bit mask still use frame temporaries, and BCC's
particular AL/CL reuse has not yet been selected.  GCC/LLVM remain the
best-case flat-i386 structural references; the BCC listing remains the
medium-model legality reference.  This is phase-4 progress, not a completed
code-quality gate.

Focused split, far-load, and dead-index-fold regressions pass in `0.22s`.
The broader allocator group was deliberately stopped while still running to
honour the development test-time budget; it is a phase-boundary suite, not a
claimed green gate for this iteration.

### 25. Committed QCport loop listing validation — 2026-09-18

The clean paired listing for qbopt
`45031078e4d7cd6f1e17bf57dd2c7bc6e7a72b1a`, CPU `386`, and QCport
`18f5e1f9e8d4ad54622da847dd5a412e6726ab50` records source SHA-256
`e5abf5f8fb67c3cac011579fcb980bff446ab068c130dd2c196652bbf802fef0`, BCC
listing SHA-256 `e7a79276d99101ebb86bc6060ec67c3f2714c765067642d0d001e01e476f9975`,
and qbopt listing SHA-256
`9234aca827aa9117854392544d267c8aef10d8c911455b525a3ed33beb5a284b`.

Raw assembly verifies the intended target-specific result.  qbopt's marked
face loop has `les bx,dword ptr [si+38]` and later
`les bx,dword ptr [di+1014]`, with no `[bp+6]` or `[bp+8]` reload in between.
Those are the same medium-model-legal owner/field roles as BCC's DI/SI/BX
form, though allocation is free to exchange SI and DI.  This is a listing
comparison, not a DOSBox timing claim.

The next remaining structural gap is also hand-derived rather than inferred
from a ratio.  BCC carries the loop counter in DX, leaving AX/AL to load,
shift, and form the one-bit mask; qbopt carries its counter in AX, retains the
unshifted face in CX, and therefore performs the shifted bitmap index through
a frame word.  The next candidate must jointly select a loop counter,
temporary value, and byte/shift roles from the target register constraints
and spill cost.  It must not merely swap AX and DX for this function.  Flat
GCC/LLVM listings remain best-case structural references only; this BCC
listing is the constrained ABI reference.

### 26. Counter/temporary role trace — 2026-09-18

The committed stage dump proves this is not a missing MIR transformation.
Before allocation, the loop counter is a normal phi (`v347`/`v82`), the face
load is `v64`, its shifted form is `v70`, the shift-count form is `v74`, and
the one-bit result is `v75`; all are independent, target-legal LIR values.
The face-to-shift copy is eligible for normal two-address coalescing and the
counted shift already has the required CL constraint.

The first unwanted decision is `04-RegAlloc`: generic allocation order gives
the counter EAX.  The face remains in ECX for the CL use, the one-bit value
uses DX, and the shifted face has no register left, so the existing legal
dead-base fold turns it into `sar [frame]; add bx,[frame]`.  This explains the
listing exactly; no source fact, alias fact, or MIR pass is missing.

The next candidate is therefore a target-priced **role allocation** for a
short loop kernel: compare counter placement together with the temporary
chain and byte/CL requirements, then accept only a complete legal allocation
whose weighted cost falls.  It must work from LIR live ranges and target
classes, must include the cost of any new copies/spills, and must not be an
AX/DX rewrite keyed to this procedure or loop shape.  No code-generation
change is claimed in this evidence-only iteration.

### 27. Counter-role feasibility experiment — 2026-09-18

A non-committed constrained allocation confirms the role analysis.  Pinning
only the loop counter away from EAX produces `add dx,1`, `cmp dx,...`, and
uses AL for the one-bit result; the form is legal and is not blocked by the
ABI.  It still leaves the shifted face in a frame word.  When the bitmap
index fold is withheld so that the shifted face may take EAX, allocation
correctly reports an unplaceable short reload instead of emitting invalid
code.

That result identifies the missing complete plan.  The unshifted face must
be split at its two constrained consumers: store it once, give the shifted
successor EAX until it has formed the bitmap address, then reload its byte
into CL for the mask.  This is exactly the live-range shape in BCC's listing,
but the implementation must be a general constrained-use split and
spill-web/home-sharing candidate, jointly evaluated with alternate loop
counter placement.  It cannot be a counter pin or a rule for face/bitmap
operations.  No code-generation change is claimed by this experiment.

### 28. Direct move reload prerequisite — 2026-09-18

The constrained-use experiment exposed and fixed one general spill-rewrite
defect.  When a long value is spilled immediately before a short copy, the
old rewrite loaded it into a fresh temporary and then copied again, requiring
three registers at the very point the plan is trying to relieve pressure.
`spiller` now folds that source directly into the move as
`mov short,[frame-slot]`.  The fail-first regression records the r_walk
symptom and verifies that the successor names the frame cell with no reload
interval.

This is necessary but not sufficient for the complete role plan.  Re-running
the non-committed counter/source/index experiment still reaches a genuinely
unplaceable short reload after the direct move is available.  The allocator
therefore remains correct to refuse it.  The next implementation must choose
and split the mutually dependent counter, shifted successor, and CL reload
as one plan; this change merely removes an accidental temporary from that
candidate and improves every frontend's ordinary move spill path.

### 29. Soft register-role preferences — 2026-09-18

The allocator now distinguishes a role preference from a hard ABI pin.
`allocate(..., preferred={value: register})` tries the preferred legal
register first but freely falls back when it is occupied; it cannot turn a
valid body into an unplaceable fixed-register problem.  The fail-first
allocator regression proves both halves: a free DX preference is honoured,
and the same preference falls back when an overlapping hard DX range exists.

This is the required foundation for the next joint role-plan evaluator.  It
lets that evaluator compare counter and temporary roles using real complete
allocations, while `pinned` remains exclusively an ABI/encoding fact and the
unplaceable-reload guard continues to reject impossible candidates.

### 30. Explicit role preferences outrank copy hints — 2026-09-18

The first complete `r_walk` experiment used the new DX counter preference and
proved it is a legal alternative, but its allocated spill traffic and emitted
hot-path instruction count were unchanged: it only exchanged AX and DX.
That makes BCC's particular register spelling an unsuitable acceptance target
by itself.  The candidate remains rejected until the target pricing can show a
net gain for the complete loop plan.

The experiment also exposed one allocator-contract defect.  `preferred`
initially moved its register to the front of the allocation order, then copy
hints re-sorted that same order and could put the hinted register back first.
An explicit role request therefore silently lost to an ordinary coalescing
heuristic.  Preferences now reorder after copy-hint voting.  They still are
not pins: if the requested register is occupied, allocation tries every legal
fallback.

The fail-first regression constructs the exact source-dead copy shape: AX is
available through a copy hint, DX is explicitly requested, and both are legal.
It previously assigned AX and now assigns DX.  The existing overlapping hard
DX case still falls back, proving this did not turn a role preference into an
ABI constraint.  Focused allocator checks pass in `0.06s`.  GCC/LLVM i686
listings remain best-case structural references; BCC's medium-model listing
continues to constrain only legal ABI/address forms, not register spelling.

### 31. Constrained-reload stage trace — 2026-09-18

The next fail-first-style diagnostic used the normal spill machinery on the
unshifted face, withheld only the competing index fold, and asked for the
otherwise legal DX counter role.  It does **not** establish a production
candidate: allocation correctly ends with `Unplaced: value#1061 cannot be
spilled and no register is free for it`.

The adjacent LIR dumps identify why.  The existing generic direct-move fold
already produces the desirable first half of the split: it stores the face
once, then emits `v70 <- [face-slot]` directly before `sar v70,3`.  The
remaining byte use is different.  It first creates a fresh `v1046 <- byte
[face-slot]`, then passes that value through the semantic-less transfer into
the pre-existing shift-count child `v1036`, whose actual use requires CL.
When pressure spills that new reload, the next required child is the
unplaceable `v1061`.

The missing general phase-4 operation is therefore a **constrained reload
fold**, not a counter rule: where a spill-cell read feeds an immediately
following value-transfer whose destination has a fixed one-instruction use,
materialize the legal width slice directly into that constrained child.  It
must be proved over LIR transfer edges, register requirements, slot width and
partial-register interference; it must not mention this loop, CL, AX, or DX.
The corresponding regression must first reproduce the unplaceable reload,
then assert one direct constrained load and a complete legal allocation.  No
code-generation or performance claim is made by this diagnostic commit.

### 32. Refreshed GCC/LLVM best-case listing evidence — 2026-09-18

`tools/quality.py --references` now has fresh, generated listings for the
compact far-load loop from Apple Clang 21 and the installed `i686-elf-gcc`
16.2.0.  The report retains their raw paths, exact flags, compiler versions,
source hash, and the `best-case-flat-i386-structural-reference` contract.

The references differ in static shape—Clang reports 21 normalized
instructions, GCC 33—but agree on the important direction: qbopt's dynamic
operation estimate remains about `2.03x` Clang and `1.72x` GCC.  Both reports
attribute the first excess loads and stores to `lir-regalloc`.  This supports
the constrained-reload/pressure investigation but is **not** a performance
target: both references use a 32-bit flat ABI and omit our segment selection,
far-pointer traffic, restricted 16-bit address forms, and far-call frame
contract.  A hard target still requires an independently hand-derived
medium-model listing.

### 33. Byte direct-move spill fold — 2026-09-18

The constrained-reload trace identified a concrete general omission in the
existing direct move spill fold: it admitted word and dword moves but rejected
the identical byte form.  `mov cl, byte ptr [slot]` is a legal load just as
`mov cx, word ptr [slot]` is, so the old gate created an unnecessary byte
reload interval and a second copy before a constrained child.

`spiller.folded_source()` now accepts width one for `MOVE` only; arithmetic
and comparison folds retain their prior word/dword gate.  The fail-first LIR
regression models the observed masked-count shape and requires the byte spill
cell to become the move source directly, with no virtual reload use.  It
failed under the old gate and the focused direct-move, byte-move, and indexed
spill checks now pass in `0.06s`.

Re-running the deliberately forced QCport pressure candidate removes that
particular transfer but still reaches a different unplaceable short range.
The result is therefore a correct general cleanup, not a claim that the
complete counter/face role plan is now accepted.  The next candidate must
continue to price every remaining constrained range together.

### 34. Remaining far-address conflict — 2026-09-18

The post-byte-fold stage capture resolves the next refusal without guesswork.
The unplaceable short value is `v1060`, the reload feeding the final far byte
RMW's offset base.  It is not a count reload and it is not a bad fixed
requirement: with the shifted index retained, the medium-model final operand
needs both its unique word-base role and an independently legal word-index
role.  The candidate already protects the loop-scoped owner, so no remaining
address-class register can satisfy the new base reload at that point.

This is the complete role-plan boundary.  The candidate must compare the
alternative of folding the shifted index into a dying base against retaining
the index and keeping the far offset base live, together with counter and
byte roles.  Each form must include the exact legal 16-bit addressing classes
and all reload/spill traffic.  Trying to retain one more value or reserving BX
for this loop would only convert this honest refusal into a hidden special
case.  No production code changed in this evidence iteration.

### 35. Composed pointer-recurrence basis — 2026-09-18

The loop analysis now recognizes a pointer offset whose offset operand is an
already-proven affine formula, rather than only the direct `base + i` form.
Strength reduction initializes the complete `base + (start * scale +
invariants)` formula in the preheader, advances that whole pointer at the
latch, and uses the carried phi as the address base for eligible same-block
memory users.  The latter is deliberately local: any use in another block,
at a join, or after loop exit retains the value-producing copy until a later
dominance-and-exit reconstruction implementation proves a wider replacement.

The fail-first regression models `base + (i * 2 + 6)`, requires the derived
pointer formula, and verifies that the store addresses the carried phi rather
than a redundant copy.  During this implementation the shared SSA substitute
helper was found to rename ordinary load/store cells but not `memory_values`;
it now renames that MIR memory-bearing form too, with a pointer regression.
The focused composed-address and pointer-substitution checks pass in `0.04s`.

The original NDARR xfail remains intentionally unresolved.  Its pointer
offset starts from a sign-extended narrow counter for which the loop bound
does not prove no wrap, so treating it as one monotonic long pointer would be
a miscompile.  This iteration supplies the general path for formulas already
proved wide and affine; extending the narrow-to-wide proof requires an exact
trip-bound proof first.  GCC and LLVM listings continue to be best-case,
flat-i386 structural references only; BCC/WC medium-model output remains the
source of address-form and ABI legality evidence.

### 36. Partial-result logical loop bounds — 2026-09-18

The follow-up stage dump disproved the remaining NDARR assumption: its
`rowIndex = -1 .. 0` loop has an exact `OR i,i` zero bound and a unit stride.
The only rejected fact was a partial-result merge on the word OR.  That merge
records preservation of an unrelated upper half of the numeric destination;
it cannot change the word operation's flags, which are the sole input to the
branch.  The induction bound recognizer now accepts that general flag/value
separation for its existing compare and self-AND/OR forms.

The new fail-first unit regression uses a partial result and requires the
zero bound.  The former NDARR strict xfail is now an active structural
regression across PDS, QuickBASIC, and VBDOS: at `r02-strength`, where the
promoted counter and its exact bound first coexist, every pointer store names
a carried phi.  Later exact unrolling removes those phis by design, so final
MIR is not the evidence point.  The targeted logical-bound and three-fixture
checks pass (`9 passed`, `16.38s`).

This closes the narrow-counter proof needed by the preceding composed-pointer
iteration for this finite signed loop; it does not relax the non-wrapping
requirement for unknown bounds, XOR/masked tests, different widths, or other
extensions.  GCC/LLVM listings remain advisory best-case structural
references, while BCC/WC medium-model output remains the ABI/address-form
legality baseline.

### 37. Dominance-scoped pointer bases — 2026-09-18

The initial composed-pointer implementation rewrote only consumers later in
the same block.  That was safe but unnecessarily left a recomputed pointer
alive at ordinary successor and exit-block uses.  It now applies the same
replacement to direct operation uses when the original `PTR_OFFSET` dominates
the use block, while retaining the original copy for joins and phi incoming
edges that require edge-specific reconstruction.  The rule is CFG-based and
does not depend on a particular frontend, pointer shape, loop, or address
register.

The composed-pointer regression now puts the consuming store in the loop's
exit block.  It first failed because the old same-block-only rule kept the
original pointer variable; it now receives the carried recurrence's SSA
variable.  The focused composed-pointer and memory-substitution checks pass
in `0.08s`.  This advances phase 5's recurrence-use selection; broad formula
pricing, loop versioning, and pressure forecasting remain open.

### 38. Nbody dynamic-measurement audit — 2026-09-18

At `15b77c9`, a fresh `tools/quality.py bench/c/nbody.c --cpu 386
--references` run reports 598 bytes / 138 raw instructions for qbopt,
against 292 / 267 raw instructions for Clang 21 / i686 GCC 16.2.  Raw
assembly explains the apparent contradiction in the heuristic's dynamic
headline: GCC and Clang have unrolled the fixed three-body interaction nest,
whereas qbopt retains its `i < 4` and `j < 4` loops.  The current CFG model
assigns ten trips independently to every surviving natural loop, producing
qbopt 33,632 estimated operations versus Clang 1,174 and an implausible
28.65x ratio.

That is not an accepted code-quality conclusion.  It compares different
unrolling shapes with a profile-free ten-trip model and magnifies fixed loops
only on one side.  The raw listings and static counts are retained as
best-case GCC/LLVM structural evidence, subject to the medium-model caveat;
the dynamic comparison must instead use audited exact trip counts or a
runtime-input execution trace before it can be a target or a performance
claim.  The next measurement iteration needs a fail-first nbody regression
for this asymmetric fixed-loop case, then a general bound-aware dynamic
estimator.  No optimizer change is claimed here.

### 39. Canonical post-tested trip proofs — 2026-09-18

The measurement follow-up first added a fail-first rotated-loop regression:
a counter initialized to zero, incremented in its body, and tested after the
body with unsigned `next < 4` had no exact trip count and therefore received
the heuristic ten-trip weight.  `induction` now proves the general canonical
post-tested form when it has one latch, no early exit, a direct immediate
affine update, a single flags-producing comparison, an invariant bound, and a
non-wrapping finite exit.  The recognizer asks MIR's general `stepping()`
operation for the update; it does not name `inc`, nbody, or a frontend.
Pre-tested loops retain the existing proof path.

Exact MIR header counts now flow into lowered LIR **only as measurement
facts**.  The CFG estimator uses them at either the header (pre-tested) or
the unique latch (rotated post-tested), while arbitrary body exits retain the
ten-trip fallback.  The focused MIR and estimator regressions both failed
before their respective mechanisms and now pass (`3 passed`, `0.14s`).  On a
fresh nbody report, `_bench_nbody` proves its fixed middle loop has four trips
and the estimate falls from 33,632 to 13,652 operations; Clang remains 1,174
and i686 GCC 5,131.5 under their independent CFG estimates.  The resulting
11.63x/2.66x reference ratios are still **not performance conclusions**:
the outer bound is an input and the remaining nested loop is not yet exactly
profiled, while both GCC/LLVM listings are best-case flat-i386 structural
references rather than medium-model targets.  BCC/WC listings remain the
authority for ABI, segmentation, and legal address-form constraints.

### 40. Partial-result semantic CSE — 2026-09-18

The strict HARR descriptor-address CSE regression was rerun fail-first and
failed on all PDS, QuickBASIC, and VBDOS fixtures: the same descriptor-based
address add survived twice around the store.  The former xfail explanation
was stale—the FAR access already had its allocation identity.  Adjacent MIR
dumps instead showed both ADDs carrying a word-result merge, which represents
the old register's preserved high half.  CSE correctly refuses an observable
merge, but it had also refused that merge after `halves()` proved the result's
high half dead.

`subexpressions()` now constructs a merge-free **semantic identity** only
when every preserved high half is dead.  It still performs ordinary alias and
path checks between the two computations; this is neither a descriptor rule
nor a frontend/register exception.  A companion regression makes the later
result observable at width four and proves both ADDs remain.  The original
HARR regression now passes normally across all three compiler layouts and
retains its mutation guard, so an intervening unknown store still prevents
reuse.  Focused checks pass (`7 passed`, `0.32s`).

This is Phase-3 scalar/CSE progress, not a claim that the GCC/LLVM listing is
an ABI target.  The fresh BCC/WC medium-model listing remains the source for
the legal far-address form; flat-i386 GCC/LLVM listings remain best-case
structural evidence only.

### 41. Explicit merged operands remain effective — 2026-09-18

The next loop-motion regression, `IVWORD`, was exercised fail-first with
unswitching disabled.  Its unchanged `branchChoice` cell was loaded and
tested on every trip of the ten-iteration loop, on both the PDS `/G2` and
QuickBASIC `/O` fixtures.  Stage dumps showed that LICM proved the load
non-aliasing and collected its `LOAD` plus `AND` as an invariant run.  It
then correctly retained the flags-producing `AND` in the loop, but
incorrectly discarded the load as non-crossing.

The cause was the effective-value seed, not a loop-specific condition:
`and value,value` both explicitly consumes its low word to produce flags and
merges its preserved upper word into its narrow result.  The seed treated
every merged use as preservation only.  It now delegates to MIR's single
`consumed()` definition, which retains explicit operands and address bases
even when the same value is merged, while excluding preservation alone.  The
existing regression is now active and checks the emitted loop has no memory
source load while retaining its conditional test.  It failed before the
change and now passes for both compiler layouts (`2 passed`, `1.49s`).

While checking adjacent induction coverage, the old HARR structural test
failed identically with and without this change: the current optimizer fully
unrolls its fixed 10-by-10 fixture, leaving no conditional branch for an
assertion spelling `jne` to count.  The raw listing contains the expected
constant stores and no retained loop.  The test now measures natural loops
instead of a particular branch mnemonic: a retained shape must have the
expected number of loops and recurrence increments, while a fully unrolled
shape must have no increments.  This keeps the test useful across legal
lowering branch choices and complete unrolling without treating a stronger
result as a regression.

This advances Phase 5 LICM and Phase 4's machine-neutral value accounting.
It does not infer a target from the result: GCC/LLVM flat-i386 listings are
still advisory best-case structural references, while BCC/WC medium-model
output defines the ABI, segmentation, and legal address-form constraints.

### 42. ADDRM exit-sum regression audit — 2026-09-18

The remaining strict ADDRM exit-sum xfail was re-run fail-first across PDS
`/G2`, QuickBASIC `/O`, and VBDOS `/G3`.  It failed before any optimizer
change because the assertion required one surviving natural loop, while the
current pipeline already fully unrolls this literal 1..20 fixture.  Final
MIR has no loop, retains a `(source-address, 20)` repetition fact for quality
accounting, has no 32-bit ADD, and supplies the final `U=210` value as a
constant PRINT argument.  The raw three-layout MIR inspection confirms the
twenty `b(i)` stores still have their individual values, so this is not a
closed-form replacement of observable per-iteration storage.

The active regression now states that semantic result and permits either a
retained store loop or complete unrolling.  In the latter case it requires
the 20-trip repetition marker; in both forms it requires no long accumulator
ADD and an explicit constant 210 at the output boundary.  It passes (`3
passed`, `8.41s`).  This is a Phase-5 test/measurement correction, not new
loop algebra: the general exit-evaluation mechanism already has dedicated
hazard and LCSSA regressions.  GCC/LLVM output remains best-case flat-i386
structural evidence only; BCC/WC medium-model code remains the address-form
and ABI authority.

### 43. Guarded-record sink staging — 2026-09-18

The UDTRNG dominating-address xfail was first run fail-first and exposed two
different test-stage mistakes.  Capturing final MIR saw no loop because later
exact cloning had already removed it.  Capturing before promotion retained
the loop but correctly refused the field stores: each iteration still loaded
the preceding field value, so moving only the store would change the next
iteration.  Neither result was evidence against the sink mechanism.

The regression now deliberately disables only the automatic sink and captures
round-two MIR immediately after LICM.  At that structural point scalar
promotion has made the guarded `Coord` fields independent recurrences, LICM
has put the runtime-selected slot address in the preheader, and the loop CFG
still exists.  The test invokes the sink explicitly: the dominating address
case moves the final store, while a header-phi (changing) base and an
uncomputed base remain.  The former strict xfail is active and all three
cases pass (`3 passed`, `2.15s`).

This confirms the intended Phase-3-to-Phase-5 composition—scalar promotion,
invariant address formation, then exit-store sinking—without treating a
post-unroll listing as the proof.  GCC/LLVM listings remain advisory flat
i386 structural references; BCC/WC medium-model output remains the legality
authority for the emitted addressing form.

### 44. HOTLOP fresh-emitter regression — 2026-09-18

The historical HOTLOP invariant-product xfail was run fail-first and did not
find an allocator failure: `imul` was already absent.  It failed only when
the retired byte-rewriter assertion searched for a jump over a loop that the
fresh emitter now folds away completely.  Raw fresh emission confirms a
single straight-line body rather than a retained loop.

The regression now observes MIR at the public fresh-emission boundary.  It
requires no natural loop and no integer multiply, then verifies the known
literal result reaches the PRINT argument as 630.  It passes (`1 passed`,
`0.22s`).  This retires a legacy-path listing assumption and confirms that
the Phase-5 loop cleanup is already stronger for fully literal HOTLOP; it
does not claim progress on the Phase-4 pressure problem exercised by dynamic
HOTLPX and PRESS.  GCC/LLVM listings remain advisory flat-i386 structural
references, and BCC/WC medium-model output remains the emitted-form legality
authority.

### 45. Strict floating loop-exit facts — 2026-09-18

FPCSE's strict floating recurrence had an exact ten-trip exit (487.5), but
the numeric proof was blocked by the word loop-counter increment's preserved
upper-half merge.  `floatfacts.repeated()` now permits such a narrow merge:
the integer evaluator records only the operation's declared result width, so
a later wider consumer stays unknown rather than inheriting an invented upper
half.  The existing fail-first FPCSE read regression then proved the exit
load folds to binary32 `0x43f3c000`.

That proof exposed two correctness requirements.  First, ordinary `folded()`
must not turn a loop's strict x87 sequence into static stores merely because
the exit is numeric; storage folding is now limited to loop-free bodies, and
FP loop specialization retains the final checked iteration.  Second, an
edge fact is true at its successor, not after a call.  Constant-memory
propagation therefore invalidates explicitly supplied edge facts at every
call while retaining its existing precise call handling for ordinary facts.

Five former strict xfails are now active: exact exit-read folding, edge
scoping through calls/aliases/bypasses, alternate trip bounds, stage-dump
evidence, and repeated storage rounding.  The focused suite passes (`19
passed`, `1.15s`).  This is Phase-5 strict-FP loop analysis progress; it does
not claim the separate FP loop-specialization or global x87-allocation gates
are complete.  GCC/LLVM flat-i386 output remains advisory structural
evidence only; BCC/WC medium-model output remains the legality baseline.

### 46. Strict FP specialization before LICM — 2026-09-18

The FPCSE integration trace showed that the strict-FP specializer already
worked on canonical MIR but was called from `Strength` after LICM.  LICM had
then moved invariant x87 preparation out of the latch, leaving the
specializer no longer able to recognize the checked recurrence.  A dedicated
`floatloop` pipeline transform now runs immediately after LCSSA and before
LICM; `Strength` no longer invokes it too late.

This is not a static replacement of the final result.  The specialized body
stores the rounded penultimate value 438.75 (`0x43db6000`) with its original
symbol ownership, retains the original final x87 store to produce 487.5, and
writes the final counter value 11 before the exit.  The updated three-layout
regression observes that stage, requires no remaining natural loop, the
symbol-owned seed, the final strict store at the original accumulator, and
the original counter cell (`3 passed`, `1.97s`).  This advances Phase 5
strict FP loop specialization; final x87 allocation and runtime-matrix
acceptance remain separate gates.  GCC/LLVM listings remain advisory
flat-i386 structural references; BCC/WC medium-model output remains the ABI
and addressing-form authority.

### 47. Strict FP final-iteration gate — 2026-09-18

The final FPCSE strict-xfail was re-run after moving specialization before
LICM and became an XPASS on PDS `/G2`, QuickBASIC `/O`, and VBDOS `/G3`.
It directly exercises `floatloop.specialized()` and confirms the exact loop
is removed only after preserving the original floating/checkpoint sequence,
installing the storage-rounded penultimate seed, and executing one original
final iteration.  The xfail is now active.  The complete focused integration
file passes (`18 passed`, `3.31s`).

This closes the currently registered strict-FP loop-exit regressions, not the
whole FP roadmap: global x87 allocation, more general recurrence forms, and
runtime-matrix acceptance remain open.  GCC/LLVM listings remain advisory
flat-i386 structural references; BCC/WC medium-model output remains the ABI
and legal-address-form authority.

### 48. QuickBASIC strict-FP literal proof — 2026-09-18

The QuickBASIC `/O` FPCSE literal-initializer xfail now passes through the
same general floating recurrence analysis as PDS and VBDOS.  It proves the
ten-trip exit and its `0x43f3c000` final storage fact while preserving the
original literal-initializer side table; supplying no initializer facts still
correctly refuses the proof.  The regression is active (`1 passed`, `0.22s`).

This extends Phase-5 strict-FP evidence across all three supported BASIC
frontends/layouts.  It does not expand the proof to unknown initial memory or
relax the storage-rounding and exception constraints.  GCC/LLVM remains
advisory flat-i386 structural evidence; BCC/WC medium-model output remains
the ABI/address-form legality authority.

### 49. NESTED outer-accumulator regression audit — 2026-09-18

The NESTED outer-store xfail was re-run fail-first and showed no remaining
store in any loop—or at entry/exit—because the literal nested program now
fully evaluates to its final `T=675` PRINT argument.  Its former requirement
for stores at the entry and offset `0x9c` described only the weaker retained
loop shape, not the semantic requirement.

The active regression now requires no accumulator write in any retained loop.
If the loops are fully eliminated, it requires no accumulator store, the
constant 675 output, and the retained six-trip measurement marker; otherwise
it requires the previous entry/outer-exit store placement.  It passes (`1
passed`, `4.07s`).  This is a Phase-5 test/measurement correction; dynamic
nested loops still require pressure-aware formula selection.  GCC/LLVM
listings remain advisory flat-i386 structural references, and BCC/WC
medium-model output remains the legality baseline.

### 50. Fresh-OMF code-address relocation and EVTRAP registration — 2026-09-18

The real PDS and VBDOS `EVTRAP` fixtures were still refused by fresh OMF
emission: an address of code in its own code segment was mistakenly treated
as a bare DS/DGROUP data reference merely because its instruction had no
segment-override prefix.  That conflated the target address space with the
machine selector spelling.  The writer now frames every non-DGROUP
`SEGMENT` address at its target SEGDEF, while ordinary DS-relative data
references continue to use DGROUP.

The fail-first handler regression then exposed a second, independent defect.
Constant propagation may legally make the far-address stack push immediate
while the runtime call still requires the same logical handler address in
AX.  The allocator materialized that AX input at the call, and a newly
inserted symbolic `mov` retained the handler's pre-layout offset while the
source-owned push relocation moved.  The emitted fields named different code
addresses.  Generated in-code symbolic immediates now pass through the same
final layout map as source-owned code relocations, and handler discovery
accepts the equivalent `push cs / push offset / mov ax, offset` sequence
only when both relocations resolve to the same in-module entry.

The regression was first observed failing at the lower boundary and then
after fresh emission.  It now actively covers the shared call-input case,
both EVTRAP compiler families, relocation mismatch rejection, handler entry
discovery, and the real event-stub path (`4 passed`, `0.71s`); the complete
runtime-interface file also passes (`334 passed`, `1.17s`).  This advances
Phase 2's direct-LIR fresh-OMF boundary and its relocation architecture gate;
it is not a code-quality claim.  GCC and LLVM listings remain best-case
flat-i386 structural references only.  BCC/WC medium-model listings remain
the authority for segmented ABI setup, legal address forms, and emitted
relocation semantics.

### 51. PRESS literal-loop regression audit — 2026-09-18

The last strict `PRESS` xfail was run fail-first through the legacy
byte-rewriter and failed only because it required a surviving back edge.  A
fresh-LIR stage dump and listing show that the production pipeline now
evaluates all four literal invariant products and the ten literal iterations;
there is no loop and no multiply left to allocate.  The old assertion about a
particular `mov bx` in a loop was therefore neither a current regression nor
evidence for Phase-4 splitting.

The active regression observes the final fresh MIR boundary instead.  It
requires no natural loop or multiply and an explicit `R=7500` PRINT argument,
the independently calculated result of the original program (`1 passed`,
`0.34s`).  Dynamic `PRESSX` remains the proper Phase-4 pressure witness; no
allocator claim is made from this all-literal fixture.  GCC/LLVM listings
remain best-case flat-i386 structural references only, while BCC/WC
medium-model output remains the emitted-form and ABI authority.

### 52. Nbody triangular-loop listing audit — 2026-09-18

A fresh `--cpu 386 --references` quality report at `889fdae` records
qbopt's `_bench_nbody` as 598 bytes, 138 static instructions and 1435 model
cost.  Its flat-i386 comparison headline is still 11.63x Clang and 2.66x
i686 GCC estimated instructions, but it is explicitly not a performance or
target claim: the outer time-step bound is an input, and the reference
compilers use different flat ABI, frame, address, and unrolling strategies.
The raw listings and stage dumps, not that ratio, decide the next work.

In particular, the remaining pair loop is **not** a missed constant four-trip
loop.  Its MIR header starts `j` from `i + 1`, so its exit count is triangular
and depends on the enclosing iteration.  The existing exact unroller correctly
finds and accepts only the separate four-element velocity-update loop.  It
does not and must not clone the pair loop under a fabricated fixed bound.
Clang's fully expanded flat-model body is useful best-case structural evidence,
but not proof that such expansion is legal or profitable under the medium
model and strict x87 semantics.

The next Phase-5 mechanism is therefore an exact triangular-loop
peeling/versioning candidate that derives the nested bound, preserves the
zero-trip and final-counter behavior, and prices the complete x87/memory and
code-size tradeoff.  It must be accepted only against an independently
checked answer and a candidate-ABI audit.  No code-generation change is
claimed in this measurement iteration; the focused report completed in
11.2s, and GCC/LLVM remain advisory best-case listings while BCC/WC remain
the medium-model legality and ABI authority.

### 53. Dynamic-quality ratio evidence gate — 2026-09-18

The nbody audit found the quality report still divided two profile-free
ten-trip loop estimates and printed the result as an estimated executed
instruction ratio.  That made the 11.63x/2.66x flat-reference numbers look
more authoritative than their evidence allowed.  The estimates remain in the
per-function report for diagnosis, but structural comparison now withholds a
dynamic ratio whenever either status says it used the ten-iteration fallback.
The console prints the remaining static ratio as `dynamic withheld`; a ratio
is produced only for a fully measured or exact-trip-backed pair.

The regression first observed the false 11.63x ratio and now requires the
withheld status.  It also retains the natural-loop reference and ordinary
symbol-spelling comparison checks (`3 passed`, `0.11s`), and a fresh nbody
report confirms the dynamic labels are withheld.  This advances Phase 1's
measurement trustworthiness; it does not alter nbody code generation or
create a hard target.  GCC/LLVM assembly remains best-case flat-i386
structural evidence only, with BCC/WC medium-model output the ABI and
address-form authority.

### 54. Medium-model default address-form gate — 2026-09-18

The shared `transform.applied()` boundary still supplied `{1,2,4,8}` when a
direct MIR caller omitted CPU form facts, despite every public CPU profile and
both production frontends correctly supplying `{1}`.  That was a target-model
leak: a 16-bit medium-model effective address can combine a base and index but
cannot encode a flat 32-bit scaled-index form.  Such a caller could therefore
make a formula decision using a price for code the emitter cannot legally
produce.

The default is now `{1}`.  The regression first observed the old flat set at
the `Strength` boundary and now proves an omitted profile has exactly the same
legality fact as the default 386 frontend; the focused CPU-profile suite passes
(`10 passed`, `0.34s`).  An explicit profile still owns its own machine-neutral
capacity, costs, and legal-form facts.

A fresh `bench/c/nbody.c --cpu 386 --references` run at the preceding clean
revision generated both Apple Clang and installed `i686-elf-gcc` strict-x87
flat-i386 listings.  qbopt remains 598 bytes / 138 static instructions;
Clang and GCC are 0.45x and 0.49x respectively on normalized static
instructions.  The dynamic comparison is correctly withheld because the
runtime outer-step bound still uses the profile-free loop heuristic.  Reading
the listings confirms the next gap is not an illegal address form: the fixed
four-body triangular pair loop is nested inside a strict floating region.  The
current general CFG cloner deliberately refuses a floating loop with an
internal conditional until it has an edge-by-edge stack-equivalence proof.
That proof, rather than a source-specific nbody unroll, is the next Phase-5
work item.

GCC and LLVM remain best-case flat-i386 structural listings only.  Their
algorithmic loop shape and memory traffic guide the audit; BCC/WC medium-model
listings remain the authority for ABI, segmentation, legal address forms, and
any hard target.

### 55. Self-contained strict-FP CFG cloning — 2026-09-18

The nbody listing investigation reached the real Phase-5 barrier: its exact
four-body outer loop contains the triangular pair loop, so the existing CFG
cloner rejected it solely because that nested conditional performs floating
work.  The old blanket rule was sound but unnecessarily broad.  A new
machine-neutral MIR proof admits a conditional floating CFG only when every
floating SSA value is defined and consumed in its own block, no floating value
crosses a phi or CFG edge, and no raised raw stack-effect operation appears.
Lowering can then allocate independently owned floating regions on each arm;
it never has to reconcile an x87 stack position selected by different cloned
paths.  The earlier cross-edge case remains refused.

The positive and negative regressions were run fail-first.  With the proof,
the fresh nbody candidate expands the six fixed interactions and changes its
listing from 598 bytes / 138 instructions / 13,652 heuristic CFG operations
to 1,095 bytes / 280 instructions / 2,066 heuristic CFG operations.  The
instruction count is now structurally close to the installed i686 GCC listing
(267), but these are not timing claims: the dynamic values still contain the
outer-step profile-free fallback, and GCC remains a flat-i386 advisory
reference.

The first complete C oracle attempt demonstrated a separate compile-time
hazard before reaching DOSBox: large branchy floating candidates can make
strict fixed-point rebuilding disproportionate.  `Peel` now has a general
512-semantic-operation ceiling for conditional floating clones, in addition
to its existing 4,096-operation general CFG ceiling.  A fail-first 513-trip
regression proves the candidate is refused before cloning; the focused clone
and resource suite passes (`3 passed`, `0.21s`).  The small nbody candidate
remains below that bound.  The full DOS C oracle was intentionally stopped
without recording a pass, to retain the agreed test-time budget; it remains
the next correctness gate before a performance or hard-target claim.

This advances Phase 5's guarded exact cloning and Phase 1's listing audit.
GCC/LLVM remain best-case flat-i386 structural listings; BCC/WC medium-model
listings remain the ABI, segment, and legal-address-form authority.

### 56. Transaction-scoped constant-analysis reuse — 2026-09-18

Profiling the accepted nbody floating-CFG candidate found a general optimizer
cost rather than an invalid clone: 63.4 seconds under `cProfile`, with the
dominant work in repeated constant-memory solving (`consts.cells`, `_kills`,
and alias overlap) while successive MIR passes were still examining the same
immutable body.  Re-running the same analysis was not a new proof and did not
improve code; it merely made the full C oracle spend its time compiling.

`consts.reusing()` now establishes a dynamically scoped cache for ordinary
constant requests keyed by the actual immutable body, data-group facts, and
call summaries.  Explicit entry and edge facts remain uncached because they
are mutable proof inputs.  Cached results are copied before return, so a
consumer can retain the existing mutable-dictionary API without contaminating
the next analysis request.  `transform.applied()` owns one such scope, which
covers its normal fixed point and nested peel/unroll candidate evaluation but
cannot leak between compilations.

The fail-first regression proves both properties: an unchanged body solves
once in one scope, and mutation of the first returned map cannot poison the
second (`4 focused checks passed`, `0.14s`).  The real optimized C nbody
compile remains cloned and falls to 19.55 seconds wall time in an unprofiled
run, down from the preceding 63.4-second instrumented diagnostic (the two
figures are not a timing ratio because profiling adds its own overhead).

The full C DOS known-answer oracle was started against this revision and was
still compiling after 2m52s, before it had launched DOSBox.  It was stopped
under the agreed test-time budget and is **not** recorded as a pass.  The
oracle remains the required correctness gate for the new CFG clone; this
iteration establishes only the resource mechanism and its focused regression.
GCC/LLVM remain best-case flat-i386 structural listings, while BCC/WC
medium-model listings remain the ABI and legal-address-form authority.

### 57. C nbody oracle measurement gate — 2026-09-18

The targeted DOS nbody run caught a measurement error before it became an
optimization conclusion.  The first focused test asserted a hand-written
result of `2`; the independent committed corpus oracle in
`bench/c/expected.json` specifies `4774160`.  Fresh OMF emission, LINK, and a
real DOS 386 produce that recorded value both with normal optimization and
with optimization disabled.  Disabling unrolling independently produces the
same result.  The earlier apparent strict-FP clone failure was therefore not
a compiler failure and no valid candidate is withdrawn.

The test now compiles only nbody but reads the same independent expected-answer
source as the full C suite.  It would have caught either an incorrect emitted
result or a future test that selected the wrong corpus expectation; it no
longer duplicates an unverified number.  This is Phase 1 measurement
hardening, not a Phase-5 code-quality claim.  The accepted nbody listing from
iteration 55 remains provisional until its normal performance and complete
suite gates; GCC/LLVM are best-case flat-i386 structural references, while
BCC/WC medium-model output remains the authority for ABI, segments, and legal
addressing.

### 58. Individually bisectable FloatLoop stage — 2026-09-18

The focused validation exposed two historical generic-unroll tests that
prepared FPDEEP through the normal optimizer, then expected a loop to remain.
That preparation is no longer a stable stage boundary: `FloatLoop` runs before
the generic unroller and can legally consume an exact recurrence first.  The
old assertions therefore described an implementation accident rather than a
program property.

`transform.applied()` now accepts `floatloop_=False`, matching the existing
per-pass switches and restoring the stated bisection contract: every MIR pass
can be disabled for a stage comparison.  The regression first requires the
new switch—so it fails on the former API—and proves `only="floatloop"` removes
FPCSE's exact loop only when that pass is enabled.  The generic-unroll tests
now explicitly disable both earlier structural consumers (`FloatLoop` and
`Peel`) when they inspect unrolling mechanics; their public end-state checks
continue to run the normal pipeline.

The affected focused tests pass (`7` unroll checks in `18.49s`), and the
isolated nbody OMF/LINK/DOS oracle remains green from iteration 57.  This is
debuggability and test-boundary work in Phases 1 and 5, not a new performance
result.  GCC/LLVM remain best-case flat-i386 structural references; BCC/WC
medium-model listings remain the ABI and encoding authority.

### 59. Refreshed nbody structural listing audit — 2026-09-18

The current committed `tools/quality.py bench/c/nbody.c --cpu 386
--references` report records qbopt at 1,095 emitted bytes / 280 raw
instructions (274 ABI-normalized instructions).  Apple Clang 21 emits 290
normalized instructions and installed i686 GCC 16.2 emits 263.  This makes
the valid conclusion deliberately narrow: instruction *count* is structurally
near the best-case flat-i386 listings, not that their ABI, bytes, or timing are
matched.

The useful remaining signal is memory traffic.  Against Clang, qbopt has 158
loads / 85 stores versus 74 / 51; against GCC it has 158 / 85 versus 71 / 106.
The quality report attributes qbopt's excess loads (and Clang-relative stores
and branches) first to `lir-lower`, after the machine-neutral MIR stages have
already settled.  Dynamic ratios remain withheld because both sides include
the profile-free natural-loop fallback.  These facts direct the next work to
legal medium-model memory-form selection and post-lowering traffic, not an
ABI-incompatible copy of a flat compiler's stack frame or addressing modes.

The report embeds the `best-case-flat-i386-structural-reference` contract,
source hash, compiler versions, listing paths, and stage attribution.  It is
Phase-1 measurement evidence only; BCC/WC medium-model listings remain the
hard authority before any target or timing claim is registered.

### 60. x87 rounded-cell equivalence — 2026-09-18

The stage trace behind the remaining nbody memory gap showed that the issue
was not an allocator preference for reloading a value with many readers.
After MIR cloning, each `fld` result has exactly one reader, even when the
same rounded frame temporary is loaded repeatedly.  The existing x87 stack
allocator can retain one SSA value across consumers, but had no way to know
that those separately named direct loads read the same current scalar cell.

`FloatAlloc` now derives a region-local equivalence map for direct `fld` and
`fild` reads of a stable 32- or 64-bit cell.  A later load joins the first
value only while no opaque operation, address redefinition, possibly-aliasing
write, or volatile access crosses it.  The allocator then uses its ordinary
live-value, duplication, spill, and memory-operand logic; it does not add a
new LIR optimization tier or expose a machine fact to MIR.  Extended 80-bit
loads remain distinct because they are the allocator's precision-preserving
spill representation, and the existing stack tests demonstrate that they
must not be conflated.

The primary regression was written fail-first: two independently named `fld`
results from one unchanged rounded frame cell previously emitted two direct
memory loads, and now emit one while preserving both arithmetic results.  The
same commit adds negative checks for an intervening cell write and a volatile
read.  The float allocator suite passes (`75 passed`, `0.11s`) and the
independent DOS nbody oracle still returns `4774160` (`1 passed`, `13.81s`).

The provisional fresh nbody quality listing is 1,071 bytes / 280 raw
instructions / 146 loads / 85 stores / weighted cost 4,625, versus 1,095 /
280 / 158 / 85 / 4,721 before this iteration.  This is a static candidate
measurement, not a runtime target or a comparison against an ABI-incompatible
listing.  GCC/LLVM remain best-case flat-i386 structural references for loop
and traffic audits; BCC/WC medium-model output remains the authority for
segments, legal address forms, ABI costs, and hard targets.

### 61. Profile-aware x87 allocation boundary — 2026-09-18

The CPU-profile audit found one remaining propagation hole: `flow.machine()`
passed the selected immutable profile to general register allocation but built
`FloatAlloc` with no profile.  That left every future x87 keep-versus-reload,
spill, and stack-form decision exposed to an implicit 386 policy even when a
caller had selected P5, P6, K5, K6, K7, or Core.

`FloatAlloc` and its direct `allocated()` API now resolve and retain the same
`cpu.Profile` as the rest of the machine pipeline; direct callers retain the
public default of `386`.  The current x87 policy is deliberately unchanged:
this is the explicit architecture boundary needed before a costed choice can
be introduced, not a fabricated performance change.  The fail-first pipeline
regression previously found no `FloatAlloc.cpu`; it now proves that the P5
profile object is the exact immutable object received by both allocators.

The focused CPU-profile suite passes (`10 passed`, `0.19s`) and the x87
allocator suite passes (`75 passed`, `0.07s`).  This advances Phase 1's
per-CPU plumbing and Phase 4's x87 allocation work.  GCC/LLVM remain
best-case flat-i386 structural references; BCC/WC medium-model output remains
the ABI and legal-form authority.

### 62. Costed x87 memory-form selection — 2026-09-18

With the complete CPU profile at the x87 allocator, direct floating arithmetic
no longer chooses a memory operand solely because one is encodable.  For an
available cell value, `FloatAlloc` compares the profile's arithmetic-with-memory
form to the explicit alternative (`fld` plus register arithmetic), including
the distinct add/subtract, multiply, and divide cost families.  Missing form
prices intentionally preserve the old legal memory-folding behavior: unknown
data is never interpreted as a free instruction.

The regression was fail-first with a synthetic but complete profile whose
memory multiply costs 99 while `fld + fmul` costs 2; the old allocator still
emitted `fmul [cell]`, while the new one materializes the cell and uses a
stack-register form with the same result.  A companion matrix verifies that
each of the eight public profiles emits exactly the form its own audited table
chooses.  Their present tables retain the established memory fold, so this
does not manufacture a benchmark improvement merely by retuning a rule.

The cost-form checks pass (`9 passed`, `0.08s`), the complete float allocator
suite passes (`84 passed`, `0.08s`), CPU-profile suite passes (`10 passed`,
`0.18s`), and the independent DOS nbody oracle remains `4774160` (`1 passed`,
`13.38s`).  This advances Phase 4's target- and pressure-driven x87
allocation.  GCC/LLVM remain best-case flat-i386 structural references;
BCC/WC medium-model output remains the ABI and legal-form authority.

### 63. Relocatable-address rematerialization — 2026-09-18

The spill path already rebuilt constants, BP-relative frame addresses,
conversions, stable loads, and established frame homes.  It nevertheless
assigned a separate frame slot to a direct `lea` of a SEGDEF or EXTDEF symbol.
That address has no dynamic input: retaining it in a slot costs both a store
and a reload, whereas recreating the `lea` at a use costs only the relocation
the fresh OMF emitter already knows how to own.

`spiller._addresses()` now admits precisely those direct segment and external
addresses, as well as the existing BP-relative frame form.  It still rejects
general register-address expressions, FAR selector-dependent addresses, and
indexed forms: those require a value-availability proof rather than this
local rematerialization rule.  The test was run fail-first for both SEGDEF and
EXTDEF: each formerly allocated a private spill slot.  It now proves no slot
is allocated, the inserted `lea` carries the same address operand, and direct
fresh-OMF encoding emits the expected offset relocation for `_descriptor`.

The focused spill regression passes (`2 passed`, `0.05s`).  This is a Phase-4
local-rematerialization increment, not a timing or hard-target claim.  GCC
and LLVM remain best-case flat-i386 structural listings only; BCC/WC
medium-model output remains the ABI, segmentation, and legal-addressing
authority.

### 64. Direct low-half extraction — 2026-09-18

The mixed-width aggregate-copy audit also exposed a general lowering cost:
MIR's exact `EXTRACT(v32, 0)` was expanded as `push dword` followed by two
word pops, even though the low word of every 32-bit general register is a
directly encodable operand.  This was not an SROA reason to split aggregate
copies indiscriminately—the high-half form still needs a separately costed
representation—but a target-lowering gap shared by long-pair and scalarized
code.

Lowering now spells the low-half view as one word `mov`, retaining the same
abstract source value at word width.  No MIR pass names a register and no
allocation choice is made here; allocation resolves the value's physical root
and the emitter uses its AX/BX/CX/DX low-word view.  The focused regression
was run fail-first: it formerly emitted `push`, `pop`, `pop`; it now proves
the single move's operands, definition, and use are exact.  The existing
high-half regression continues to prove that its stack transfer leaves flags
untouched.

This advances the shared Phase-3 scalarization substrate and Phase-4 pressure
work without registering a performance target.  GCC and LLVM remain best-case
flat-i386 structural listings only; BCC/WC medium-model output remains the
ABI, segmentation, and legal-addressing authority.

### 65. Reachable x87-edge allocation — 2026-09-18

The first CFG allocation audit found an erroneous kind of join before the
larger stack-state work: `FloatAlloc` counted every syntactic predecessor of
a block, including a block unreachable from the procedure entry.  That made
one executed floating edge look like a join, so an otherwise unchanged x87
stack was either bridged through an 80-bit frame cell or refused when no owned
frame was available.

The allocator now computes reachability from the LIR body entry and builds
its predecessor and straight-edge facts only from executable edges.  Dead
blocks remain in the body for layout and ordinary emission; this is solely an
x87 allocation fact.  The fail-first regression adds an unreachable incoming
edge to a live floating block: it formerly raised `floating region crossing
requires an owned frame`, and now retains the ordinary straight stack path.
The same matrix continues to refuse a reachable fork and a value unavailable
at the entry, so this does not claim general stack reconciliation across real
joins.

This advances Phase 4's global-x87 prerequisite but leaves canonical stack
states, edge shuffles, and live values at genuine CFG joins open.  GCC and
LLVM remain best-case flat-i386 structural listings only; BCC/WC medium-model
output remains the ABI, segmentation, and legal-addressing authority.

### 66. Conservative profile-aware machine scheduling — 2026-09-18

The post-allocation pipeline had physical CSE and DCE but did not consume the
CPU profile's dependency-latency data at all.  `schedule.py` now performs a
deterministic list schedule for the narrow region it can prove complete:
allocated integer operations whose operands are only general registers and
non-relocated immediates.  Physical register *and flag* lanes form RAW, WAR,
and WAW dependencies.  Memory, segment and stack state, x87, calls, control
transfer, opaque/source-map boundaries, relocations, and allocator-owned
instructions terminate a scheduling window rather than being approximated.

On out-of-order profiles, an independent operation can therefore fill a
measured producer-to-consumer gap: the fail-first P6 regression changes
`imul; add; mov` to `imul; mov; add`.  It also proves that flag writers retain
their original order and a potentially trapping memory load prevents any
crossing.  386 and 486 keep their established source order.  P5 is deliberately
unchanged: its U/V pairing rules are not represented by `issue_width`, so
using that scalar as if it were a pairing model would manufacture a claim the
profile cannot justify.  The pipeline regression proves the scheduler receives
the same immutable profile object as the general and x87 allocators.

Focused checks pass (`14 passed`, `0.19s`); Tier 1 passes (`179 passed`,
`31 deselected`, `0.96s`).  This is Phase 7's first safe scheduling slice,
not a performance target or a claim of P5/later-core issue-model completeness.
GCC/LLVM listings remain best-case flat-i386 structural references for
dependency and expression shape; BCC/WC medium-model output remains the
authority for ABI, segment state, legal addresses, and any hard target.

### 67. Audited Pentium U/V pairing — 2026-09-18

The available GCC Pentium scheduling description supplies a necessary fact
that `issue_width = 2` did not: an original non-MMX P5 has distinct U and V
pipes.  We used that as an audit reference, not as transplanted code.  The
immutable CPU profile now explicitly marks P5 pairing, without changing any
existing positional caller shape.  The other seven profiles remain false.

The scheduler recognizes only its existing safe register/immediate window:
operand/address-size prefixes and immediate shifts are U-only; immediate
forms and `imul` are unpairable; remaining plain register ALU/move forms are
U/V eligible.  It puts an eligible U-slot form before an independent U/V form
when that creates a pair, while the same lane RAW/WAR/WAW graph prevents a
partial-register overlap, flags, or register dependency from crossing.  The
fail-first regression recreates the listing symptom `mov di,si; mov eax,ecx`:
the prefixed 32-bit move was formerly second and could not pair; P5 now emits
it before the independent word copy.  Existing boundary checks continue to
keep all memory, segment, x87, call, branch, relocation, and opaque work out
of the pairing model.

Focused scheduler/profile tests pass (`15 passed`, `0.19s`); Tier 1 passes
(`180 passed`, `31 deselected`, `1.05s`).  This advances Phase 7 but does not
claim memory pairing, complex P5 forms, x87 overlap, or a final per-CPU
throughput model.  GCC/LLVM listings remain best-case flat-i386 structural
references; BCC/WC medium-model output remains the ABI, segment, legal-form,
and hard-target authority.
