# Bounded unrolling: MIR prototype, emission not ready

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

## Observed emission blocker

Experimentally allowing repeated floating provenance through the old
sequence guard produced a PDS FPDEEP timeout. The emitted-stage dump showed
the cause directly: instead of each iteration's `push; call`, emission
grouped the three pushes, then the three calls. Only the first call carried
its relocation; following calls were bare `call 0:0`.

`backend/layout.py` sorts operations by original address. Clones deliberately
share source provenance, but must not share placement identity. The current
input-field-to-output-field relocation mapping likewise cannot represent
one input fixup used by three emitted instructions. These are backend
requirements, not reasons for MIR to invent machine addresses.

The experimental pipeline integration and relaxed sequence guard were
removed. Default optimization is unchanged. The regression explicitly
requires lowering to refuse this prototype until clone-aware emission exists.
The failed execution is evidence of a miscompile, not successful unrolling.

## Next implementation boundary

1. Separate ordered emitted occurrences from original byte ownership.
   Preserve the LIR block/instruction order; source addresses remain provenance.
2. Attach relocation requests to emitted occurrences, allowing one source
   fixup to supply multiple output fields. Keep exactly one owner for consumed
   original bytes and one branch target mapping for each original label.
3. Verify output call order, relocations, SSA live-outs and real FPDEEP output
   before enabling the pass. Only then let the ordinary passes fold indexed
   constants across the expanded iterations and compare cost against 1086.

The MIR regression fails when the transformation is disabled. With it enabled,
the loop disappears, effect identities repeat in their original order three
times, definitions are unique, and applying it again changes nothing.
This is structural coverage, not a proof of full unrolling correctness.

Failed-run artifacts and full stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-unroll-_nme4yh1`.
