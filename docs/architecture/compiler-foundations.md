# Compiler foundations: correctness, code quality, and the MIR boundary

## Goal

Produce the code a modern optimizing compiler should produce, without making
MIR a model of x86. Every documented target must be within **1.5×** of an
independently derived reference, under equivalent semantics. Correctness is
a prerequisite; refusals and unchanged fallbacks are not optimization wins.

This is a plan, not a claim that these foundations are finished. It applies
the boundary in [split.md](split.md) to the priorities below. Historical
implementation inventories in other documents need remeasurement before use.

## What has gone wrong

We have repeatedly made the next program work without completing the shared
mechanisms it exposed. A passing program established coverage of that case,
not general support for its language constructs, ABI, or loop structure.

The “Compare qbopt performance” task provides a concrete example. BUMPY first
hit rejected dead `SELECT CASE` jumps, unowned private `GOSUB` bodies,
unsupported FAR-memory lowering, and missing runtime contracts. A later
rebuild corrupted arguments by reserving spill space before `B$ENSA`
established the frame. These are reconstruction problems, not optional
optimizations.

After support fixes, the same task reported this hot-procedure expansion:

| Stage | Count |
| --- | ---: |
| Original BC instructions | 187 |
| Optimized MIR operations | 162 |
| After phi elimination | 220 |
| After two-address conversion | 269 |
| Final emitted instructions | 270 |

MIR operations and machine instructions are not interchangeable costs, but
the backend stage differences locate the copy expansion. In its fixed-light,
500-frame benchmark, BC achieved **31.29 FPS**, qbopt **28.89 FPS**, and a
handwritten JWasm reference **91.95 FPS**. These are transcript-reported
DOSBox results (`core=dynamic`, fixed 75k cycles), not hardware timings or a
proof that the handwritten version is optimal. Two framebuffer checksums
matched; that is not a full equivalence proof. The work was on the separate
`codex/bumpy-support` branch, not evidence that this checkout has those fixes.

The lesson is twofold: incomplete reconstruction keeps blocking new inputs;
excessive copies and spills can erase valid MIR improvements afterward.

## Concern and ownership

| Concern | Where it belongs |
| --- | --- |
| OMF records, relocations, code/data classification, procedure and `GOSUB` ownership | Object reader, decode, control-flow discovery |
| LONG pairs, runtime idioms, emulator instructions, physical partial-write semantics | Raise: translate into explicit value semantics |
| Types, values, comparisons, memory objects/effects, exceptions, control flow | MIR |
| CSE, DSE, promotion, LICM, ranges, loop recurrences | Machine-independent MIR analysis and passes |
| Symbol resolution, selected library members, transitive call effects | Link-input and contract analysis |
| Semantic call effects: reads/writes, escape, return relationships, exceptional exits | Contract summaries consumed by raise/MIR |
| Argument locations, register clobbers, stack cleanup, frame establishment | Physical ABI summaries consumed by raise/backend |
| Encodings, addressing modes, target timing tradeoffs, operand constraints | Lowering/instruction selection |
| Phi realization, parallel copies, tied operands, coalescing, splitting, spills | Backend legalization and register allocation |
| Physical redundant moves and segment reloads | Post-allocation peephole |
| Branch distances, instruction layout, rewritten relocations | Layout and emission |

Backend legalization and allocation are not a second optimization pipeline.
They implement MIR efficiently; they must not rediscover CSE, LICM, or source
loop transformations in LIR.

## How to achieve it

### 1. Establish correct reconstruction

- Define a support matrix by construct, compiler, ABI, and relevant flags—not
  just program name. Include mixed BASIC/C inputs, arrays, FAR accesses,
  local subroutines, events, and exceptional control flow.
- Preserve byte-identical OMF read/write round trips separately from semantic
  raise/lower rebuilds. A rebuild need not retain BC's register assignment.
- Run representative real programs with MIR optimization disabled. Compare
  observable output and rendered data; diagnose unsupported inputs explicitly.
- Turn each discovered failure into a general boundary rule and a small
  fail-first regression, plus a real-program check for the related batch.

Exit criterion: the declared support matrix reconstructs correctly, without
program-name/address exceptions or hidden fallback. New combinations remain
useful tests; no finite corpus proves all programs correct.

### 2. Make the backend competent

- Use BUMPY and qrender loops to trace phi copies, tied-operand copies,
  rematerialization, spills, and edge jumps separately.
- Coalesce compatible values before materializing unnecessary copies; resolve
  parallel copies correctly and place required copies on the right edges.
- Keep frequently used loop values live where profitable. Rematerialize cheap
  constants rather than storing and reloading them through extra frame slots.
- Price instruction choices with their legalization costs, not just the
  arithmetic instruction in isolation. Target-specific choices stay here.

Exit criterion: representative rebuilt hot paths have no unexplained copy
or spill expansion and no reproducible runtime regression against BC under
matched workloads. Necessary spills remain legal; minimizing instruction
count alone is not the objective.

### 3. Make semantic improvements survive emission

- Remove unobserved frame stores using escape and memory-effect proofs.
- Hoist invariant array metadata and row bases; express addresses as loop
  recurrences, leaving register and addressing-mode choices to the backend.
- Use proven ranges for arithmetic simplification without weakening numeric,
  bounds, event, or exception policies.
- Add full OBJ/LIB analysis where missing contracts block these proofs. Honor
  ordered symbol/archive resolution; solve recursive call components to a
  conservative fixed point. Unknown or unsupported callees remain unknown.
- Keep semantic effects separate from physical ABI contracts, versioned and
  tied to the actual input libraries. MIR may ask whether a call modifies an
  array; it may not ask whether it preserves SI.

Exit criterion: correct emitted hot paths close the independently measured
target gaps. Report modeled cost and runtime separately; neither substitutes
for the other.

## How not to achieve it

| Pattern we must stop repeating | Required replacement |
| --- | --- |
| Patch each new program until it passes, then declare support complete | Fix the shared construct/contract and verify related configurations |
| Teach a MIR pass about registers, original bytes, or runtime calling sequences | Complete recognition in raise or implementation in the backend |
| Preserve BC's incidental register state as artificial loop-carried values | Model actual semantic dependencies and physical boundary obligations separately |
| Add more MIR passes while their savings disappear in allocation | Fix the first stage that introduces avoidable expansion |
| Treat smaller objects or passing test counts as speed evidence | Inspect hot-path assembly and run matched workloads |
| Re-run broad suites while guessing at the defect | Dump every stage, diff adjacent stages, then run the focused regression |
| Accept refusal, a weakened assertion, or a raised denominator as progress | Keep the failure visible and the reference independent |

## Enforcing the boundary and the working method

MIR contains semantic operations, not chosen instructions. Necessary widths,
bit preservation, pointer behavior, and effects must be explicit. Original
storage and byte provenance belong in side tables for diagnostics and the
backend; optimization decisions must not read them. No new machine-access
exceptions in MIR passes, and no emit/re-raise optimization loop.

Use the existing architectural checks and add semantic invariance checks:
changing only diagnostic provenance or original register assignments must
not change a MIR pass's decisions. Study LLVM/GCC mechanisms in
`~/work/other` where useful, but adapt their principles rather than copying
a machine-optimization tier across llrm's boundary.

For each bounded change: capture before assembly and all stage dumps, locate
the first defect or expansion, fix its owning mechanism, run a fail-first
and mutation-checked regression, and show after assembly. Run the expensive
integration gate for a related batch, not after every investigative edit.
Honor the repository commit gate. Record result, evidence, and remaining gap;
do not substitute repeated testing or narration for implementation.

**Review question:** could the MIR transformation still be stated if this
body were lowered to a different machine? If not, move the decision to the
appropriate side of the boundary.
