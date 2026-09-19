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
| Per-CPU measurement | in progress | CPU profiles distinguish native medium-model addressing from the complete costed secondary 67h form, explicitly price x87 stack exchange, LES/LFS/LGS complete far-pointer loads, and read-only memory comparisons separately from read/modify/write ALU forms, and carry the complete-peel iteration budget; the C corpus, paired BASIC/C known-answer gates, an exact final-allocated scalar parity gate, static and frequency-weighted structural metrics, static and CFG-frequency-weighted per-CPU cost rankings, reference listings, and exact-count preservation across recurrence rewinds, loop rotation, and zero-byte-header threading exist; audited targets and runtime profiles remain. |
| MIR/LIR provenance and fresh OMF | complete in production | allocated LIR emits directly with external source maps/allocation hints; unreachable and reachable passage blocks retain inert decoded-byte owners after control simplification; the remaining compatibility views are test-only and cannot route a compilation through record rewriting. |
| SROA and scalar promotion | partial | precise SSA pointer identity, including address-of canonical frontend cells, and scalar TBAA for both direct and indirect lvalues now refine coarse frontend operand annotations before observer analysis, DSE, GVN, LICM, packed-pointer splitting, and SROA, while exact-address incompatible views retain the conservative union rule; packed 16:16 dereferences are normalized into independent offset/selector SSA values before SROA; fixed/disjoint, singleton-indexed, and single-generation affine dynamic-allocation leaves promote; direct and exact-near-pointer C aggregate copies expand into exact leaves, and structural candidates transact leaves made singleton by scalar convergence with finite-capacity pressure pricing; far aggregate copies, overlap, volatile, general indexed copies and broader aggregate decomposition remain. |
| Pressure-aware allocation | partial | spilling, slot colouring, byte RMW selection, local/block/region splitting, constrained native-address occurrence splitting, dying-base indexed-form unfolding, direct composition of spilled frame addresses into their sole memory use, and local constant, frame, relocatable-address, and provenance-disjoint incoming-argument rematerialization exist; GVN now retains store-crossing providers under existing pressure only when the selected target has a secondary address form cheaper than the reload/spill alternative; typed far-pointer loads are selected before allocation only when a virtual address owner would otherwise be lost, while fixed addresses retain independent rematerialization and late LES/LFS/LGS selection; x87 allocation composes complete target-priced reread and retained-home candidates independently at empty-stack regions after all shuffles are materialized; global integer splitting/rematerialization, CFG-frequency weighting within a nonempty x87 region, and broader global x87 allocation remain. |
| Loop optimization | partial | exact pre/post-tested recurrences and symbolic sentinels, target-priced exact nested-recurrence rewind, complete nested-initializer LICM, dead-control countdowns with zero-trip guards, complete-affine spill/recompute pricing, precise-volatile-aware LICM, costed 67h addressing before spill/recompute, specialization, rotation, peeling and exact unrolling with exact-trip-amortized growth plus a pre-folding complete-sequence/pressure proof and bounded public defaults, removal of impossible zero-trip edges for positive exact counts, post-specialization associative integer constant composition, and machine-neutral whole-range pressure forecasting exist; versioning, partial unrolling, constraint-complete candidate-set forecasting, and compile-time candidate memoization remain. |
| Whole-module optimization | partial | summaries, direct private readonly-effect and no-return proofs (including closed recursive SCCs in C and object paths), constant returns, a direct-call IPSCCP fixed point for source and MIR-derived actuals, including costed per-call cloning when other callers stay dynamic, post-inline constant folding through phi edges and linear corridors, private immutable numeric-data initializer facts, private procedure DCE, and conservative private-data DCE exist; recursive/full IPSCCP and broader global-elimination proofs remain. |
| Post-allocation quality | partial | copy propagation, machine CSE/DCE, shared final block placement/threading/fall-through elision and fresh-tail sharing, plus byte-neutral source-unowned terminal-return duplication, epilogue-aware dead-register analysis, direct one-use memory comparison folding including self-addressed loads, pre-allocation selection of those comparison operands, dead-register frame-copy shuttles, dying-input commutative result transfer, dead-upper-lane register-move narrowing, synthetic high-word reload narrowing, target-priced 67h LEA selection and repeated-base promotion including exact byte, prefix, length-changing-prefix and partial-register costs, conservative later-core/P5 scheduling of register work and direct frame LEAs, and partial-register edge delays exist; source-map-aware decoded-tail sharing, x87/segment scheduling, memory pairing, and full issue modelling remain. |

## Iteration log

### 135. Final-code frontend parity and aggregate scalar closure — 2026-09-19

The previous paired gate established only that equivalent BASIC and C programs
returned `1789`.  That was not a code-generation parity claim: a frontend
could still leave a loop, helper call, spill, or wider operation behind and
pass the same oracle.  A second scalar pair now compares the complete final
allocated operation stream.  The only excluded instructions are BASIC's
named `B$ENRA`/`B$EXSA` frame-entry and frame-exit ABI boundaries.  No generic
cleanup or assembly normalization is performed, so any retained loop,
`B$MUI4`, spill, recomputation, or operand-width difference fails.

Before this iteration the BASIC scalar form retained its eight-iteration
loop and runtime multiply, while the C side had already folded the answer but
materialized it as a dword and split its high half.  The shared final form is
now exactly:

```
mov ax, 1789
mov dx, 0
retf
```

The BASIC call contracts now distinguish registers conservatively inspected
on hidden runtime transfer paths from operands read on the explicit returning
edge.  This removes false data dependencies without weakening the all-path
contract.  Physical liveness then narrows a register-only dword move when all
upper lanes are dead; memory and volatile accesses remain untouched.  The
fail-first contract, MIR, post-allocation, and paired-final-LIR regressions
cover both parts.

The aggregate pair exposed two independent shared optimizer gaps.  Exact
pointer facts may now refine a conservative same-object indexed lane, and
one-generation dynamic allocations acquire scalar leaves by normalizing
single-root affine address expressions.  Signed extension is deliberately
not address preserving: its dedicated regression prevents equating it with
the 386 zero-extended address.  Once the loads fold, observer analysis treats
the unescaped allocation as private only after no body load remains, allowing
DSE to remove its now-unobserved contents without deleting a store before a
later scalarized load.  Positive exact-count full unrolling also deletes the
impossible zero-trip edge and its exit-phi input.  The C aggregate therefore
falls from frame construction, array traffic, multiply/address work, and a
retained entry guard to the same two constant return moves above.

BASIC's aggregate still has language/ABI work that the C fixed stack array
does not: `B$DDIM`/`B$ERAS`, the descriptor, frame entry/exit, and preservation
of the result across erasure.  Those differences are kept explicit rather
than normalized away.  Its shared computation now contains no allocation
loads or stores, multiply helper, arithmetic loop, or conditional branch and
produces the same literal `1789`; the scalar pair is the exact-code gate for
the case with no language-specific storage obligation.

Finally, stage dumps located a fresh-OMF refusal after successful unrolling:
final jump threading treated a reachable block containing only decoded-byte
anchors as a transparent passage, redirected around it, and stranded 53
source bytes.  A passage is now removable only when every instruction is
source-unowned.  Separate fail-first regressions cover source-owned
fall-through jumps, unreachable inert owners, and reachable inert passages.
This advances Phases 1, 3, 5, and 7; it does not claim whole-corpus frontend
identity or the final per-CPU GCC/LLVM target gates.

### 134. Canonical address publication across BYREF calls — 2026-09-19

The QB compatibility program Q45P04 first passed uninitialized frame slots to
`ADDLONG&`: optimized MIR had replaced the stores of `100000` and `23` with
`NOTHING` while retaining their addresses and the call.  The QB HIR already
described both pointee read/write effects, so disabling DSE in that frontend
would have hidden a shared alias/observer defect rather than fixed it.

The fail-first shared-MIR regression recreates the complete symptom.  An
`ADDRESS` operation takes a canonical frame `Cell`, publishes its result to a
call with an explicit pointee footprint, and asks DSE whether the preceding
store is observable.  The first bad analysis was pointer provenance: the
shared points-to solver recognized `FrameAddress` but not the equivalent
canonical-cell spelling.  Address-of now inherits that cell's bounded object
provenance.  This is an object-identity rule available to every frontend, not
a QB call or temporary exception.

Recovering the provenance exposed the second error.  Observer analysis found
the exact published frame object, but when that made its private-object test
false it fell through to the older raw BP-offset rule and made the same object
private again.  Canonical identity is now a complete answer in both
directions: a bounded current-frame object is private exactly when none of its
canonical identities has been published.  Unknown direct-address publication
retains the existing conservative all-frame fallback.

The fail-first regression now passes with the surrounding alias/private-frame
suite (`31 passed`).  In the QB integration worktree, the former strict xfail
passes normally, frontend emission reports zero failures, and the exact qb45
Q45P04 linked DOS verdict reports zero failures.  This advances the shared
alias/observer part of Phases 3 and 6; it does not add a frontend-specific DSE
switch or weaken elimination of genuinely unobservable frame stores.

### 133. Paired BASIC/C parity gate and allocation closure — 2026-09-19

The first committed paired corpus expresses the same aggregate, loop, signed
extension, multiplication, and 32-bit accumulation in BASIC and C and binds
both programs to one independently calculated result, `1789`.  Its bounded
host gate requires both frontends to finish the shared optimizer, allocator,
layout, and fresh OMF writer.  All three parity gates remain in Tier 2 because
the paired host compilation alone costs 1.88 seconds; the fail-first mechanism
regressions stay in Tier 1.  The runtime gates execute both emitted objects on
the DOS 386 path and compare each with that same oracle.  Passing this corpus
establishes semantic parity for the covered mechanism; it does not claim that
all programs or source-language corner cases are now equivalent.

The initial BASIC object failed allocation while the C form emitted.  Stage
dumps located the first problem after spill selection: a spilled exact frame
address was rematerialized as `lea temporary,[bp-38]` solely to feed
`mov value,[temporary+10]`.  All four legal 16-bit address registers were
occupied, although `[bp-28]` was directly encodable.  The spiller now composes
only an unrelocated, unscaled, near BP-relative address and literal displacement
into the memory cell.  Relocated, selected, far, indexed, constrained, and
mixed data/address uses retain ordinary rematerialization.  The fail-first
regression records the original unallocatable one-instruction range.

That exposed a separate provenance defect at fresh layout.  No-return pruning
removed a dead jump after `B$STOP` from the same block, including its only
source identity, so layout reported eight unowned bytes before the following
procedure.  Successor blocks were already normalized through the shared inert
ownership mechanism; same-block tails now use that mechanism too.  Source-free
work still disappears, while source-owning occurrences become effect-free
`NOTHING` markers.  Its fail-first regression checks identity, lack of control
effect, and the removed CFG edge rather than the layout patch.

The real optimized BASIC baseline/rewrite and optimized C object all produce
`1789`.  The three parity gates pass in 3.36 seconds, and the focused ownership,
allocation, and emission checks pass.  This advances the Phase 1 measurement
corpus and Phase 4 allocation closure without declaring general frontend
equivalence.  GCC and LLVM remain best-case flat-i386 structural references;
BCC/WC and the executed medium-model objects remain authoritative for ABI,
segments, legal forms, and answers.

### 132. Carry indirect TBAA through pressure-priced scaled addressing — 2026-09-19

The redundant operations in `indexed._lru_use` were not frontend residue.
The final shared MIR loaded `sc->bnext` three times because a store through
`bprev` was conservatively treated as able to overwrite the pointer field.
Both accesses already carried their C scalar access classes.  GCC's
`tree-ssa-alias.cc` applies strict-alias disambiguation after its pointer and
declaration checks, and LLVM's TBAA access tags likewise describe indirect
loads and stores.  The common alias query now applies incompatible scalar
TBAA to indirect lvalues too.  An incompatible view at the same explicit
address remains may-alias for the union case, and character/untyped accesses
remain fully conservative.  A fail-first MemorySSA/GVN regression records the
actual pointer-field/short-store symptom.

Simply retaining the pointer exposed the next issue: allocation spilled its
offset because the native medium-model spelling also carried `b * 2` as a
seventh value.  The GCC and Clang references carry the original index and use
scaled addresses instead.  Acyclic address selection now recognizes every
pure `base + index * {2,4,8}` whose uses are encodable cells, proves the index
non-negative from dominating branch edges, and tries the profile's secondary
32-bit address form before a spill or recomputation.  Untyped/BASIC addresses
still require the complete scaled range to fit 16 bits.  A typed C lvalue may
use the wider form over its defined object range; an out-of-segment access was
already undefined.  Existing word definitions are promoted in place: a plain
load selects `movzx r32,m16`, while a complete far-pointer load keeps
LES/LFS/LGS and zero-extends its offset through a short-lived temporary.  The
scaled product and address additions disappear before allocation.

Legality is not profitability.  The first complete CPU run showed that always
selecting 67h improved 386/K5/K6/K7/Core but regressed 486/P5/P6.  The immutable
address form now ranks its extension and per-use cost against both a direct
reload and the native scale/address/move plus displaced-value store.  Under an
existing MIR pressure wave, GVN also keeps a source reload when retaining its
provider would cross a store and the chosen secondary form loses that price
comparison.  Store-free reuse remains enabled.  This reproduces the profitable
old P5/P6 shape without weakening the new alias fact, and records the decision
in fast pressure and per-profile regressions.

One more lowering issue was found on the same path.  A one-use load followed
by a comparison was folded only after allocation, so a value that emitted no
instruction still competed for a register.  The legal memory comparison is
now selected before allocation, across only inert zero-byte anchors; the late
fold remains for shapes created afterwards.  Its fail-first regression asserts
that the virtual range is gone as well as the final load instruction.

On 386, `_lru_use` improves from iteration 131's 158 bytes, 55 instructions,
241 static weighted units, 103.4375 dynamic weighted units and 22.21875
estimated executed instructions to 144 bytes, 44 instructions, 210,
101.71875 and 20.90625 respectively.  Static/dynamic loads fall from 14/5.28125
to 12/5.1875, with no spill, reload or rematerialization marker.  K5 changes
64/27.1875 to 42/23.0625, K6 66/27.6875 to 45/23.8125, K7 87/36.1875 to
59/30.5625, and Core 114/59.5 to 98/56.15625.  The costlier secondary form is
not selected on 486, P5 or P6; their prior static/dynamic costs remain exactly
163/70.53125, 113/42.625 and 112/56.9375, with the same 158-byte no-spill
shape.  Current listings and stage dumps are under
`build/quality/iter132-indexed-final`.

### 131. Fold return-edge memory tests with epilogue-aware liveness — 2026-09-19

The GCC and Clang listings test qc-port's `bprev[b]`, `bnext[b]`, and
`lhead[c]` directly in memory.  The shared MIR already represented each as a
one-use load followed by a comparison, but the allocated output materialized
the value and then tested the register.  This was not frontend inefficiency
surviving MIR: all MIR and allocated-LIR dumps were identical before and after
the fix, and `Peephole` was the first changed stage.

The late fold already knew how to turn `mov r,[m]; cmp r,x` into `cmp [m],x`,
but physical liveness treated SI and DI as semantic inputs to every complete C
return.  Their ABI values are actually restored from the prologue's stack
saves by the generated epilogue; transient body values in those registers are
dead.  Complete returns now retain only explicit results plus BP/SP and the
architectural segment state.  The MASM emitter continues deriving SI/DI
push/pop preservation from surviving body uses, so removing a dead final write
also removes an unnecessary save when no other use remains.

A second fail-first case records the related machine rule.  A load may use its
destination register to address its own cell, as in `mov di,[bx+di]`.  Folding
its sole comparison leaves the old DI address in place instead of overwriting
it with the loaded word; that is sound precisely when the destination is dead
after the comparison.  Read/modify/write folding retains the stricter refusal
because its later store still needs the original dynamic address.  Both rules
are general physical-liveness and operand-legality facts, independent of the
source frontend and symbol names.

The first measurement exposed an instrument defect and received a fail-first
regression of its own.  The scorer classified `cmp [mem],imm` and
`test [mem],imm` as memory-writing ALU operations.  They only read memory, and
the allocator's fold pricing already used the read-only form.  Unifying the
classification removes the impossible report in which one fewer instruction
and byte cost more.  The parent code shape was remeasured under the corrected
instrument before comparing results; its numbers are unchanged and are stored
under `build/quality/iter131-indexed-corrected-before`.

On 386, `_lru_use` falls from 162 to 158 bytes, 58 to 55 instructions, and
23.09375 to 22.21875 estimated executed instructions.  Static and dynamic
weighted costs remain tied at 241 and 103.4375: the 386 read-only memory
comparison costs exactly the separate load/register-test pair, so the smaller
form wins without claiming a cycle saving.  The advisory dynamic ratio falls
from 1.42x to 1.37x against Clang 21 and from 1.18x to 1.14x against i686 GCC
16.2.  The raw listing now contains the same structural forms as both
references: `cmp word ptr gs:[bx+si],0`, `cmp word ptr es:[bx+di],0`, and a
direct comparison of the inline list head.

Every CPU emits 158 bytes and three fewer static instructions.  Corrected
before/after static weighted costs are: 386 241/241, 486 163/163, P5 113/113,
P6 122/112, K5 68/64, K6 70/66, K7 96/87, and Core 127/114.  Dynamic weighted
cost is never worse and improves on K7 from 37.0625 to 36.1875 and Core from
60.375 to 59.5; estimated executed instructions fall by 0.875 on every target.
Current listings and stage dumps are under `build/quality/iter131-indexed-cpus`,
with fresh GCC and Clang references under
`build/quality/iter131-indexed-final386`.  The real linked medium-model run
still maps runtime input `block=1` to the independent checksum 60.  Eleven
focused regressions pass in 0.09 seconds, that DOS oracle in 2.37 seconds, and
Tier 1 passes 281 tests with 31 deselected in 1.15 seconds.

### 130. Split constrained address occurrences, then price 67h — 2026-09-19

An inefficient frontend spelling is not an excuse for inefficient output.
After iteration 129, `indexed._lru_use` already had one semantic SSA value for
its stable list owner, but native 16-bit memory operands confined that value's
entire live range to BX/SI/DI.  Under pressure, allocation reconstructed the
same owner at each access.  MIR and pre-allocation LIR were already canonical;
`lir-regalloc` was the first stage to introduce the redundant traffic, so this
iteration changes no C, BC, or QB frontend rule.

The allocator now treats the narrow address class as a constraint on each
memory occurrence.  A value used only as a repeated base or index receives a
short coalescible copy at every constrained instruction and may otherwise live
in any general register.  Retained stable bases are considered in acyclic code
as well as natural loops, and commutative native `[base+index]` roles reserve
the unique BX side for the transient value.  Mixed ordinary, selector, grouped
or fixed-register uses refuse the split; a fail-first selector/base-alias
regression proves that no partially renamed memory operand can escape.

After physical allocation, the common peephole may replace several allocator
copies with one zero-extension and 67h addressing.  It recognizes only the
short copies allocation inserted, validates every consumer, compares exact
encoded byte totals, and uses the selected CPU's complete cost.  The latter
includes ordinary prefix cost, P6/Core length-changing-prefix stalls and the
16-to-32-bit partial-register transition.  A fail-first per-CPU regression
records the measurement defect: the incomplete model selected the smaller
67h form on P6/Core even while the authoritative scorer reported it slower.
The corrected model keeps native copies there and selects 67h on targets where
the complete price wins.  This is the requested ordering: 67h is considered
before a spill or recomputation, but is not assumed free.

On 386, `_lru_use` improves from 170 to 162 bytes, 62 to 58 instructions,
279 to 241 static weighted units, 111.4375 to 103.4375 dynamic weighted units,
23.71875 to 23.09375 estimated executed instructions, and 7.40625 to 5.28125
estimated loads.  Regalloc spill-reload markers fall from twelve to zero; the
peephole turns the repeated native copies into one `movzx edx,dx` and addresses
the far fields through EDX.  The advisory dynamic ratio falls from 1.46x to
1.42x against Clang 21 and from 1.22x to 1.18x against i686 GCC 16.2.

The complete CPU matrix reduces code size from 170 to 162 bytes everywhere.
Static/dynamic weighted cost changes are: 386 279/111.4375 to 241/103.4375,
486 167/71.4375 to 163/70.53125, P5 117/43.53125 to 113/42.625, P6
130/61.1875 to 122/56.9375, K5 78/30.65625 to 68/27.1875, K6 78/30.65625
to 70/27.6875, K7 108/41.28125 to 96/37.0625, and Core 141/65.875 to
127/60.375.  The explicit audited tradeoff is that 486, P5, P6 and Core keep
native short copies, raising their static count from 62 to 63--64 and estimated
executed count by less than one instruction while reducing bytes, memory
traffic and their own target-weighted execution cost.  Raw listings and stage
dumps are under `build/quality/iter130-indexed-cpus-final`.  The nine focused
regressions pass in 3.37 seconds, the linked DOS checksum oracle in 5.17
seconds, and Tier 1 passes 278 tests with 31 deselected in 3.37 seconds.

### 129. Defer lossless far-load selection until physical allocation — 2026-09-19

Frontend-independent cleanup requires selecting a representation at the stage
that has enough information to make it profitable.  Iteration 128 correctly
recomposed typed far-pointer fields whose virtual base and selector would be
reconstructed separately by allocation.  Applying the same early selection to
a fixed BP-relative parameter was counterproductive, however: it fixed the
selector in ES before allocation, introduced an ES-to-FS copy, and forced the
offset into a new frame spill slot.  The final physical peephole could already
have selected `lfs bx,dword ptr [bp+6]` directly.

The fail-first regression records the general stage boundary rather than the
one function: fixed memory addresses must remain as independent semantic values
until allocation, because their exact address cannot be lost and either half
may be rematerialized independently.  A memory operand with a virtual base,
selector, or index is fused before allocation, because reconstructing those
components independently would destroy the shared effective address.  The
existing type, adjacency, volatility, and dependency proofs remain mandatory.
The C integration regression additionally rejects the frame growth and spill
slot caused by early fixed-address fusion.

On the stable 386 `indexed._lru_use` measurement, this iteration improves the
iteration-128 result from 178 to 170 bytes, 66 to 62 instructions, 295 to 279
weighted units, and 26.21875 to 23.71875 estimated executed instructions.  The
new spill store and its frame slot disappear.  Relative to the pre-recomposition
baseline, the complete change is 239 to 170 bytes and 81 to 62 instructions.
The Clang advisory dynamic ratio falls from 1.62x to 1.46x and the i686 GCC
ratio from 1.34x to 1.22x.  Pre-allocation LIR now deliberately retains the
fixed parameter halves; allocated LIR is the first stage at which they become
one `LFS`, while virtual far-struct fields remain complete loads before that
point.  Listings and reports are under
`build/quality/iter129-{indexed,lru}-after`.

A committed qc-port-shaped C benchmark now drives the same dynamic far-array
and list update through the Watcom frontend, shared optimizer, fresh OMF
emitter, DOS linker, and a real 386 run.  Its input is supplied at runtime so
GCC and Clang cannot replace the computation with a known answer; the linked
program returns the independently derived checksum 60.  The two focused
far-load tests pass in 0.03 seconds, the recorded frontend regression in 1.29
seconds, and the linked DOS oracle in 2.96 seconds.  This iteration changes no
frontend cleanup rule: both verbose and already-canonical input reach the same
shared selection machinery.

### 128. Recompose typed far pointers before allocation — 2026-09-19

Frontend verbosity must not survive merely because its address owner is
dynamic.  `indexed.lru_use`, the C analogue of qc-port's cache-list update,
entered shared MIR with each far-pointer field split into adjacent offset and
selector word loads.  The first common fixed point removed sixty redundant
logical loads, but lowering refused to recombine the surviving typed halves
because the containing far struct had no named allocation provenance.
Allocation then rematerialized the struct base independently for both words,
spilled pointer offsets, and turned 47 pre-allocation instructions into 81.

The fail-first unit regression records the actual semantic proof: both words
carry the C `pointer4` access type and have the same exact effective address
apart from the required two-byte displacement.  Allocation provenance answers
alias questions; it is not necessary to prove that these two typed halves are
one value.  Complete far-load selection now uses the type plus exact address
identity, retains the volatile/address-dependency guards, and no longer asks
the frontend to manufacture an allocation identity for a dynamic field.  An
integration regression compiles the recorded `indexed` stream and requires
complete LES/LFS/LGS field loads with no independent selector-half loads.

The measurement exposed a second issue and received its own fail-first
regression: LFS and LGS were reported as unknown even though they have the same
complete far-pointer load cost class as LES.  All three selector spellings are
now explicitly classified as that audited form; no emitted work is silently
free or unpriced.

On the stable 386 run, `_lru_use` falls from 239 to 178 bytes, 81 to 66
instructions, 355 to 295 weighted units, 43 to 24 static loads, 19 to 10 spill
reload markers, three to one spill stores, and fifteen to zero
rematerializations.  Estimated executed work falls from 29.34375 to 26.21875
instructions and from 13 to 7.40625 loads.  `lir-lower` is the first changed
stage: the seven pointer pairs are already seven complete loads before
allocation.  The raw reports and listings are under
`build/quality/iter128-indexed-{before,after}`.

Clang 21's advisory flat-i386 listing estimates 16.21875 executed
instructions; GCC 16.2 estimates 19.5.  The medium-model result is therefore
1.62x and 1.34x respectively, down from 1.81x and 1.50x, with selector traffic
and restricted 16-bit addressing still accounting for part of the gap.  These
references remain structural best cases, not ABI-equivalent hard targets.
The remaining repeated owner reload/spill decision is allocator work; it is
not delegated to either frontend, and 67h remains the costed secondary form
before a genuine spill or recomputation when a native retained base is not the
cheaper complete allocation.

The four focused lowering/pricing cases pass in 0.12 seconds and the recorded
C integration regression passes in 3.27 seconds.  That integration compile is
Tier 2: while it was temporarily in the fast manifest, Tier 1 passed all 275
collected tests but took 6.31 seconds, so it was moved back out rather than
quietly breaking the 1--3 second development-tier contract.  The full linked C
corpus gate was stopped without a result after 170 seconds of optimizer
candidate construction to keep aggregate test execution within the explicit
10% wall-clock cap; it reported no failure before interruption.

### 127. Compose x87 candidates at semantic region boundaries — 2026-09-19

Frontend verbosity must disappear in the common pipeline.  A source frontend
may spell the same rounded floating value as repeated loads and stores; once
that value reaches shared LIR, an unrelated source region must not determine
whether those repetitions survive.  The previous complete-candidate safeguard
still made one all-body choice, however.  In a function with an x87-empty call
or barrier, an expensive retained form after the boundary could reject a
profitable retained form before it.

The fail-first regression contains exactly those two independent regions.  The
first has one rounded home with three consumers and can replace two memory
reads without adding an instruction.  The second has nbody's strict-order
shape: retaining its common value would require an expensive 80387 exchange,
so three ordinary reads are cheaper.  Before this iteration the whole-body
choice selected the reread form everywhere and the first home appeared three
times; the regression now requires one first-region reference, three
second-region references, no `fxch`, and executes the resulting x87 stream over
concrete values to prove all six stored answers and an empty final stack.

Both complete allocations now carry region labels established only where the
x87 stack is empty.  Every region is priced after its actual loads,
memory/register arithmetic forms, duplication and exchanges exist, and the
cheaper complete slice is composed into the final body.  This is a shared LIR
allocation rule: neither C nor QB source form, symbol name, or frontend hint is
consulted.  Calls, barriers, disconnected roots and other empty-stack
boundaries therefore isolate profitability without weakening strict exception
order inside a region.

The real `floatregions.c` probe combines the profitable three-use expression,
a call, and the nbody-like rejected expression.  On 386 the old whole-body
choice emitted 111 bytes, 41 instructions, 21 normalized loads, seven stores,
and 660 weighted units.  Region composition emits 109 bytes, the same 41
instructions, 19 loads, seven stores, and 652 units.  The raw diff replaces two
rounded-home memory operands with the retained x87 value before the call and
leaves the three cheaper reads after it unchanged.  All MIR and pre-allocation
LIR dumps are identical; `FloatAlloc` is the first changed stage.

Clang 21's advisory flat-i386 listing makes the same region-local structural
choice and has 32 normalized instructions, eight loads, two stores, and one
call.  GCC 16.2 under the strict reference flags has 51/18/12/1, including its
own rounded temporaries; neither is treated as an ABI-equivalent target.  The
raw before/after, stage dumps, GCC and Clang listings are under
`build/quality/iter127-floatregions-{before,current}`.

This completes independent empty-stack candidate composition, not global x87
allocation.  The next allocator step is profile-weighted composition within
nonempty CFG regions; the larger QB-vs-BC footprint work remains a shared
canonicalization/allocation problem, never a request for the QB frontend to
pre-optimize its HIR.  The complete focused allocator module passes 94 tests
in 0.12 seconds, and Tier 1 passes 271 tests with 31 deselected in 1.41
seconds.

### 126. Select complete x87 retention candidates — 2026-09-19

The C nbody listing repeatedly read its rounded `dx`, `dy`, and scale homes.
The optimized MIR already contained one SSA value for each rounded temporary;
the first extra read appeared in `FloatAlloc`, where every stable load was
unconditionally converted into a rereadable memory home.  This is therefore a
shared backend issue, not inefficient C HIR to hide in the frontend.

The first local fix retained every individually profitable multi-use home.  It
proved why the allocated form must be measured: the focused probe dropped four
loads but grew from 18 to 20 instructions, while nbody fell from 1,234 to 1,036
estimated loads yet regressed from 2,085 to 2,346 estimated instructions.  The
overlapping retained values forced stack duplication and `fxch`, costs absent
from the local comparison.  That rejected result is covered by a fail-first
regression which forces the locally attractive candidate and requires the
complete 386 allocation to choose the cheaper reread form.

Allocation now builds both the ordinary reread candidate and a conservative
retained-home candidate, then prices the complete allocated instruction stream.
The comparison includes every `fld st(i)`, memory/register arithmetic form,
store, and final `fxch`; no frontend spelling participates.  Retention starts
only at a fully priced self-use, leaves two stack positions free, and does not
open a second overlapping retained interval.  Those restrictions bound the
current candidate set while final candidate pricing remains the authority.

The measurement failure exposed by the rejected candidate has its own
regression.  `fxch` was previously `unknown`, so any function using it lost all
weighted cost.  It is now an explicit form in every CPU profile: 18 units on
80387, four on 486, one issue slot on P5, zero added execution cost on P6/Core,
and conservative two-unit AMD rankings.  Local GCC scheduling descriptions,
Open Watcom's x87 optimizer/scheduler and Pentium pairing notes, and the
published 486/Intel optimization material are the structural evidence; these
remain target rankings rather than DOSBox measurements.

On the three-use `floatreload.c` regression, 386 output improves from 44 to 42
bytes and from 260 to 252 weighted units, with the same 18 instructions and
loads reduced from eight to six.  Every profile except K5 selects that retained
form; K5 correctly retains the 44-byte memory form because its published
memory multiply is cheaper.  GCC 16.2 emits 20 instructions/four loads/two
stores and Clang 21 emits 13/two/zero under the documented flat-i386 reference
contract; qbopt emits 18/six/two under the medium-model ABI and mandatory
rounded local.

For 386 nbody, complete pricing rejects the stack-shuffling candidate and
preserves iteration 125 exactly at 1,083 bytes, 283 instructions, 2,085
estimated executed instructions, and 39,081 weighted units.  Its independent
DOS/OMF oracle still returns 4,774,160.  The first changed stage in the accepted
probe remains `FloatAlloc`; every MIR pass and pre-allocation LIR dump is
identical.  Artifacts are under `build/quality/iter126-final` and
`build/quality/iter126-nbody-after4`.

This advances Phase 4's target- and pressure-aware x87 allocation without
claiming full global stack scheduling.  The next step is a block-weighted
candidate composition that can retain one interval in a profitable region
without forcing an all-body choice; the present complete comparison guarantees
that such work cannot regress a profile merely by reducing memory references.

### 125. Make pointer provenance a shared optimization boundary — 2026-09-19

The QB integration exposed repeated descriptor loads and address reconstruction
in `SC_INIT`.  Caching those values in the source frontend would make one
listing smaller while leaving the optimizer unable to clean equivalent HIR
from another frontend.  The frontend's contract is semantic identity only:
an owned array pointer may name its allocation, but the frontend remains free
to emit every repeated load and computation.

Pointer analysis previously gave an operand's attached provenance absolute
priority over the SSA value actually dereferenced.  Thus a conservative
`PARAMETER` spelling on a memory operand hid a precise `ALLOCATION` fact on
its pointer.  MemorySSA then had to assume that a store into array data could
overwrite the array's global descriptor, so GVN and LICM retained the next
descriptor load.  A new common structural pass canonicalizes references from
current SSA pointer facts before pointer splitting and SROA.  Concrete derived
identity replaces only abstract `UNKNOWN`, `NONLOCAL`, or `PARAMETER`
annotations.  Conflicting concrete identities are unioned conservatively and
therefore cannot manufacture a false no-alias proof.

The fail-first `SC_INIT` model is deliberately inefficient MIR: descriptor
load, allocation store through a coarsely annotated pointer, then the same
descriptor load.  Before the change both loads survive; with the allocation's
semantic identity present, ordinary shared GVN leaves one.  No frontend cache
or QB-specific operation is involved.

The first implementation also exposed a measurement guardrail.  It required
the explicit pointer flag before deriving any SSA address identity, which
removed provenance from C nbody's ordinary frame-array expressions.  The
allocated listing shrank from 1,083 bytes/283 instructions to 616/143, but the
hot loop stopped exact specialization and the estimator rose from 2,085 to
31,149 dynamic instructions.  A second fail-first regression records that
exact missing-provenance symptom.  The corrected rule retains the historic
derivation for an unannotated computed address and uses the pointer flag only
when refining an already attached annotation.

C nbody is consequently byte-for-byte identical to iteration 123/124 at
1,083 bytes and 283 instructions, with the same 2,085 estimated dynamic
instructions and byte-identical GCC 16.2 and Clang 21 references.  Real VBDOS
`/R` D_SURF is also stage-for-stage and assembly-byte identical to iteration
124 at 15,734 emitted code bytes and 5,017 assembly instructions.  That
identity is intentional for an architectural commit: the current QB frontend
still has to supply the stable allocation identity, then remove its descriptor
value caches so the shared regression's deliberately redundant shape is what
production exercises.  Artifacts are under
`build/quality/iter125-provenance`.

This advances the Phase 3 alias/SROA boundary without claiming general
descriptor recognition, arbitrary pointer-load ownership, or a completed
quality gate.  Inferring allocation ownership from arbitrary pointer-shaped
loads would be unsound; it is a source-language semantic fact, not a request
for frontend optimization.

### 124. Share fresh allocated tails in the common pipeline — 2026-09-19

Identical-tail merging and its machine-DCE fixed point formerly ran in the C
driver after the common allocated-LIR pipeline.  A fresh frontend consuming
`flow.machine()` therefore retained independent copies of identical failure
and result tails.  The allocated form and profitability question are not
frontend-specific, so the existing fixed point now belongs to the common
final-control-flow phase.  The later C-only copy was deleted.

Only fresh instructions are eligible.  Decoded/source-owned instructions
retain distinct byte, relocation and line anchors until source-map-aware tail
sharing can reconcile those records.  One regression fails before the move by
retaining two fresh zero-result tails; a second asserts that the same physical
shape with source ownership remains two tails.

Real VBDOS `/R` D_SURF, compiled through the uncommitted QB integration with
the current main backend, changes as follows:

| metric | iteration 123 | shared fresh tails | change |
|---|---:|---:|---:|
| code bytes | 15,802 | 15,734 | -68 (-0.4%) |
| object bytes | 25,119 | 25,039 | -80 (-0.3%) |
| emitted procedure instructions | 5,039 | 5,017 | -22 (-0.4%) |
| gap to BC's 12,570 code bytes | 3,232 | 3,164 | -68 |

The raw listing shows four independently reached failure/zero-result families
redirected to one existing physical tail; no source operation was duplicated
or discarded.  A small C structural probe likewise emits one shared zero
return, matching GCC 16.2's shape; Clang 21 shares the final return after
selecting a branchless value computation.  The probe reports the same nine
estimated dynamic instructions for qbopt, GCC and Clang after ABI
normalization.  Production C nbody is byte-for-byte unchanged at 1,083 bytes
and 283 instructions, as are both reference listings.  Raw objects, stage
dumps and references are under `build/quality/iter124-shared-tails-r`,
`build/quality/iter124-shared-tails-only`, and
`build/quality/iter124-shared-tails`.

### 123. Put final block placement in the shared machine pipeline — 2026-09-19

The final threader formerly lived in the C driver after the shared machine
pipeline.  A frontend consuming `flow.machine()` directly therefore kept the
source block order.  Final listing had to materialize its implicit false edges,
and fresh QB D_SURF contained 189 `jcc body; jmp exit` pairs plus 389 total
unconditional jumps.  This was a backend pipeline omission, not a property of
QB control flow.

The common last machine phase now performs the existing general block placer
and then the existing threader.  Placement first makes every implicit edge
explicit and follows loop-local traces; threading then removes edges made free
by that physical order, redirects passage blocks, folds converged diamonds,
and preserves exact-trip loop headers.  The C driver's later placement remains
idempotent and available for its tail-merging fixed point.  No frontend-specific
branch rule was added.

The fail-first regression models the actual missed shape: one conditional
instruction with two CFG successors and no explicit false-edge jump in LIR.
Before the phase, final listing inserted `jmp exit`; after final placement it
orders the false successor next and emits only the conditional.  An earlier
regression draft used an already-explicit jump and therefore tested the
threader rather than the pipeline defect; the real D_SURF stage diff exposed
that bad instrument before it could become the committed test.

The current main backend was again overlaid on the uncommitted QB frontend and
used to compile real VBDOS `/R` D_SURF.  The shared `jumps` phase is the first
changed stage.  It rotates the two counted `SC_INIT` loops into preheader/body/
test order, matching BC's one taken backedge per iteration rather than adding
an exit jump on every test.

| `D_SURF` metric | iteration 122 | shared final placement | change |
|---|---:|---:|---:|
| code bytes | 16,029 | 15,802 | -227 (-1.4%) |
| object bytes | 25,346 | 25,119 | -227 (-0.9%) |
| emitted procedure instructions | 5,182 | 4,962 | -220 (-4.2%) |
| unconditional jumps | 389 | 169 | -220 (-56.6%) |
| adjacent `jcc`/`jmp` pairs | 189 | 1 | -188 (-99.5%) |
| `SC_INIT` bytes | 789 | 781 | -8 (-1.0%) |
| `SC_INIT` instructions | 202 | 198 | -4 (-2.0%) |

The byte saving is smaller than two bytes per removed jump because the new
physical order lengthens some formerly short conditional displacements.  The
placer deliberately ranks executed loop work before static size: the removed
edge was executed on every loop test, while each retained preheader jump runs
once.  BC remains at 12,570 D_SURF code bytes and 547 bytes/161 instructions
for `SC_INIT`; the gaps are now 3,232 bytes (25.7%) and 234 bytes/37
instructions (42.8%/23.0%).

The production C nbody output remains byte-for-byte identical at 1,083 bytes
and 283 instructions.  Its GCC 16.2 and Clang 21 reference listings are also
byte-identical to iteration 122, with normalized static-instruction ratios of
1.05x and 0.96x respectively.  Raw objects, stage dumps, candidate listings
and references are under `build/quality/iter123-shared-jumps`.

### 122. Elide explicit edges made free by final block order — 2026-09-19

The fresh QB frontend carried every CFG edge explicitly through allocation.
The C driver already ran its separate block-placement cleanup, but the common
fresh emitter printed an unconditional jump even when its target block was
physically next.  Real D_SURF contained 83 instances, including three in
`SC_INIT`; every one encoded as a two-byte short jump to the following label.

Final listing is now symmetric about physical fall-through.  It already adds
a jump when block placement separates an implicit CFG edge; it now omits the
last emitting unconditional jump when the block has exactly one successor and
that successor is the next physical block.  Trailing `NOTHING` source anchors
remain, all non-adjacent and conditional edges remain explicit, and both MASM
text and native OMF consume the same listing.  This is a general final-layout
rule rather than a QB- or D_SURF-specific branch pattern.

The focused regression failed first by printing `jmp L0_2` directly before
`L0_2`.  It presents allocated LIR without calling the C driver's earlier
threader, which is the frontend-independent contract that exposed the defect.
After the change its listing has the two labels adjacent and no encoded jump.

The current main backend was overlaid on the real uncommitted QB frontend and
used to compile qrender's `D_SURF.BAS` with VBDOS `/R` array order.  The raw
listing diff contains exactly 83 removed `jmp` lines and no other assembly
change; the 166-byte code reduction therefore reconciles exactly with the 83
two-byte encodings.

| `D_SURF` metric | iteration 121 | final fall-through | change |
|---|---:|---:|---:|
| code bytes | 16,195 | 16,029 | -166 (-1.0%) |
| object bytes | 25,523 | 25,346 | -177 (-0.7%) |
| emitted procedure instructions | 5,265 | 5,182 | -83 (-1.6%) |
| adjacent-target unconditional jumps | 83 | 0 | -83 |
| `SC_INIT` bytes | 795 | 789 | -6 (-0.8%) |
| `SC_INIT` instructions | 205 | 202 | -3 (-1.5%) |

BC remains at 12,570 D_SURF code bytes and 547 bytes/161 instructions for
`SC_INIT`, leaving respective gaps of 3,459 bytes (27.5%) and 242 bytes
(44.2%).  Thus this cleanup is exact but not the dominant remaining issue:
the 36 excess descriptor-field loads in `SC_INIT` still account for nearly
all of its 41-instruction gap.

A freestanding adjacent-label C probe was compiled with GCC 16.2 for i386 and
Clang 21.  Both place the store immediately after entry and emit no branch,
matching the rule.  The production C nbody path contained zero candidates
because its earlier block placer had already done this work; disabling the
new common rule produced byte-identical nbody assembly.  Its fresh 386 report
is 1,083 bytes/283 instructions, with normalized static-instruction ratios of
0.96x against Clang and 1.05x against GCC.  Raw D_SURF, probe, nbody and
reference artifacts are under `build/quality/iter122-fallthroughs`.

### 121. Make omitted complete-peel tuning bounded — 2026-09-19

The QB frontend integration called the shared MIR optimizer with 386 costs and
register capacities but omitted the profile's two complete-peel limits.  The
public boundary interpreted both omitted values as zero, and zero meant
unbounded.  Thus `SC_INIT`'s exact 25-trip initialization loop became 25
separately encoded far stores even though every established production driver
already passed the default profile's 16-trip and 200-operation safeguards.
This was an API default defect, not evidence that the 386 cost model preferred
the expansion.

The two target-independent defaults now have one machine-neutral definition in
`model.passes`: 16 iterations and 200 semantic operations.  `Where`, the public
`transform.applied()` boundary, and the default fields of every immutable CPU
profile share those constants.  A selected target may still override them,
and an explicit zero still deliberately requests an unbounded policy.  No MIR
pass receives a CPU name or machine form.

Two regressions were observed fail before the fix.  The public-boundary probe
received `(0, 0)` instead of `(16, 200)`, and a profitable-looking 25-way
straight-line expansion returned no rejection instead of
`iteration-growth`.  The latter is the output symptom: it asserts the code
growth decision, not only the fields carrying the policy.

With the real uncommitted QB frontend and qrender's `D_SURF.BAS`, omitting the
limits now produces assembly byte-for-byte identical to supplying the 386
profile explicitly (`sha256
df48f40eac4963cbec288b421a2fe1225c455526924d5eb4febbfdfe6866f8e1`).  The stage trace first proposes
the 25-trip candidate at 410 semantic operations, settles it to 187, and then
records `unroll-rejected-iteration-growth`.  It separately accepts the
seven-trip, three-array initializer, so the safeguard is a bounded policy
rather than a blanket disabling of exact unrolling.

| `D_SURF` metric | iteration 120 | bounded default | change |
|---|---:|---:|---:|
| code bytes | 17,243 | 16,195 | -1,048 (-6.1%) |
| object bytes | 26,582 | 25,523 | -1,059 (-4.0%) |
| emitted procedure instructions | 5,502 | 5,265 | -237 (-4.3%) |
| `SC_INIT` instructions | 270 | 205 | -65 (-24.1%) |

BC's object contains 12,570 code bytes and its `SC_INIT` has 161 instructions,
so the remaining gaps are now 3,625 bytes (28.8%) and 44 instructions (27.3%).
The retained loop executes more counter/address work once during startup than
the former 25-way clone; this is an explicit cold-code size tradeoff, not a
runtime-speed claim.  Partial unrolling is still required for a better middle
ground.

Fresh freestanding `-O3 -march=i386` references make that limitation visible.
GCC 16.2 partially unrolls the analogous 25-dword fill by two; Clang 21 fully
unrolls it.  Both fully unroll the analogous seven-trip, three-array loop.  The
references therefore support retaining small expansion while disagreeing on
the 25-trip form; they do not justify declaring either flat-model result
optimal under the medium-model selector and six-register constraints.  Raw
listings, the two QB builds, the fresh OMF object and stage trace are under
`build/quality/iter121-bounded-unroll`.

### 120. Expose packed far-pointer address operands before SROA — 2026-09-19

The new QB frontend represents a 16:16 pointer as one 32-bit scalar, which is
the right program value but was the wrong memory-operand boundary.  A load or
store through that scalar reached lowering as an opaque packed pointer.
Lowering could only preserve the selector and unpack the two words locally:

```asm
push cx
push dx
pop ecx
push es
push ecx
pop si
pop es
mov dword ptr es:[si],0
pop es
```

That sequence hid the offset and selector from value numbering, LICM and the
allocator, so every dereference repeated the stack traffic.  This was the
largest directly redundant component in the frontend's qrender output.

The general rule is now explicit at the structural MIR boundary.  A packed
far-pointer *scalar* stays packed.  A memory reference through it is rewritten
to two ordinary, independently allocatable word SSA values: an offset and a
selector.  The normalization names no register or opcode and runs before SROA;
lowering remains responsible for realizing the abstract far reference.  GVN
can reuse identical extractions, and algebraic simplification turns extraction
from a known `CONCAT(selector, offset)` back into the original word values.
That common frontend shape now emits:

```asm
mov es,word ptr [bx+2]
mov si,word ptr [bx+10]
mov dword ptr es:[si],0
```

The fail-first regression models exactly that descriptor-to-`CONCAT`-to-store
shape.  Before the change its optimized store still held `pointer=True` and
lowering emitted `push`/`pop`; now it holds a split `FAR` reference to the
original offset and selector, with neither stack operation nor residual
`CONCAT`/`EXTRACT`.  `r01-split_pointers` is the first changed stage, before
`r01-sroa`.

The same uncommitted frontend source was overlaid on the previous and current
backend revisions and used to compile qrender's `D_SURF.BAS` with VBDOS `/R`
array order.  These are raw emitted listings and fresh OMF sizes, not inferred
scores:

| metric | before | after | change |
|---|---:|---:|---:|
| code bytes | 20,446 | 17,243 | -3,203 (-15.7%) |
| static instructions | 7,923 | 5,579 | -2,344 (-29.6%) |
| all `push`/`pop` | 2,886 | 750 | -2,136 (-74.0%) |
| `push es`/`pop es` | 958 | 57 | -901 (-94.1%) |
| `SC_INIT` instructions | 689 | 270 | -419 (-60.8%) |

BC's production object is still smaller: 12,570 code bytes, leaving the fresh
frontend 4,673 bytes or 37.2% larger.  Raw BC `SC_INIT` has 161 instructions,
so the current 270 remain 67.7% above it.  The next gap is visible rather than
speculative: BC retains short counted initialization loops and keeps descriptor
words live; the frontend fully unrolls them and repeatedly reloads the same
descriptor fields.  Loop-form selection plus alias-backed descriptor CSE is
therefore the next general mechanism, not another pointer-unpacking exception.

As a flat-i386 structural reference, GCC 16.2 either keeps the low word as a
view or obtains the high word with `shr`; Clang 21 directly loads `[p]` and
`[p+2]`.  Neither materializes the parts through the stack.  They cannot model
our segmented memory operand, so this supports only the split-value boundary,
not a claim that their final addressing is ABI-comparable.

On the older HUGE2 object path, where the packed value is not already a known
concatenation, the normalization reduces the emitted listing from 215 to 210
instructions and its `push`/`pop` count from 55 to 37.  Code grows from 1,784
to 1,800 bytes (+0.90%) because independently live selectors sometimes use
FS/GS prefixes; that is below the plan's one-percent cross-target limit and
buys fewer executed operations.  The focused fail-first and private raise-view
regressions pass in `0.13s`; complete iteration artifacts are under
`build/quality/iter120-pointer-split`.

### 119. Rematerialize incoming arguments across proven local objects — 2026-09-19

P5 matmul copied its incoming `seed` from `[bp+6]` to a private frame slot at
entry, then read that copy once per initializer iteration.  The spiller knew
that reloading the original cell was cheaper than a spill slot, but every
indexed store to either local matrix conservatively invalidated it.  Final LIR
could not repair the proof: selection spells a local pointer as an arbitrary
address-register access, so its physical address no longer says `FRAME` even
though the attached MIR store still has exact current-activation provenance.

The first regression deliberately uses that production shape: a positive
BP-relative load, followed by an indexed selected store whose MIR provenance
is confined to one bounded `FRAME` object, followed by a use.  Before the fix
the value acquired a new `[bp-2]` spill home.  A companion exact-offset case
and five pre-existing call, pointer, and frame-effect variants define the
conservative boundary.

Spill rematerialization now asks one general object question.  A non-negative,
fixed BP-relative incoming cell cannot overlap storage allocated below BP by
the current activation.  A modeled store whose nonempty provenance consists
only of `memory.Kind.FRAME` objects therefore preserves that cell even when
its selected address is indexed or has lost the abstract address-space
spelling.  Unknown provenance, escaped pointers, unmodeled writes, calls, and
any store set not wholly covered by that proof retain the previous
conservative invalidation.  This reuses the canonical alias object model; it
does not parse a frontend-specific identity tuple or name matmul.

`lir-regalloc` is the first changed stage; every MIR stage and LIR through
coalescing is identical.  The emitted initializer changes from:

```asm
mov ax, word ptr [bp+6]
mov word ptr [bp-520], ax
...
add ax, word ptr [bp-520]
```

to the foldable original home:

```asm
add ax, word ptr [bp+6]
```

Spill-slot recolouring also reduces the frame by four bytes.  Against the
iteration 118 P5 result:

| metric | before | after | change |
|---|---:|---:|---:|
| bytes | 625 | 617 | -8 |
| static instructions | 171 | 169 | -2 |
| estimated dynamic instructions | 4,077 | 4,075 | -2 |
| static / dynamic P5 cost | `509 / 14,644` | `507 / 14,642` | `-2 / -2` |
| loads / stores | `46 / 20` | `45 / 19` | `-1 / -1` |
| spill reloads / stores | `3 / 11` | `3 / 10` | `0 / -1` |

Fresh advisory flat-i386 references remain structurally much more aggressive:
Clang emits 366 static and an estimated 879 dynamic instructions after
unroll-and-jam; i686 GCC emits 860 static instructions and has no complete
profile-free dynamic estimate.  The candidate's smaller static listing is not
parity: its 4,075 dynamic instructions expose the still-open loop-form gap.
This iteration advances Phase 4 rematerialization only.

The fail-first indexed test originally failed with `{1: -2}` and now passes
with the exact-offset and conservative-effect variants (`7 passed`, `0.14s`).
The production candidate, all stage dumps, GCC/Clang listings, and JSON reports
are under `build/quality/iter119-local-provenance-p5{,-v2}`.  The exact emitted
assembly was assembled to OMF, linked by the DOS toolchain, and run on the real
386 path; `VALUE.BIN` contained `b0 65 05 00`, the independent `353712`
oracle, with no linker error.  Tier 1 passes (`261 passed`, `31 deselected`,
`2.64s`).  Aggregate test execution was under one percent of this iteration's
wall time.

### 118. Make complete-peel growth prove its pressure benefit — 2026-09-19

The profile's 200-operation complete-sequence ceiling initially fixed P5
matmul's code explosion but also rejected nbody's profitable fixed interaction
specialization.  Simply exempting the first CFG peel restored nbody and 386
matmul, but P5 matmul then grew to 3,856 bytes and 958 instructions.  Its
allocation contained 26 spill reloads, 94 spill stores, 641 total loads and 380
total stores.  This was not accepted as a surprising performance result: the
raw candidate trace showed that the profitability instrument was wrong twice.

First, the settled-body calculation let candidate folding erase evidence of
the source loop cloned by the transformation.  A fail-first regression builds
a 30-operation loop with eight exact trips and a 150-operation settled body;
the old instrument called that a 150-operation sequence instead of retaining
the conservative 240-operation pre-folding bound.  `_expanded_operations` now
uses the greater of the settled attributable body and `source loop size × exact
trip count`.  It still subtracts unrelated procedure work, so a large caller
does not make a small loop ineligible.

Second, a later P5 candidate escaped because the MIR spill lower bound fell by
one unit.  The real trace was:

```text
                         before  after  sequence  dynamic cost  spill bound
P5 matmul, rejected         187    459       392   19542→15576    2025→4301
P5 matmul, old accepted     187    383       456   22208→21835    2341→2340
386 nbody, accepted         166    380       278  292701→109413    8010→3984
```

The one-unit matmul change was not an allocation certificate; it triggered the
958-instruction cascade.  A second fail-first regression captures that exact
class.  Above the CPU profile's sequence ceiling, a spill-prone candidate must
now both strictly reduce the spill lower bound and save enough dynamic target
cost to pay for the complete cloned sequence.  Nbody clears that proof by
orders of magnitude; marginal noise does not.  The rule is shared by straight
unrolling and CFG peeling and names no source program, peel order, register or
opcode.

`mir-peel-rejected-operation-growth` is the first changed production stage for
P5 matmul.  Nbody still reaches `mir-peel-accepted`; lowering and allocation are
unchanged.  Against the committed capped baseline:

| CPU/program | before bytes/ins/dynamic/cost | after bytes/ins/dynamic/cost |
|---|---:|---:|
| 386 nbody | `592 / 138 / 13959 / 220965` | `1083 / 283 / 2085 / 39081` |
| 386 matmul | `502 / 154 / 10780 / 47363` | `1625 / 421 / 1023 / 4070` |
| P5 matmul | `502 / 154 / 10780 / 22867` | `625 / 171 / 4077 / 14644` |

The 386 speed/size trade is intentional under the requested performance
priority.  Its emitted structure is credible against fresh flat-i386 best-case
references: normalized nbody is 277 static / 2,085 estimated dynamic
instructions versus GCC's 263 / 5,132 and Clang's 290 / 1,174; matmul is
413 / 1,023 versus Clang's 356 / 879 (GCC's 849-instruction listing has no
complete dynamic estimate).  These are advisory structural references, not
medium-model targets.

P5 matmul is explicitly not closed: the rejected full specialization had 958
dynamic instructions and estimated cost 2,512, but was 2.7 times Clang's
static instruction count and mostly memory traffic.  The next general work is
pressure-aware scalar lifetime/formula selection so that fixed work can remain
specialized without hundreds of allocator-created loads and stores.  Repeated
candidate fixed points also make compilation take minutes; memoization or an
earlier sound pressure bound remains a separate compile-time requirement.

The focused policy suite passes (`10 passed`, `0.20s`), Tier 1 passes (`261
passed`, `31 deselected`, `1.14s`), and fresh OMF plus the DOS linker and a
real 386 return nbody's independent `4774160` oracle (`1 passed`, `13.85s`).
The combined nbody/matmul structural run was stopped at the test-time ceiling
after nbody passed; its matmul assertions are independently covered by the
completed emitted-code report above. Candidate listings, all MIR/LIR stages,
and reports are under
`build/quality/iter127-pressure-proof-{386,p5}`.  Audited candidate-ABI targets,
loop versioning, partial unrolling and final cross-profile acceptance remain
open.

### 117. Compose associative bitwise constants — 2026-09-18

Specializing CRC's constant input exposed two adjacent modular XORs at the
entry.  Both GCC and Clang combine them before their first polynomial round:

```asm
; before                              ; after, GCC and Clang
mov eax, dword ptr [bp+6]             mov eax, dword ptr [bp+6]
xor eax, 4294967295                   xor eax, 4294967246 ; -50
xor eax, 49
```

The fail-first real-source regression found the two immediates in the emitted
body.  The focused mechanism regression covers `AND`, `OR`, and `XOR` at both
16-bit modular edges and representative interior values.  Algebraic MIR now
composes constants through a same-kind, same-width intermediate only when that
intermediate has one reader.  It refuses memory effects, barriers, merges,
width changes, a shared intermediate, an observed intermediate flag, or any
non-flag secondary result.  Final bitwise flags are retained because the final
value and bitwise flag semantics are identical.  This is one associative
integer rule rather than a CRC or complement idiom.

That boundary matches the local compiler references.  GCC's
`gcc/tree-ssa-reassoc.cc:is_reassociable_op` requires a single-use SSA result;
LLVM's `llvm/lib/Transforms/Scalar/Reassociate.cpp` gives constants rank zero,
canonicalizes them to the right, and descends through a same-op node only when
it is reassociable.  qbopt deliberately implements the profitable two-node
integer case rather than importing either compiler's general expression-tree
framework or permitting floating-point reassociation.

The raw candidate trace is unchanged through `candidate-unroll-r01-strength`.
`candidate-unroll-r01-algebraic` is the first changed stage:

```text
- xor (v2, 49)          -> v9
+ xor (v1, 4294967246)  -> v9
```

The immediately following `dead` pass turns the now-unused `xor v1,-1` into
its source anchor.  No lowering, allocation, or machine pass participates.
Against iteration 116 on every CPU profile:

| metric | before | after | change |
|---|---:|---:|---:|
| selected bytes | 1,680 | 1,676 | -4 |
| static/estimated executed instructions | 450 | 449 | -1 |
| loads / stores / branches | 1 / 0 / 0 | 1 / 0 / 0 | unchanged |
| estimated executed 386 cost | 993 | 991 | -2 |

Every other profile also improves: 486 `984→982`, P5 `899→897`, P6
`1185→1184`, K5/K6 `307→306`, K7 `309→308`, and Core `755→754`.  The candidate
now executes 449 instructions versus i686 GCC's 454 and Clang's 394.  Their
flat i386 listings agree exactly on `xor input,-50`; Clang's subsequent wider
CRC algebraic reduction remains a separate opportunity under the documented
ABI caveat.

Fresh OMF emission, the DOS linker, and a real DOS 386 return the independent
canonical answer `3421780262` (`CBF43926`).  Candidate, fresh GCC/Clang
listings, every MIR/LIR phase, and the all-profile report are under
`build/quality/iter126-crc-bitwise`.  Focused checks pass (`22 passed`,
`6.10s`) and Tier 1 passes (`250 passed`, `31 deselected`, `1.14s`).  This
advances scalar algebraic quality without claiming arbitrary expression-tree
reassociation, floating-point reassociation, or Clang's complete CRC lowering.

### 116. Specialize bounded constant-data loops — 2026-09-18

CRC's eight-round inner loop was already completely expanded, but its exact
nine-trip outer loop survived and read the same private constant table on every
function call:

```asm
; before                              ; after (first two source bytes)
xor bx, bx                            xor eax, 49
Louter:                               ; eight polynomial rounds
movzx ecx, byte ptr _data[bx]         xor eax, 50
xor eax, ecx                          ; eight polynomial rounds
; eight polynomial rounds
add bx, 1
cmp bx, 9
jb Louter
```

The first fail-first policy regression returned `growth`; the real-source
regression independently found nine dynamic branches and ten loads.  The raw
MIR trace showed an outer candidate simplifying from 418 to 405 operations and
saving target-priced 386 work (`1180→947`), then losing because all 346 added
operations were charged once against work already counted across all nine
executions.  Static clone cost is now amortized over the proven trip count.
The profile's independent 16-iteration ceiling, the builder's 512-operation
resource bound, dynamic target cost, register-pressure price, and strict
positive-savings gates remain.  A two-trip loop whose saved branch does not pay
even the amortized growth is still rejected.

This follows the structure of GCC's
`gcc/tree-ssa-loop-ivcanon.cc`: complete peeling has independent iteration and
expanded-body ceilings, rejects calls/branchy candidates, and permits bounded
straight-line specialization rather than requiring dynamic savings to exceed
the complete static clone on every invocation.  Both fresh `-O3 -march=i386`
i686 GCC and Clang listings completely specialize these nine bytes.

Expansion alone exposed a second issue.  Open Watcom records `_data` as
`FE_CONSTANT|FE_INTERNAL` and supplies all ten bytes, but the C path left
`MirBody.initial` empty.  The frontend now transfers complete numeric bytes of
private, nonvolatile constant objects into that existing loader-fact side
table.  Mutable, exported/imported, volatile, relocatable, partially
understood, or inline-assembly-referenced objects are refused.  Ordinary
MemorySSA/alias effects remain responsible for invalidation, and each body
receives facts only for objects it directly names so a large qcport table does
not enlarge unrelated SCCP problems.  Optimization passes receive only MIR
facts, not HIR records.  The first candidate `Fold`
already resolved each indexed address to the correct byte, but constant
evaluation accepted extension from held values and literals only.  Its scalar
rule now also accepts a memory cell proved by the same lattice, after which
dead code removes every table address and load.

Against the immediately preceding committed build on 386:

| metric | before | after | change |
|---|---:|---:|---:|
| selected bytes | 225 | 1,680 | explicit speed/size tradeoff |
| static instructions | 63 | 450 | full specialization |
| estimated executed instructions | 487 | 450 | -7.6% |
| estimated executed loads | 10 | 1 | -90.0% |
| estimated executed branches | 9 | 0 | -100% |
| estimated executed 386 cost | 1,130 | 993 | -12.1% |

Every CPU profile improves dynamically: 386 `1130→993`, 486 `1066→984`, P5
`963→899`, P6 `1212→1185`, K5/K6 `334→307`, K7 `336→309`, and Core
`782→755`.  The code-size increase is intentional and audited against the
requested speed objective: the candidate executes 450 instructions with one
load and no stores, versus i686 GCC's `454/1/0` and Clang's `394/2/1`.  Flat
reference byte sizes remain non-comparable to operand-prefixed medium-model
code; a candidate-ABI size target is still not registered.

Fresh OMF emission, the DOS linker, and a real DOS 386 return `3421780262`
(`CBF43926`) for the independent canonical input.  Candidate, GCC, Clang and
stage evidence are under `build/quality/iter125-crc-final`; the all-profile
candidate report is `all.json`.  The baseline report proves CRC was the only C
corpus program with a growth-rejected unroll candidate; floats and Mandel are
byte-identical in the interrupted corpus audit, and the remaining programs
contain neither such a candidate nor a private constant initializer.  Focused
checks pass (`6 passed`, `9.49s`) and Tier 1 passes (`250 passed`, `31
deselected`, `3.78s`).  This advances loop and
whole-module phases without claiming partial unrolling, arbitrary readonly
external memory, Clang's CRC algebraic reduction, or final acceptance.

### 115. Narrow synthetic high-word reloads — 2026-09-18

After iteration 114, Mandel's medium-model long return still widened a frame
reload solely to throw its low half away:

```asm
; before                              ; after
mov edx, dword ptr [bp-4]             mov dx, word ptr [bp-2]
shr edx, 16
mov ax, word ptr [bp-4]               mov ax, word ptr [bp-4]
```

The first unit-only attempt passed but the production listing did not change.
The raw allocated LIR showed why: the local dead-lane walker cleared all facts
at `retf`, although whole-CFG liveness already had the return's complete ABI
read contract.  Treating that passing unit as proof would have repeated the
measurement failure this project explicitly guards against.  The regression
now uses the real complete return contract, failed with the wide load intact,
and `regthrash._dead_after` now consumes the same declared return/call effects
as the whole-body solver.

The physical cleanup recognizes only a zero-byte, non-source-backed frame
reload followed immediately by an unsigned 16-bit shift of the same dword
register.  It reads the high word directly only when the register's preserved
upper lanes and every flag the shift modifies are dead.  It refuses source
memory operations, volatile or declared loads/stores, live full-width results,
live flags, relocations, indirect/indexed cells, clobbers, constraints, groups,
spread ownership and spill stores.  The removed shift remains a virtual
anchor.  This is a general extract-from-compiler-storage fold; it does not name
Mandel, a return register, or one frame offset.

Stage files are identical through `ParallelCopy`; `Peephole` is the first
changed phase.  Against the immediately preceding committed listing:

| metric | before | after | change |
|---|---:|---:|---:|
| bytes | 196 | 191 | -2.6% |
| instructions | 54 | 53 | -1 |
| ABI-normalized instructions | 46 | 45 | -1 |
| static 386 cost | 283 | 280 | -1.1% |
| estimated executed instructions | 71,565 | 71,564 | -1 |
| estimated executed 386 cost | 452,811.63 | 452,808.63 | -3 |

The isolated form shrinks from eight bytes to three and improves on every
profile: 386 `7→4`, 486 `5→1`, P5 `4→1`, P6 `4→3`, K5 `3→2`, K6 `3→2`, K7
`4→3`, and Core `5→4`.  The candidate is now exactly `1.00×` i686 GCC's 45
ABI-normalized instructions and `0.87×` Clang's 52.  The flat references return
the long in EAX, so they are structural evidence that no extraction shift is
needed, not an ABI-equivalent prescription for DX:AX.

Real `xOffset=0` again passes through fresh OMF, the DOS linker and a DOS 386
and returns the independent oracle `8873`.  Candidate, GCC, Clang and stage
listings are under `build/quality/iter124-mandel-highword`.  Focused physical
liveness/folding checks pass (`17 passed`, `0.12s`), the real-source regression
passes (`1 passed`, `48.30s`), and Tier 1 passes (`249 passed`, `31 deselected`,
`3.74s`).
This advances Phase 7 without claiming arbitrary source-load narrowing,
non-adjacent extraction folding, fewer accumulator spills, or final
acceptance.

### 114. Transfer commutative results into a dying input — 2026-09-18

The C Mandelbrot inner loop first emitted a three-instruction value transfer
even though both multiply inputs died at the operation:

```asm
; before                    ; after
imul ebx, ecx               imul ecx, ebx
mov  ecx, ebx
sar  ecx, 7                 sar  ecx, 7
```

This was not a MIR algebra gap.  Stage dumps are byte-identical through
`ParallelCopy`; `Peephole` is the first changed phase.  Allocation had already
settled the physical fact needed for the rewrite: after
`A = commutative(A, B); B = A`, A's register lanes are dead.  The post-allocation
cleanup now emits `B = commutative(B, A)` directly for register `add`, `and`,
`or`, `xor`, and two-operand `imul`.  It retains the removed synthetic move as
a zero-byte virtual anchor, and refuses a live first operand, a source-owned
copy, unequal widths, aliased roots, spills, implicit requirements/results,
clobbers, groups, spread ownership, or a changed encoding length.  Thus the
mechanism depends on exact physical liveness and commutativity, not on Mandel,
one opcode, or one register assignment.

The fail-first regression raised `AttributeError` before the mechanism existed;
the real-source assertion independently found the adjacent
`imul ebx,ecx; mov ecx,ebx` symptom.  Tier-1 coverage exercises all five
commutative forms plus the live-input and source-owned refusal boundaries.
The current Mandel listing contains `imul ecx,ebx; sar ecx,7` with no transfer.
GCC 16.2 uses `imul ebx,eax; sar ebx,7`; Clang 21 uses
`imul eax,edi; sar eax,7`.  Their registers differ under the flat 32-bit ABI,
but all three now make the same dying-input choice.

Disabling only this rewrite in memory gives the exact before/after result on
the 386 candidate:

| metric | before | after | change |
|---|---:|---:|---:|
| bytes | 199 | 196 | -1.5% |
| instructions | 55 | 54 | -1 |
| ABI-normalized instructions | 47 | 46 | -1 |
| static 386 cost | 285 | 283 | -0.7% |
| estimated executed instructions | 74,839.11 | 71,565.00 | -4.4% |
| estimated executed 386 cost | 459,359.84 | 452,811.63 | -1.4% |

The isolated pair shrinks from seven to four bytes and improves on every
profile: 386 `24→22`, 486 `29→27`, P5 `13→11`, P6 `5→4`, K5 `5→4`, K6
`4→3`, K7 `6→5`, and Core `4→3`.  ABI-normalized static instruction ratio
improves from `1.04×` to `1.02×` against i686 GCC and from `0.90×` to `0.88×`
against Clang.  Dynamic reference ratios remain withheld because the reference
CFG estimator uses heuristic loop counts where the candidate carries two
proved trip counts.

Real input `xOffset=0` passes through fresh OMF, the DOS linker and a DOS 386
and returns the independent result `8873`.  Raw before/after, GCC, Clang and
stage listings are under `build/quality/iter122-mandel-transfer`.  Focused
Tier-1 checks pass (`7 passed`, `0.11s`), the real-source regression passes
(`1 passed`, `35.02s`), and Tier 1 passes (`245 passed`, `31 deselected`,
`1.81s`).  This advances Phase 7; it does not claim general non-adjacent
recolouring, memory operand commutation, or final acceptance.

### 113. Weight per-CPU cost by executed CFG paths — 2026-09-18

Iteration 112 exposed an instrument contradiction.  Duplicating `choose.c`'s
two-byte terminal return removed an executed jump at unchanged byte size, yet
the only CPU-cost field rose from 50 to 65 on 386.  Raw assembly showed why:
the static sum charged both mutually exclusive copies of `pop bp; retf` as if
one path executed both.  That is a size-oriented ranking, not hot-path work.

The report now retains `weighted_cost` unchanged as the compatibility/static
ranking and adds `dynamic_weighted_cost`.  One shared extent builder maps the
exact emitted bytes to final LIR blocks, rejects hidden call/interrupt/`rep`
work and irreducible or unbounded CFGs, and supplies the same exact-trip or
explicit profile-free frequencies used by dynamic structural metrics.  The
prologue is priced once; each block is priced independently for the selected
CPU and multiplied by its expected executions.  Missing forms remain null and
named.  Later-CPU dependency scoring restarts at block boundaries because the
profile-free CFG does not identify a cross-edge schedule; the status calls the
result an estimate rather than measured cycles.

The fail-first real-source regression raised `KeyError` because the executed
cost did not exist.  Its hand audit now pins 386 `choose` to 41 units:
prologue 4, load block 4, compare/branch block 9, and two 24-unit return arms
at one-half frequency each.  A zero or a repeated static total cannot pass.
Disabling only terminal-return duplication supplies the exact prior layout;
the all-profile comparison is:

| CPU | shared-tail executed cost | duplicated-tail executed cost | result |
|---|---:|---:|---:|
| 386 | 44.5 | 41 | -7.9% |
| 486 | 26.5 | 25 | -5.7% |
| P5 | 11.5 | 11 | -4.3% |
| P6 | 24 | 24 | unchanged |
| K5 | 10 | 9 | -10.0% |
| K6 | 10 | 10 | unchanged |
| K7 | 12 | 12 | unchanged |
| Core | 26 | 25 | -3.8% |

No selected bytes or program semantics change in this measurement iteration.
The real inputs 0 and 1 still produce `0a 00 08 00` (10 and 8) through fresh
OMF, the DOS linker and a DOS 386.  The qbopt listing remains GCC-shaped with
two direct return arms; GCC emits the same control structure, while Clang's
best-case flat-i386 listing remains the outstanding `sete; lea` if-conversion.
Candidate and raw reference listings for every supported profile are under
`build/quality/iter120-choose-cost`; the source SHA-256 remains
`84551e148b8a0517c83840c2d800c0157b32eb425f1464fd3256a939709ab5cf`.
This advances Phase 1 and makes future branch duplication, loop cloning,
peeling and unrolling comparisons honest; it does not claim a runtime profile,
if-conversion, audited hard target, or final acceptance.  Focused measurement
checks pass (`3 passed`, `57 deselected`, `1.49s`), the hidden-callee refusal
passes (`1 passed`, `59 deselected`, `0.55s`), the DOS execution regression
passes (`1 passed`, `2 deselected`, `2.56s`), and Tier 1 passes (`238 passed`,
`31 deselected`, `5.36s`).

### 112. Duplicate byte-neutral terminal return tails — 2026-09-18

After edge-sensitive folding, `choose.c` still executed one unconditional jump
on one of its two equally likely arms.  GCC duplicates its terminal return;
Clang if-converts the entire choice.  The candidate can take GCC's structural
choice without spending a byte because a short jump and the medium-model
`pop bp; retf` sequence are both two bytes:

```asm
; before                         ; after
mov ax, 8                        mov ax, 8
L0_16:                           pop bp
pop bp                           retf
retf
                                 ; other edge
mov ax, 10                       mov ax, 10
jmp L0_16                        pop bp
                                 retf
```

Final C layout now duplicates a terminal tail only when every instruction in
it is source-unowned, it has no phi, successor, fixup, group, spread, call,
barrier, data, branch, or jump, and every predecessor is a dedicated
unconditional edge.  It selects the tail and the emitter's implicit frame
teardown to exact bytes, prices the removed jump at its shortest possible
encoding, and accepts only no-growth results that remove at least one jump.
Larger tails and source-owned tails remain shared.  This is a general
post-allocation layout choice; it names neither `choose` nor a register.

The fail-first source regression observed `jmp L0_16` and one `retf`.  Focused
LIR regressions cover the byte-neutral decision and both refusal cases, while
an emitter regression proves the implicit `pop`/`leave` byte is included.
The execution regression sends real inputs 0 and 1 through fresh OMF, the DOS
linker, and a DOS 386 and receives the independent bytes `0a 00 08 00`.

The exact `386` candidate remains 20 bytes.  Raw static instructions rise
from 10 to 11 because the mutually exclusive `pop; retf` pair is represented
twice, while ABI-normalized instructions stay 7.  Estimated executed
instructions improve from 8.5 to 8.0 and branches from 1.5 to 1.0.  The
dynamic structural ratio improves from 1.21x to 1.14x against i686 GCC and
from 1.70x to 1.60x against Clang.  The profile-insensitive static CPU price
sum rises 50 to 65 because it sums both mutually exclusive return copies; raw
paths establish that no executed path gained work, so this is an explicit
audited static/dynamic tradeoff rather than a latency claim.

Candidate and raw reference listings are under
`build/quality/iter119-choose`; the source SHA-256 remains
`84551e148b8a0517c83840c2d800c0157b32eb425f1464fd3256a939709ab5cf`.
The report records Apple Clang 21.0.0 and i686-elf GCC 16.2.0.  This advances
Phase 7 without claiming if-conversion, a hand-audited target, or final
acceptance.  Focused layout regressions pass (`2 passed`, `0.15s`), the
emitted-source regression passes (`1 passed`, `0.35s`), the frame-price
regression passes (`1 passed`, `0.26s`), the DOS execution regression passes
(`1 passed`, `2.05s`), and Tier 1 passes (`238 passed`, `31 deselected`,
`5.77s`).

### 111. Fold pure constants through phi edges — 2026-09-18

The real C fixture `choose.c` reduced two private helpers to a conditional
choice followed by one add, but ordinary SCCP correctly did not call
`phi(7, 9)` a single constant.  The final medium-model listing therefore did
runtime work whose answer was already known on each incoming edge:

```asm
; before                         ; after
mov ax, 7                        mov ax, 8
L0_16:                           L0_16:
add ax, 1                        pop bp
pop bp                           retf
retf
                                 ; other edge
mov ax, 9                        mov ax, 10
jmp L0_16                       jmp L0_16
```

Fold now translates one pure integer expression through every complete phi,
evaluates it independently for each incoming value, and joins fresh constants
at the original phi block.  It follows only a single-entry/single-exit linear
corridor from that join; the arithmetic's inert marker stays at its original
source position.  Every incoming edge must be unconditional.  Memory,
volatile and opaque effects, pointer provenance, floating semantics, partial
writes, live secondary results, division and remainder remain refused.  This
is edge-sensitive constant propagation in MIR, not an instruction or register
special case.

The source-level fail-first regression originally observed `mov ax, 7/9;
add ax, 1`; the focused MIR regression independently constructs the same
diamond and verifies SSA after producing 8 and 10.  The stage dump identified
`fold` as the first changed pass after inlining and CFG cleanup exposed the
linear corridor.

On the exact `386` candidate, `_choose` changes from 23 to 20 bytes, from 8
to 7 ABI-normalized instructions, and from 9.5 to 8.5 frequency-weighted
instructions.  The dynamic structural ratio improves from 1.36x to 1.21x
against i686 GCC and from 1.90x to 1.70x against Clang.  GCC duplicates the
small return tail and emits direct 8/10 returns; Clang if-converts the result
to `sete; lea`.  Those remain best-case flat-i386 references: duplicating the
medium-model frame teardown is not yet a static-size win, and a target-costed
if-conversion mechanism has not been implemented.

The candidate and both raw reference listings are in
`build/quality/iter118-choose`; the source SHA-256 is
`84551e148b8a0517c83840c2d800c0157b32eb425f1464fd3256a939709ab5cf`.
The report records Apple Clang 21.0.0 and i686-elf GCC 16.2.0.  Focused
MIR and emitted-C regressions pass (`1 passed` each, `0.24s` and `0.82s`),
and Tier 1 passes (`236 passed`, `31 deselected`, `4.90s`).  This advances
Phase 6's post-inline optimization without claiming a hand-audited target or
final acceptance.

### 110. Weight memory and control traffic by CFG execution — 2026-09-18

The current nbody report appeared to identify a clear lowering defect: qbopt
had 146 static loads versus Clang's 74 and GCC's 71, and the scoreboard named
`lir-lower` as the first stage after which the excess remained.  Raw assembly
showed that the programs did not have comparable static shapes.  qbopt peels
or unrolls five of the six fixed interaction bodies while Clang retains one
large loop; counting each textual load once rewards the looped spelling even
when it executes repeatedly.

The quality instrument now applies its existing CFG frequency solution to
every structural metric, not only total instructions.  Candidate and
reference function reports carry a `dynamic` map for instructions, loads,
stores, branches, calls, and address calculations.  The legacy
`dynamic_operations` field remains the instruction member of that map, so
existing targets and callers stay compatible.  Structural comparisons expose
both raw maps and compute per-metric ratios only when the existing evidence
policy declares the dynamic extents comparable.  Calls, interrupts, repeated
instructions, irreducible CFGs, unmapped blocks, and heuristic loop counts
retain their previous refusal or withheld status.

The fail-first reference regression uses one read/modify/write memory
instruction in a ten-iteration loop.  Its static one load and one store must
report exactly ten dynamic loads and stores alongside 32 executed
instructions.  Candidate reports prove their dynamic instruction member is
identical to the compatibility field, and a synthetic candidate/reference
comparison proves each dynamic ratio is taken from the corresponding metric
rather than from static totals.

At the current unsigned revision, the real nbody input produces this raw
profile-free evidence:

| listing | normalized static ins/load/store | estimated dynamic ins/load/store/branch |
|---|---:|---:|
| qbopt 386 medium model | 277 / 146 / 85 | 2085 / 1234 / 485 / 20 |
| Clang 21 flat i386 | 290 / 74 / 51 | 1174 / 328.5 / 189.5 / 6 |
| GCC 16.2 flat i386 | 263 / 71 / 106 | 5131.5 / 1438 / 1228 / 169 |

The static load comparison says qbopt is about twice either reference; the
frequency-weighted estimate says it is 3.76x Clang but 0.86x GCC.  That
direction reversal proves static memory counts cannot identify a runtime
traffic gap across these different unroll/loop choices.  None of the dynamic
ratios is published: nbody's `steps` argument is not statically known and all
three estimates use at least one profile-free ten-trip fallback.  Clang and
GCC remain best-case structural listings; a runtime profile or independently
audited medium-model target is still required before changing optimization
policy from these numbers.

The authoritative report and raw listings are under
`build/quality/iter117`.  The source hash is
`4086d97bc336ee110eaf1ae04c822cd47f9817f94b1e89599e2b2f65a76a6c43`;
the report records Apple Clang 21.0.0 and i686-elf GCC 16.2.0 with strict x87,
SSE-disabled flags.  This advances Phase 1's measurement trustworthiness; it
does not claim a Phase 4 x87 improvement or a final nbody target.  Focused
measurement checks pass (`6 passed`, `52 deselected`, `0.34s`) and Tier 1
passes (`235 passed`, `31 deselected`, `2.37s`).

### 109. See physical scale/add tails through source anchors — 2026-09-18

The retained matmul row loop still emitted a loaded term as a separate shift
and accumulation.  Clang's flat-i386 best-case loop consistently uses a
scaled LEA for the same accumulator shape:

```asm
; before
mov edx, dword ptr [bp-526]
shl edx, 1
add ecx, edx

; after
mov edx, dword ptr [bp-526]
lea ecx, [ecx+edx*2]

; Clang's structural reference uses the same form
lea ecx, [ecx+2*edx]
```

The first synthetic regression established that metadata-only `NOTHING`
anchors must not hide physically adjacent work, then passed while the
production listing did not change.  Narrow diagnostics over the raw allocated
LIR found exact use count two and genuinely empty intervening anchors; the
unrolled load/shift/add operations themselves all retained conservative
`symbol=True` ownership.  Thus adding a broader liveness model would have
answered a question the production candidate was not asking and was
discarded.

The selector now walks across only no-byte anchors with no virtual edge and
keeps the existing whole-body exact-use proof, so a shifted value read in a
successor still rejects the fold.  The load and resulting ADD survive the
rewrite and retain their ownership markers.  A register-only shift cannot
carry a relocation, so it becomes an explicit no-byte LIR anchor instead of
being deleted.  The fail-first regression recreates the complete symbolic
chain, asserts the owner transfer, retains both intervening anchors, and
passes LIR verification.  This is the same general provenance normalization
already required for register-only extension followers, not an exception for
matmul.

On 386 the authoritative matmul result changes from `1626/422/1840` to
`1625/421/1837` bytes/instructions/weighted cost.  The one selected form is
inside the exact eight-iteration row loop, so the profile-free dynamic
estimate falls `1031 -> 1023`.  Loads (`108`), stores (`88`), spill traffic,
branches, and calls do not change; address calculations rise `44 -> 45` as
the scale moves into LEA.  ABI-normalized instructions are now 413 versus
Clang's 356 (still 1.16x).  The unchanged GCC/Clang listings remain
best-case flat-i386 structural references, not medium-model hard targets.

The authoritative exact-state candidate is `build/quality/iter115/report.json`,
with reference listings under `build/quality/iter109`.  Its assembly is
byte-identical (SHA-256 `e59d6e157daf83a95b439ae84dd2e1ba6f4361df7d494ef12510566a688b6582`)
to the saved listing JWasm assembled for iteration 114.  The VBDOS linker
accepted it, and DOSBox-X returned the independent result `353712`
(`b0 65 05 00`).  Focused loaded-scale tests pass (`3 passed`,
`192 deselected`, `0.20s`) and Tier 1 passes (`235 passed`, `31 deselected`,
`2.30s`).  This advances Phase 7 while preserving hard
gates for flags, whole-body value use, register overlap, target cost, source
byte ownership, and relocation-bearing operands.

### 108. Select source-owned constant sums as costed 67h LEAs — 2026-09-18

The retained-row matmul listing still initialized 28 scalar cells with a
copied two-address add.  Clang's flat-i386 best-case listing uses one LEA for
every corresponding expression.  A 16-bit destination makes the low word of
the 32-bit effective address exactly the required wrapping result, and the
address-size override remains cheaper and one byte smaller than the pair:

```asm
; before (5 bytes, two priced operations)
mov bx, ax
add bx, 1

; after (4 bytes, one priced operation; 67h is emitted)
lea bx, [eax+1]

; Clang's best-case flat-i386 structural reference
lea ecx, [eax+1]
```

The first fail-first regression established constant displacement selection,
signed modulo normalization (`65535` is displacement `-1`), and a byte-growth
guard for larger immediates.  That synthetic case passed while the production
listing did not change, so the instrument was wrong before the subject was:
the test omitted the conservative source-map marker carried by every unrolled
ADD clone.  A second fail-first regression reproduces that exact ownership.
The fold now proceeds only when the one symbolic owner is the instruction
that survives as the LEA; ownership on the removed copy, or two independent
owners, remains a rejection.  Thus no relocation or source anchor is silently
dropped merely to obtain the shorter form.

The same ownership rule also exposes one existing register-sum opportunity,
so the authoritative matmul listing replaces 29 `mov; add` pairs in total.
On 386 it changes from `1655/451/1898` to `1626/422/1840`
bytes/instructions/weighted cost.  The profile-free dynamic estimate falls
`1067 -> 1031`; loads, stores, spill traffic, calls, and branches are
unchanged.  ABI-normalized static instructions fall `443 -> 414`, putting
qbopt at 1.16x Clang's 356-instruction structural reference (previously
1.24x).  Its 108 loads and 88 stores remain 1.33x and 1.13x Clang's 81 and
78; dynamic comparison remains withheld because the reference trip count is
heuristic.  GCC's structurally different near-expansion remains advisory, not
a target.

The authoritative candidate artifact is
`build/quality/iter111/report.json`; the unchanged GCC/Clang source listings
are retained under `build/quality/iter109`.  The saved qbopt listing assembled
with JWasm, linked with the VBDOS linker, and executed under DOSBox-X returns
the independent result `353712` (`b0 65 05 00`).  The focused address-form
file passes (`17 passed`, `4 deselected`, `0.06s`) and Tier 1 passes
(`235 passed`, `31 deselected`, `1.96s`).  This advances Phase 7 without
claiming that 67h always wins: the selected CPU profile, partial-register
penalty, exact encoding length, dead flags, and source ownership all remain
hard profitability or legality gates.

### 107. Transact converged aggregate leaves with register pressure — 2026-09-18

Matmul's exact structural clone crossed SROA before scalar convergence.  The
convergence itself made another set of indexed aggregate accesses singleton,
so the candidate was priced before those leaves existed and committed in a
different memory shape.  The structural transaction now crosses SROA again,
reconverges scalar MIR without opening another structural search, and prices
the settled result before accepting it.

The first implementation exposed why this must be a pressure-aware
transaction rather than an unconditional second SROA run.  It produced MIR
with one load and no stores, then allocation recreated 576 loads and 383
stores: 846 final instructions at a 4,803 weighted 386 cost, worse than the
3,396 baseline.  A single maximum pressure peak did not see the defect because
matmul has successive disjoint groups of leaves.  The machine-neutral model
now retains each cheapest whole-live-range spill choice across program points;
overlapping peaks share that choice, while disjoint waves pay for independent
stores/reloads.  Literal and fixed-address reconstruction retain their cheaper
rematerialization price.

SROA profitability is intentionally asymmetric.  The unscalarized side pays
its explicit aggregate loads and stores through ordinary semantic cost; also
charging every address and stored value as a full-lived register double-counted
that memory representation (8,815 estimated versus 3,396 after allocation).
The settled side has removed those homes and therefore adds finite-capacity
whole-range spill traffic.  This is an aggregate-replacement rule, not a
matmul name, array-size threshold, register name, or CPU allow-list.

```asm
; before: first of eight cloned row bodies (the next begins immediately)
movsx eax, word ptr [bp-132]
movsx ebx, word ptr [bp-130]
mov dword ptr [bp-520], ebx
; ... seven specialized column expressions ...
mov dword ptr [bp-516], edx

; after: one row loop, eight specialized column expressions
mov bx, word ptr [bp-518]
shl bx, 5
movsx ecx, word ptr ss:[si]
movsx eax, word ptr ss:[si+2]
lea eax, [ecx+ecx]
add eax, dword ptr [bp-526]
mov dword ptr ss:[bx], eax
add word ptr [bp-518], 1
cmp word ptr [bp-518], 8
jb  L0_68
```

On 386, qbopt changes from `3644/906/3396` to `1655/451/1898`
bytes/instructions/weighted cost.  Loads fall `305 -> 108`, stores
`192 -> 88`, spill reloads `0 -> 4`, and spill stores `56 -> 9`.  The selected
row loop raises the exact/profile-free dynamic estimate `906 -> 1067`
(+17.8%) while lowering target-weighted cost 44.1%; this is an explicit
386 memory-traffic tradeoff rather than a claim that every instruction has
equal runtime cost.

Clang's current flat-i386 reference makes the same broad row-loop choice and
reports 356 normalized static instructions, 81 loads, 78 stores, and 879
estimated dynamic instructions.  qbopt is now 1.24x/1.33x/1.13x on the first
three structural measures; the dynamic comparison remains withheld because
the reference uses a heuristic trip count.  GCC instead nearly expands the
nest and reports 849 normalized instructions, so qbopt is 0.52x its static
count.  Neither flat ABI is promoted to a medium-model hard target.

The authoritative artifacts are `build/quality/iter109/report.json` and its
raw qbopt/GCC/Clang listings.  Assembling the saved qbopt listing, linking it
with the VBDOS linker, and executing it on the real DOS 386 returned the
independent matmul result `353712` (`b0 65 05 00`).  Fail-first regressions
cover leaves exposed only after scalar convergence, a settled SROA pressure
loss, one unavoidable pressure peak, two independent spill waves, and the
separate x87 value class not consuming integer-register capacity.

The instrumented quality run still takes 3:43 for this pathological candidate.
An attempted immediate post-SROA rejection reduced compile time but restored
the inferior 3,396-cost body because DSE and scalar folding had not yet exposed
the candidate's benefit; it was discarded.  The next compile-time iteration
must accelerate or cache the settled fixed point rather than reject it before
its semantics are visible.

### 106. Fold terminal offsets without forcing a worse address class — 2026-09-18

The retained-row matmul experiment exposed a boundary defect before it
exposed an allocation policy.  Its MIR expressed every cell after the first
as a copied base plus a constant, and lowering correctly changed the memory
operand to `ss:[base+disp]`.  It nevertheless emitted the original symbolic
ADD as well.  Machine DCE must retain symbolic operations because they may
own relocations; the first wrong stage was therefore lowering, not DCE.

Lowering now treats a copied or constant-adjusted 16-bit address as completely
folded when all of its readers are encodable based cells, its flags are dead,
and it has no ordinary or phi use.  Both FAR cells and non-relocated LITERAL
cells can own the displacement.  SEGMENT, EXTERNAL, and GROUP relocations
cannot: a fail-first negative regression showed that the initial rule deleted
the ADD feeding a SEGMENT cell even though the relocation still required its
dynamic result.  The final proof checks every cell using the value and refuses
the fold if any one has a relocated or otherwise unencodable displacement.

On the experimental retained-row body this changed
`1655/451/1067` bytes/static instructions/dynamic operations from
`1753/482/1315`, removing 31 static and 248 estimated dynamic operations.  It
does not change the current production matmul winner, whose completely
expanded body remains at 906 dynamic operations, so these are candidate
diagnostics rather than a claimed benchmark-gate improvement.

The next tempting fold was wrong.  Encoding the parent row as
`ss:[bp+index+disp]` constrained the long-lived output index to the 16-bit
address register class.  Allocation then spilled that index and reloaded it
before every output store; the candidate regressed to
`1691/454/1091`.  That experiment and its structural-settle transaction were
discarded.  The structural transaction was also responsible for pathological
corpus compile time.  The result confirms the intended ordering: use native
16-bit addressing only when its class pressure is affordable, otherwise
consider 67h before spill or recomputation.

A separate post-allocation selector now catches the residual
`mov temp,[spill]; shl temp,k; add total,temp` artifact.  When the ADD flags
are dead, the selected CPU prices prefix plus address formation no higher than
the shift and add, and a whole-body SSA use count proves no successor needs
the shifted temporary, it emits the load followed by
`67h lea total,[total+temp*scale]`.  The whole-body condition is regression
tested: the first block-local implementation silently changed a value read by
a successor.  This cleanup cannot substitute for pressure-aware preallocation
selection, but it removes a shift when a legal spill has already happened.

Both accepted changes have fail-first positive and refusal regressions.  The
focused address and peephole sets pass, as does the ordinary pre-commit pytest
gate.  The two implementation commits are explicitly unsigned.  GCC and
Clang remain flat-i386 best-case structural references; BCC/WC remain the
authority for medium-model segment, relocation, ABI, and legal-address
behavior.

### 105. Audit nested peeling before changing its policy — 2026-09-18

The refreshed 386 corpus makes matmul the largest current structural outlier:
3,644 bytes and 906 instructions, against Clang's normalized 356 instructions
and GCC's 849.  The raw listings show that these references choose different
strategies: Clang retains a row loop while specializing columns and products;
GCC nearly completely expands the nest; qbopt completely expands it.

The unroll trace explains how qbopt gets there.  Successive profitable
candidates add `3`, `31`, `272`, `196`, and `75` MIR operations.  Each loop
has only eight trips, so the existing trip-count bound is not a useful bound
on the composed nest.  However, adding a second static expansion ceiling was
not a valid solution.  A 200-operation ceiling reduced matmul to 498 static
instructions but increased its dynamic estimate from 906 to 2,731, and a
cumulative 320-operation budget increased it to 3,844.  Ranking candidates by
saving/growth and charging semantic static cost also produced slower results.
Those experiments were discarded rather than landing a smaller but slower
program.

The raw Clang listing identifies a more general missing mechanism: it proves
the fixed values in `b[8][8]` and neither materializes the 64-element constant
array nor reloads it through indexed memory.  qbopt still emits 64 constant
stores followed by indexed loads.  Bounded indexed-array scalar promotion is
therefore the next SROA step; loop-policy selection should be measured again
after that changes the candidate bodies.

The production results remain 906 estimated dynamic operations for matmul and
2,085 for nbody.  Fresh OMF emission, LINK, and real DOS execution during the
audit returned the independent answers `353712` and `4774160`.  One attempted
scalar-only peel boundary regressed nbody to 13,959 while passing its former
20,000-operation test.  The regression now uses a deliberately loose 3,000
ceiling, which fails on that observed defect without pinning exact bytes or a
single allocation.

### 104. Bound complete peeling without blocking constant collapse — 2026-09-18

The current 386 Shellsort listing was not a memory-folding failure.  Raw
assembly showed that exact CFG peeling had expanded both 64-trip setup and
checksum loops.  The initializer alone became 64 separately encoded stores;
the final function was 1,838 bytes and 463 instructions.  Its profitability
transaction priced the saved branch work, but had no full-peel count budget,
so a 3.6% estimated semantic-cost saving could multiply the whole body.

GCC's local source defines `max-completely-peel-times` as 16.  Both installed
i686 GCC and Clang retain Shellsort's 64-store initializer; Clang separately
specializes other loops, while GCC retains the directly comparable loop
structure.  The immutable CPU profile now carries that 16-iteration default
and every C, BASIC-object, and shared-flow entry point threads it into the
machine-neutral `Where` context.  The one shared unroll/peel profitability
gate rejects an over-budget candidate only when its optimized body is larger
than the original.  A long exact loop which folds to no growth therefore
remains eligible, as do all normally costed candidates at or below the
profile limit.  No pass names a CPU or frontend.

```asm
; before: one of 64 separately encoded initializer elements
mov ebx,eax
xor ebx,37
mov word ptr [bp-132],bx
; ... 63 more constant-displacement stores

; after: the retained recurrence and one store site
mov dx,cx
shl dx,7
xor edx,ebx
xor edx,eax
lea si,[ecx+ecx]
mov word ptr ss:[bp+si-132],dx
add cx,1
add ebx,109
cmp cx,64
jb  L0_8
```

On 386, Shellsort changes from `1838/463/1829` to `192/66/255`
bytes/instructions/static weighted cost; static stores fall from 68 to 5 and
spills from one reload to none.  The final ABI-normalized body has 58
instructions versus GCC's 57.  Its exact/profile-free mixed dynamic estimate
is 6,727 versus GCC's 6,506.37 (1.034x).  This is an explicit size/runtime
tradeoff against qbopt's previous expansion: qbopt's own estimate rises from
6,016 by 11.8%, while bytes fall 89.6%.  P6 similarly changes from
`1779/447/769` and 6,453.68 estimated operations to `192/70/58` and 7,336.68;
the missing next mechanism is bounded partial unrolling, not restoring a
64-way static clone.

Every final profile emits 192 bytes: 386 `66/255`, 486 `66/173`, P5
`66/112`, P6 `70/58`, K5 `68/36`, K6 `68/37`, K7 `68/45`, and Core `70/50`
instructions/weighted cost.  The production symptom regression failed first
at 68 static stores and now requires fewer than 16; a Tier-1 unit regression
failed first on the absent profile interface and covers over-limit growth,
growth erased by folding, and at-limit profitability.  Independent per-CPU
hard targets remain unregistered, so the GCC comparison is audited structural
evidence rather than a completed acceptance claim.  Tier 1 remains bounded at
224 passes in 1.51 seconds, and the affected fresh-OMF/LINK/DOS run returns
Shellsort's independent oracle value, `6384478`.

### 103. Rewind exact nested recurrences by target cost — 2026-09-18

Mandelbrot's column coordinate reaches `xStart + 32 * 24` on the only exit
from every row, then the next row copied the separately saved `xStart` back
into its frame cell.  GCC likewise reloads a saved row start; Clang rebuilds
the coordinate from its counters.  Those flat-i386 forms are structural
references rather than automatic winners under the six-register medium-model
ABI.  The third general formula is to carry the completed recurrence around
the outer backedge and subtract its proved distance once.

The new MIR transform accepts only one exact inner exit contained by one
outer loop, requires the exit value to dominate the outer latch, preserves an
explicit loop-closed exit value, and refuses another use of the saved start.
Pre-tested loops rewind the exiting header phi; post-tested loops rewind the
update.  A start defined inside the outer loop or known/rematerializable as a
constant is refused.  That last rule was found from raw production output:
the first implementation rewound Mandel's constant `px = -16` control before
the coordinate recurrence existed and grew P6 from `199/55/88` to
`219/62/95` bytes/instructions/weighted cost.

Profitability is semantic and per-profile.  The recurrence is considered
only at register-capacity pressure, when ADD is no dearer than MOVE and a
memory update is no dearer than load plus store.  Thus 386, 486, and P5 keep
the two-MOV copy; P6, K5, K6, K7, and Core select the rewind.  This rule is
not a CPU-name allow-list.  Address formulas continue to use the independent
ordering native 16-bit form, then legal costed `67h`, then spill/recompute;
this scalar recurrence has no competing address form.

```asm
; before, once per row
mov eax,dword ptr [bp-8]
mov dword ptr [bp-20],eax

; after, once per completed row
add dword ptr [bp-4],-768
```

The exact trip proof exposed two measurement defects.  Threading an empty
proved outer header redirected its backedge into the inner header and merged
two natural loops, reporting an impossible `74839 -> 1065` dynamic change.
Exact zero-byte headers now remain CFG anchors.  Rotation then changed the
inner header without moving its retained trip fact, producing another false
ten-trip estimate.  MIR records exact positive counts whose syntactic proof a
semantics-preserving transform consumed; rotation remaps the fact to its new
header, lowering retains it only for a still-existing natural loop, and jump
threading preserves that header without emitting a byte.  The corrected hand
check is exact: one fewer executed instruction on each of 24 rows,
`74839.105263 -> 74815.105263`.

P6 changes `199 -> 199` bytes, `55 -> 54` instructions, and weighted cost
`88 -> 84`.  Final all-profile results are: 386 `199/55/285`, 486
`200/56/224`, P5 `199/55/143`, P6 `199/54/84`, K5 `199/54/37`, K6
`199/54/37`, K7 `199/54/43`, and Core `199/54/68`.  The first semantic
change is MIR `r03-strength`; the accepted final CFG retains outer/inner
counts 24 and 32.  Fail-first regressions cover the pre-tested exit, the
slower 386 refusal, the constant-control regression, exact-count retention
through rotation, and zero-byte-header threading.  Independent hard targets
remain unregistered, so this is an audited candidate improvement rather than
a final parity claim.  Tier 1 remains inside its budget at 223 passes in
1.46 seconds, and the affected fresh-OMF/link/real-386 Mandel executable
returns its independent answer, `8873`.

### 102. Shuttle spilled parallel copies through a dead register — 2026-09-18

Iteration 101 exposed a post-allocation artifact in Mandelbrot's row reset.
Parallel-copy scheduling must remain correct when all registers are live, so
its universal frame-to-frame spelling is a balanced PUSH-memory/POP-memory.
At this particular edge EAX is dead and immediately redefined.  LLVM's
`TuningSlowTwoMemOps` describes the general target rule: memory PUSH/POP forms
should be unfolded through a register where one is available.

The post-allocation pass now uses whole-body physical liveness to choose a
same-width dead GPR, verifies both selected MOV forms, and compares the four
form costs in the selected CPU profile.  It is restricted to the synthetic
same-address pair emitted by parallel-copy scheduling; source PUSH/POP traffic,
live scratch lanes, flags, relocation ownership, frame adjustment, and
non-frame operands are refused.  No register is reserved, and the stack form
remains the correct fallback when pressure really consumes all six GPRs.

```asm
; before
push dword ptr [bp-8]
pop  dword ptr [bp-20]

; after
mov eax,dword ptr [bp-8]
mov dword ptr [bp-20],eax
```

This also found a measurement defect: `cycles.classify` charged POP-memory as
`pop_r`.  The measurement regression failed first.  The 386 and 486 entries
now use the Intel instruction tables (5 and 6 clocks); P5/P6/K6/K7/Core use
the local GCC scheduling descriptions, with K5 conservatively inheriting the
K6 entry because GCC has no distinct K5 schedule.  Every CPU profile prices
the emitted form explicitly.

On 386 the row-reset pair falls from `6 + 5 = 11` weighted units to `4 + 2 =
6`; over 24 rows that removes 120 dynamic weighted units.  Corrected whole-body
static cost falls `290 -> 285`.  Bytes (`199`), instructions (`55`), the
independently checked dynamic instruction estimate (`74839.105263`), loads,
stores, and spill counts are unchanged because this is a form-quality change,
not a MIR operation deletion.  The first changed stage is post-allocation
Peephole.  Native BP-displacement addressing expresses both operands, so the
secondary `67h` form is not involved; its native/secondary/spill ordering is
unchanged.

The fail-first Tier 1 regression asserts the Mandel shape and the chosen EAX
shuttle. Companion regressions keep the stack pair when no register is dead
and refuse a source-backed PUSH/POP pair. The Intel/GCC timing evidence and
the classification correction are recorded in `docs/timing-audit.md`.

### 101. Hoist complete nested-loop initializers — 2026-09-18

Mandelbrot's column recurrence starts at `xOffset - 512` and stops at
`xOffset + 256`.  Both expressions are invariant across all 24 rows, but LICM
treated every value which initialized a nested phi as a mutable reset.  That
conservative rule was necessary for the historical SEGld failure: moving its
literal counter reset made the next outer iteration start from the preceding
iteration's final counter.  It was too broad for a complete SSA result.  A
pure expression has its own immutable value; the nested recurrence updates
later versions, not the result which initialized it.

LICM now admits a single complete non-floating result with no memory,
barrier, merge, stack effect, or readable flags.  COPY, carry/borrow, and
two-result divide forms remain excluded.  The first changed production stage
is MIR `r03-hoist`; the reset copy itself still executes on every outer
iteration.

```asm
; before: six instructions on every one of 24 rows
mov eax,[xOffset]
mov [x],eax
add [x],-512
mov eax,[x]
mov [xEnd],eax
add [xEnd],768

; after: four instructions once, two on every row
add [xStart],-512
mov eax,[xStart]
mov [xEnd],eax
add [xEnd],768
row:
push dword ptr [xStart]
pop  dword ptr [x]
```

The raw control count is independently checkable: `6 * 24 = 144` dynamic
setup instructions becomes `4 + 2 * 24 = 52`, exactly the quality report's
`92`-instruction reduction.  Every CPU profile loses two bytes and one spill
store, with unchanged raw instruction, load, and store counts.  Estimated
dynamic operations fall by 92 on all eight profiles.  The straight-line
ranking rises on 386 (`285 -> 289`), 486 (`224 -> 230`) and P5 (`143 -> 144`),
is unchanged on P6/K5/K6, and improves on K7 (`46 -> 44`) and Core (`68 ->
67`).  This is an audited hot-path trade: using the profile's in-order form
costs, the changed setup alone falls from 672 to 262 cycles on 386 and from
240 plus 144 operand-prefix clocks to 200 plus 52 prefix clocks on 486.  The
static ranking counts each textual instruction once and therefore cannot
represent that repetition.

GCC's flat-i386 listing still computes `xOffset - 512` once per row; Clang
rebuilds the affine column expression per pixel.  They remain useful
structural references, not candidate-ABI authorities.  The fail-first Tier 1
regression reproduces the complete computed initializer and distinguishes it
from the recurrence step; the existing SEGld regression continues to protect
the mutable COPY case.

The address-form terminology is also made unambiguous in production code: the
legal non-native form is now named `secondary`, and selection remains native
16-bit form, then costed `67h`, then spill or recomputation.  The former
`fallback` constructor/attribute remains as a synchronized compatibility alias
for existing Python callers.  The extra prefix byte is kept separate from its
target-specific execution cost.

### 100. Fold allocated sums through the secondary 67h form — 2026-09-18

The refreshed C comparison showed that nbody's normalized instruction count
was already between Clang and GCC (`277` versus `290` and `263`), despite its
different x87 memory balance. Mandelbrot retained a smaller but concrete
machine-form gap shared by both references: qbopt emitted `mov edi,edx / add
edi,esi / cmp edi,1024`, while GCC and Clang both emitted one LEA for the
dead-flags sum.

The post-allocation address selector now recognizes the general three-address
shape `result := left + right` after two-address allocation has spelled it as
a copy followed by ADD. When ADD's flags are dead, both operands are exact
same-width allocated registers, and the selected CPU prices the replacement
no higher, it emits one 32-bit-address LEA. In 16-bit code this is the legal
`67h` form. Word results remain exact because the low sixteen bits of the
32-bit effective-address sum equal modulo-16-bit addition; profile-specific
partial-register penalties are charged for each distinct word input.

This is a secondary address form, not a last-resort spill form. Native 16-bit
addressing remains preferred where it can express the operation. The complete
loop-form ordering established in iteration 95 is unchanged: native form,
then legal costed `67h`, then spill or recomputation. This iteration applies
the same legal form to a sum exposed only after physical allocation. It rejects
live flags, stack-pointer indexes, mismatched widths, relocation ownership,
parallel copies, frame adjustments, clobbers, and nontrivial source spans.

```asm
; before
mov edi,edx
add edi,esi
cmp edi,1024

; after
lea edi,[edx+esi]       ; address-size override 67h
cmp edi,1024
```

The fail-first Tier 1 regression initially retained MOV/ADD for both word and
dword sums. It now verifies one LEA, both SSA inputs and the final definition,
the actual `67h` prefix, and the word/dword forms; a companion regression
proves that a conditional branch observing ADD's flags retains the original
instructions. The emitted 386 Mandelbrot body changes `202 -> 201` bytes,
`56 -> 55` raw instructions, and weighted cost `287 -> 285`. Its normalized
body changes `48 -> 47` instructions, against the unchanged advisory Clang
`52` and i686 GCC `45` listings. The first changed stage is post-allocation
Peephole. No hard candidate-ABI target is registered by this comparison.

GCC/LLVM listings remain best-case flat-i386 structural references; BCC/WC
remain authoritative for medium-model ABI, segment, and legal-address
semantics.

### 99. Count dead dynamic induction variables down on their flags — 2026-09-18

After iteration 98 hoisted `floats`' immutable bound, the raw listing still
spent three hot integer operations controlling an otherwise x87-only loop:
`add ax,1 / cmp ax,bx / jb`.  Clang's strict-x87 reference instead consumes
the dynamic trip count as a countdown.  GCC retains the ordinary up-counter;
both are structural references, while the selected form remains constrained
by the medium-model ABI.

The new MIR formula applies to a canonical unsigned `0..n-1` loop only when
the induction value has no observable use.  It tests `n` once before entry,
moves `n` into the loop phi, replaces the unit increment with a decrement,
and makes the backedge branch consume that decrement's flags directly.  The
entry guard preserves the source's zero-trip behavior.  Multi-block bodies,
exit phis, non-unit or wrapping recurrences, shared comparison/step flags and
any non-control counter use are refused rather than partially rewritten.  The
first changed stage is the final MIR `rotate` stage:

```asm
; before
xor ax,ax
mov bx,word ptr [bp+6]
loop:
    ; floating body
    add ax,1
    cmp ax,bx
    jb loop

; after
mov ax,word ptr [bp+6]
or ax,ax
je exit
loop:
    ; floating body
    dec ax
    jne loop
```

The fail-first compiled regression initially found no decrement.  The fast
semantic regression now requires both the one-time `EQ` guard and a loop
backedge whose `NE` condition reads the decrement's own flags, so an unsafe
post-test cannot satisfy it; a companion case proves an observed source
counter refuses the rewrite.  The actual C benchmark has its own emitted-shape
regression.  Real DOS executions independently return `1000` for the zero-trip
call and the corpus oracle `162635` for 1000 trips.

The first quality run implausibly reported dynamic work `96 -> 57`.  The raw
change removes one instruction per executed trip, not 39.  The instrument had
treated the new guard as a generic 50/50 branch while treating the equivalent
old pretest as the documented 90/10 loop convention.  A second fail-first
regression now covers guarded natural-loop frequencies, and the estimator
recognizes only the exact shape “one successor is this loop's header and the
other bypasses it.”  The corrected dynamic estimate is `96 -> 85`; the false
`57` result is recorded here so it is not quoted as a speedup.

All eight CPU profiles emit the same 91-byte, 29-instruction body, down from
95 bytes and 30 instructions.  Static weighted cost changes `355 -> 353` on
386, `273 -> 272` on 486, `107 -> 106` on P5, and is unchanged on P6, K5,
K6, K7 and Core (`137, 141, 134, 109, 133`).  Loads, stores, branches, peak
live values and spill traffic are unchanged.  The regenerated strict-x87
references report 25 normalized candidate instructions against 26 for Clang
and 25 for i686 GCC; those flat-i386 counts remain advisory, not a registered
medium-model target.  The three fast countdown/measurement checks pass
(`3 passed`, `0.09s`), the actual C shape check passes (`1 passed`, `0.22s`),
and Tier 1 passes (`213 passed`, `31 deselected`, `1.75s`); no phase-boundary
full suite was run.

### 98. Hoist disjoint work around precise volatile accesses — 2026-09-18

The next raw C/reference comparison found a concrete loop miss in `floats`.
qbopt loaded the immutable `iterations` argument from `[bp+6]` at every trip;
GCC retains the bound in a register and Clang converts the loop to a
countdown.  The x87 body uses no general registers, so this was neither
medium-model pressure nor an ABI limitation.

Adjacent MIR dumps first differ at `r01-hoist`.  Before the change that pass
is a no-op: `_invariant_run` refuses the entire loop as soon as it sees the
volatile double store.  MIR already distinguishes a source-language volatile
access, whose explicit memory footprint is complete, from an opaque machine
barrier.  The LICM gate had collapsed them back together.  It now retains the
blanket refusal for calls, escapes, and genuine opaque barriers, while allowing
pure nonvolatile work on proven-disjoint storage to be considered around a
precise volatile access.  The volatile operation itself remains immovable, so
the observable volatile sequence is unchanged.

The fail-first MIR regression requires a disjoint frame load to leave a loop
containing a precise volatile store and, in the same test, proves that replacing
the store with an opaque machine barrier still refuses the move.  The compiled
C regression checks the emitted argument cell occurs exactly once and outside
every natural loop.  The resulting change is intentionally small:

```asm
; before
loop_test:
    mov bx,word ptr [bp+6]
    cmp ax,bx
    jb loop_body

; after
    mov bx,word ptr [bp+6]
loop_test:
    cmp ax,bx
    jb loop_body
```

Bytes, static instructions, static weighted costs, loads, stores, branches,
peak live values and spill counts are unchanged on all eight CPUs.  Under the
quality tool's explicit ten-trip convention for this input-dependent loop,
estimated executed instructions fall `105 -> 96`: exactly nine avoided loads.
Every CPU emits the same placement, so none regresses.  GCC and Clang are
structural evidence for retaining or consuming the bound; they remain flat
i386 references rather than medium-model targets.

The independent DOS known-answer run still returns `162635`.  Both fail-first
checks pass (`2 passed`, `0.19s`), and Tier 1 passes (`210 passed`, `31
deselected`, `1.51s`).  No phase-boundary full suite was run.

### 97. Carry profitable complete affine formulas through pressure — 2026-09-18

The full C corpus first exposed two superficially large listing gaps that were
not both optimization defects.  Matmul had 906 candidate instructions against
Clang's roughly 360 because Clang retains a row loop while qbopt and GCC 16.2
fully expand it.  A measured peel cap reduced static size (`3644 -> 3130`
bytes and `906 -> 770` instructions) but raised estimated dynamic work
`906 -> 3315` and weighted 386 cost `3396 -> 4515`; that size-for-speed trade
was rejected.  Shellsort has the same fully-expanded-versus-looped distinction.
Neither reference listing is being mistaken for an ABI-equivalent target.

Mandelbrot was different.  GCC carries its two 32-bit coordinates and advances
each by 24; Clang rebuilds them with flat-mode LEAs.  qbopt rebuilt both in
frame slots from 16-bit `px`/`py` counters.  Adjacent MIR dumps first differed
at strength selection: induction analysis already proved the extended affine
forms, but the selector's profitable-overflow result never reached the rewrite.
After `_formula_set` had compared spill traffic with recomputation, a second
`added >= room` gate silently imposed a register-only budget.  Removing that
contradictory gate lets a formula which is cheaper even when spilled reach the
allocator.  Cheap overflow formulas are still rejected by the selector.

The first fail-first compiled regression then reduced only the outer formula.
The inner leaf denotes the complete `24*x - 128 + seed` expression, but its
cost was priced as its final addition.  Complete-formula pricing now uses the
canonical scale, every invariant offset, and the final pointer formation when
present.  A fast fail-first policy regression records that distinction.  This
is the resulting central listing:

```asm
; before: rebuilt from a narrow counter in the loop
movsx eax, ax
mov dword ptr [bp-4], eax
shl dword ptr [bp-4], 1
add dword ptr [bp-4], eax
shl dword ptr [bp-4], 3
sub dword ptr [bp-4], 128
add dword ptr [bp-4], seed
add word ptr [bp-20], 1

; after: complete 32-bit coordinate recurrence
add dword ptr [bp-16], 24
mov eax, dword ptr [bp-16]
cmp eax, dword ptr [bp-20]
jne inner
```

The 67h ordering from iteration 95 is unchanged: a proven legal native or
costed address-size-override form is considered before this spill/recompute
choice.  Mandel's coordinates are arithmetic values consumed by the fixed
point kernel, not memory indexes, so 67h is not a competing spelling here.

The first post-change quality report claimed dynamic work fell
`85553 -> 24713`.  That contradicted the raw instruction delta and was rejected
as an instrument result.  Strength reduction had converted the inner exit to
a rotated equality against `start + 768`; exact-trip analysis understood that
symbolic sentinel only in a pre-tested header.  The estimator consequently
substituted ten trips for the real 32.  A fail-first synthetic regression now
covers rotated symbolic sentinels, and lower again records the exact count.
The corrected dynamic estimate is `85553 -> 78569` (8.2%), consistent with the
raw loop.  The invalid `24713` is retained here explicitly so it cannot be
quoted later.

On 386, Mandel falls `233 -> 202` bytes, `68 -> 56` instructions, weighted
cost `327 -> 287`, loads `20 -> 15`, stores `17 -> 10`, and spill reloads
`2 -> 1`.  The final listing is byte-identical across all eight profiles;
their post-change weighted costs are 287, 224, 144, 80, 38, 38, 46 and 68 for
386 through Core.  GCC's recurrence is structural evidence for the selected
shape; Clang's LEA spelling remains useful evidence for targets with different
register and addressing constraints.  No hard target is registered yet.

The complete-formula and 67h selector checks pass (`3 passed`, `0.17s`), the
post-tested exact-count checks pass (`2 passed`, `0.07s`), the compiled
Mandel listing check passed after the code-generation change (`1 passed`,
`79.19s`), and the final quality run independently confirms the corrected
exact-trip metric and identical final assembly.  Tier 1 passes (`210 passed`,
`31 deselected`, `1.49s`).  No phase-boundary full suite was run; focused test
execution remained bounded while the corpus and eight-profile measurements
dominated this iteration.

### 96. Unfold dying indexed bases before frame spill — 2026-09-18

`farloadloop._mark` still shifted both mutually exclusive indexes through one
frame slot. Lowering had correctly folded each `base + index` into its far
memory operand, but in 16-bit mode that confined both short-lived indexes to
the four address registers. The allocator therefore spilled them even though
the base dies at each access and an explicit `add base,index` can consume the
index from any word register. The spill recovery already emitted exactly that
add, so keeping the folded spelling was buying no dynamic address operation.

The allocator now compares a general dying-base unfolded form before the
frame-spill form. It accepts the trial only when the complete allocation
protects the selected long-lived owners and strictly reduces loop-weighted
spill traffic. The same trial receives ordinary constant/frame/address
rematerialization before comparison. A semantic last-use proof guards the
destructive base add. While writing its fail-first coverage, a second red
regression exposed that the old proof counted only encoded memory-base uses;
it could therefore mutate a base read later as an ordinary register. A third
red case showed that the final instruction can also use its base or index as
data. The proof now counts every LIR semantic use once per instruction and
requires every same-instruction occurrence to belong to the indexed cells.

Adjacent dumps first differ at `RegAlloc`: the two frame-cell shifts become
register shifts followed by the same two base adds; MIR and lowering remain
unchanged. The central hot-path change is:

```asm
; before
mov word ptr [bp-2], ax
shl word ptr [bp-2], 1
add di, word ptr [bp-2]

; after, 386/486/P5/K5/K6/K7
lea cx, [eax+eax]
add di, cx

; after, P6/Core
mov cx, ax
shl cx, 1
add di, cx
```

The last distinction matters. The first candidate run improved six profiles
but regressed P6 and Core because the post-allocation peephole read all of EAX
after the loop had written AX. The fail-first target regression records that
symptom. Scaled-LEA selection now prices the address prefix and the profile's
16-to-32-bit partial-register merge penalty; a tie still selects the shorter
67h form. This keeps 67h ahead of spilling/recomputation while avoiding it
where its actual target cost exceeds the equivalent narrow register work.

Against iteration 94's committed baseline, bytes fall `71 -> 62`. Instruction
counts fall `29 -> 27`, except P6/Core at 28. Weighted costs change: 386
`133 -> 118`, 486 `75 -> 71`, P5 `46 -> 42`, P6 `44 -> 43`, K5 `24 -> 19`,
K6 `24 -> 20`, K7 `29 -> 25`, and Core remains 42. No CPU regresses. Focused
allocation, safety, peephole, and C output checks pass (`48 passed`, `306
deselected`, `0.71s`); Tier 1 passes (`209 passed`, `31 deselected`, `2.07s`).
The independent hard-target table remains open, so these are candidate deltas,
not a final parity claim. GCC/LLVM listings remain best-case flat-i386
structural references; BCC/WC remain authoritative for the medium-model ABI,
segments, and legal address semantics.

### 95. Costed secondary 67h addressing before spill/recompute — 2026-09-18

The preceding medium-model audit had conflated “not a native 16-bit
effective-address form” with “not legal.” On a 386 target, address-size
override `67h` permits a 32-bit SIB address in 16-bit code. It is not free: it
adds one byte, may carry a profile-specific prefix cost, and requires exact
32-bit base/index values. But those costs come before updating a derived
counter in a frame slot or rebuilding the complete address every iteration.

The immutable CPU profile now exposes both families to MIR in machine-neutral
terms: index width, legal scales, extra bytes, per-use cost, extension cost,
and whether the family is secondary. The preferred compatibility view
remains native `{1}`; C, object, and recursive unswitch optimization now
receive the complete form tuple. MIR still sees neither `67h`, SIB, opcodes,
nor physical register names.

Strength formula selection now keeps native indexed leaves free, admits
register-resident recurrences that fit the pressure budget, then activates
the cheapest legal secondary addresses for any overflow. Only overflow that
has no legal secondary form reaches sibling collapse and spill/recompute pricing.
A 32-bit-index form still requires the existing exactness proof: zero start,
unit step, a known nonwrapping last counter, and a bounded far allocation.
The proof is now requested for the particular induction value being selected,
rather than accidentally widening the first qualifying counter in the loop.

The fail-first policy regression initially failed because no fallback-form
selection existed. It now proves that a scale-two far index at zero recurrence
capacity selects the fallback rather than disappearing into recomputation.
The profile regression separately proves every CPU has native 16-bit and
costed 32-bit `{1,2,4,8}` families, and the backend regression verifies the
actual far scaled load begins `26 67` (`26 67 8b 04 4e`,
`mov ax,es:[esi+ecx*2]`). Focused profile/driver/unswitch checks pass
(`12 passed`, `21 deselected`, `0.39s`); the full-tier encoding check passes
(`1 passed`, `0.06s`), and Tier 1 passes (`209 passed`, `31 deselected`,
`1.80s`).

The dynamic-bound `farloadloop` listing remains unchanged. Its `first`/`last`
range and pointer-field allocation do not prove that 32-bit address arithmetic
is equivalent to the source's 16-bit wrapping offset, so using `67h` there
would be an unsound response to pressure. This iteration corrects the legal
candidate set and its selection order; it does not claim a production-corpus
speedup. GCC/LLVM SIB listings remain best-case structural references, while
BCC/WC remain authoritative for the medium-model ABI and segment semantics.

### 94. Spill-aware induction formula rejection — 2026-09-18

The compact far-load loop was reconsidered from the raw GCC 16.2 and Clang
21 listings rather than from BCC's register assignment.  Both flat-i386
references carry one induction variable, while qbopt carried the source
counter and a derived `i * 2` recurrence.  Their exact forms are not directly
portable: Clang uses a 67h SIB scale that needs exact 32-bit-address proofs,
and GCC carries a 32-bit byte offset and reconstructs the dynamic exit.
Replacing that with a 16-bit
equality recurrence would be unsound because a step of two repeats after
32,768 iterations.  The transferable result is therefore the candidate-set
rule—choose one profitable complete IV set—not either reference's encoding.

The fail-first C regression captured the actual excess: `_mark` updated the
derived offset in `[bp-4]` on every trip.  Suppressing that formula measured
better, so strength selection now combines its existing recurrence budget
with the loop's MIR live peak.  A candidate beyond that capacity is retained
only when recomputing its operation costs more than its predicted memory
update and uses.  A power-of-two product is priced as the shift it will become,
while a true variable multiply retains its multiplication price.  Liveness is
computed once per strength pass and shared by all loops so the Tier 1 budget
does not pay one whole-body analysis per candidate.

Adjacent stage dumps are identical through `r01-promote`; `r01-strength` is
the first difference, retaining the local multiply and removing the derived
phi, seed, and latch update.  Later algebraic lowering turns that multiply
into the expected shift.  The result is therefore attributed to formula
selection rather than inferred backwards from allocation or assembly.

On every CPU profile the loop falls from 75 bytes / 30 instructions to 71 /
29.  Weighted cost changes are: 386 `141 -> 133`, 486 `78 -> 75`, P5
`49 -> 46`, P6 `44 -> 44`, K5/K6 `24 -> 24`, K7 `29 -> 29`, and Core
`43 -> 42`; no selected profile regresses.  The emitted result still spills
the two mutually exclusive short-lived shifts through one frame slot.  That
is now isolated as a Phase 4 local allocation/splitting problem rather than
being obscured by a globally unprofitable second recurrence.

The same Tier 1 run exposed a brittle sieve tail-sharing check that asserted
the benchmark's unrelated total instruction count.  It now directly builds
the cost counterexample and requires rejection when a static saving adds hot
dynamic work.  The focused formula and emitted-code regressions failed before
the change; Tier 1 passes (`208 passed`, `31 deselected`, `2.88s`).  The slow
matrix comparison was stopped when it exceeded this iteration's test budget;
the full corpus/profile matrix remains a phase-boundary gate.  GCC/LLVM remain
best-case flat-i386 structural references, while BCC/WC remains authoritative
for medium-model ABI, segments, legal forms, and audited hard targets.

### 93. Reject incomplete counter/mask role trial — 2026-09-18

The compact far-load stage dump still spills the face's shifted bitmap index
through `[bp-2]`.  A fail-first trial of soft DX-counter/AX-mask preferences
was deliberately rejected: even after correcting the two-address update
recognizer, its complete allocation did not lower weighted spill traffic.
The trial and red assertion were removed; retaining a BCC-shaped register
swap without a profitable complete recovery plan would violate the allocator
cost rule.

The evidence narrows, rather than completes, Phase 4: a future role candidate
must model the header comparison, latch increment, counted-shift mask, and
far-address index as one full natural-loop allocation, including the required
constrained-use split and its recovery cost.  The existing far-owner regression
passes again (`1 passed`, `0.25s`).  GCC/LLVM remain best-case structural
references; BCC/WC medium-model output remains the ABI and legal-form target.

### 92. Split local gaps across block boundaries — 2026-09-18

The allocator's local split rung was described as a single-block mechanism,
but its selector rejected a value as soon as *any* other block also referenced
it.  That is not a safety proof: a local carve copies into the selected gap
and restores the original at that region's boundary, so later or predecessor
uses remain on the original value.  The over-restriction withheld a normal
constrained-use split from multi-block live ranges and left the allocator with
an avoidable spill candidate.

`splitkit._local()` now prices every same-block gap and selects the widest
one, regardless of references outside that block.  It retains the existing
minimum-gap rule and uses the unchanged generic carve/restore logic, so this
is neither a register preference nor a QCport-specific split.  The
fail-first LIR regression has two uses separated in one block and an
additional exit-block use; it previously received no plan and now receives
the local region beginning at the later use.  All splitkit checks pass (`8
passed`, `0.06s`).

`test_splitkit.py` is also now Tier 1: all eight tests are hermetic core-LIR
coverage and complete well inside the fast gate budget.  This advances Phase
4's split ladder, not the still-open global splitting/rematerialization,
x87 allocation, or complete target-priced constrained-role planning.
GCC/LLVM listings remain best-case structural references; BCC/WC
medium-model listings remain the authority for ABI, segments, legal address
forms, and hard targets.

### 91. Put terminal CFG cleanup in Tier 1 — 2026-09-18

Iteration 90's fail-first terminal-successor regression was hermetic MIR, but
had been added to the Tier 2 object-fixture module.  That left the exact
mechanism which prevents terminal-detached stores from lowering outside the
ordinary development gate; the real QCport-derived object proof has a
different purpose and cost.

The synthetic regression now lives beside the existing Tier 1 direct
interprocedural no-return coverage.  It continues to assert the observable
property—after a terminal call cuts the only predecessor edge, the successor
is an inert source-byte owner rather than executable store work.  The
QCport-derived object regression stays Tier 2 to retain the end-to-end
contract, so this moves a fast mechanism check rather than relabeling a slow
integration test.

The terminal/no-return subset passes (`5 passed`, `0.05s`) and Tier 1 passes
(`199 passed`, `31 deselected`, `1.74s`).  This makes Phase 6's existing
terminal cleanup more durable; it does not advance any unfinished Phase 3
aggregate proof, Phase 4 allocation mechanism, Phase 5 versioning, or Phase
6 recursive/full IPSCCP.  GCC/LLVM listings remain best-case structural
references; BCC/WC medium-model listings remain the authority for ABI,
segments, legal address forms, and hard targets.

### 90. Normalize terminal-detached byte owners — 2026-09-18

The shared no-return cleanup removed an edge after a proven terminal call,
but the object path invokes it after its ordinary MIR fixed point.  A
successor reachable only through that edge could therefore still hold an
executable store and be lowered despite being dead.  This was a real
control-flow quality and object-emission provenance gap, not an excuse to
borrow a flat-i386 calling convention from a reference listing.

After trimming a terminal block, `noreturn.after_terminal_calls()` now hands
the changed body to the established source-map-preserving unreachable-block
normalizer.  Dead blocks retain their original byte ownership as complete
inert MIR markers, but have no executable operations, edges, phis, or
lowerable stores.  The shared operation therefore gives C and the object
pipeline the same post-terminal CFG result without inventing a frontend
special case.

The fail-first regression constructed a terminal call with a successor store:
before the change that store remained executable; afterward the successor is
an inert owner.  Object no-return checks pass (`4 passed`, `5.10s`) and the
C/MIR terminal subset passes (`4 passed`, `0.04s`); Tier 1 also passes
(`198 passed`, `31 deselected`, `1.80s`).  This advances Phase 6's
terminal control-flow cleanup only; recursive/full IPSCCP, broader
global-elimination proofs, and complete CFG cleanup remain open.  GCC and
LLVM listings remain best-case structural references; BCC/WC medium-model
listings remain the hard authority for ABI, segment semantics, legal address
forms, and performance targets.

### 89. Shared terminal-call MIR cleanup — 2026-09-18

The object path previously used no-return only to choose a lower-level frame
shape.  The QCport `HOST_SHUTDOWN` regression retained an unreachable call at
`0x17fc` and `RETURN` at `0x1801` after its proven terminal `B$CEND` call at
`0x17f7`; C had already removed equivalent tails.  The initial focused test
failed in both terminal-control variants before the transform existed.

`noreturn.after_terminal_calls()` is now the one MIR operation for both
frontends.  It retains the physical terminal call, truncates only later
operations in that block, and removes that block's outgoing CFG edges while
leaving any independently reachable successor block intact.  It is
idempotent and MIR verification remains valid.  The object pipeline applies
it after no-return inference and exposes `mir-noreturn` in stage watches;
the real QCport stage probe now ends `HOST_SHUTDOWN` at `0x17f7`.  C's named
call wrapper delegates to that same mechanism rather than carrying a second
spelling.

The full object no-return file passes (`3 passed`, `4.49s`), the C/MIR
terminal and SCC checks pass (`5 passed`, `0.17s`), and the regression covers
both recognizing and withholding `B$CEND` as terminal.  This is Phase 6
control-flow cleanup, not a claim that all unreachable blocks, recursive
IPSCCP, or broader global elimination are complete.  GCC/LLVM remain
best-case flat-i386 listing references; BCC/WC medium-model output remains
the hard authority for ABI, segments, legal forms, and performance targets.

### 88. Object-path no-return SCC parity — 2026-09-18

The C named-body summary was now SCC-capable, but the shared object/BASIC
analysis still began with an empty terminal set and therefore missed the same
closed local recursion.  That left an implementation-state discrepancy
between the two frontends even though both operate on the same machine-neutral
MIR control fact.

`noreturn.inferred()` now uses the same greatest fixed point: begin with the
locally defined bodies, remove any body with a reachable normal return,
fallthrough, or path through a nonterminal call, and repeat until stable.
Established runtime `NEVER` calls remain independent terminals.  This proves
only a closed local SCC; a normal returning member removes its callers from
the set, and it does not infer anything about unknown external procedures.

The fail-first object-path MIR regression records `a → b → a` formerly
producing an empty result.  The full no-return file passes (`3 passed`,
`4.50s`), including both sides of QCport's existing shutdown-control proof;
the C SCC regressions also pass (`2 passed`, `0.11s`).  This brings Phase 6
no-return SCC reasoning to both frontends, not recursive IPSCCP or broader
call-effect inference.  GCC/LLVM remain best-case flat-i386 listing
references; BCC/WC medium-model output remains the hard authority for ABI,
segments, legal forms, and performance targets.

### 87. Closed private no-return SCCs — 2026-09-18

The earlier no-return summary used a least fixed point.  That correctly found
an independently terminal body, but missed a closed private `a → b → a`
cycle: neither callee was initially proven, although a normal return is
impossible through the complete SCC.  This was a real Phase 6 recursive
summary omission, not a reason to treat GCC/LLVM's flat calling convention as
applicable to the medium-model ABI.

The summary now begins with all eligible private bodies and monotonically
removes a body if MIR control can reach a normal return without first passing
through a currently terminal direct private call.  The resulting greatest
fixed point proves a closed recursive SCC only when every member has no such
escape.  Unknown, external, exported, address-taken, or returning edges are
not candidates and therefore make their callers returning.  The physical
calls remain; ordinary MIR cleanup cuts only code after an established
terminal call.

The fail-first MIR regression records the former empty result for the mutual
cycle; a companion rejects a cycle member with an actual `RETURN`.  The C
fixture checks that `entersRecursiveSpin` keeps `call _spinFirst` but no
longer emits `mov ax, 9; retf`.  Focused checks pass (`6 passed`, `0.22s`).
This advances only private no-return SCC summaries: recursive IPSCCP,
public/address-taken, indirect/external call facts, and broad global
elimination remain unfinished.  GCC/LLVM remain best-case flat-i386 listing
references; BCC/WC medium-model output remains the hard authority for ABI,
segments, legal address forms, and performance targets.

### 86. Reconcile loop-rotation plan state — 2026-09-18

The live table still listed rotation as future work.  That was stale: the
production C and object pipelines both invoke `rotate.entered()` after the
ordinary MIR fixed point, and the C regression verifies that `_dot` and
`_fill` have no backward unconditional jump.  The implementation deliberately
requires a proven nonempty loop, one latch, a test-only header, and no unsafe
phi move; it remains a conservative general MIR transform rather than a
frontend or source-name special case.

The table now records rotation as present.  It does not claim loop work is
finished: loop versioning, broad register-pressure forecasting, and a
complete legal-form selector remain open.  The latest GCC/LLVM reports remain
best-case flat-i386 structural listing evidence, while BCC/WC medium-model
listings remain the hard ABI, segment, address-form, and performance-target
authority.

### 85. Keep no-return summaries private — 2026-09-18

The first no-return fixed point considered every raised C procedure.  Its
control proof was sound for a direct local call, but that exceeded Phase 6's
declared selective-internal scope by manufacturing an IPA summary for an
exported or address-taken body.  The full interprocedural ABI/visibility proof
is still unfinished, so widening this fact would be an assumption rather than
a needed optimization.

The summary now receives the existing private eligibility set and accepts only
those named bodies.  Direct calls still retain their physical call and prune
only a tail after an already-proven private terminal callee.  The new negative
MIR regression fails before the boundary is supplied: an otherwise-terminal
exported body must not enter the private summary set.  The original caller-tail
regressions remain unchanged.  Focused checks pass (`3 passed`, `0.22s`).
This is a conservative Phase 6 scope correction; public/address-taken,
indirect, external, and recursive claims remain unfinished.  GCC/LLVM remain
best-case flat-i386 listing references; BCC/WC medium-model output remains the
hard authority for ABI, segment, legal-address, and target-performance facts.

### 84. Direct private no-return caller-tail pruning — 2026-09-18

`ipa_noreturn` first emitted `call _spinForever`, followed by the impossible
`mov ax, 7; retf` tail in `entersSpin`.  The terminal callee is a private,
direct C body whose only reachable path is an exact infinite loop.  Keeping
that tail is both dead work and a barrier to the caller becoming terminal in
the next whole-module round.

The named-procedure fixed point now reuses the established MIR control proof:
a body joins the no-return set only when each of its paths is independently
terminal, or reaches a call proven terminal in an earlier round.  At every
such direct call it keeps the physical `CALL` in source order and removes only
the following operations and CFG edges whose execution would require a
return.  The ordinary optimizer then handles the newly unreachable body.
Already-terminal blocks are unchanged, so the fixed point is idempotent.
Mutual recursion cannot bootstrap itself, and indirect, external,
address-taken, and public-call claims remain conservative.

The fail-first C regression now checks that `_entersSpin` retains
`call _spinForever` but contains neither `mov ax, 7` nor a return.  Its Tier 1
MIR companion separately proves that only the impossible caller tail is cut.
Focused checks pass (`2 passed`, `0.22s`).  This advances Phase 6 direct
no-return summaries only; nocapture/writeonly facts, recursive SCC summaries,
and broader global elimination remain unfinished.  GCC/LLVM remain best-case
flat-i386 listing references; BCC/WC medium-model assembly remains the hard
authority for ABI, segments, address legality, and performance targets.

### 83. Refreshed compact far-load quality evidence — 2026-09-18

The current 386 quality report for `fixtures/c/farloadloop.c` is pinned to
`b510ab4`, source SHA-256
`db481a6d48836c17a464a313401e8fe9486a7b370a08c852d331cdf999fd61d8`.
`_mark` emits 75 bytes / 30 raw instructions / weighted cost 141.  After the
report's ABI boilerplate normalization, qbopt has 22 instructions, 13 loads,
6 stores and two branches.  Clang 21 has 21/8/1/2; installed i686 GCC 16.2
has 33/12/5/2.  The dynamic ratios are correctly withheld because the loop
weight remains a profile-free heuristic rather than an execution trace.

The raw listings explain the qualified result.  Both flat references are
best-case structural evidence only: their SIB addressing, 32-bit pointers,
flat frame and no segment loads cannot be adopted by the medium-model target.
The matching target concern is instead qbopt's carried scaled offset and
shifted face falling into frame slots.  The report attributes the excess
loads/stores against both references first to `lir-regalloc`; it attributes
the small Clang instruction excess first to `lir-phielim`.  It reports no
branch/call excess and does not manufacture a hard target.

This is Phase 1 measurement evidence for the existing joint role-plan work,
not a performance claim or a source-specific allocator rule.  The next
implementation must price and compare complete legal medium-model allocations
for an induction counter, shift/byte temporary, far base and index—not copy a
flat compiler's register spelling.  GCC/LLVM remain best-case listing
references; BCC/WC remains the hard authority for ABI, segments, legal
addresses and any registered target.

### 82. Pentium LEA pairing audit correction — 2026-09-18

The first frame-LEA scheduling increment classified every LEA as P5 U-only.
Before relying on that inference, the local GCC source was checked directly:
`gcc/config/i386/pentium.md` includes non-prefixed `lea` in its U/V pairing
class, while prefixes make an instruction U-only.  The safe LIR boundary was
correct; the pipe category was not.

The scheduler now models frame LEA as U/V.  The regression uses the form that
actually benefits: it starts with `lea bx,[bp-4]; mov eax,ecx`, where the
operand-size-prefixed move must occupy U, and verifies the scheduler emits the
move first so the LEA can take V.  The existing symbolic/non-frame negative
case remains.  Focused scheduler checks pass (`8 passed`, `0.05s`); the
bounded Tier 1 rerun passes (`194 passed`, `31 deselected`, `1.65s`).  This is a profile-audit
correction to Phase 7, not a new claim about memory pairing, x87/segment work,
or full issue modelling.  GCC is evidence for the CPU issue model here;
BCC/WC remain the authority for medium-model address legality and ABI.

### 81. Frame-LEA scheduling — 2026-09-18

The P5 scheduler treated every `lea` as a memory boundary, leaving
`lea bx,[bp-4]; mov eax,ecx` unpaired even though the LEA reads only BP,
writes BX, and can issue in Pentium's U/V slot.  That was an
over-conservative representation boundary: unlike a load,
the selected frame-address LEA has no memory, segment-state, fault, or
relocation effect.

The safe scheduler window now admits exactly a one-source, non-relocated
`FRAME` address LEA with ordinary GPR base/index lanes.  Its latency uses the
audited `lea` profile entry and P5 classifies an unprefixed LEA as U/V, as in
GCC's local Pentium scheduler model.  Symbolic,
non-frame, far/segment-selected, stack, memory, call, x87, source-mapped
allocator-artifact, and control forms remain boundaries.  Thus this is a
physical LIR dependency fact; no MIR pass gains an opcode or register name.

The fail-first scheduler regression now places an independent prefixed U-only
move before a frame LEA on P5, making the pair eligible.  Its negative companion
proves a non-frame symbolic address is still refused.  Focused scheduler
checks pass (`8 passed`, `0.05s`), and Tier 1 passes (`194 passed`, `31
deselected`, `1.67s`).  This is a narrow Phase 7 scheduling increment, not a claim of memory
pairing, x87/segment scheduling, or a complete issue model.  GCC/LLVM remain
best-case flat-i386 structural references; BCC/WC medium-model listings
remain authoritative for address legality and ABI behavior.

### 80. Direct private readonly-effect elimination — 2026-09-18

`ipa_readonly` first retained a call to private `sample()` even though its
only work was reading an ordinary static word and its return was unused.
The previous `pure_procedures` predicate deliberately required all memory to
be frame-local; that was right for inlining, but too strict for C's
unobservable nonvolatile static reads and left a callable dead-effect gap.

The new fixed-point readonly predicate is separate from inlining purity.  It
admits only acyclic, returning direct bodies with no barrier, trap, floating,
escape, opaque, or nonlocal write; direct near `SEGMENT` reads and frame/stack
traffic are allowed only when nonvolatile.  It follows only already-proven
readonly private callees.  Pointer-based, far/externally selected, volatile,
unknown, recursive, and indirect work remain conservative.  Dead-result call
removal consumes this narrower semantic fact, while the original frame-only
purity predicate continues to govern cloning.

The fail-first C regression now emits `discardSample` as `mov ax, 7` with no
`_sample` call or body.  Its volatile counterpart retains
`call _sampleVolatile`; the Tier 1 MIR test independently rejects volatile
and global-store bodies.  Focused checks pass (`2 passed`, `0.14s`); the
bounded Tier 1 rerun passes (`192 passed`, `31 deselected`, `1.64s`).  This advances Phase 6's
user-procedure readonly and dead-call work only.  Nocapture/writeonly/noreturn
inference, recursive SCC handling, indirect calls, and public/address-taken
procedure facts remain unfinished.  GCC/LLVM listings remain best-case
flat-i386 structural references; BCC/WC medium-model listings remain the
authority for ABI, segments, legal forms and OMF linkage.

### 79. MIR-derived constant call-site cloning — 2026-09-18

`ipconst_return_site` first retained `choose(4)` after `seed()` had already
produced its interprocedural result.  A second, dynamic caller correctly
prevented whole-body specialization; the source-literal-only call-site
inliner therefore had no safe way to exploit the one current-MIR constant
call.

The reusable direct-call fact analysis now exposes facts per surviving call,
not only their all-callers agreement.  The ordinary target-costed private
leaf inliner consumes those facts after each IPSCCP return round.  It clones
only the known call, reuses the existing MIR pipeline to fold it, and then
repeats the direct-call fixed point.  The dynamic call stays semantically
dynamic; once the constant clone is gone, the pre-existing one-use policy may
inline its remaining private body, but its emitted comparison and `+29`
fallback remain.

The fail-first output regression proves both facts: `returnedConstantChoose`
is `mov ax, 11`, while `dynamicChoose` retains `cmp ax, 4` and `add ax, 29`.
Its Tier 1 companion proves that one unknown call blocks body-wide parameter
specialization while preserving the separate constant call fact.  Focused
checks pass (`3 passed`, `0.22s`), and Tier 1 passes (`191 passed`, `31
deselected`, `1.24s`).  This extends Phase 6's direct-call IPSCCP/inlining boundary only.
Recursive SCCs, indirect calls, public/address-taken functions, floating or
effectful clone candidates, and broader global elimination remain
conservative.  GCC/LLVM listings remain best-case flat-i386 structural
references; BCC/WC medium-model listings remain authoritative for ABI,
segments, legal forms and OMF linkage.

### 78. Direct-call IPSCCP fixed point — 2026-09-18

`ipconst_chain` first left a call to branchy private `choose` after the
private `seed` call had already been summarized to the constant `4`.  The
original parameter-specialization proof read only source-time literal facts,
so the newly materialized MIR argument could never seed `choose`; it was a
one-pass boundary in what must be a direct-call fixed point.

The interprocedural analysis now reads the current surviving `CALL` and its
contract-owned `ARG` operations, asks SCCP for each actual's value, and
requires every direct private call to agree before seeding a formal.  A
missing contract, malformed argument sequence, dynamic actual, public or
address-taken target is an unknown fact, never a specialization permission.
Return propagation and this parameter proof alternate while either discovers
a new fact; their per-call and per-entry seeds are idempotent.  Thus a
constant may traverse arbitrarily many acyclic direct private calls without
duplicating copies or making a source-name exception.

The fail-first C regression now proves `chainedConstant` returns `11` with no
`_choose` call or body.  Its Tier 1 MIR companion proves the returned-value
case independently.  Focused checks pass (`2 passed`, `0.17s`), and Tier 1
passes (`190 passed`, `31 deselected`, `1.19s`).  This advances Phase 6 direct-call IPSCCP only: recursive SCC
summaries, public/address-taken specialization, indirect calls, and broader
interprocedural global elimination remain conservative.  GCC/LLVM listings
remain best-case flat-i386 structural references; BCC/WC medium-model
listings remain authoritative for ABI, segments, legal address forms and OMF
linkage.

### 77. Direct allocated-LIR OMF emission audit — 2026-09-18

The architecture document still described an `omfwrite._as_mir` compatibility
seam even though that function no longer exists. The production BC path takes
allocated `LirBody` objects directly into `layout.rebuild()` and fresh OMF
serialization; `rewrite.py` invokes that route through `wholeseg.emitted()`.
The only retained `SourceMap.applied()` API is a non-mutating low-level test
view, and the existing production-path guard proves it cannot be called while
emitting a real object.

Phase 2 is therefore complete for production output: neither the QB-object
nor C frontend rewrites an existing OMF record stream, and neither converts
allocated LIR back to MIR before layout. This does not claim final acceptance:
the full matrix, byte-identity gate and quality targets remain their separate
plan gates. GCC/LLVM listings remain best-case structural references; BCC/WC
medium-model output remains authoritative for the ABI, segmentation and OMF
semantics.

### 76. Exit conditions visible during scalar raising — 2026-09-18

`arith-p-evt` first reached the fresh-OMF emitter with a source-free 32-bit
add at `0x1c6`, then correctly refused it because a condition was live across
the replacement and visible at the machine exit.  The source sequence is
`add`/`adc`: it leaves the final word's condition codes, whereas a native
32-bit add would leave different zero/parity/auxiliary flags.  The apparent
contradiction was temporal, not a lowering rule: exit observations were
attached only after the full raise, so the pair recognizer could not see the
condition it was obliged to preserve.

Exit-visible values are now materialized as ordinary MIR `exits` immediately
after frame annotation, before any recognition or optimization.  Scalar-long
recognition derives per-operation flag liveness from those semantic edges and
retains a pair whenever a final-word condition is observable.  The fail-first
fixture now emits fresh LIR for event-enabled ARITH; it proves the boundary
outcome rather than a particular source encoding.  This is a Phase 2
MIR-boundary correctness increment, not an event-specific fast path and not
a relaxation of strict BASIC flags.

GCC and LLVM flat-i386 listings remain the best-case reference for expression
and loop structure.  They cannot overrule this result: BCC/WC medium-model
listings and the actual segmented ABI remain authoritative for flags,
segments, legal addressing and OMF behavior.

### 75. Target-scoreboard fresh-emission refusals — 2026-09-18

The complete QB target audit stopped at `ARITH` when fresh lowering refused a
flagged widened long add. That refusal must remain visible—lowering cannot
silently alter a condition's source-observable flags—but one unsupported
module previously aborted the entire audit before later benchmarks could be
measured.

`tools/opportunity.py` now converts only its explicit `rewrite.Unsupported`
fresh-emitter outcome into `Unmeasured`. The existing board loop reports that
module as `UNMEASURED`, returns a failing status, and continues through the
remaining corpus. It never falls back to BC bytes. The fail-first regression
injects the exact strict-emitter exception and proves both the diagnostic and
the non-success result. Unexpected rewrite exceptions remain exceptions, so
instrument or programming failures are not hidden.

This is Phase 1 measurement integrity, not a code-generation fix. The
flagged-long lowering refusal remains a separately diagnosed correctness item;
GCC/LLVM flat-i386 listings cannot justify weakening its medium-model flag
semantics. BCC/WC remains the authority for any eventual emitted fallback.

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
scaled address forms `{1,2,4,8}` to MIR as though they were native and free.
The ordinary 16-bit medium-model effective address is only `[base+index]`;
32-bit SIB requires a costed `67h` address-size form and exact widened-address
proofs that this iteration did not yet model. The preferred profile view was
therefore reduced to native `{1}` consistently. The new
fail-first CPU-profile regression verifies that contract for every supported
CPU, and the focused profile plus C RMW tests pass.

This corrects the target interface rather than choosing a QCport formula.  It
does not itself remove `r_walk`'s word-offset recurrence: that recurrence is
the then-modelled fallback for `i * 2` under native 16-bit addressing. The next iteration
still needs pressure-aware selection between that register recurrence and a
stack/recomputed offset, with BCC's medium-model listing as the ABI reference.

### 10. Address-form validation listing — 2026-09-18

The clean paired `r_walk` build at qbopt `a8eb96e` keeps the same qbopt listing
hash, `ccbacccfc7ff5f46553d0e8b41dd78652f7bb7ad8ad7aa486dc629c92511badc`.
That is the expected result for this loop: `i * 2` has no native scaled
16-bit address form, and its dynamic bounds did not prove a safe widened
67h address, so the existing strength reducer chose its register-recurrence
fallback. The profile correction prevents future MIR formula selection from
incorrectly pricing SIB as native/free; it is not
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

The shared `transform.applied()` boundary still supplied `{1,2,4,8}` as a
native/free set when a
direct MIR caller omitted CPU form facts, despite every public CPU profile and
both production frontends correctly supplying `{1}`.  That was a target-model
leak: a native 16-bit medium-model effective address can combine a base and
index, while a 32-bit scaled-index form needs a costed `67h` form and
widened-address proof. Such a caller could therefore make a formula decision
using a free price for code the emitter could encode only with work the
decision had not represented.

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

### 68. Partial-register dependency scheduling — 2026-09-18

The per-CPU profile already recorded the measured 8/16-bit-to-32-bit merge
stall, but only the offline scorer used it.  `schedule.py` now applies that
number to the actual physical dependency edge from a byte/word GPR write to a
later full-width read of the same root.  It is not a generic additive cost: a
32-bit write between the two replaces the partial value and explicitly removes
the delay.  This keeps the fact at the post-allocation boundary where register
roots and widths are known, rather than leaking register terminology into MIR.

The fail-first P6 regression recreates the CRC32 shape `mov ax,bx; add
eax,esi; mov di,si`.  Before the edge carried only normal move latency, so the
consumer was selected first; with P6's recorded merge delay, the independent
word move fills the gap.  Profiles with zero partial-register penalty retain
their existing behavior.  Register, flag, memory, segment, x87, and provenance
boundaries remain exactly those of iterations 66–67.

Focused scheduler/profile checks pass (`16 passed`, `0.20s`); Tier 1 passes
(`181 passed`, `31 deselected`, `0.98s`).  This advances Phase 7's profile
fidelity only; it neither claims every partial-register mitigation has been
selected nor registers a performance target.  GCC/LLVM listings remain
best-case flat-i386 structural references; BCC/WC medium-model output remains
the ABI, segment, legal-form, and hard-target authority.

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

### 69. Exact direct aggregate-copy scalarization — 2026-09-18

The C fixture `aggregatecopy` first failed with a direct global-to-frame
`dword` copy followed by word reloads from `[bp-8]` and `[bp-6]`. The old
SROA rule correctly rejected the whole copy because it properly overlapped
the two field leaves; it had no general way to express that the copy itself
could be partitioned before scalar promotion.

SROA now recognizes an adjacent C-raised load/store pair only when both
references are direct, nonvolatile, exact canonical ranges; their source and
destination objects are proven disjoint; and existing scalar leaves form a
complete contiguous partition of the destination range. It expands that one
move into source-leaf load/store pairs, then the ordinary scalar-promotion
pipeline removes the redundant frame reloads. An untyped aggregate access
adopts a unique already-established scalar leaf type only within this
C-raised, unowned form; explicit incompatible types retain the existing
type-pun rejection.

The same proof deliberately rejects far, indexed, pointer, volatile,
overlapping, source-mapped, and incomplete-partition copies. Splitting any
of those could change fault or tearing behavior, or lose source-map byte
ownership. The fail-first output regression now emits three direct `_source`
word reads with no `[bp-8]`/`[bp-6]` reload. Its companion regression proves
that an unbounded far aggregate still retains its one `es:[bx]` dword read
and store. Focused C checks pass (`3 passed`, `0.13s`).

This advances Phase 3 without claiming general pointer-copy SROA, C library
`memcpy` recognition, or aggregate-copy lowering through the fresh OMF path.
GCC/LLVM listings remain best-case flat-i386 structural references; BCC/WC
medium-model output remains the authority for segment behavior, legal address
forms, ABI constraints, and hard targets.

### 70. Exact pointer-source aggregate scalarization — 2026-09-18

`aggregatecopyptr` first failed after the direct-copy work: a local pointer
was proven to designate one six-byte frame object, yet its four-byte source
load was represented as one contiguous stride-one element of width four.
SROA recognized only the equivalent byte-range spelling, so the pointer copy
remained a dword frame store followed by two local word reloads.

The leaf proof now normalizes those two exact contiguous-slice spellings to
the same object byte interval. Copy expansion admits an exact source base
when its complete MIR `uses` set is precisely that base and optional segment
value; every generated source leaf carries those values forward. Destination
storage remains direct and the existing nonvolatile, disjoint-object, and
complete-partition proofs still apply. Thus an exact local pointer is not a
special case: it is one expression of the same bounded object identity.

The fail-first C regression now has no `[bp-14]` or `[bp-12]` aggregate
reloads. Direct and unbounded-far aggregate regressions continue to pass,
establishing both the wider exact proof and its conservative boundary. This
advances Phase 3, but arbitrary pointer, far-pointer, indexed, overlapping,
volatile, and incomplete aggregate copies remain memory until a comparably
complete range and alias proof exists. GCC/LLVM remain best-case structural
references; BCC/WC medium-model listings remain authoritative for legal
addressing, segments, and ABI behavior.

### 71. Exact pointer-destination aggregate scalarization — 2026-09-18

`aggregatecopydestptr` first failed with a local pointer whose single,
bounded frame target was known, but whose adjacent four-byte store stayed a
wide aggregate access. The scalar reads of that local then reloaded
`[bp-8]` and `[bp-6]`. The earlier exact-pointer rule accepted a pointer
source only; it incorrectly treated a proven pointer destination as if it
were necessarily an unbounded aggregate write.

The aggregate-copy proof now accepts one exact, near destination pointer
when its complete `uses` set is the copied value followed by that address
value. Splitting carries that address use to every
generated scalar store, so allocation and lowering cannot detach a piece
from its proven address. The existing requirements remain unchanged: both
references have exact canonical ranges, their objects are disjoint, the
destination range has a complete scalar partition, and neither access is
volatile, far, indexed, source-backed, or partially overlapping.

The fail-first regression now lowers `*destination = source` to three direct
`_source` word reads and a register sum, with no local wide copy or field
reload. This advances Phase 3's bounded aggregate scalarization; it does not
permit arbitrary, far, indexed, overlapping, volatile, or incompletely
partitioned pointer copies. GCC/LLVM remain best-case flat-i386 structural
references, while BCC/WC remain the medium-model authority for legal
addressing, segments, ABI behavior, and hard targets.

The companion `aggregatecopybothptr` coverage keeps one exact local pointer
on each side of the copy. It verifies that generated scalar reads and stores
retain both address dependencies rather than silently falling back to the
direct-address-only form.

### 72. Costed repeated private-leaf inlining — 2026-09-18

`inline_twice` first retained two near calls, two argument pushes, two caller
cleanups, and its private `increment` procedure despite that helper reducing
to one integer add after its ordinary local pipeline. The old inliner treated
one surviving call as an absolute eligibility condition. That preserved a
call boundary even where cloning the semantic body was decisively cheaper.

The whole-module candidate policy now admits every direct call to a private,
pure, single-block leaf when the selected CPU profile's total call cost is
strictly greater than the MIR semantic work duplicated by cloning. Single-use
leaves retain the existing broader CFG policy; repeated leaves with phis or
control flow remain excluded. Thus the mechanism is target-priced and
general, rather than an exception for one helper name or call count.

The fail-first C regression now emits no `_increment` procedure or call. Its
caller is the direct `value + 1 + value + 1` form, simplified to `add ax, 1`
followed by `add ax, ax`. The focused inlining suite passes. This advances
Phase 6 selective MIR inlining; recursive/public/address-taken procedures,
calls with unmodelled results, effectful or floating bodies, CFG cloning, and
private-data DCE remain outside this increment. GCC/LLVM remain best-case
flat-i386 structural references; BCC/WC remain the medium-model ABI and
addressing authority.

### 73. Conservative private-data elimination — 2026-09-18

`private_data_dce` first failed with `_unusedValue` still emitted in `_DATA`
after its only public procedure had reached its optimized MIR body. Procedure
reachability already removed uncalled private code, but fresh OMF emission
unconditionally serialised every data item from the C stream.

The C path now derives object spans from `DGLabel` boundaries and roots only
labelled, non-procedure, non-imported, non-public symbols from the MIR bodies
that will actually be lowered. It recognizes both direct relocation operands
and memory-cell references, including far-data selector relocations. It then
closes those roots over `DGFEPtr`/`DGBackPtr` initializer relocations and over
the exact relocation table of any opaque inline-assembly body. Thus the
fixture retains `_retainedPointer` and its initializer target
`_retainedValue`, while deleting only the unreferenced `_unusedValue`.

No-optimization mode retains the original data stream unchanged. Anonymous
literal labels, public/imported data, procedures, unlabelled bytes, and any
object outside the established OMF evidence remain conservative roots. This
is a Phase 6 linkage-safe DCE increment, not an assumption that flat-i386
GCC/LLVM section garbage collection is applicable to this medium-model OMF
layout. GCC/LLVM listings remain best-case structural references; BCC/WC
remain the authority for ABI, segment, address-form, and linkage constraints.

### 74. Constant call-site specialization — 2026-09-18

`ipconst_site` first left `zeroAdjusted` calling private branchy `adjust(0)`
because `dynamicAdjusted(value)` prevented whole-body parameter
specialization. The existing repeated-call inliner correctly refused the
branchy body, but its all-callers-agree rule meant local SCCP never saw the
known zero actual.

The inliner now has a target-costed call-site candidate layer. A private,
pure, legal MIR leaf whose direct call has at least one known constant actual
may clone into that caller when its semantic work is cheaper than the selected
profile's direct-call cost. The ordinary body pipeline immediately revisits
the clone, so it owns branch folding and dead code removal. The original body
continues to serve dynamic callers; after the constant call vanishes, the
existing one-use policy may independently inline that remaining call.

While exercising the branchy clone, the regression also found that fresh
inline values were numbered only against the caller. Independently raised
callees restart value numbering, so a materialized actual could collide with a
callee source ID and violate SSA dominance. Fresh values are now unique across
both bodies before substitution. The source-level regression first failed on
the retained constant call, then proves `zeroAdjusted` is `mov ax, 7` while
the dynamic path still contains its runtime test and `+3` case.

This advances selective MIR inlining and IPSCCP's constant-call edge handling,
but recursion, address-taken/public functions, effectful or floating bodies,
large CFG cloning, and general multi-version procedure emission remain out of
scope. GCC/LLVM listings remain best-case flat-i386 structural references;
BCC/WC medium-model listings remain authoritative for ABI, segments, legal
addresses, and OMF linkage.

### 75. Pressure-bounded complete peeling — 2026-09-18

The C performance pass exposed a bad P5 matmul choice: complete peeling
reduced MIR's estimated dynamic work, but the expanded loop crossed the
allocator's practical pressure limit and selected 958 machine instructions
where the unexpanded form selected 421. The earlier profitability test
amortized both static growth and its spill-risk estimate over the trip count;
that made a pressure-bearing expansion look cheaper precisely when MIR's
traffic model was least able to certify the final constrained allocation.

The immutable CPU profile now carries a machine-neutral complete-expansion
budget of 200 optimized semantic operations, matching GCC's target-independent
`max-completely-peeled-insns` default as a structural reference. The unroller
measures only the expanded loop sequence, not unrelated operations in its
procedure. A candidate predicted to create register pressure pays all of its
static growth and is rejected when the optimized sequence exceeds the budget;
a register-fitting candidate can still amortize growth over its exact trip
count. That preserves the useful CRC constant specialization while refusing
the matmul explosion. Regressions cover the budget boundary, procedure-local
measurement, pressure-sensitive accounting, and propagation through every
optimization entry point.

This advances Phase 5's target- and pressure-driven loop policy. The 200-op
limit is a conservative default rather than an audited optimum for every CPU;
future per-profile tuning still requires allocated listings and registered
runtime measurements. GCC/LLVM remain best-case flat-i386 structural
references, while BCC/WC remain authoritative for the medium-model ABI and
legal address forms.

### 76. Optimized qc-port integration gate — 2026-09-18

A complete WC-frontend `--opt --cpu 386` qc-port build found eight defects
that unit and corpus compilation had not jointly exercised. Each now has a
fail-first symptom regression:

- far-load selection pins the implicit selector result of `les`, `lfs`, and
  `lgs`, independent of whether a later memory operand happens to mention it;
- commutative 16-bit `[base+index]` graphs are bipartitioned globally so the
  smaller, non-call-crossing side consumes scarce `bx`, while the other side
  uses `si`/`di`;
- a retained hot-loop-base plan is a costed candidate and falls back to the
  ordinary split/spill ladder if reload insertion later makes it unallocatable;
- inlining materializes an actual whose value number collides with the
  independently raised callee namespace, preserving SSA dominance;
- C function designators decay in scalar contexts, and a 16:16 indirect call
  is lowered through one reusable owned frame cell to the ISA's `m16:16` form;
- discarding an x87 return emits a real `fstp st(0)` stack pop;
- far-load pair recognition conservatively rejects a typed access with no
  provenance instead of dereferencing absent analysis evidence; and
- the complete-peeling pressure guard above prevents the allocator-hostile
  matmul expansion encountered during the application build.

Segment registers do not replace a general address register: `es`, `fs`, and
`gs` hold the selector half, while the effective offset still needs an integer
register. Native commuted 16-bit addressing is therefore chosen first. The
existing 67h 32-bit address form remains a secondary legal alternative before
an actual spill or recomputation when its inputs are already suitable; it is
costed rather than forced when exact 16-bit inputs would first require two
extensions.

The final executable linked with the prepared, reindexed WC/BCC-compatible
library, passed the 64-KiB DGROUP check with 7,264 bytes of near heap, and ran
the title demo visibly in DOSBox with dynamic core, maximum cycles, high
priority, lightmaps, and SB16 sound configured. Tier 1 passes 258 tests in
1.15 seconds. This is an integration gate for the current Phase 4 allocation
and C ABI work, not a claim that Phase 4 or the broader GCC/Clang-quality plan
is complete. The raw qc-port loops and GCC/LLVM reference listings still drive
the remaining spill, folding, scheduling, and whole-module work.
