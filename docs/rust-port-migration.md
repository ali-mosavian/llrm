# Rust port migration ledger

This ledger records the evidence and elapsed verification cost for each port
iteration.  Command durations include compilation triggered by the command.

## Iteration 0: restore the merged baseline

Base: `3aeb1857c3340d0aa5901d4829083b5023aeaf15` on `main`.

The first default Python run took 84.02 seconds: 458 tests passed, two HIR
tests failed, and 31 were deselected.  The first QB Cargo run found two stale
generated-parser golden gates and later exposed three HIR assertions tied to
the pre-source-symbol JSON layout.  Rust also warned about an unused `DATA`
row binding.

The parser snapshots were migrated only after a batched comparison of all 79
accepted programs.  Seventy-eight matched the legacy snapshots after removing
only four representation additions.  `qb45/memorymodel` was the sole semantic
delta: its source had changed in `11c6d519`, while its snapshot still described
the earlier program.  The current gate compares the complete AST exactly and
has no compatibility filter.

Pytest now builds the in-tree QB frontend once per session and uses the
executable path reported by Cargo.  Compatibility-help validation now indexes
the inherited case list once instead of reparsing it for every topic.  Eight
measured end-to-end backend and OMF regressions remain in the `full` tier rather
than the Tier 1 loop.

Final gates:

| Command | Result | Wall time |
| --- | --- | ---: |
| `cargo test --manifest-path frontends/qb/Cargo.toml -q` | 43 unit, 2 generated-lexer, 4 parser-golden, and 130 semantic tests passed | 3.35 s |
| `uv run pytest -q --durations=15` | 459 passed, 39 deselected | 12.42 s |
| `uv run pytest -q tests/test_qbcompat.py` | 20 passed | 3.80 s |

The default Python gate fell from 84.02 to 12.42 seconds.  Recorded verification
execution, including fail-first and mutation runs performed by delegated
agents, stayed below 240 seconds during the first 2,428 seconds of wall time.

The repository-wide `ty` pre-commit gate is not green on the base commit.  The
locked 0.0.75 tool reports 3,016 diagnostics, and the manifest's original
0.0.1a20 constraint reports 2,416.  This is an inherited typing backlog rather
than dependency drift or a regression from the port.  The Python commits ran
the formatting, import, whitespace, merge-marker, and focused behavioral gates;
the known-global `ty` gate was skipped and remains explicit debt.

The first commit attempt also invoked the old 77-second pytest selection before
the separately reviewed tiering commit was staged.  That accidental duplicate
temporarily raised cumulative verification to roughly 328 seconds over 2,757
seconds of wall time (11.9%).  No further broad verification runs are permitted
until implementation and review time bring the cumulative ratio below 10%.

## Iteration 1: freeze the source-frontend contract

The portable Rust IR contract is fixed in `docs/rust-ir.md`.  It defines the
HIR/IR/Machine IR boundaries, verifier and editor rules, object-frontend change
map, and the one sanctioned post-allocation target peephole.  This is the
semantic contract for the port; the Rust implementation will not translate the
Python MIR data structures.

Two provenance-complete external gates were added for later differential use:

- `tools/qrender_gate.py` emits every QB module once, records all link inputs,
  checks the benchmark and four existing qrender oracles, and refuses footprint
  regressions.
- `tools/gorillas_gate.py` derives a pinned deterministic graphics probe from
  Microsoft's original source, compares BC45 and direct-frontend behavior, and
  refuses BASIC-owned or complete linked-code footprint regressions.

The user explicitly deferred qrender because its external project is not yet a
working oracle.  No qrender compilation or runtime result is claimed.  A real
Gorillas run was also stopped after 40.20 seconds during allocation when the
verification-budget instruction was reiterated; its partial receipt is not a
passing result and runtime execution remains deferred.

Stage capture no longer recompiles a diagnostic reconstruction.  The optional
QB compiler observer records HIR, both MIR boundaries, LIR, each machine phase,
final allocated LIR, and the exact assembly model from the same invocation that
emits the OMF object.  Its focused regression proves that observing the pipeline
does not change the emitted object bytes.

Focused verification for the new instruments, including delegated fail-first
and primary-review runs, consumed about 52 seconds.  Together with Iteration 0,
cumulative verification is roughly 380 seconds over 6,213 seconds of elapsed
work (about 6.1%).  No broad suite or external runtime gate was completed in
this iteration.

The qrender runtime criterion is therefore a named deferred acceptance item,
not evidence of correctness.  Per the user's direction, the port proceeds to
the dedicated branch and keeps future checks scoped to the subsystem being
implemented.

## Iteration 2: lowercase paths and branch

The port now develops on `rust-port`.  Every tracked path is lowercase except
Cargo's required `Cargo.toml` and `Cargo.lock`; the repository gate checks both
uppercase paths and case-folding collisions.  Stale manifest, fixture, script,
and documentation references were repaired in the same iteration.

No qrender work was performed.  A focused pytest request was abandoned when
repository-wide startup made it cease to be a focused check.  The commit hook's
broad inherited gate was likewise stopped; subsequent reviewed commits use
explicit subsystem checks and skip that hook.

## Iteration 3: one Rust package

The root Rust 2024 package is named `llrm`, with Rust 1.87 as its MSRV.  The
merged QB frontend lives under `src/frontend/qb`, `buildprs` remains a generator
tool, and the crate exposes the planned LLVM-style subsystem boundaries.  The
legacy `qbfront` binary remains temporarily for the Python comparison oracle.
The production-facing frontend binaries are `llrm-qb`, `llrm-c`, and
`llrm-omf`; `llrm-qb` is the package default while the QB vertical slice leads
the port.

## Iteration 4: typed HIR and `.qhir`

The QB frontend now constructs an owned typed HIR program with typed IDs,
opcodes, terminators, storage, linkage, address spaces, and floating evaluation
categories.  The old private JSON builder was deleted after exact compatibility
comparisons; the remaining JSON encoder is an edge adapter fed only by verified
typed HIR.  New Rust consumers do not parse that JSON.

`.qhir` is a deterministic versioned textual assembly with a strict parser,
printer, and verifier boundary. The first `llrm-qb` vertical slice accepts QB
source, constructs verified HIR through `driver`, and emits `.qhir`.

Focused verification in this implementation stretch included:

| Command | Result | Wall time |
| --- | --- | ---: |
| `cargo check --all-targets` | all current library and binary targets compiled | 1.07 s |
| exact QB-to-HIR-to-`.qhir` test | passed; 130 tests filtered out | 2.85 s |
| exact static procedure-array compatibility test | passed; 130 filtered out | 0.04 s |
| `cargo run --quiet --bin llrm-qb -- ... suite/arith.bas` | emitted canonical 9,085-byte `.qhir` | 2.54 s |

## Iteration 5: OMF foundation (in progress)

Rust now owns lossless record framing, including intentional preservation of
BC's checksum-invalid FIXUPP records, a bounded zero-copy primitive reader,
typed one-based LNAMES/EXTDEF tables, SEGDEF, LEDATA/LEDATA32,
LINNUM/LINNUM32, GRPDEF, PUBDEF/PUBDEF32, and LPUBDEF/LPUBDEF32. Names remain
raw bytes where OMF does not require UTF-8.

OMF library framing retains page padding and the opaque dictionary while
exposing page-aligned modules with absolute record offsets. The common file
entry point distinguishes standalone objects from libraries and round trips
either form without changing bytes. `llrm-omf` now exercises that path for
an untouched rewrite, and `llrm-objdump` reports record framing, checksum
state, public and external symbols, and resolved relocations. FIXUPP and
FIXUPP32 thread state is decoded across records. A decoded-module facade joins
the symbol, segment, declaration, data, line, and relocation views, while the
segment-image builder records overlap ownership instead of silently hiding
backpatches. THEADR/LHEADR identity and MODEND/MODEND32 entry references are
typed as well. MODEND review corrected two format details before acceptance:
its displacement-suppression bit is forbidden, and an absolute physical start
retains its 16:16 form even in MODEND32. Record editing and CodeView remain in
progress.

The record-framing tests ran in 2.09 seconds, primitive-reader tests in 1.67
seconds, and symbol-table tests in 1.63 seconds.  Each selected only its own
module and executed the assertions in 0.00 seconds after compilation.

Every newly decoded record family carries focused Rust tests beside its
implementation, including malformed-record cases. Primary review ran one
representative test per slice; the archive, LEDATA, LINNUM, file dispatch,
declaration, driver, and objdump checks together consumed under 15 seconds.
A real regression object also passed byte-for-byte through the `llrm-omf` CLI in
2.4 seconds.

## Iteration 6: portable IR (started)

The portable SSA model is implemented directly in Rust.  It defines
typed arenas for types, globals, functions, blocks, instructions, and values;
opaque pointers with explicit address spaces; typed constants; CFG
terminators; phi inputs; calls and intrinsics; and explicit memory, trap, and
observable effects. It imports no frontend, HIR, object, MC, CodeGen, or target
module.

The structural verifier now checks IDs and references, function signatures,
CFG targets and phi predecessor sets, result arity, nested constants, and
return shape without mutating IR. `.qir` has a deterministic versioned printer
and strict parser covering every current model variant; parsing ends by running
the verifier. `llrm-opt` provides the first standalone replay and verification
path.

The focused interpreter executes integer SSA, phi nodes, branches, switches,
calls, and the supported exact casts with a deterministic step limit. The first
HIR lowering slice handles scalar CFGs, integer and floating operations,
comparisons, intrinsics, and exact-width casts. HIR booleans become IR `i1`;
BASIC's 16-bit boolean mask remains a distinct value connected by an explicit
extension. Integer/float conversion is refused until the IR records signed
conversion semantics rather than guessing.

Side-effect-free analyses now include deterministic def-use indexing, CFGs,
dominators, and natural-loop membership with nesting, latches, and exits.
Unreachable and irreducible cycles are excluded explicitly.

The verifier and `.qir` commits include Rust-side valid and malformed-input
tests. Primary review found and corrected three compile/API errors in the
delegated verifier, a missing non-void return invariant, a stale no-verifier
assumption in the delegated text parser, and an escape-column error before
acceptance. The scalar-lowering review corrected boolean width and ambiguous
numeric-cast behavior before acceptance. Focused IR checks consumed under
twelve seconds, including compilation; one exact nested-loop check consumed
1.03 seconds after full diff review.

Across the post-baseline work above, measured Rust compile checks and focused
test commands remain far below 10% of elapsed implementation and review time.
No broad suite or external runtime gate was run.

## Iteration 8: analysis and pass infrastructure (started)

The first function pass manager runs an ordered pipeline without printing or
owning concrete analyses. Passes report changes and coarse preservation,
instrumentation observes immutable before/after/failure states, and optional
post-pass verification attributes diagnostics to the exact pass and function.
A pass failure or rejected result restores the original function before the
manager returns, so partial mutations do not escape.

Primary review added that rollback after the delegated implementation left a
failed candidate installed. One focused verifier-rejection check, including
the rollback assertion, ran in 3.24 seconds. Analysis caching remains
deliberately deferred until more than one pass needs it.

`llrm-opt` now runs named, ordered pass pipelines with optional verification
after each pass. Exact integer constant folding, constant branch and switch
simplification, and fixed-point dead-instruction elimination are real Rust
passes. Branch simplification repairs only phi edges that the rewritten
terminator actually removes. Dead-code elimination retains traps, memory
effects, observable operations, and all calls until the IR can prove that a
call returns.

The three transformations ship with focused Rust tests beside their
implementations. The CLI composition test runs
`constant-fold,simplify-branches,dead-code-elimination` on textual `.qir` and
checks the adjacent result. Primary review and the selected transform and CLI
checks consumed about 14 seconds, including one fail-first text-syntax error.

## Iteration 9: Machine IR foundation (started)

The target-independent Machine IR model now has typed IDs, explicit virtual and
physical registers, target-defined register classes and opcodes, operand
use/def roles, fixed/class constraints, two-address ties, frame indices,
symbols with addends, block successors, and instruction properties. It imports
no x86 definitions and contains no SSA values.

Review added explicit virtual-register declarations and rejected fake register
roles on immediates, blocks, frames, and symbols. Load, store, and volatile
properties are represented for later scheduling and verification. The focused
two-address construction check ran in 2.06 seconds.

`.qmir` now has a deterministic versioned parser and printer covering every
current Machine IR field. The parser reports one-based source locations and
leaves structural acceptance to the independent Machine IR verifier. A
side-effect-free liveness analysis computes deterministic virtual-register
live-in and live-out sets to a fixed point, including read-before-write
`UseDef` semantics and explicit malformed-reference errors.

The initial x86 target describes 8-, 16-, and 32-bit integer views, segment and
x87 registers, alias families, allocation classes, stable semantic opcodes,
and all condition-code inversion pairs.

Machine liveness now feeds a deterministic interference graph and deliberately
limited greedy allocator. The generic allocator takes target-owned candidate
and alias hooks; the x86 adapter prevents overlapping views such as `AX` and
`EAX` from being assigned to interfering values. Allocation application is a
separate immutable step which refuses missing, stale, tied, or fixed-register
inconsistent assignments before replacing virtual operands.

Focused `.qmir`, x86, and liveness checks consumed about 9 seconds after full
primary diff review. The first liveness compile exposed an invalid mutable map
index in delegated work; the primary replaced it with deterministic map
replacement before acceptance.

## Iteration 10: MC layout foundation (started)

The MC layer now owns typed sections, fragments, symbols, expressions, fixups,
and physical instructions without SSA values or virtual registers. Its
verifier rejects malformed references and ranges. Deterministic one-shot
layout assigns section-relative fragment and symbol offsets with checked
arithmetic and a single target instruction-size hook; it does not encode,
relax, or mutate fragments.

Primary review corrected a delegated type-inference failure before accepting
layout. The focused model, verifier, and layout checks consumed about 6.5
seconds.

The first exact x86 encoder handles byte, word, and dword physical-register
forms for moves, core arithmetic and logical operations, two-operand multiply,
unary negation/complement, stack operations, and near/far returns in 16-bit
default mode. It emits the operand-size prefix only for dword forms and refuses
every unresolved expression, memory form, control transfer, segment register,
and x87 form it cannot yet encode. A strict Machine-IR-to-MC boundary rejects
virtual registers and stale allocation metadata. Its adjacent regression runs
an allocated move through MC into the expected instruction bytes.

## Current source-to-IR vertical slice

`llrm-qb --emit qir` now composes QB parsing, typed HIR construction, HIR-to-IR
lowering, IR verification, and deterministic `.qir` printing through the
driver. The first end-to-end minimal-source regression exposed that the QB
frontend deliberately carries unused built-in opaque types, callable
declarations, and empty internal data scaffolding in every module. Lowering now
selects only types referenced by lowered functions and ignores only empty,
internal data declarations; referenced unsupported semantics and nonempty or
externally visible data still fail explicitly.

External QB runtime calls now cross the same source-to-IR path. Lowering checks
the exact far/callee-cleanup ABI record, preserves its argument permutation,
creates deterministic typed external declarations, and marks unknown runtime
effects conservatively. Primary review caught missing declaration parameter
values and orphan ABI metadata before integration. A real `screen 0`/`end`
source now emits verified `.qir`.

Module and static places now lower to typed global addresses, loads, stores,
and address values. Static objects retain their exact bytes, linkage,
mutability, source order, and otherwise-empty relocation targets. Portable IR
represents symbolic near, far, huge, code, and segment patches directly in a
relocatable byte initializer; its text format round trips patch order and its
verifier rejects missing targets, overlapping patches, generic address spaces,
and out-of-range writes. The focused vertical regression compiles the real
`readonly-data.bas` fixture and retains VBDOS's segment selector, far string
payload, and near descriptor patches in verified `.qir`. Local and parameter
storage remain explicit refusals until stack allocation is represented.

The pass pipeline also contains exact integer algebraic simplification beside
constant folding, branch simplification, and dead-code elimination. Shared
operand rewriting is exhaustive over the portable IR instead of being copied
into each pass.

Same-block common-subexpression elimination, unreachable-block elimination,
and conservative dead-store elimination are now implemented and exposed by
`llrm-opt`. Dead-store elimination keys direct global stores by pointer type,
global, addend, and stored value type; primary review rejected an earlier form
that could have treated an 8-bit overwrite as killing a 32-bit store. Loads,
unknown addresses, volatile operations, and memory-affecting calls remain
barriers, and the pass never crosses a block.

The initial integer selector now feeds `llrm-llc --emit qmir` through the
driver, preserving the rule that only the driver assembles the whole pipeline.
`llrm-qb --emit qmir` also composes QB source through the same driver path. Direct
void runtime calls materialize and push their i16/i32 ABI arguments in the
order already established by HIR lowering, retain a symbolic far external
target, and omit declaration-only functions from Machine IR. An IR
`unreachable` terminator produces a zero-successor Machine-IR block without
inventing an instruction. The real `terminal.bas` fixture now reaches verified
`.qmir` with its `screen`, `width`, `sleep`, and `end` calls.

Focused selection, Machine-IR round-trip, and adjacent Machine-to-MC byte tests
cover this path. Recent focused verification consisted of individual tests or
one real CLI invocation: static place lowering (2.38 s), relocatable global
planning (1.30 s), real QB string relocation lowering (3.17 s), unreachable
CLI wiring (0.04 s after compilation), all six DSE cases (0.04 s from the warm
build), runtime call selection (2.07 s), and real QB-to-`.qmir` emission
(1.44 s). No broad suite, qrender, or DOSBox gate ran during this work.

That regression was observed failing before the general declaration-liveness
rule was implemented, then passed with a parseable `.qir` result. All focused
verification in these implementation batches, including compile failures used
for review, remains below the ten-percent wall-clock budget. No qrender,
DOSBox, Python suite, or broad Rust suite was run.
