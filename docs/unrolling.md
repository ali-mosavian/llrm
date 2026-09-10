# Bounded unrolling: integrated emission, default still off

## Current state

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

The generic cost model's ten-trip assumption exaggerates this improvement:
FPDEEP actually runs three iterations. Re-costing both sides with three
iterations gives the following static estimates, **not runtime timings**:

| Compiler | Normal optimized | Expanded + folded | Target |
|---|---:|---:|---:|
| PDS | 4174 | 4047 | 1086 |
| QB | 4372 | 4209 | 1086 |
| VBDOS | 4168 | 4045 | 1086 |

The reduction is only about 3–4%, while PDS's object grows from 1497 to
2176 bytes. Expansion therefore remains off by default: the next gain must
come from eliminating the now-known computations, not from merely copying
them. Exact facts and argument propagation are implemented; see
[constant-index.md](constant-index.md).

## Earlier prototype history

`optimize/unroll.py` is deliberately **not in the pipeline**. It expands
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

`backend/layout.py` sorts operations by original address. Clones deliberately
share source provenance, but must not share placement identity. The current
adapter also discarded cloned calls' relocation provenance. These are backend
requirements, not reasons for MIR to invent machine addresses.

Follow-up inspection corrected the initial diagnosis: `objwrite.written` and
`relocate.as_records` already support one-to-many fixup destinations. The loss
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
