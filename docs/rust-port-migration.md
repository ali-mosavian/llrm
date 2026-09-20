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

Pass completion is governed by the [pass fidelity ledger](pass-port-fidelity.md).
The current Rust transforms are narrow foundations and are not yet recorded as
faithful ports of the substantially richer Python passes with similar names.

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
payload, and near descriptor patches in verified `.qir`.

Defined QB procedure calls now resolve through callable identity, normalized QB
names, exact result and ABI-ordered parameter types, and the established
far/callee-cleanup ABI. QB-style by-reference parameters require an exact
pointer-to-element type; unsupported array and segmented shapes refuse
explicitly. Local places lower to distinct stack allocations, and zero-offset
indirect parameter accesses retain their conservative volatile semantics.
Nonzero indirect byte offsets and parameter places remain explicit refusals
until pointer arithmetic and parameter-home semantics are represented. The
real `procedure.bas` fixture now reaches verified `.qir` with its call bound to
the existing function, its by-reference input, and its local function-result
slot intact.

That procedure now also reaches verified x86 Machine IR. `.qmir` version 3
preserves explicit entry blocks, initialized data objects, symbol linkage,
source ABI signatures,
incoming-argument homes, and direct references to defined functions. The x86
selector maps distinct stack allocations to distinct frame objects and keeps a
BYREF parameter as a near pointer loaded from its incoming home. Its published
pointee load remains volatile. LONG call results are defined as the two fixed
word deliveries AX and DX and merged into one semantic i32; returns perform the
inverse split and carry the exact two-byte callee cleanup. The target verifier
checks those contracts independently of the generic Machine-IR verifier.

The next target-owned slice ports the Python source backend's BASIC runtime
frame calculation without changing Machine IR. An immutable frame plan keeps
ordinary far-Pascal parameters above BP, lays the first of two LONG formals at
BP+10 and the second at BP+6, places locals and spills below the measured
QB45/PDS71/VBDOS runtime headers (10/18/20 bytes), rounds the `B$ENRA` local
reservation to a word, and carries both the `retf` cleanup byte count and the
source-derived temporary-STRING count. Unsupported conventions, far-pointer
parameters, outgoing stack objects, incomplete incoming homes, and
unrepresentable sizes fail explicitly.

The related Python regressions were ported with the mechanism: the PDS
`Twice&(n AS LONG)` parameter/local pair remains BP+6/BP-22, VBDOS's 4096-byte
local reaches BP-4116, and the historical `SUBTRACTPAIR` argument-order bug is
covered by the BP+10/BP+6 assertion. The real `procedure.bas` Rust vertical
slice independently produces BP+6 for its BYREF parameter and BP-24 for its
four-byte VBDOS local. Primary review checked the implementation against
`qbopt/frontend/qb/compile.py::_runtime_frame`, the Pascal layout in
`qbopt/frontend/qb/abi.py`, and the original regression assertions rather than
accepting an agent summary; that review corrected an agent's erroneous
BP+12/BP+8 interpretation before integration.

This slice deliberately ends at verified `.qmir`. Frame layout must still add
the runtime-specific `B$ENRA`/`B$EXSA` envelope, and allocation must model call
clobbers before this procedure can be emitted as executable OMF. Relocatable
data initializers remain explicit selection refusals rather than losing their
patches.

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

The procedure Machine-IR work used two compile checks (1.80 s and 1.71 s),
three fail-first real-fixture invocations (4.46 s, 1.65 s, and 1.96 s), and the
accepted real-fixture invocation (1.30 s). Delegated Machine-IR schema checks
consumed 6.8 s before unrelated integration errors stopped them; the focused
x86 verifier tests passed in 1.67 s. Primary review then ran the exact
procedure regression in 2.58 s and 0.04 s from the warm build, the amended
negative target-verifier regression in 1.43 s, and the qmir round-trip and
unchanged runtime-call selector checks concurrently in 0.04 s. No broad suite
was run. Updating the adjacent incoming-argument selector regression used one
0.04 s fail-first run, a 1.33 s correction run, and a 1.28 s accepted run; the
seven-test selector module then passed from the warm build in 0.04 s.

Frame-planning verification ran only the five adjacent x86 tests (5.64 s before
the final representation tightening and 3.16 s after it) plus the real
`procedure.bas` regression (0.08 s). Primary review then caught the Python
word-rounding boundary with one 1.40 s fail-first regression and one 3.31 s
accepted run. No broad suite or external runtime gate ran. The next frame work
is ABI expansion for `B$ENRA`/`B$EXSA`, exact call clobbers, and frame-index
lowering; the pure plan is not yet an emitted runtime shell.

The source pipeline now expands that plan into the measured BASIC runtime
shell. At the HIR-selected entry block it loads the word-rounded local size
into CX, loads the exact number of owned local `STRING` descriptors into BX,
and calls `B$ENRA`. Every far return is preceded by `B$EXSA`, with AX:DX kept
live across teardown for a LONG result and the independent `retf` cleanup left
unchanged. Ordinary far calls receive deterministic AX, CX, DX, BX, SI, and DI
clobber definitions before allocation; `B$EXSA` retains the Python backend's
measured AX:DX preservation. BP and EBP are unavailable to BASIC allocation,
so the current no-spill allocator refuses pressure instead of corrupting the
runtime frame chain.

The tests were ported with the behavior. The real `procedure.bas` path checks
the entry and every return in production `.qmir`, while the historical
`managed-temporaries.bas` regression proves nested expression temporaries do
not inflate BX beyond the one owned local descriptor. Focused ABI, clobber,
verifier, allocation, driver, and CLI checks consumed about 24 seconds,
including two fail-first corrections of incorrect test block assumptions and
one compile-time assertion correction. Primary review also corrected delegated
entry inference, ID reservation, malformed fixtures, and BP ownership. No
broad suite, Python suite, qrender, DOSBox, or external runtime gate ran.

The immutable BASIC frame plan now records its owning Machine function and a
target-owned frame-index finalizer turns each abstract load, store, or address
into the LLVM-style x86 operand tuple `BP, displacement`. Incoming far-Pascal
arguments remain above BP, locals include the QB45/PDS71/VBDOS runtime header
exactly once, and frame displacements remain literal rather than relocations.
The adjacent encoder chooses signed disp8 when possible, emits disp16
otherwise, and preserves x86's mandatory displacement for `[bp+0]`.

Primary review rejected the delegated suggestion to add a generic Machine IR
memory operand: x86 addressing belongs to the target opcode's explicit operand
tuple. It also rejected a speculative nonzero frame-addend extension after the
Python audit confirmed that the current selected source path has one final
frame displacement. Ported regressions cover the `SUBTRACTPAIR` `BP+10/BP+6`
ordering bug, the 4096-byte VBDOS local at `BP-4116`, the ordinary incoming
slot at `BP+6`, VBDOS's four-byte local at `BP-24`, single application of the
displacement, literal Machine-to-MC bytes, and explicit malformed/repeated
refusals. Focused compilation and test execution consumed under eight seconds;
no broad or external gate ran.

The driver now composes deterministic x86 allocation, immutable assignment
application, and BASIC frame-index materialization for the real
`procedure.bas` source path. The resulting functions contain no virtual
registers or abstract frame operands, retain `BP+6` and `BP-24`, and pass the
x86 verifier in their allocated form. The verifier accepts the same semantic
contracts on constrained virtual operands before allocation and on exact
physical operands afterward; it does not weaken call ordering, alias checks,
register widths, or BP-address legality.

The production regression was first run against target verification and
failed at the first adjacent boundary: physical address registers, ABI
operands, and word pseudos were still being judged by pre-allocation-only
rules. General phase-independent register checks fixed that boundary. One
fail-first run, two accepted runs, and two focused inherited verifier checks
consumed about eight seconds. No broad or external gate ran.

Machine IR now records its entry block explicitly. This removes a layout
assumption from module-level MC lowering and lets a function symbol bind to the
semantic entry even when block order changes. Portable IR still defines its
first block as entry; x86 selection captures that normalized boundary once,
then Machine IR preserves the ID independently. The generic verifier rejects
empty definitions and unknown entry IDs. `.qmir` version 3 writes the entry ID
in every function header and rejects version 2 rather than guessing it.

Primary review removed the delegated compatibility argument that duplicated
`MachineFunction.entry` in BASIC ABI expansion, leaving one source of truth for
`B$ENRA` placement. The non-first-entry text round trip and unknown-entry
verifier checks consumed 10.5 seconds in delegated verification. Primary
integration compiled the changed targets in 15.6 seconds, then ran the exact
non-first-entry BASIC ABI and selection regressions in 0.13 seconds. The
initial compile command selected zero tests because its exact name was stale;
it is not claimed as behavioral evidence. No broad suite or external gate ran.

Allocated x86 Machine modules now lower through a deterministic module-wide MC
boundary. The target creates stable text, read-only-data, and data sections;
defines data, function, and block symbols; anchors each function at its
explicit Machine entry; preserves symbolic addends; and declares external
symbols in first-use order. Every block receives a zero-byte anchor so empty
and non-first entry blocks remain definable. Residual virtual registers, frame
indices, allocation metadata, unknown references, duplicate defined names,
and malformed roles fail with function/block/instruction/operand context.

The driver now composes selection, allocation, frame-index materialization,
and this symbolic MC lowering for the real `procedure.bas` fixture. This is not
yet executable object emission: long pseudos, calls and returns, target fixups,
branch relaxation, and the BASIC OMF envelope remain later explicit stages.
Agent source checks consumed under 0.1 seconds. Primary registration exposed
and corrected three issues before acceptance: one missing test lifetime, two
borrowed-pattern errors, and a duplicate invalid-role error path. The two
compile-fail review runs, the ten-test module run, the three inherited
instruction-lowering tests, and the production driver witness consumed about
14.8 seconds total. No broad suite or external gate ran.

That regression was observed failing before the general declaration-liveness
rule was implemented, then passed with a parseable `.qir` result. All focused
verification in these implementation batches, including compile failures used
for review, remains below the ten-percent wall-clock budget. No qrender,
DOSBox, Python suite, or broad Rust suite was run.

The allocated x86 boundary now consumes the Python backend's established LONG
ABI forms before MC lowering. `MergeWords` becomes `push high; push low; pop
dword`; low-word extraction uses the physical word view directly; high-word
extraction uses the flag-preserving `push dword; pop word; pop word` spelling.
Far-call register operands remain visible through allocation, then disappear
as allocator metadata. Far returns similarly retain only their checked callee
cleanup immediate after the AX-low/DX-high contract has been validated.

This is a target-owned calling-convention finalizer, not a new optimizer pass
or representation feature. No QB runtime name, LONG marker, or frontend fact
was added to portable IR, generic Machine IR, MC, or their passes. The driver
alone selects the BASIC ABI hook. The real `procedure.bas` regression was
observed failing while its word pseudos still reached MC, then passing after
the finalizer was placed between allocation and MC lowering.

The x86 encoder now emits a direct far-call skeleton only through a
relocation-aware API: `9a 00 00 00 00` plus one typed 16:16 fixup at byte one.
The byte-only compatibility API refuses instead of dropping that fixup.
`retf` now emits `cb` for no cleanup and `ca imm16` for callee cleanup. Ported
tests retain the exact DX:AX merge bytes, flag-preserving high-word bytes,
cleanup bytes, symbolic addend, malformed boundary refusals, and immutable
input behavior. Delegated focused checks and primary review checks consumed
about eleven seconds in total; no broad suite, Python suite, qrender, DOSBox,
or object-link gate ran.

Address-independent physical x86 instructions can now cross the next target
boundary as same-ID MC data fragments. Existing data, alignment, zero-fill,
symbol definitions, and order remain unchanged; encoder-produced fixups move
with their bytes. A preexisting instruction fixup is refused rather than
merged speculatively, and address-dependent branches remain explicit until a
layout/relaxation stage can choose their form.

The adjacent real-procedure probe identified two general missing x86 forms in
order: a relocatable near-address `lea` and sixteen-bit register-indirect
loads/stores. Both now have exact encodings and focused tests; the former uses
a typed absolute-16 fixup and neither contains QB-specific policy. The next
adjacent refusal is the procedure's first unconditional branch, confirming
that layout and branch relaxation—not an object or frontend special case—is
the next boundary. Four focused MC-encoding tests and the two fail-first
adjacent probes consumed about ten seconds. No broad or external gate ran.

An architecture audit removed source-domain calling-convention labels from
portable IR and Machine IR before more backend work could depend on them. The
former `basic` and `runtime` variants described the same implemented ABI and
are now the single language-neutral `far_pascal` convention: far calls,
left-to-right arguments, and callee cleanup. Linkage continues to distinguish
defined procedures from external declarations. `.qir` is now version 2 and
`.qmir` version 4; both reject the old source-language spellings.

The focused schema run exposed that the `.qmir` writer used its version
constant while the parser still hard-coded version 3. That regression failed
before the parser was made to consume the same constant. Three Machine-IR text
tests, the two new IR ABI-schema tests, HIR runtime-call lowering, and x86
far-call selection then passed in about 5.1 seconds. The deliberately broader
text filter also exposed an unrelated pre-existing invalid aggregate fixture;
it was not pursued in this slice. No broad suite or external gate ran.

Unconditional local x86 branches now pass through a target-owned immutable
layout and relaxation boundary. It ports the Python writer's short-first,
grow-only fixed point: `eb rel8` at the inclusive signed-byte endpoints and
`e9 rel16` only after a complete-layout range check. Displacements are derived
from the final instruction address, local branches carry no relocation, and
undefined, cross-section, addended, malformed, or out-of-range targets fail
explicitly. Direct fallthrough uses fragment occurrence identity; self loops,
intervening bytes, and alignment padding are retained.

Primary review rejected the delegated draft's end-of-instruction “self” test,
incorrect backward-near displacement, unsafe alignment-sensitive fallthrough
test, and Machine/MC opcode-ID mismatch before acceptance. The ported tests
cover the historical growth-before-backward-target regression, exact range
boundaries, `eb fe` self loops, chained fallthrough, alignment, immutable
input, other-instruction fixups, malformed inputs, and signed-16 refusal.
One compile-fail review run and the accepted twelve-test target module run
consumed about 13.5 seconds. The driver now exposes the same target stage
without adding object policy; the real `procedure.bas` path reached verified,
all-data MC with retained far-call fixups in another 4.6 seconds. No broad
suite or external gate ran. The final-address endpoint regression was then
mutated back to the wrong short-form base, observed failing on the emitted near
bytes, and restored to green in 2.7 seconds.

Fresh OMF emission now has an explicit three-layer boundary. Generic MC still
contains only opaque target fixup identities. The x86 adapter maps its
`far pointer 16:16` and `absolute offset 16` fields to target-neutral OMF
relocations and materializes expression addends exactly once. The OMF writer
then owns names, one-based indices, SEGDEF/EXTDEF/PUBDEF declarations,
LEDATA/FIXUPP construction, checksums, and MODEND without importing MC or x86.
It emits explicit target-frame fixups and never depends on inherited THREAD
state. BASIC module headers, runtime entry conventions, and DGROUP policy are
not inferred by any of these layers and remain frontend/driver adapter work.

The Python writer's relocation-boundary regression was ported with its
1,000-byte LEDATA policy: a chunk boundary moves before a field rather than
splitting it. Related tests cover semantic decode of every constructed record
family, explicit non-THREAD fixups, deterministic output, overlapping and
uninitialized relocation refusals, external far pointers, defined-symbol
addends, zero-fill gaps, alignment fill, public/local symbol policy, and
unsupported fixup/linkage cases. The real `procedure.bas` path now proceeds
from encoded MC to a fresh OMF record stream whose relocations and runtime
externals decode through the independent Rust reader. This is a structurally
valid generic object, not yet the QB-specific module envelope.

The first writer run failed because the mandatory empty LNAMES entry consumes
index one while the new class name had initially also been assigned index one.
The general one-based table rule was corrected before acceptance. Two focused
implementation-agent turns were stopped when they did not produce files; the
primary implemented and reviewed the frozen interface directly. A separate
Terra audit confirmed that analyses, transforms, and MC contain no
source-language branching and that the x86-adapter/object-writer split keeps
the boundary intact. It also identified pre-existing target-width inference
in portable IR relocation verification as the next architecture debt to
remove rather than propagate. The failed and accepted writer checks, adapter
checks, and real-procedure vertical check consumed about 15 seconds total. No
broad suite, Python suite, qrender, DOSBox, or linker gate ran.

Portable IR relocation bounds no longer infer an emitted x86/OMF field size
from address-space names. `GlobalRelocation` now records an explicit,
target-neutral patch width; the verifier checks only target existence,
nonzero width, initializer bounds, and overlap. The HIR adapter supplies the
already-established 16-bit ABI representation and explicitly refuses a
relocation with no concrete address representation. Target lowering remains
responsible for choosing an instruction/object relocation kind.

This changes `.qir` to version 3: each `relocbytes` entry includes its width,
and version 2 is rejected instead of being guessed. The new generic-address
regression was observed failing when the former address-space rejection was
temporarily restored, then passed with explicit-width verification. Focused
IR verifier, `.qir` round-trip, and HIR relocation tests plus one all-target
compile check consumed about nine seconds. The compile check reported one
pre-existing test-only `llrm-opt` helper warning; no broad test suite or
external runtime gate ran.

The test-only canonicalization helper is now compiled only for tests, so the
all-target check is warning-free; the confirming check took 0.07 seconds.

The fresh OMF model now represents GRPDEF membership, grouped PUBDEF bases,
and explicit segment, group, external, or target-derived FIXUPP frames.  These
are target-neutral object facts: neither the model nor the writer assigns
DGROUP or any source-language segment policy.  The x86 adapter supplies empty
groups, ungrouped publics, and target-derived frames until a frontend-owned
object adapter states otherwise.  Decode-backed writer tests cover record
ordering, all supported frame methods, namespace validation, and incoherent
public bases.  The delegated focused run took 5.27 seconds including
compilation; primary review repeated the seven writer tests from cache in
0.04 seconds.  No broader gate ran.

## Iteration 15: WCC capture frontend (started)

The Rust C path now owns the lexical `.cgs` capture-stream parser.  It ports
the existing Python protocol rather than adding a C parser: result handles,
no-result markers, calls, positional arguments, named fields, quoted values,
and the shim's lowercase `\\xNN` escaping retain their established meaning.
Diagnostics report typed malformed-stream reasons with source line and column,
and record fields use deterministic ordering.

Primary review rejected two stricter behaviors that were not faithful to the
Python frontend: unknown call spellings and unrecognized escapes remain data
instead of becoming new refusals.  Review also caught a delegated regression
that incorrectly treated `f1` as a positional argument rather than the call's
result handle.  Seven focused Rust tests now cover the protocol and parse the
real 127-record `choose.cgs` fixture.  Agent and primary compile/fail/fix runs
consumed about 11.4 seconds total; the accepted run took 1.33 seconds.  No
broad suite or backend gate ran.

## Iteration 11: QB object envelope (started)

The QB frontend now owns the measured 48-byte `MODULE_CODE` header builder.
It ports BC's exact object-name spelling, fixed fields, and runtime-consumed
`U_FLAG` values for QB 4.5, PDS 7.1, VBDOS, row-major arrays, and PDS alternate
math.  Unsupported profile combinations and non-ASCII module names are
explicit refusals.  This code remains in `frontend::qb`; it is not yet attached
to generic MC or OMF output, and no C path can invoke it.  Primary review
removed four out-of-scope files changed by recursive formatting before
acceptance.  The delegated five-test run took 1.17 seconds and the independent
review run took 1.50 seconds.  No broader gate ran.
