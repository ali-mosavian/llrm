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
semantic contract for the port.  Superseded: the port translates the Python
MIR data structures and algorithms 1:1; see the port rule in `agents.md`.

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

That header can now be prepended immutably to an explicitly selected generic
MC text section.  The QB-owned adapter validates the input, allocates stable
fragment and symbol IDs, and lets ordinary MC layout move existing symbols and
relocation sites by exactly 0x30; generic MC and generic OMF remain unaware of
BASIC.  This is deliberately only the runtime prefix: the six header
relocations and the remaining BASIC object envelope are still explicit work,
not silently guessed by the generic object writer.  Three focused tests cover
layout, generic-versus-QB OMF offsets, immutability, and refusal paths; the
delegated run took 1.67 seconds and the primary review run took 1.46 seconds.
No broader gate ran.

The next QB-owned adapter now surrounds that generic code object with the
measured Microsoft BASIC envelope for the scalar vertical slice: `BC_CODE`,
the ordered runtime support segments, exact class/combine/alignment policy,
`DGROUP`, the six `MODULE_CODE` offset relocations, and the `BC_SA` far
registration pointer.  Statement-table and data-segment targets remain
explicit inputs; generic data sections are refused until QB placement is
ported rather than guessed.  Independent decode of the constructed OMF checks
all seven relocation sites and frames.  Its three focused tests passed in 6.7
seconds; the cached post-review run took 0.04 seconds.  No linker, runtime,
broad, or optimization gate ran.

The WCC frontend now also owns a typed, source-ordered capture unit above the
lexical parser.  Distinct IDs preserve symbols, segments, backs, nodes, calls,
temporaries, and source files; raw WCC names, attributes, calling conventions,
segments, data directives, and inline-code fixups remain inside
`frontend::wcc`.  `SETSEG 0`, interleaved procedure/data state, anonymous
backs, reverse capture argument order, intrinsic object names, and alias-cycle
termination follow the Python frontend.  One corpus test builds all 28
committed `.cgs` captures without fixture-specific production logic.  The
delegated implementation was stopped after it remained a fail-first stub; the
primary implemented and reviewed the audited contract.  Focused compile/fix
and accepted runs consumed about 10.7 seconds; no shared-HIR lowering or broad
gate ran.

The first C semantic slice exposed a shared ABI distinction that could not be
faithfully hidden in the frontend.  HIR now maps generic near/caller,
far/caller, and far/callee procedure and call shapes to IR `c`, `far_cdecl`,
and `far_pascal` conventions respectively; near/callee remains an explicit
refusal.  Direct calls compare the site's ABI with the defined target instead
of assuming Pascal, while source capture attributes and WCC spellings remain
inside `frontend::wcc`.  The public `.qir` schema is version 4.  x86 selection
explicitly refuses `far_cdecl` until the C ABI backend is implemented, so this
representation work cannot silently route C through BASIC code generation.
Thirty focused ABI tests passed in 1.95 seconds.  Primary review separated a
same-name runtime calling-convention conflict from a type-signature conflict
and added its regression; no optimization or broad gate ran.

The real `iparg.cgs` capture now raises into generic HIR and then portable IR:
an internal near-cdecl `twice`, an exported far-cdecl
`answer_from_argument`, one direct call, and the source-order scalar argument.
WCC handles, node spellings, raw attributes, and calling-class bits terminate
inside `frontend::wcc`; scalar temporaries become value bindings rather than
invented stack places.  Unsupported nodes remain source-cue refusals.  Primary
review replaced a fixture-coincidental raw linkage bit with the capture
model's export rule and restricted WCC's scalar `O_POINTS` convention to the
established name, temporary, and call-result shapes.  Both review issues have
regressions.  Delegated focused runs took 5.45 and 4.25 seconds; the reviewed
four-test run, including HIR verification and HIR-to-IR lowering, took 2.59
seconds.  No x86 C ABI, optimizer, broad, or external gate ran.

The generic backend now carries near caller-cleanup and far caller-cleanup as
distinct Machine IR conventions.  Target selection preserves source operand
order in IR, pushes C arguments right-to-left, constrains 16-bit results to
AX, emits near or far calls and returns from the convention, and performs the
exact caller stack adjustment.  Existing far-Pascal selection is unchanged;
WCC names and attributes do not cross the frontend boundary.  The public
`.qmir` schema is version 5.  The delegated focused selection run took 3.90
seconds; primary review inspected the complete three-file diff, corrected the
remaining QB CLI schema expectation, and repeated the selection in 1.68
seconds.

`llrm-c` now drives a real `.cgs` capture through parsing, typed capture
construction, source-neutral HIR verification, portable IR, and x86 Machine
IR.  It emits deterministic `.qir` or `.qmir` text and explicitly refuses
unsupported output kinds.  The real `iparg.cgs` regression verifies both text
round trips and the mixed far-cdecl/near-cdecl call path.  Focused CLI checks
took 6.21 seconds for the IR slice and 0.55 seconds for the added Machine IR
slice.  No allocation, MC, OMF, linker, runtime, broad, or optimization gate
ran.

## End-to-end frontend milestone

No additional optimization pass will be ported until both `llrm-qb` and
`llrm-c` compile real programs through the Rust frontend and shared
optimization-disabled backend, emit OMF, link, and run successfully.  The
gate expands by semantic family across the ten checked-in BASIC/C pairs under
`bench/parity`: `algebra`, `branch`, `control`, `loop`, `memory`, `parity`,
`qbsp`, `qlight`, `qmove`, and `scalar`.  It then covers
`bench/nbody.bas` against `bench/c/nbody.c`.  The C-only `sieve`, `crc`,
`floats`, `lru`, `mandel`, `matmul`, and `shellsort` programs extend C
coverage without being mislabeled as language pairs.  Each rung uses its
established expected result and ports the corresponding focused and regression
tests with the implementation.  Failures are localized at adjacent stage
boundaries; this gate does not authorize QB or WCC policy in shared HIR, MIR,
LIR, MC, x86, or generic OMF code.

The first unoptimized C object slice is now executable.  The real
`fixtures/c/iparg.cgs` capture passes through WCC capture construction, HIR,
portable IR, x86 Machine IR, allocation, conventional near/far cdecl frame
finalization, MC, encoding, and fresh OMF emission from `llrm-c --emit obj`.
The linker-visible names remain WCC's `_twice` and
`_answer_from_argument`.  A checked-in assembly caller links the Rust object
with Microsoft LINK 5.31 and the resulting DOS program writes the independent
answer 42 under DOSBox-X.

This slice follows the existing Python convention rules rather than defining
a new ABI: C arguments are pushed right-to-left, cleanup is caller-owned,
near/far distance selects the call and return encoding, scalar results use AX,
and allocation-only call/return operands are removed at the same final target
boundary where Python's emitter ignores them.  Primary review caught and
removed an unnecessary virtual register for SP; the caller cleanup now names
physical SP directly.  It also restored Python's frameless behavior when a C
function has no frame objects.  The focused Rust MC, C ABI, driver, and OMF
checks consumed about 16 seconds in this slice.  The single link-and-run gate
took 6.0 seconds, including 1.52 seconds in DOSBox-X.  No broad suite or
optimizer test ran.

`iparg` proves the production path but is not counted as one of the paired
acceptance programs.  A fresh WCC capture of `bench/parity/scalar.c` now gives
the next implementation boundary: the Rust frontend explicitly refuses its
first 32-bit integer type.  The next work therefore ports that existing Python
semantic family and its regression before attempting the remaining parity
pairs, `nbody`, or the C-only `sieve` rung.

That boundary is now closed for the scalar C rung.  The WCC raiser applies the
same signed constant conversions as Python, x86 selection emits the established
word-to-long extension, and the allocated C ABI returns a long in `DX:AX` using
the same terminal high-word extraction as Python's post-allocation rewrite.
Primary review rejected an initial delegated implementation that retained
allocation-only return operands on the physical return.  The corrected path
compiles the real `fixtures/c/parity/scalar.cgs`, links it through Microsoft
LINK, runs under DOSBox-X, and writes the independent paired answer `1789`.
The checked-in runtime gate took 2.15 seconds; no broad suite or optimizer gate
ran.

The first QB object/runtime slice now follows the Python emission path rather
than reconstructing a new one.  The frontend validates and consumes its exact
14-byte `(function, block, instruction, line)` metadata rows before generic IR
lowering.  The final QB adapter emits `$QB$STAT` after all real procedures as
the measured `55 8b ec` private frame prefix, relocated `(offset, line)` rows,
and zero terminator.  `MODULE_CODE.OF_STA` resolves to the table data after the
prefix.  Generic IR, Machine IR, MC, x86, and OMF contain no BASIC statement
meaning.

For the initial data-free object slice, empty generic C data sections are
removed explicitly because their mere presence changes BASIC's DGROUP and heap
boundary; any nonempty source data remains an explicit refusal.  The existing
`frontends/qb/fixtures/emission.bas` regression now passes through `llrm-qb
--emit obj`, the exact QB45 header/support-segment envelope, JWASM, Microsoft
LINK with `BCOM45.LIB`, and DOSBox-X.  Its far-Pascal `ADDONE(41)` call returns
the long value `42` in `DX:AX`.  The independent manual link/run took 1.90
seconds and the checked-in focused runtime gate took 2.13 seconds.

The scalar FOR boundary also ports Python's complete ten-entry integer branch
table, preserving `cmp left, right` and mapping signed and unsigned predicates
to the corresponding x86 conditions.  Primary review rejected the delegated
three-predicate subset as unfaithful and required equality plus every ordered
integer predicate already represented by Python.  The real function body from
`bench/parity/scalar.bas`, without its not-yet-ported string-printing module
statements, now reaches OMF; the full source still stops at relocatable QB data.
Metadata, statement-table, comparison, object-envelope, and runtime checks in
this slice consumed under 40 seconds in total, below ten percent of elapsed
implementation time.  No broad suite, qrender, or optimization pass ran.

The initialized-data boundary now retains the Python frontend's complete
symbolic facts instead of recovering them from names at object emission.
Portable IR globals carry their own storage address space in `.qir` version 5;
Machine IR uses typed data-object IDs and carries ordered `(offset, target,
addend, width, address-space)` relocations in `.qmir` version 6.  X86 selection
preserves those fields exactly.  MC maps near offsets, selector words, and far
16:16 pointers to distinct target fixup kinds, and OMF emits selector words as
zero-initialized, target-framed `BASE` relocations.  Unsupported width/space
pairs and nonzero selector addends are explicit refusals.  Primary review
inspected both delegated diffs, added the selector-addend regression, and
independently reproduced the selection, text round-trip, verifier, and OMF
boundaries.  Delegated and primary focused verification consumed under 35
seconds; no broad suite ran.

The next adjacent scalar boundary was the existing BASIC runtime's near
string-descriptor argument, not a new calling convention.  Far-Pascal now
accepts that standard 16-bit near pointer, materializes its address with
`lea`, pushes it in source order, and leaves stack cleanup to the callee.  Its
focused `B$PSSD` regression passed in 6.06 seconds.  The complete existing
`bench/parity/scalar.bas`, including both print statements and the function
body, now reaches verified Machine IR.

The next slice ported Python `_data()` and the relevant `_object_data()` and
BASIC-envelope behavior rather than inventing a new layout.  The QB MC adapter
preserves source order while grouping initialized objects into `BC_DATA`,
`BC_CN`, and VBDOS `FSL_CONST`, including the exact six-byte `BC_DATA` prefix.
The object adapter retains those bytes and relocations while assigning the
measured BASIC segment order, classes, combines, alignments, DGROUP frames, and
private far-data segments.  The later array-descriptor far-pointer-to-DGROUP
form remains an explicit refusal; it is not approximated by an ordinary far
pointer.

`llrm-qb` now compiles `bench/parity/scalar.bas` through Rust to OMF.  VBDOS
LINK accepts that object, and the linked program completes under DOSBox with
the checked-in `RESULT= 1789` and `DONE` output.  Primary review inspected the
complete delegated diffs, corrected the generic `.text` interface mismatch,
ran the two focused data/object tests in 5.52 seconds of parallel wall time,
and reproduced object emission in 4.31 seconds.  The delegated end-to-end gate
took 12.80 seconds.  An independent Terra source-and-object audit found no
scalar data-layout discrepancy and identified the intentionally refused array
descriptor boundary above.  A commit hook also spent 17.4 seconds exposing
pre-existing repository-wide Python type and compatibility failures; it was
not rerun.  Total verification remained below ten percent of elapsed work, and
no qrender or optimization suite ran.

The register-allocation port now has the Python allocator's segmented weighted
live intervals and its target-neutral greedy selection core.  The selector
reserves fixed ranges first, assigns the largest ranges first, uses costed
eviction and cascade ordering, preserves short reloads and protected ranges,
and returns a complete spill batch for the later rewrite loop.  Primary review
rejected a new restriction which required a pin to occur in the ordinary class
order: Python instead treats a pin as its complete singleton order.  The
corrected six-test selector check took 0.03 seconds from cache.  Copy hints,
register masks, sibling/fold pricing, splitting, rewriting, and the twelve-round
outer coordinator remain explicit port work; this partial selector is not yet
wired into either production driver.

The next C slice ports Python's size-only treatment of WCC local aggregates.
The real `fixtures/c/cells.cgs` capture retains `TYPE T25 size=8` without
inventing its discarded alignment, carries WCC's explicit `index * 2` byte
offset into generic pointer arithmetic, and uses typed indirect `i16` loads and
stores.  HIR lowering keeps the operation source-neutral, and x86 selection
adds an already-scaled 16-bit byte offset to a near address without learning C
array semantics.  Primary review caught two silent semantic changes before
acceptance: arbitrary scalar `O_POINTS` had become an identity operation, and
big-data `TY_POINTER` would have been narrowed from Python's far 32-bit pointer
to a near 16-bit pointer.  The former existing regression was observed failing
before the refusal was restored; a new fail-first regression now makes the
latter an explicit unsupported case until far pointers are ported.

The focused WCC raise module passed eight tests in 2.13 seconds, the three x86
pointer-offset selection tests passed in 0.04 seconds, and the checked-in C
capture was independently regenerated byte-for-byte in 0.3 seconds.  The real
capture then reached `.qmir` in 6.06 seconds and fresh OMF in 0.15 seconds.
The first runtime invocation omitted the repository's `--full` selector and
deselected the one requested test in 0.64 seconds; it was not mistaken for a
pass.  The corrected focused gate took 2.08 seconds: Microsoft LINK accepted
the fresh object, and DOSBox returned the caller's independent value `1234`
after `fill_cells` stored and reloaded it through its local `short cells[4]`.
No broad suite, optimization pipeline, qrender, or unrelated runtime gate ran.

The aggregate C rung now ports Python `_Raise.binary` and `_Raise.offset`
rather than recognizing only the first observed array shape.  Near-pointer
expressions evaluate left then right, commute only addition with the address
on the right, subtract only from an address on the left, materialize an
aggregate address once, and permit an existing near-pointer value to feed the
next byte offset.  WCC's `Pair points[8]` remains the capture's size-only
32-byte opaque local; no structure layout or alignment fact enters generic
HIR, IR, Machine IR, or x86 code.

The checked-in `parity.cgs` was independently regenerated byte-for-byte from
`bench/parity/parity.c`.  Its focused real-capture and commuted/subtracted
pointer regressions passed in 3.11 seconds of primary execution.  The ported
Python runtime oracle then compiled the real capture through `llrm-c`, emitted
fresh OMF, linked with Microsoft LINK, ran under DOSBox-X, and returned the
established result `1789`; that one focused run took 19.01 seconds including
build and harness startup.  This closes the unoptimized C aggregate rung in
commit `eb680f1e`; it does not claim optimizer parity.

Generic HIR lowering now preserves `Indirect` byte displacements by inserting
a typed IR `getelementptr` immediately before the original load or store.  The
access retains its original instruction ID, access type, volatility, and
readonly checks; a zero displacement remains structurally unchanged.  New IDs
are deterministic and begin after both source HIR IDs and planned stack
allocation IDs.  This is the portable equivalent of Python `hir/lower.py`'s
address-component handling, not a QB-specific lowering rule.

Offset type selection also follows Python's segmented-pointer distinction:
near and far pointers advance a 16-bit offset component, huge pointers use
their full pointer width, and code, segment, or unclassified address spaces
are explicitly refused.  The three focused indirect-offset tests passed in
0.04 seconds under primary review.  With commit `ad07baee`, the real BASIC
`parity.bas` path no longer stops at descriptor offsets 2 and 10; its next
adjacent failure is now function 2, block 5, instruction 22, the existing far
pointer `Concat` operation.  Locating that boundary took 1.51 seconds.

A read-only ABI comparison confirmed that Rust already carries the required
near/far cdecl and far-Pascal concepts.  No new calling-convention design is
needed.  The remaining Python parity gaps are concrete implementation work:
DX:AX results from defined cdecl calls, multiword and pointer arguments,
external far-cdecl calls, float arguments/results, and C's preservation of SI
and DI.  The current no-argument `parity_kernel` exercises none of those gaps.

The first aggregate commit attempt unexpectedly invoked the repository-wide
Python type checker and host suite.  They exposed the existing type errors and
eight compatibility failures caused in part by stale `frontends/qb/Cargo.lock`
references; the hook spent about 17 seconds and was not rerun.  The reviewed
commit was made with only those two unrelated broad hooks skipped.  A separate
local 1Password signing failure consumed waiting time but no verification or
code changes.  Focused verification remained below ten percent of this
iteration's elapsed implementation and review time; no qrender or optimizer
gate ran.

The BASIC aggregate exit now follows Python's established construction rather
than allocating around a Rust-only lifetime.  Python's pre-optimization
`PARITYKERNEL` MIR stores the completed LONG result, calls `B$ERAS`, reloads
the result, exposes its low and high words, calls `B$EXSA`, and returns through
`DX:AX`.  Rust had loaded the result before appending local cleanup, keeping a
dword live across `B$ERAS`.  Commit `992eca3f` leaves scalar exits cleanup-only
until `B$STDL` and `B$ERAS` have run, then reloads the result place.  STRING
results remain a distinct measured ABI: `B$SCPF` first copies the owned result
descriptor to the runtime temporary chain, and the returned descriptor address
survives cleanup.  Focused typed-HIR regressions cover both orders.

Primary review rejected an alternative `B$ERAS` name exception in the x86
call-clobber materializer.  Its SI/DI preservation is a true measured runtime
fact, but using it to retain the premature Rust result value would have hidden
the frontend mismatch.  Precise per-callee clobbers remain future contract-data
work; generic allocation still uses its conservative far-call default, and no
QB runtime name was added to generic Machine IR or allocation.

The corrected exit exposed a pre-existing x86 verifier failure after successful
allocation.  Frame layout correctly rewrote `Store(frame, source)` as
`Store(BP, displacement, source)`, but the verifier classified every
three-operand store ending in a register as segmented memory.  Commit
`902fc62d` distinguishes the materialized-frame and segmented forms by the
second operand and reports the actual source position.  The existing legal
frame-store test was observed failing first, then passed together with a new
diagnostic regression and the segmented-memory verifier test.

With commit `234f7153`, the real `bench/parity/parity.bas` now compiles through
`llrm-qb` to fresh OMF, links with Microsoft LINK and the VBDOS runtime, runs
under DOSBox-X, and matches the checked-in `RESULT= 1789` and `DONE` oracle.
This joins the already-complete `llrm-c` aggregate parity rung without invoking
an optimization pass.  Delegated focused checks for the semantic fix consumed
8.15 seconds; primary focused Rust tests consumed 4.95 seconds; the three
adjacent-stage object/QMIR probes consumed 4.32 seconds; and the single new
external runtime gate took 13.11 seconds wall time.  Verification stayed below
ten percent of the iteration's elapsed work, and no broad suite or qrender run
was used.

The paired algebra rung now compiles through both Rust drivers without an
optimization pass.  `llrm-qb` and `llrm-c` independently emit fresh OMF, link
with the existing Microsoft toolchains, run under DOSBox-X, and return the
established Python-era result `702774`.  The port keeps Python's actual ABI
construction: 32-bit integer results are born in `DX:AX`, C calls clobber
`AX`, `BX`, `CX`, and `DX`, C callees preserve the low `SI` and `DI` words,
and spill rewriting replans each frontend's existing frame rather than
inventing a calling convention.  Commits `351e2540`, `64bfe912`, `1562d4c7`,
and `4aa503ce` separate those target-independent allocation and x86 ABI
responsibilities; commit `aade437` integrates the reviewed vertical slice.

The first C executable linked but overwrote its own final `retf`: live DOSBox
inspection found `DS:0006` aliasing the code byte at linear `08366h`.  The
Rust object had target-framed member offsets `6` and `8`, while Python's OMF
writer and the medium memory model require DGROUP offsets `0416h` and `0418h`.
Primary review rejected both a WCC class-name rewrite and a delegated split of
near data into extra MC sections: the former lost near-external provenance,
and the latter broke QB's existing data placer and far-data path.  Commit
`859c5aa1` instead adds the target-owned `NearData16` relocation fact and an
opt-in x86 DGROUP OMF policy.  Generic and QB OMF lowering remain group-free;
only the C medium-model driver selects DGROUP.

Delegated target verification took about 10.2 seconds.  Primary verification
for this continuation used 7.25 seconds for the discarded envelope unit,
13.39 seconds for one accidentally deselected invocation, 8.02 seconds for
the interim C runtime gate, 6.94 seconds for the accepted DGROUP units, 22.14
seconds for the final QB runtime gate, and 2.49 seconds for the final C runtime
gate.  A commit hook unexpectedly spent another 19.05 seconds reproducing the
known unrelated Python type errors and stale `frontends/qb/Cargo.lock`
compatibility failures; those two broad hooks were not rerun.  The roughly
89.5 seconds of verification stayed below ten percent of 1,267 seconds of
elapsed implementation and review.  No qrender or optimizer gate ran.

A read-only paired-program audit selected `branch` as the next rung.  Its
checked-in BASIC and C sources share the existing `-87904` oracle and add one
two-arm signed conditional while reusing algebra's calls, globals, and return
ABI.  The next work is to capture its real WCC stream, add the independent C
caller, and run the two existing Rust runtime helpers before considering
sieve, nbody, or any optimization pass.

Commit `d994ecf9` completes that branch rung without changing compiler code.
The checked-in `branch.cgs` was generated twice with the production WCC medium
model, 386, floating-point, packing, include, and capture flags; the two 3,330
byte streams matched exactly before one was retained.  `llrm-c` compiled the
real signed-short comparison and explicit join, and `llrm-qb` compiled the
paired `IF/ELSE`; both fresh objects linked and returned the established
`-87904` result in DOS.  The delegated focused runs took 2.84 and 1.99 seconds,
and primary review reproduced both together in 4.05 seconds.  The 8.88 seconds
of verification remained below ten percent of the iteration's elapsed work;
no broad suite, optimizer, or qrender gate ran.

Commit `89aa900e` adds the next paired rung, `memory`, again without compiler
changes.  Its 3,755-byte WCC stream was independently captured twice with the
production command and compared byte-for-byte.  The C path stores an `i16`
through an incoming near pointer, reloads it twice, sign-extends both values,
and returns their LONG square; the BASIC path performs the corresponding
by-reference `INTEGER` update.  Both fresh objects linked and returned the
existing `361001` oracle.  Delegated runtime gates took 1.61 and 1.84 seconds;
primary review reproduced both together in 3.79 seconds.  The 7.24 seconds of
verification remained below ten percent of elapsed work, with no broad suite,
optimizer, or qrender gate.

Commit `e409e63b` adds the paired `loop` rung without compiler changes.  Its
4,386-byte production WCC stream was captured twice and matched byte-for-byte.
The C path preserves a real backedge, local `INTEGER` induction value, signed
exit comparison, loop-carried LONG total, and pointer loads; the BASIC path
preserves the corresponding `DO WHILE`.  Both fresh objects linked and returned
the existing `130991` oracle.  Delegated runtime gates took 1.77 and 1.96
seconds, and primary review reproduced both together in 3.85 seconds.  The
7.58 seconds of verification stayed below ten percent of elapsed work; no
broad suite, optimizer, or qrender gate ran.

Commit `6e2bbcd8` adds the paired `control` rung and closes two reduced WCC
raiser tables against Python `cfront/raise_hir.py`.  The real capture first
failed at `O_EQ`, then advanced to `O_AND`; Rust had retained only `O_LT` from
Python's six comparisons and only four of Python's ten integer binary forms.
The port now preserves every comparison and integer binary spelling, including
remainder, bitwise operations, shifts, and signedness-selected right shift, as
existing source-neutral HIR operations.  No C operation or type fact entered
generic HIR, IR, Machine IR, MC, or a pass.

The 4,923-byte production capture was regenerated twice and matched both copies
and the checked-in fixture byte-for-byte.  `llrm-c` and `llrm-qb` then emitted
fresh objects, linked, ran under DOSBox-X, and returned the established `15007`
oracle.  The two fail-first adjacent-stage runs, delegated table checks,
primary focused unit checks, final paired runtime gate, and capture comparison
used about 45 seconds of command execution, below ten percent of the elapsed
implementation and review interval.  No broad suite, optimizer, or qrender
gate ran.

Commits `864f9862`, `57b5b429`, and `4173b0ec` complete the paired `qlight`
rung.  The target port preserves Python's ordinary integer mechanisms rather
than adding a program-shaped route: signed and unsigned quotient/remainder at
word and dword widths, explicit `DX:AX`/`EDX:EAX` Machine IR effects, all
dword comparisons, word/dword casts, and word results at both far-Pascal and
far-cdecl call boundaries.  Division leaves the divisor as a flexible live
operand while constraining only the short architectural occurrences.  The
encoder emits the exact `cwd`, `cdq`, `div`, `idiv`, and `movzx` forms.  BASIC
and C facts remain confined to their x86 ABI finalizers; no language fact was
added to generic IR, Machine IR, MC, or a pass.

After those target mechanisms were in place, `llrm-c` returned the established
`200100255` oracle while `llrm-qb` returned `3492255`.  Adjacent QIR, QMachine
IR, and assembly dumps located the first divergence before CodeGen: QB had
typed the unsuffixed decimal `1000000` as INTEGER, emitted `mov ax,4240h`, and
then sign-extended the already-truncated value.  The lexer now applies the
measured QB decimal rule generally: magnitudes through `32767` are INTEGER,
larger values through `2147483647` are LONG, and explicit `%`/`&` suffixes
retain their range contracts.  Based literals keep their distinct bit-pattern
rules.  The regression was observed failing first at the lexer boundary and
then proves the LONG type through HIR as well as the original program symptom.

The 3,646-byte production `qlight.cgs` capture was generated twice with
byte-identical SHA-256
`36708cc5419bb91ceb3502b40149b4fa5ec606af39345e2278b2817991b3eec8`.
The final exact two-test gate built fresh objects through `llrm-c` and
`llrm-qb`, linked and ran both under DOSBox-X, and returned `200100255` in
5.94 seconds.  Focused fail-first, unit, adjacent-stage, and runtime commands
used about 145 seconds across the multi-hour implementation and primary-review
interval, remaining below ten percent.  Two attempted exact pytest commands
were deselected by the repository's default full-test filter; one still spent
14.33 seconds in setup before that was diagnosed, and neither was counted as a
passing gate.  Two delegated cached target-wide invocations also exposed the
same pre-existing `Copy`/`Mov` expectation failure and were not repeated.  No
optimizer, complete host, matrix, or qrender gate ran.

Commits `9633ec48` and `f9e7c0db` advance the paired `qmove` rung through
verified portable IR.  The implementation follows Python's storage/evaluation
split: WCC `TY_SINGLE` is binary32 in memory and at call boundaries but uses
extended80 SSA values between those boundaries.  Loads extend, stores and
rvalue call arguments truncate, and assignment expressions reload their
stored value.  C float-to-integer conversion carries toward-zero rounding;
BASIC conversion carries the dynamic x87 rounding rule.  Those semantic facts
are selected in their frontends and represented generically in HIR and IR;
no C, BASIC, WCC, register, or x87 name enters a generic pass.

The public schemas advance to qhir 2, qir 7, and qmir 7.  The real 7,317-byte
`qmove.cgs` capture now emits a 178-line canonical qir 7 artifact, which parses,
verifies, and prints byte-for-byte identically through `llrm-opt`.  Its first
adjacent failure has moved from the WCC frontend to x86 selection, which now
explicitly reports `unsupported IR type 6` for binary32.  This is the next port
boundary: the established Python x87 selector and floating-stack allocator,
not a new floating backend design.

Primary review rejected six delegated lowering errors before acceptance:
unused evaluation types in integer modules, declaration-order drift, base
loads for projected places, omission of direct binary32 place arguments,
same-format literal truncation, and casts chosen from storage rather than
evaluation formats.  Focused compilation, fail-first checks, frontend tests,
schema round trips, and the two adjacent real-capture probes used about 51
seconds of command execution.  No broad suite, optimizer pipeline, matrix,
runtime gate, or qrender command ran; verification remained below ten percent
of the implementation and review interval.

## Iteration 10: paired qmove CodeGen (in progress)

The first Rust-only QB and C qmove executables now both compile, link, and run
to the existing `100405` oracle without invoking an optimization pass.  This
is not yet the CodeGen acceptance milestone: primary assembly review still
shows target-lowering and cleanup sequences which do not match Python's
unoptimized construction or code quality, so they remain port work rather
than being excused as a new Rust backend.

The C path initially produced the right answer by eagerly loading every
floating formal at function entry, carrying those extended values across the
CFG, and bridging them through two m80 frame homes.  Python does none of those
things.  Its WCC raise retains every formal as its addressable incoming frame
cell and emits a load only at the source use.  Primary review rejected the
delegated cross-CFG reread exception in the x87 allocator and restored that
boundary literally:

- qhir 3 spells an incoming cell as indexed
  `Storage::Parameter { index }`;
- qir 8 represents its address with the source-neutral `ParameterAddress`;
- WCC `CGFEName` binds to that place while the unused formal SSA value remains
  only in the ABI signature;
- selection reuses the existing `IncomingArgument` frame object and emits no
  instruction for the address itself;
- unused formal SSA values no longer trigger entry loads, while the existing
  QB value-parameter route is unchanged when the value is actually used.

The focused qmove regression checks the original machine symptom as well as
runtime behavior.  `_pl_ground_accel` now loads its binary32 float formals
directly from `[bp+10]` and `[bp+14]`, emits no `fstp tword` bridge home, links
with the independent far-cdecl caller, and returns `100405`.  Calling-
convention offsets remain solely in the generic C ABI frame planner; no WCC,
C, QB, or fixture name entered IR, Machine IR, allocation, or an optimization
pass.

Focused verification for the parameter-cell correction used approximately 64
seconds of command execution: delegated frontend checks, four HIR/IR/selection
regressions, compile checks, one adjacent object inspection, and one 17.47
second DOS runtime gate.  No broad suite, optimizer pipeline, matrix, or
qrender command ran.  This remains below ten percent of the multi-hour qmove
implementation and primary-review interval.

Primary review then completed a line-by-line comparison of the affected x87
stack rules with `qbopt/backend/floatalloc.py` and
`qbopt/backend/floatregions.py`.  The Rust port now follows the Python
mechanisms instead of accepting behaviorally convenient substitutes:

- every value crossing a floating region receives Python's definition-time
  m80 bridge and one reload in each consuming region;
- stable m80 cells are rereadable homes, but remain ineligible for x87 memory
  arithmetic exactly as Python's separate `_rereadable` and `_memory_name`
  decisions require;
- a load receives a home only after `_rereadable` succeeds;
- `_may_write` considers unknown operations, actual overlapping memory
  destinations, and definitions of address components rather than treating
  Machine IR effect flags as invented destinations;
- register stores use their Machine IR value width for overlap accounting;
- arithmetic preserves an operand already at `st(0)`, orders materialization
  by definition position, and uses Python's distinct top, buried-destination,
  and pop orientation cases.

Four initially failing x87 expectations were audited against Python.  Two
were real Rust divergences and two expected behavior Python never had: the
ninth stack value creates one m80 spill, and a value crossing a CFG fork still
requires its m80 bridge even when an unrelated store cannot alias its source
cell.  The corrected subsystem has 26 focused regressions.  The register-store
width regression was also observed failing with the previous one-byte rule
before passing with class-derived widths.  One subsystem run, the focused
fail-first/rerun, compile checks, delegated read-only oracle probes, and one
fresh C qmove DOS gate used about 85 seconds of additional command execution.
The fresh object loads both C float formals from their incoming binary32 cells,
contains no entry m80 bridge, and returns `100405`.  No broad suite, optimizer,
matrix, or qrender command ran.

The next adjacent-stage comparison used Python's unoptimized `qmove` dumps as
the specification and found the first remaining divergence before allocation:
Rust materialized every constant near-pointer GEP as `Copy; Add`, including
zero, while `qbopt/backend/addressforms.py` folds that chain into the consuming
memory cell.  The Rust selector now carries the same base-plus-signed-word
displacement instead.  Frame field offsets remain abstract until the frame
plan combines them exactly once, register-based cells use the complete 16-bit
address-register encoding, and direct symbolic cells retain their OMF fixup.
The paired Rust regression checks both `[base]` and `[base+4]` in the real
qmove selected Machine IR and rejects the former `Copy; Add(0|4); X87Load`
shape without banning unrelated integer arithmetic.

Primary review caught and corrected three delegated approximations before
acceptance: local fields were initially left behind a `lea`, explicit zero
used the longer noncanonical ModR/M spelling for every base, and the old
BP-only encoder disagreed with the newly general verifier.  Focused selector,
frame-index, verifier, and exact-byte encoder regressions now cover those
boundaries.  The real qmove program still links and returns `100405`.

With address construction aligned, the next diff was Python FloatAlloc's
`fld m32; fmul m32` versus Rust's `fld; fld; fmulp`.  Rust's explicit
f32-to-f80 `Copy` had prematurely materialized an otherwise deferred scalar
home; Python's LIR represents that same storage-to-evaluation transition as
one extended value loaded from a four-byte cell.  A dying no-op x87 copy now
transfers the deferred home only after rerunning Python's rereadability proof
for the destination.  The same newly reachable direct-memory comparison also
now carries its required load effect.  The first qmove dot products, scalar
subtraction, multiplication, and comparison consequently match Python's x87
memory forms.  `_quake_move_demo` moved from offset `0xc2` before these two
ports to `0x8c`.

The following adjacent HIR diff exposed two discarded `CGPreGets` update
results.  Python's pre-get stores the computed SINGLE and returns an abstract
FloatCell; `CGDone` discards that cell without loading it.  Rust had eagerly
loaded each just-stored `vel` component.  The WCC raiser now shares one
load/arithmetic/store update path: an ordinary expression reloads the rounded
cell, a discarded update defers it, and a later DAG reuse reloads it exactly
once without repeating the store.  Focused fail-first tests cover both cases.
The unused loads and their final x87 cleanup pops are gone, moving
`_quake_move_demo` to `0x83`.  Python is at `0x7b`, so the CodeGen milestone
remains open rather than treating the remaining eight-byte construction
difference as an optimizer task.  The first remaining representation diff is
Python's equal-width SINGLE cell assignment, which `put_float` spells as a
four-byte integer move while Rust still routes it through x87; after allocation
Python's peephole also removes two repeated parameter-pointer reloads.

Known focused verification in this interval used about 125 seconds of command
execution across more than twenty minutes of implementation, delegated work,
primary diff review, exact-stage inspection, and correction.  It included the
two qmove acceptance tests, narrow selector/address/encoder/frame/x87 tests,
and fresh adjacent-stage/object dumps.  No broad suite, optimizer pipeline,
matrix, or qrender command ran; verification remained below ten percent.

The next Python construction difference is now translated literally.  In
`qbopt/cfront/raise_hir.py::_Raise.put_float`, an equal-width `FloatCell`
assignment is not an x87 operation: Python loads its four bytes as
`TY_UINT_4`, stores the same four bytes into the destination cell, and returns
that destination `FloatCell`.  The WCC Rust raiser now retains that cell fact
long enough to emit the same typed HIR `Load; Store` pair.  It uses a `u32`
projection only for the memory transfer; no C or floating-domain fact entered
HIR lowering, IR, Machine IR, or an optimization pass.  A discarded assignment
still ends at the store, while the existing deferred-cell route loads the
destination only if the capture DAG later consumes the assignment value.

The focused regression was observed failing first because the transport value
was still `f32`, then passed with a `u32` source projection, `u32` result, and
`u32` destination projection.  A fresh real qmove object confirms that the
conditional `accelspeed = addspeed` block is now exactly the two dword moves
Python raises, rather than Rust's former `fld; fstp`.  This deliberately makes
the faithful Rust function two bytes longer than its shorter but non-matching
x87 spelling: `_quake_move_demo` now begins at `0x85`, versus Python's `0x7b`.
The remaining ten bytes are completely accounted for by existing Python
post-allocation mechanisms: `spillforward.forwarded` removes the two repeated
three-byte parameter-pointer loads, and Python's final control-flow layout
avoids two two-byte unconditional jumps around the copy and update blocks.

Primary review independently reran the exact unit regression, rebuilt
`llrm-c`, inspected the fresh QIR and object bytes, and ran the existing qmove
DOS oracle (`100405`).  Delegated fail-first/pass runs, the primary checks, one
Python dump, and one accidentally deselected pytest invocation used about 29
seconds of command wall time.  No broad suite, optimizer pipeline, matrix, or
qrender command ran, keeping verification below ten percent.

The reload-forwarding port was not approximated.  A read-only audit traced the
exact call chain to `qbopt/backend/spillforward.py::{_held, _available,
_transfer, forwarded}` and its tests in `tests/test_peephole.py`.  Rust's
current post-allocation Machine IR erases Python's `spill_reload`/
`spill_store` provenance and has no zero-byte `lir.anchor` equivalent retaining
logical ownership.  The representation must first port those facts; deleting
the loads, calling the transformation machine CSE, or adding a qmove-specific
rule would not be a faithful port.

That representation prerequisite is now present as a direct Machine-IR
translation.  Allocator-created loads and stores carry explicit provenance,
and `MachineInstruction::anchor` retains only a removed instruction's logical
virtual def/use operands and stable instruction identity while clearing every
physical effect.  Assignment preserves those logical operands and their
declarations.  The x86 target owns a `Nothing` pseudo which lowers to an empty
MC data fragment at the same lineage position; it cannot reach the encoder or
turn into a hardware `nop`.  The new flag meanings make `.qmir` version 9 an
intentional schema break rather than silently assigning semantics to formerly
invalid bits.

Primary review caught that the first verifier draft recognized spill
provenance only while its address was an abstract `FrameIndex`.  Frame-index
materialization preserves the marker but rewrites the operand to the canonical
`BP + immediate` tuple, so the verifier now accepts exactly those two forms.
It still rejects a marker on the wrong opcode, a non-spill abstract object, or
a noncanonical materialized address.  An independent focused run of the
address-spill allocation/materialization regression passed in 2.74 seconds.
Together with the delegate's three focused filters, verification for this
foundation used about 20 seconds of command wall time and remained below ten
percent of its implementation and review interval.

The two post-allocation mechanisms which accounted for qmove's remaining gap
are now ports of the Python implementations rather than fixture-shaped
shortcuts.  `target::x86::spill_forward` translates
`qbopt/backend/spillforward.py::{_held,_available,_transfer,forwarded}` with
exact frame cells, physical byte lanes, the same top/bottom data-flow lattice,
predecessor intersection, overlapping-store invalidation, unknown-operation
refusal, and zero-byte anchors for eliminated reloads.  It deliberately
forwards ordinary program frame loads as well as allocator reloads, matching
Python.  Calls, unknown encodings, unknown stores, BP writes, and Python's x87
status-word `BARRIER` end every fact.  The last point was found by the raw
qmove diff: treating `fnstsw ax` as a completely known AX write carried BX/SI
facts farther than Python and removed six extra bytes.  Its regression was
observed failing with the known-write behavior before passing with the exact
barrier boundary.

`target::x86::control_flow` is the bounded translation of
`qbopt/backend/jumps.py::{placed,_onward,_tests,threaded,_step,_passage,
_through,_retargeted,_predecessors,_reachable}`.  Tail merging, machine DCE,
and encoded-cost preference remain explicitly outside this slice; calling it
the full Python `optimized` routine would be inaccurate.  Source facts which
fresh Machine IR cannot infer are supplied separately: non-inserted
instructions govern removal anchors, source-byte ownership retains inert
orphans, and measured loop headers remain protected.  Fresh QB and C source
use empty fact sets; the future object frontend must populate them from its
rewrite ledger.  Primary review rejected an earlier approximation which tried
to infer these facts from instruction shape, then caught two tests that had
accidentally composed placement with threading while claiming to cover the
Python threading helper alone.  The corrected helper-boundary and integrated
placement regressions pass.

Logical reload anchors exposed one representation assumption in the existing
finalizers: word composition and both ABI expanders rejected their retained
virtual declarations before MC could lower the anchors to zero-byte fragments.
They now admit only declarations used by logical anchors and continue to reject
every virtual operand on an encodable instruction.  The word-merger regression
exercises the complete boundary.  This is one shared Machine-IR rule, not an
exception in C or BASIC code generation.

The final two-byte qmove difference was upstream of control-flow placement.
Python `FunctionRaiser.branch` handles `O_IF_FALSE` on a `CGCompare` by
inverting the relational test and branching directly to the capture label;
Rust had materialized the original boolean relation and selected its false
edge.  Both were semantically correct, but they presented opposite edges to
the otherwise faithful placement algorithm.  The WCC frontend now performs
the same comparison inversion without caching that branch-only result, just
as Python calls `compare` directly rather than `eval`.  No C fact enters HIR
lowering, portable IR passes, Machine IR passes, or MC.

After these corrections, a fresh `_pl_ground_accel` has the same instruction
sequence and block order as Python's unoptimized backend: the return block
immediately follows the first conditional, the accelerated path follows it,
the equal-width SINGLE assignment is the same dword load/store, the same two
parameter-pointer reloads are forwarded, and the update path ends with the
same short jump to the shared return.  `_quake_move_demo` begins at `0x7b` on
both sides.  The Rust encoder may choose an equivalent spelling for an
instruction, so this is an adjacent-stage and layout equality claim rather
than a byte-identical whole-object claim.  Fresh DOS runs retained the paired
C and QB qmove oracle `100405`.

Focused verification for the anchor, spill-forward, control-flow, WCC branch,
fresh-object, and two DOS checks used about 121 seconds of command wall time
across a multi-hour implementation and primary-review interval.  That includes
the required fail-first runs and one failed parity-instrument invocation whose
stale QB JSON frontend path never reached the C oracle.  No broad suite,
optimizer pipeline, matrix, or qrender command ran; verification remained
below ten percent of wall time.

A final read-only fidelity audit found that equal-sized natural loops were
ordered by reconstructed header ID in Rust, while Python preserves first
back-edge discovery order and then applies a stable body-size sort.  That can
change which loop is protected when placement sees nested or adjacent loops.
The Rust port now records headers at the point their first back edge is
discovered, constructs loops in that order, and performs the same stable sort.
The focused regression was observed failing with `[1, 2]` where Python requires
`[2, 1]`, then passed after the correction (4.11 and 3.37 seconds).

The same audit tightened the allocation-to-MC anchor contract.  A boolean
`anchor` flag alone no longer exempts an encodable instruction or its virtual
declaration from finalizer checks.  The only admitted form is the canonical
zero-byte `Nothing` instruction whose operands are unconstrained, untied
virtual def/use lineage and whose remaining flags are clear.  Direct MC
lowering independently rejects a malformed claimed anchor, so callers cannot
bypass the finalizers and silently erase a real instruction.  Follow-up review
also required every anchor operand to have a dataflow role and a declaration
in its containing function, and made control-flow retain malformed `Nothing`
instructions so the MC boundary can diagnose them rather than losing them as
transparent markers.  Twelve focused anchor tests, including these
regressions, passed in 4.6 seconds.  No DOS or broad-suite rerun was needed
because the correction only rejects malformed Machine IR; the previously
verified canonical qmove path is unchanged.

One deliberately conservative boundary remains explicit: Rust will not remove
a volatile frame load, while the Python representation has no corresponding
volatile case in this helper.  This does not alter any ported Python input and
prevents the Rust-only qualifier from losing its required observable access.
Materialized spill provenance also still needs to be correlated with the
allocator's frame-layout ownership before a future consumer may treat a
syntactically canonical `BP + displacement` address as proof of provenance.

### 2026-09-21: paired qbsp adjacent-stage port

The paired QB and WCC QBSP programs are being advanced only by porting the
first measured Python behavior missing at each adjacent stage.  The QB path
now resolves array formals through their physical descriptor pointers,
canonicalizes opaque pointer identities by address kind where portable IR no
longer retains pointees, and canonicalizes indirect-pointer lowering in the
same way.  Focused fail-first regressions cover both boundaries.

Python's split-evaluation BASIC floating-result ABI was initially considered
at the target IR boundary and rejected there: portable pointer types no longer
contain the pointee identity needed to distinguish the hidden SINGLE and
DOUBLE result cells.  The faithful port therefore lives at the QB HIR
boundary, matching `qbopt/frontend/qb/abi.py`: eligible far Pascal callees
store into the final hidden near pointer and return it, and direct callers
receive that pointer and immediately load the semantic floating result.  The
transform is clone-only, retains argument order and exact storage types, and
leaves CDECL, runtime calls, and unrelated functions unchanged.  Five focused
tests passed in about 3 seconds.

The next QBSP Machine-IR divergence was a floating load or store through a
far 16:16 pointer.  The first Rust implementation exposed an older unfaithful
detour: it split the pointer with `LowWord` and `HighWord` and tried to
recognize the sequence after allocation.  Spilling separated those operations
and left an unencodable `mov es`.  Python already has the general answer in
`backend/lower.py::_pointer_access`: `push es`, `push pointer`, `pop offset`,
`pop es`, access, `pop es`.  Rust selection now emits that exact balanced
sequence for integer and x87 accesses.  The related selection tests assert the
same order and Dword/Address16 classes, and x87 verification and encoding
accept the ES-relative memory form.  The three focused far-memory tests and
the exact encoding test passed in about 6 seconds total.  Fresh QBSP now emits
both verified `.qmir` and a 2,159-byte OMF object.  Its focused VBDOS
link-and-run regression also passes the established `120`/`DONE` oracle in
2.4 seconds.

On the WCC side, Python `_Raise.assign` accepts near `TY_POINTER` assignments
through the ordinary typed store path; Rust had incorrectly called its
scalar-only type helper.  The capture type now reaches the existing
source-before-target/coercion/store sequence, with a focused QBSP-shaped
regression.  Python `_Raise.unary` was then ported for integer negate and
complement, including width wrapping of constants, plus SINGLE negate and
absolute-value operations and the same refusal cases.  Five unary regressions
passed in about 3 seconds.  The real capture then exposed `TY_DOUBLE` at source
1:68:1.  Its port matches Python's 8-byte storage with extended x87
evaluation, typed literal precision conversion, semantic unary/binary
operations, deferred assignments, and two-word `TY_UINT_4` cell copies.  Three
focused double regressions passed, and the real WCC QBSP capture now emits
verified `.qir`.  The next target boundary was the C floating result ABI.
Python carries both a direct C call's floating result and a C function's
floating return in x87 `st(0)`; Rust now expresses the same contract as a
fixed-ST0 definition or use in Machine IR for near C and far cdecl calls and
returns.  Return operands still pass through ordinary typed selection first,
so a literal return follows Python's evaluate-then-float path instead of being
restricted to an existing SSA value.  The focused regression covers SINGLE
and DOUBLE, both distances, and a direct literal.  Primary review independently
ran that exact test and reproduced the real capture's next refusal: a
same-width signed/unsigned integer bitcast in `_r_point_leaf`.  Python
`_Raise.convert` represents that case by returning the same held word, and the
Rust HIR lowerer records the typed view as `Bitcast`; selection now completes
that translation by reusing the selected location when both integer widths
match.  Unequal widths still refuse.  The focused i16-to-u16 mask regression
passed independently in 0.02 seconds, and the real capture now emits a
12,663-byte verified `.qmir`.  Its next adjacent object-emission boundary is
the x87 stack allocator's floating `ReturnFar` rule.  Python's nameless
`FLOAT_STORE` return arm moves the requested value to `st(0)`, refuses if any
other x87 value remains live, and leaves that one architectural result for the
caller.  Rust now performs the same operation for near and far C returns while
preserving the far cleanup operand.  The focused regression also covers the
multi-value refusal and passed independently in 0.03 seconds.  The real object
rung now reaches the next frame-planning boundary: the C ABI planner has not
yet admitted an extended x87 result that requires no AX/DX return machinery.
The C ABI frame port now accepts all three supported floating formats, requires
the exact physical `st(0)` return operand, and removes only that explicit
operand during final expansion while retaining far-cdecl's zero cleanup.
Python's native frame planner likewise has no floating result-register rule.
The six near/far format cases passed independently; the checked-in QBSP object
rung now reaches a distinct x87 region issue: a call's newly defined `st(0)`
result must belong to the region after the call boundary, while values live
before the call must still be bridged or refused.  Rust now recognizes only
one pure fixed-`st(0)` call definition as that post-call value.  Primary review
rejected an initial change that also moved returns across the boundary and a
helper that could index an empty result list for ordinary calls; the corrected
rule preserves floating returns, cannot panic on a zero-result call, refuses a
non-ST0 x87 definition, and retains the existing m80 bridge for a genuine
pre-call live value.  All three focused regressions passed independently.

The paired C QBSP artifacts now travel with the port: the canonical WCC stream
generated from `bench/parity/qbsp.c`, the standard far-cdecl DOS harness, and a
Rust end-to-end regression using the established Python-era result `120`.
Fresh Rust emission produces a 663-byte OMF object; JWASM, Microsoft LINK, and
the DOS 386 run all completed, and the focused program regression returned
`120`.  This reaches the requested pre-optimization milestone for both
`llrm-qb` and `llrm-c` on the same paired program.

This slice used only fail-first unit filters and one-program `.qir`, `.qmir`,
object, and final QBSP runtime rungs.  Command execution was roughly 50 seconds
plus one 15-second deselected invocation before the required `--full` marker
was supplied, over a much longer implementation and review interval.  No
broad suite, matrix, or qrender command ran; verification remained below ten
percent of wall time.

### 2026-09-21: WCC imported floating calls

The next paired-C expansion follows Python
`cfront/raise_hir.py:_Raise.library`, `_Raise.invoke`, and `_Raise.push`
directly.  External C and far-cdecl declarations now select a named external
call instead of requiring a Machine function body.  Floating results are
fixed to `st(0)`, DOUBLE constants are pushed high dword before low dword, and
a computed DOUBLE uses Python's exact eight-byte temporary followed by the
`+4`, then `+0`, load/push sequence.  Caller cleanup remains the sum of the
declared argument widths.  An unused result remains a real IR value; the
existing x87 allocator therefore emits Python's `fstp st(0)` discard instead
of hiding the result at selection time.  Primary review rejected a resultless
non-void test construction, required the fixed-`st(0)` constraint and computed
DOUBLE path, inspected the corrected selector, and independently ran the two
new exact regressions.

WCC's `O_SQRT` now follows Python `_Raise.library`: it converts each actual to
DOUBLE, calls the imported far caller-cleanup `_sqrt`, receives the extended
x87 result, then converts to the requested expression type.  HIR names the
external symbol and leaves `CallAbi.callee` empty; the call planner interns the
two test sites as one external far-cdecl declaration, matching Python's
`Shared.runtime` cache.  Primary review rejected an initial undefined-callable
model because a callable ID denotes a definition, then required Python's
last-first binary-library inputs to be restored to logical HIR order before
x86 cdecl selection reverses them for pushes.  Both focused library
regressions passed independently.

The real `fixtures/c/floats.cgs` probe still stops earlier, at Python
`_Raise.__init__`'s formal layout: Rust's temporary 2/4-byte guard refuses the
first DOUBLE parameter.  The next slice replaces that shortcut with Python's
general `even(max(2, size(type)))` rule.  No raw `tools/stages.py` equality is
claimed for this WCC path: the current tool accepts BC objects, while Python's
C oracle is `python -m qbopt.cfront --dump`.  A stage-instrument port must make
the corresponding WCC facts comparable before this path can satisfy the final
stage-by-stage acceptance gate.

That formal-layout slice now uses the general Python rule with checked
rounding and accumulation in source declaration order.  The regression reads
the real `floats.cgs` declarations: `_half` and `_sign` each retain one
eight-byte DOUBLE formal, while `_mag` retains two in indices zero and one and
reports sixteen parameter bytes.  The test inspects `_sign`'s declaration
without entering its independently unported `CGChoose` body, then verifies and
lowers the executable `_half`/`_mag` slice.  Primary review independently ran
the exact regression; the real `.qir` probe advanced from source line 8 to the
expected `CGChoose` refusal at source line 19.

The WCC dump audit also made a larger acceptance gap explicit.  Python writes
the ordered `stream`, source `hir`, raised `mir`, annotated/pass MIR, `lir`,
every allocation phase, and `asm` from one compilation.  Rust currently lowers
its capture directly into a different generic SSA IR and exposes only `qir`,
`qmir`, and `obj`; those artifacts cannot prove a function-for-function port
of Python MIR, LIR, allocation, or emission.  Consequently the next accepted
work starts by reproducing Python's source-HIR dump exactly, then ports the
Python MIR/LIR structures and printers before adding more WCC semantics.  The
planned `CGChoose` implementation in the generic HIR was stopped before it
edited the tree, rather than duplicating Python behavior into a temporary
representation that cannot meet the stage gate.
