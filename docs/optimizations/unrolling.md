# Bounded unrolling in the fixed-point pipeline

## Current state

Exact-trip analysis now covers unsigned relational tests as well as signed
ones. This is required by ordinary C loops: `unsigned short i; i < 8` reaches
the loop body through `jb`, while the former proof understood only `jl`/`jle`.
The proof interprets start and bound in the comparison's signedness, keeps the
recurrence step signed, and refuses the expansion when the post-loop update
would wrap at that width.

The bounded expansion mechanism now accepts integer loops. The C frontend's
explicit terminal jump back to the header is removed from the repeated work;
the object frontend's implicit CFG backedge remains equally valid. Integer
headers must be side-effect free apart from their final branch, and calls,
opaque control, barriers, and internal branches remain refusals. Existing
floating-loop legality is unchanged, including its separately validated call
and checkpoint ordering.

On the 386 quality report, matmul's exact eight-element checksum loop changes
from a backedge to eight ordered bodies. Emitted metrics change as follows:

| Metric | Before | After |
|---|---:|---:|
| Estimated dynamic operations | 11,521 | 10,800 |
| Static branches | 16 | 12 |
| Stores | 35 | 31 |
| Bytes | 412 | 519 |
| Static weighted cost | 584 | 776 |

This is an explicit speed/size tradeoff: the profile-free hot-operation
estimate improves 6.3%, while code grows 26.0%. Static weighted cost is the
sum over the body and therefore also grows; it is not a runtime-frequency
cost. The first version still missed the inner matrix-product loop after
strength reduction: the comparison read a copy of the header recurrence, and
exact-trip analysis required the PHI result itself. Copy-transparent recurrence
matching now proves that loop as eight trips too. This is the same structural
choice made by the local GCC 16.2.0 i686 reference, which also expands the
fixed-size inner product. Against the first integer-unrolling result above, the
current 386 report is:

| Metric | Before copy-transparent SCEV | After |
|---|---:|---:|
| Estimated dynamic operations | 10,800 | 8,588 |
| Static branches | 12 | 9 |
| Spill reloads/stores | 0/0 | 18/16 |
| Bytes | 519 | 697 |
| Static weighted cost | 776 | 939 |

The estimated dynamic operation count improves 20.5%. The increased spills,
34.3% code growth and static-cost growth are recorded rather than hidden: this
is a speed-first full-unroll choice, not the final pressure-aware selector.
The raw assembly contains eight multiply/add steps in source order. The report
makes no elapsed-time claim; a future partial-unroll selector should compare
size, frequency and predicted spill cost per CPU instead of treating full
expansion as universally final.

The follow-up checkpoint/ownership change reduces FPCSE's static costs from
324/359/314 to **173/182/167** (PDS/QB/VBDOS). SINGLE PRINT is a four-byte
by-value argument, verified in prnval.asm's PRINTX path; its source address
does not escape. The emitted call now pushes the bits of 487.5 directly,
instead of reloading the two words of `s` after printing the label.

Original WAIT instructions now raise as semantic FCHECK operations, retaining
their input encoding provenance for lowering. CSE may remove a repeated
checkpoint only across modeled integer work: calls, opaque operations,
barriers and floating/stack operations invalidate the proof. A surviving
checkpoint blocks dead-store deletion across its possible exception
observation. The regression explicitly checks that boundary.

Dead zero-byte inserted operations need no neighbor to inherit bytes, because
they own none. Fixing that ownership rule lets DSE actually delete the
overwritten unrolled stores it already identifies. Floating-origin markers
remain for sequence validation. Before, ten iterations retained stores and
waits; after, one checkpoint and final stores remain. The source constants
and loop-counter stores are still not minimal. The provisional target is
unchanged and these are not elapsed-time measurements.

74 focused tests pass. The 96-primary-object differential changes FPCSE,
FPDEEP, ROTATE and SPLIT on all three compilers, with no emission-status
changes; all changed programs passed their focused runtime checks. FPCSEX
and FPEMU also passed. Final FPCSE stage dumps and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-observation-guard-a9mtvkyx`.
The earlier before listing is under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-exact-loop-42yop23c/after`.

Bounded unrolling is now a transaction at its original post-placement pass
boundary. The raw expansion is sent through the ordinary scalar fixed point,
then accepted only when it removes a loop and its exact-trip dynamic cost falls
by more than the target-priced charge for added semantic operations. The cost
model is the shared machine-neutral `crates/llrm-core/src/optimize/profit.rs`; MIR sees arithmetic,
memory, call, branch and x87 prices, never registers or encodings. Unpriced work
rejects the candidate. A rejected loop is skipped while later candidates are
considered, and callers can still disable expansion with `unroll_=False`.

Waiting for the scalar fixed point before proposing the expansion was tested
and reverted: it destroyed matmul's recognizable exact inner loop. The existing
emitted regression failed with 13 branches. Asking at the original boundary and
converging only the candidate retains the eight-element expansion. On the 386
quality report the current `_bench_matmul` is 642 bytes, 165 instructions, 849
modeled static units and 6,726 estimated dynamic operations, with its independent
runtime answer unchanged. `_bench_crc` remains 225 bytes / 63 instructions and
`_bench_nbody` remains 598 bytes / 138 instructions. These are structural model
results, not elapsed-time claims.

The eight-profile scan retains 165 matmul instructions everywhere (642 bytes on
386/486/P5, 646 on P6/K5/K6/K7/Core), 63 CRC instructions / 225 bytes, and 138
nbody instructions / 598 bytes. Their profile costs differ as intended; their
hard targets are still missing, so this is cross-target consistency evidence,
not a completed GCC/Clang parity gate.

The next exact-loop boundary was structural rather than arithmetic: after the
eight-term dot product was expanded, the enclosing column loop had useful work
spread over a straight-line chain of MIR blocks. The old expander admitted only
empty bridge blocks, so it never submitted that known eight-trip loop to the
per-CPU profitability transaction. Straight-line multi-block bodies now clone
in execution order; internal branches, calls, barriers, phis, and multiple
successors remain refusals. A 512-operation construction guard is a compile-time
bound, not a profitability decision.

Every CPU profile accepts the same speed-first expansion. The emitted matmul
body changes from 6,726 to 5,046 estimated operations on 386/486/P5 and 5,136
on P6/K5/K6/K7/Core. Static size grows from 165 instructions and 642/646 bytes
to 519 instructions and 2,176/2,180 bytes; static branches fall from nine to
eight. This 25% hot-work reduction for 239% byte growth is an explicit audited
tradeoff, not final parity: Clang's scalar i386 reference still executes much
less work, and later loop/SROA work must recover the size. The regression names
the former 6,726-operation symptom and leaves room for a different future
mechanism to satisfy it.

The first multi-block expansion returned `4252537476` instead of matmul's
independent `353712` answer. Its original first-iteration bridge still read the
header phi values after the phis had been removed, so lowering exposed three
undefined array-address inputs and allocation emitted an uninitialized spill
reload. Stage validation found the defect in the raw second unroll candidate.
Entry substitution now applies to every original bridge, while cloned bridges
receive the corresponding carried substitution. The focused regression rejects
any exposed procedure input, and the seven-program DOS known-answer corpus
passes with the expansion enabled.

## Exact CFG peeling and dependent bounds

Straight-line expansion cannot prove C nbody's triangular inner loop directly:
``j`` starts at ``i + 1``, so its trip count is unknown until the four-trip
outer ``i`` loop is specialized. The production peeling pass now uses the
existing CFG cloner for that case. It first closes loop live-outs, clones every
block for the one proven trip count shared by the loop's recurrences, retains a
residual correctness loop, and submits the candidate to the ordinary scalar
fixed point. Acceptance requires that simplification remove the residual loop
and that target-priced dynamic savings exceed the semantic growth charge.

That exposes three-, two-, and one-trip inner loops, which the ordinary exact
unroller removes. On 386, `_bench_nbody` changes from 33,632 to 5,063 estimated
dynamic operations. The first emitted candidate exposed a separate memory-form
gap: chained fixed frame addresses such as ``&x[4] - 16`` occupied registers
and produced 23 spill reloads plus 10 spill stores. Address selection now folds
the complete pure constant chain to a single BP displacement. The final result
is 280 instructions, 1,095 bytes, zero address calculations and zero allocator
spills, versus the strict i686 GCC reference's 267 instructions and roughly
5,132 estimated operations. The independent nbody answer remains 4,774,160,
and all seven C corpus answers pass through OMF, LINK and DOSBox.

Peeling also exposed a correctness defect in dead-block cleanup. Once SCCP
proved the residual floating loop unreachable, the old partial erasure left
x87 semantics attached to a `nothing` marker and lowering refused it. Dead CFG
blocks now use the same complete inert-operation constructor as all other MIR
deletion. A focused fail-first regression checks both the retained source owner
and the absence of stale floating computation.

C matmul exposed the next nested-loop boundary.  Peeling its branchy inner
initializer produces 473 semantic operations in the enclosing exact eight-trip
loop; evaluating that outer specialization therefore needs 3,784 transient
operations.  CFG peeling now has a 4,096-operation construction ceiling while
the simpler straight-line unroller retains its 512-operation bound.  This is a
resource allowance, not an acceptance shortcut: the residual loop must still
disappear under the ordinary fixed point and the selected CPU's priced dynamic
savings must still exceed semantic growth.  The emitted initializer contains
no runtime `div`, branches fall from 21 to 4, and the profile-free estimate
falls from 4,481 to 3,494 executed instructions.  The emitted body is 602
instructions / 2,647 bytes, so this is a speed-first intermediate result rather
than the size target.  Its fresh OMF object links under the VBDOS toolchain and
returns the independent answer 353,712 in DOSBox.

That larger candidate also caught a fail-first address-form defect.  Recognizing
a constant-derived frame address had been treated as proof that its parent was
dead, even when the child remained an ordinary value; lowering consequently
read three ADD sources whose shared LEA definition had vanished.  Frame-address
deletion is now proved bottom-up from actual folded memory leaves.  Every child
operation must itself be deleted before its parent can be deleted.  The focused
regression preserves the root of a live derived value, while the existing
complete-chain regression still folds `&x[4] - 16` into one BP displacement.

The raw matmul comparison then identified a separate phase-ordering boundary:
after specialization, all 64 stores to `b` are constants, but SROA/promotion had
already run and therefore could not forward those cells into the dot products.
SROA now runs once on each structural candidate before its ordinary scalar fixed
point.  It remains outside the repeated scalar rounds, which avoids paying for
global pointer/range analysis after every local simplification.

That boundary exposed three pointer-provenance defects which now have focused
fail-first regressions.  SSA repair and hoist reparenting renamed operations but
left `pointer_values` and `pointer_seeds` naming discarded values; CFG peeling
and straight-line expansion did the same for fresh clones.  Finally, alias
analysis computed provenance for `pointer + constant` but discarded it unless
the frontend redundantly classified every intermediate result.  Metadata is now
renamed with values, clones inherit it, and an operation derived from a proven
pointer propagates its own fact.  A 16-bit folded displacement is interpreted at
its MIR width, so `base + 16 + 65520` is the same exact object position as
`base`, not an out-of-bounds 65,536-byte advance.

The adjacent stage dump now shows all 64 matrix loads as exact scalar leaves at
`peel-sroa`, where none were exact before.  The emitted matmul acceptance test,
which failed first with 65 `imul` instructions, now passes its fewer-than-16
gate while retaining the no-division, branch-count, exposed-input, and dynamic-
work checks.  This larger scalar candidate takes about 172 seconds to compile on
the development host versus roughly 90 seconds before the new leaves were
exposed; that compile-time cost is recorded as a remaining optimization problem.

The full report exposed 192 semantic stores after all 64 matrix-result loads had
been promoted.  Exit ownership recognized only direct `[bp+n]` cells, so DSE
could not prove that a bounded canonical frame object ceased to exist at return.
The first adjacent stage difference is `mir-r01-drop_stores` after final
specialization: 14,691 operations / 192 stores becomes 14,627 / 128; every
earlier recorded stage is identical.
Current-activation frame objects are now private when their canonical pointer is
not published through a call argument, return, escape, opaque operation, barrier,
or non-frame store.  Publication is conservative when a direct frame address has
no SSA identity to match.

The first implementation deleted too much and returned **2,990,729,762** instead
of 353,712.  DSE still called every indirect load unnamed; consequently privacy
shielded a real canonical pointer load from an overlapping private store.  The
fail-first regression records that wrong answer and requires canonical object-and-
byte provenance to count as a named reference.  Unresolved pointers retain the
old conservative behavior.  The corrected emitted object links with Microsoft
LINK 5.31 and returns the independent 353,712 answer under DOSBox.

Against the exact post-SROA baseline, the corrected 386 report is:

| Metric | Before frame ownership | After |
|---|---:|---:|
| Bytes | 3,769 | 3,385 |
| Instructions | 880 | 816 |
| Static weighted cost | 3,106 | 2,978 |
| Estimated dynamic operations | 1,702 | 1,638 |
| Loads / stores | 374 / 332 | 374 / 268 |
| Spill reloads / stores | 143 / 71 | 143 / 71 |

The 64 removed stores are the fully promoted `c` matrix.  The 128 `a` and `b`
initializer stores remain because their in-body pointer loads are observable.
The body contains one `imul`, no division, and three final branches.  Allocation
is still the dominant remaining gap: frame ownership removes semantic stores but
does not change the 143/71 spill pair.  These are static/model measurements, not
elapsed runtime timings or a completed GCC/Clang parity target.

Larger-than-four-trip loops may now expand within that same operation budget
when every extended floating result in the expanded loop is proven exact.
FPCSE's ten ordered iterations satisfy this; FPCSEX's runtime-input loop does
not. The lowerer checks actual repetition provenance and sequence length,
not the optimizer's former four-trip profitability policy.

FPCSE's `s = s + p + q` is evaluated in source order with each SINGLE store
checked for exact representation. All floating arithmetic disappears and
the final store holds `43f3c000h` (487.5). Before, each iteration emitted
`fld [s]; fadd [p]; fadd [q]; fstp [s]`; after, it uses exact constant stores
and retained exception checkpoints. Some counter stores and checkpoints
remain, so this is not yet the minimal program.

Static costs change PDS 1690→324, QB 1719→359, VBDOS 1680→314. Objects grow
878→977, 889→1002, 1054→1153 bytes respectively. These are not timings, and
the old provisional target is not validated by these improvements.
All six FPCSE/FPCSEX output checks pass on the three compilers, along with
28 focused tests. A differential comparison of 96 primary objects changes
only the three FPCSE objects, with no emission-status changes. The new
FPCSE regression failed before the expansion change. Dumps of every stage,
before/after assembly and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-exact-loop-42yop23c`.

The normal pipeline passes all 33 FPDEEP output checks across PDS, QB and
VBDOS, with no experimental wrapper. The 26 focused unrolling and exact-store
tests pass; disabling the pass makes the pipeline regression fail. Reapplying
the pipeline leaves the resulting body unchanged.

A one-off differential emission check over all 487 fixture objects took
61 seconds: only 12 FPDEEP variants changed bytes, all retaining LIR emission.
No emission outcome changed. This is a byte/outcome comparison, not 487
runtime executions.

PDS FPDEEP's object changes from 1497 to 1925 bytes. This trades code size
for removal of repeated arithmetic, not a claim of smaller code. The first
square changes from `fld; fmul; fstp` to `mov dword [scratch],43100000h`
(144), preserving floating exception checkpoints. Runtime output artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-default-unroll-8vopmthi`.
Every pass and emitted assembly, before and after:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-pipeline-unroll-stages-jhhx_l1m`.
These checks do not provide elapsed-time measurements.

The post-allocation constant peephole now retains register knowledge across
empty byte-ownership markers. FPDEEP previously emitted `mov ax,0` twice
because the removed instruction between them cleared that knowledge. It now
emits one move, reducing the PDS object from 1925 to 1922 bytes. Markers with
definitions or clobbers, unknown instructions and calls remain boundaries.
This changes no MIR semantics and adds no LIR optimization tier. The focused
regression failed first; 81 peephole tests and 33 FPDEEP output checks pass.
New stage dumps and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-marker-constant-4ljmnufn`.

Subsequent exact-store folding reduces expanded FPDEEP to roughly 2.0x
target (PDS 2166, QB 2201, VBDOS 2162). The earlier comparison below records
the integration baseline, not today's folded result. See
[the emitted before/after and runtime checks](constant-index.md).

Expanded bodies now carry explicit `(block, iteration-count)` provenance.
The floating checker requires the exact original sequence repeated that
many times; absent provenance, duplicate entries, incorrect counts, missing
operations and reordered operations are rejected. Existing operand and
semantic checks remain. Lowering carries an ordered-layout requirement
through allocation to object emission automatically. Mixed legacy/ordered
bodies are refused until per-body ordering is supported.

With only the experimental MIR transform selected, **no checker or writer
monkeypatch**, FPDEEP passes all 33 output checks across PDS, QB and VBDOS.
58 focused expansion, emission and floating-allocation tests pass.
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-clone-integrated-warr3ky_`.

The former generic cost model's ten-trip assumption exaggerated this improvement:
FPDEEP actually runs three iterations. The transactional selector now keys the
proved count by latch and uses it for this decision. The historical recosting
that motivated that correction was:
iterations gives the following static estimates, **not runtime timings**:

| Compiler | Normal optimized | Expanded + folded | Target |
|---|---:|---:|---:|
| PDS | 4174 | 4047 | 1086 |
| QB | 4372 | 4209 | 1086 |
| VBDOS | 4168 | 4045 | 1086 |

The reduction was only about 3–4%, while PDS's object grew from 1497 to
2176 bytes. Expansion therefore remained off at that point: the next gain had to
come from eliminating the now-known computations, not from merely copying
them. Exact facts and argument propagation are implemented; see
[constant-index.md](constant-index.md).

## Earlier prototype history

The original `qbopt/optimize/unroll.py` prototype was **not in the pipeline**. It expands
small, proven constant-trip straight-line floating loops, preserving each
call and floating operation in iteration order. FPDEEP has three iterations
and prints three records per iteration; numeric loop deletion is not a
valid substitute.

The implementation follows the value-remapping pattern in the local LLVM
`llvm/lib/Transforms/Utils/LoopUnroll.cpp`: entry phi inputs seed the first
iteration, each iteration receives fresh definitions, and latch values seed
the next. Final definitions replace dominated live-out uses. No instruction
timings or register choices belong to this transformation.

### Original emission blocker (fixed)

Experimentally allowing repeated floating provenance through the old
sequence guard produced a PDS FPDEEP timeout. The emitted-stage dump showed
the cause directly: instead of each iteration's `push; call`, emission
grouped the three pushes, then the three calls. Only the first call carried
its relocation; following calls were bare `call 0:0`.

`crates/llrm-core/src/backend/layout.rs` sorts operations by original address. Clones deliberately
share source provenance, but must not share placement identity. The current
adapter also discarded cloned calls' relocation provenance. These are backend
requirements, not reasons for MIR to invent machine addresses.

Follow-up inspection corrected the initial diagnosis: `objwrite.written` and
The former record-rewriting writer already supported one-to-many fixup destinations. The loss
was earlier, in treating zero-byte clones as unrelated inserted instructions.
The prototype now explicitly retains symbolic operand provenance. The assembler
recognizes an explicitly retained far-call target even without an owning node.
Its regression emits two far calls with relocation requests `(1,1)` and `(6,1)`:
two output fields, the same original source field.

`layout.rebuild` and `objwrite.written` now accept explicit `ordered=True`.
The byte-level regression changes the bad sequence `mov ax,1; mov ax,3;
mov ax,2` back to the requested `mov ax,1; mov ax,2; mov ax,3` despite the
third instruction sharing the first one's source address. Legacy callers
retain the default ordering. A trial enabling it globally changed event
objects and NOTS/PROCS variants; the PDS NOTS/PROCS output checks passed, but
that is not sufficient evidence to switch every existing caller.

The experimental pipeline integration and relaxed sequence guard were
removed. Default optimization is unchanged. The regression explicitly
requires lowering to refuse this prototype until clone-aware emission exists.
The failed execution is evidence of a miscompile, not successful unrolling.

### Earlier implementation boundaries (historical)

Block-entry occurrences are now anchored independently in the opt-in ordered
emitter. Branch relaxation recomputes those positions on every layout round;
an earlier clone no longer captures the original header's label. The prototype
also emits an explicit jump from the expanded latch to the exit instead of
falling through to the original header.

With an experimental, process-local allowance for exact repeated floating
sequences and ordered emission, FPDEEP passes all 33 output checks across PDS,
QB and VBDOS. The default pipeline and floating refusal guard remain unchanged.
Artifacts: `qbopt-clone-labels-k8wgg42a` and `qbopt-clone-compilers-a_p3y6ax`
under `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T`.

Unrolling alone does not yet establish the expected numeric facts: PDS has
zero exact floating facts before and after another optimization round.
`consts._operand` hands indexed cells directly to `_cell`, which refuses a
base even when its SSA value is constant. Resolving a proven constant offset
is the next dataflow step, before deciding whether expanded code is profitable.

1. Separate ordered emitted occurrences from original byte ownership.
   Preserve the LIR block/instruction order; source addresses remain provenance.
2. Preserve exactly one original block-label destination independently of
   repeated instruction provenance. `_placed` currently chooses the first
   occurrence of an old address, which may now be a clone preceding the original
   header. Keep exactly one owner for consumed original bytes. The OMF writer's
   existing fixup fan-out is usable once every cloned instruction retains its
   relocation request.
3. Verify output call order, relocations, SSA live-outs and real FPDEEP output
   before enabling the pass. Only then let the ordinary passes fold indexed
   constants across the expanded iterations and compare cost against 1086.

The MIR regression fails when the transformation is disabled. With it enabled,
the loop disappears, effect identities repeat in their original order three
times, definitions are unique, and applying it again changes nothing.
This is structural coverage, not a proof of full unrolling correctness.

Failed-run artifacts and full stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-unroll-_nme4yh1`.
