# QB frontend implementation plan

Execution is gated by the isolated Microsoft-runtime ladder in
[test-ladder.md](test-ladder.md). qrender and qb-quake are integration targets,
not the first test of procedure entry, stack layout, strings, or arrays.

This plan is ordered around executable vertical slices. Each phase must leave
the existing OMF frontend, MIR optimizer, and backend unchanged in behavior.

The real-program target is `~/work/personal/qb-qrender`: all 17 production
BASIC modules through this frontend, its five C modules through the WCC
frontend (then the common HIR once proven), and its two assembly modules kept
as assembly. The existing uGL, U3D, and VBDOS libraries/runtime remain link
inputs rather than being reimplemented.

## Implemented foundation

The first Phase 1 slice is present under `src/hir/`:

- typed modules, functions, values, places, blocks, terminators, dialect,
  runtime, and fixed 386-real-mode target profile;
- a strict schema-1 deterministic JSON codec and replay command;
- verification of identities, CFG, entry parameters, float evaluation,
  memory types, and fixed-array shape;
- lowering for whole integer/LONG operations, floating arithmetic semantics,
  direct places, branches, switches, returns, and source-order fixed numeric
  array offsets; and
- an identity-independent MIR projection plus a test that carries an indexed
  array access through existing LIR lowering.

The first source-compiler slice now also exists under `frontends/qb/` and
`src/frontends/qb/`: a dialect lexer and typed parser, direct semantic HIR
construction, a replayable process boundary, whole integer/LONG and floating
expressions, fixed numeric arrays, and CFG construction for IF, FOR, DO,
WHILE, labels, and GOTO. All 17 qb-qrender BASIC files pass the VBDOS syntax
frontend. Runtime statements are accepted from a deliberate allowlist and
then rejected at semantic lowering until their ABI is audited; acceptance is
not reported as compilability.

Procedure bodies/results, ordinary scalar and UDT `BYREF`, packed UDT field
layout, nested fixed-array fields, and native array-parameter element access
now reach HIR. The array-parameter descriptor path is based on listings from
all three compiler families and lowers to generic loads, 32-bit arithmetic,
whole-pointer offsets, and memory operations. Numeric SELECT lowering, logical
descriptor ownership, `REDIM` calls, and explicit `/R` versus default array
order also reach HIR. Static descriptor initialization/fixups, allocation
effects, many audited runtime calls, and fresh BASIC-shaped OMF are connected.
Complete link plans and linked execution remain gated by the phases below.

The current string bridge also distinguishes stored four-byte descriptors from
two-byte descriptor-address expression results. Measured VBDOS chains such as
fixed `STRING * n` -> `B$LDFS` -> `B$RTRM` -> `B$FMID` -> dynamic `B$SASS`
now survive HIR, semantic MIR, call physicalization, and allocation. `CHR$`
uses the same result convention, and a runtime temporary can be assigned to a
fixed string through `B$ASSN` with an explicitly constructed DS:offset source.
String comparisons are explicit HIR relations lowered to the flags returned
by `B$SCMP`, and runtime temporaries can feed ordinary `STRING` formals without
an invented descriptor copy. Concatenation is a left-to-right `B$SCAT` chain.
`MID$` assignment, `LEFT$`, both `STRING$` forms, and `ENVIRON$` now cross the
same typed descriptor boundary. Directory functions and formatted printing
remain later Phase 6 work; parser acceptance is not counted as lowering
support.

Simple file lifecycle statements are no longer keyword-only placeholders.
`FREEFILE`, `OPEN` in INPUT/OUTPUT/APPEND/BINARY modes, `EOF`, and counted `CLOSE`
have structured syntax and typed runtime operands. Their source-to-allocated-
LIR examples include `files.bas`, `open_append.bas`, `record_io.bas`, and
`positioned_io.bas`. `LINE INPUT`, `SEEK`, and both record `GET`/`PUT` forms
are implemented; access/locking/LEN clauses and formatted I/O remain Phase 6.

The first numeric/string conversions are now measured and implemented.
`VAL` calls `B$FVAL`, loads the DOUBLE DAC addressed by AX, and leaves numeric
conversion to ordinary HIR. `STR$` chooses the integer/LONG/SINGLE/DOUBLE
runtime wrapper from its typed operand and returns a descriptor address.
`LINE INPUT` and string `SELECT CASE` are also complete through allocated LIR.
The full six-body production `common.bas` module now reaches every stage
through allocated LIR; its dumps are under `build/qbstages/common-round`.

The current staging gate is all 17 qb-qrender modules through allocated LIR
and final inline-x87 staging. Saved complete stage runs include 32-body
`screen`, 26-body `model`, 15-body `r_bsp`, 7-body `mod_tex`, 6-body `common`,
5-body `snd`, 42-body `d_surf`, plus `h_bench`, `view`, and `sys`. `PRINT`, disk `INPUT`,
`DIR$`, terminal display statements, fixed-string `REDIM`, and `FRE` are now
measured and represented. SUB/FUNCTION declarations are reconciled into a
callable table during semantic analysis and call sites retain resolved symbol
IDs rather than repeating textual lookup. Module-level `ON ERROR` registration
(`main`) is emitted as a measured relocated `B$OEGA` sequence, and `DEF SEG`
state shared across procedures (`d_surf`) is the external runtime cell `b$seg`.
Fresh source objects have been emitted for all 17 modules and a complete mixed
language legacy-runtime link succeeds. Real DOS execution has passed module
initialization, static-string OPEN, COMMAND$ acquisition, and the managed-frame
exit gate. A reduced procedure with a descriptor-backed STRING array now
prints `HELLO WORLD` and returns through `B$EXSA`. Follow-up rounds corrected
near STRING-array descriptor access, Pascal SUB argument order, `B$ERS1` local
array cleanup, preservation of positive BP parameter displacements, bare
zero-argument FUNCTION resolution, and the legacy `DX:AX` LONG-result ABI. The
fresh `SYS` substitution now passes the former error-64, far-heap, and
post-font failures. The shared `ents.bin missing` result was traced to a stale
uGL archive without ZIP support, not to either compiler. After linking the
ZIP-capable archive and the previously omitted `QR_PROF.OBJ`, the all-BC
control renders and exits. A fresh-`SYS` substitution now does the same.
Isolated probes added the zero-argument `B$TIMR` pointer result and the hidden
near result slot used by Pascal SINGLE/DOUBLE functions; the latter changed
`sys_tick_hz` from zero to a measured value near the control and restored the
frame clock. Five-frame structural render counters match the control. Timed
camera coordinates and the BMP hash are not an identity gate because the runs
observe different elapsed intervals. Whole-program fresh-object substitution
and qb-quake remain later gates.

The next isolated production boundary, fresh `SCREEN.OBJ`, also renders and
exits. It exposed that module `$STATIC` array descriptors must be immutable
`BC_CN` objects pointing at mutable `BC_DATA`: putting initialized descriptor
bytes beside the elements lets BASIC startup clear them. The saved full-module
stage round is `build/qbstages/screen-static-descriptor-round`; the reduced
stage round is `build/qbstages/static-array-descriptor-round`. Continue module
substitution from this passing boundary rather than jumping directly to the
all-fresh image.

## Phase 0: freeze contracts and probes

- Record the HIR schema and 386 real-mode target contract in this directory.
- Build a dialect feature matrix from small QB 4.5, PDS 7.1, and VBDOS probes.
- Select representative probes for 16-bit math, LONG operations, floating
  evaluation, static arrays, dynamic arrays, far arrays, and huge arrays.
- For every probe, save compiler diagnostics, object-raised canonical MIR,
  linked output, and a hand-derived expected operation/memory shape.
- Add a canonical MIR projection that removes incidental identities but keeps
  widths, operations, control, floating semantics, provenance, and calls.

Gate: every initial claim has a raw source/object/runtime artifact, and the
current object frontend still passes its full suite.

## Phase 1: add the common HIR library

- Implement the entities in [the HIR model](../../architecture/hir/model.md).
- Add the versioned JSON decoder, verifier, and deterministic stage dump under
  `src/hir/`.
- Implement values, places, blocks, calls, integer/floating operations, and
  numeric array access only.
- Add program-side tables for source maps, ABI selection, and link needs.
- Write construction/verifier tests, including negative tests for machine
  details, unresolved types, invalid control flow, and operations with no
  current-MIR mapping.

Gate: hand-built HIR for each Phase 0 probe lowers to existing MIR without any
MIR/backend modification and matches the hand-derived expectation.

The fresh writer is not a new subsystem: the implementation target is the
existing `backend.masm.Module` plus `backend.omfwrite` pipeline already used by
the WCC frontend. The missing piece is a QB-owned ABI adapter that supplies
far/callee-cleanup procedure scaffolding, call arguments, runtime imports,
the fixed module header, and descriptor data/fixups without teaching MIR any
of those facts.

Call-site order, distance, and cleanup plus procedure entry ABI are now HIR
side tables, and the late QB adapter materializes them as existing MIR `ARG`
nodes and call contracts. Native prologue/epilogue, static data, final
`masm.Module` assembly, and fresh OMF emission are connected. The frontend adds
the measured 48-byte module header, BASIC segment classes, `BC_SA`
registration, `B$CEND` exit, and `B$OEGA` registration without changing MIR
or the backend. Runtime-owned cleanup/framing details and linked execution
remain to connect.

## Phase 2: extract the QBasic parser in tree

- Import the minimal Rust lexer/parser/table generator from
  `~/work/personal/qbasic-port` into `frontends/qb/`, preserving provenance
  and license notices.
- Replace p-code `EMIT` actions with named semantic actions and rollback-safe
  builder checkpoints.
- Remove executor, IDE, p-code buffer, scanner patching, and runtime code from
  the dependency graph.
- Make parser-table regeneration deterministic and check the generated table
  into the tree if builds should not require the generator.
- Establish all four dialect profiles immediately, even where later-dialect
  feature tables are initially incomplete.
- Emit the versioned common-HIR JSON document and make the Python driver retain
  it when stage dumps are requested.

Gate: QBasic grammar fixtures retain their accept/reject behavior, p-code does
not appear in the frontend API or output, and the build has no dependency on
the sibling checkout.

## Phase 3: semantic analysis and LONG vertical slice

- Implement declarations, default typing, scopes, procedure signatures,
  labels, implicit conversions, and storage classification.
- Construct HIR control flow directly after resolution.
- Lower 16-bit integers and signed 32-bit `LONG` as whole values.
- Map recognized long multiply/divide/remainder/comparison semantics to the
  same current MIR operations produced by the object raiser.
- Represent all already-audited non-native facilities encountered by the
  probes as runtime calls.

Gate: source-generated and object-raised canonical MIR agree for the integer
and LONG probes; linked programs agree on output and failure behavior under
each applicable runtime.

## Phase 4: floating and inline math vertical slice

- Implement `SINGLE`/`DOUBLE` typing, conversions, constant parsing, and
  explicit evaluation semantics without using host floating behavior as the
  specification.
- Lower only operations already present in current MIR/backend.
- Preserve floating environment and rounding effects through stores and calls.
- Resolve QB-family intrinsics through one frontend catalogue containing
  dialect availability, accepted arity, result class, observable effects, and
  lowering category. Keep lowering algorithms in semantic code; the table
  selects an algorithm and is not a bytecode interpreter or an ABI switch.
- Keep every numeric math intrinsic inline.  Intrinsics remain named HIR
  computations until the QB frontend's late physicalization step; they do
  not become `B$SIN*`, `B$COS*`, `B$TAN*`, `B$INT*`, or other math-runtime
  calls.  Operations already expressible in MIR lower normally.  The QB
  frontend owns the final x87 spelling for the remaining intrinsic forms so
  this requirement does not widen MIR or the shared backend.
- Emit through the existing native-x87 path; do not add a soft-float,
  emulator-protocol, or new x87 lowering path.

Gate: canonical MIR matches the object raiser, exact stored results match on
boundary probes, and the existing floating regression suite is unchanged.

Current vertical slice: `SIN`, `COS`, `TAN`, `ATN`, `SQR`, `ABS`, `INT`,
`FIX`, `SGN`, `LOG`, and `EXP` are inline; positive-constant-base `^` is
explicit `log2`, multiply, `exp2` HIR. An exact integral power-of-two base is
strength-reduced to its constant logarithm, so QB-Quake's pervasive `2 ^ i`
is only multiply plus `exp2`. The final QB-owned sequences are emitted only
after x87 stack allocation.
Negative or dynamically signed power bases still stop with a diagnostic until
the sign/integral-exponent control flow is represented; they are never silently
lowered through the positive-base identity.

Current production gate: all 17 qb-qrender modules reach the final staged
inline-x87 form. `DEF SEG`/`PEEK` share the external runtime `b$seg` cell, and
SINGLE-to-DOUBLE promotion aliases the existing extended80 evaluation value;
neither requires a runtime call or a new MIR/backend operation.

## Phase 5: numeric arrays and real-mode storage

- Implement fixed and dynamic numeric array declarations, `REDIM`, rank,
  bounds, and element typing.
- Implement logical descriptor/allocation identities and near/far/huge
  classification for each runtime profile.
- Lower native element accesses through the current array request, memory,
  and whole-pointer forms.
- Keep string arrays and unproved descriptor variants out of native lowering.
- Add alias, bounds, segment-crossing, and huge-pointer probes.

Gate: array MIR agrees with object-raised MIR on object identity, provenance,
bounds, stride, and pointer operations; linked programs cover both ordinary
and segment-crossing allocations.

## Phase 6: broad parser coverage through runtime calls

- Add the remaining QB/PDS/VBDOS syntax needed by real programs.
- Lower ordinary control constructs using current CFG operations.
- Map non-native statements and functions to audited runtime calls and link
  requirements.
- Keep parser acceptance and compilable-lowering status separately reported.
- Add an unsupported-lowering diagnostic for any construct whose semantics
  current MIR and call machinery cannot preserve.

Gate: the dialect corpus is truthful about accepted syntax, no feature widens
the native allowlist, and unknown effects remain conservative.

## Phase 7: QB Quake/qb-qrender bring-up

- Start with the VBDOS profile and its actual project/module organization.
- Compile one module at a time, reducing every newly found language feature to
  a small dialect or semantic fixture.
- Diff source-generated HIR/MIR stages against object-raised MIR for the same
  module where VBDOS can produce an object.
- Link against the existing VBDOS runtime and compare observable rendering and
  program behavior.
- Only after correctness, measure generated code against the project's
  hand-derived targets and optimization scoreboard.

Gate: the full program compiles, links, and behaves correctly; optimized code
uses the existing MIR passes and backend with no source-language special cases
inserted below HIR.

## Phase 8: WCC definitions and mixed-language build

- Add frontend-owned far-Pascal definition support required by all five
  qb-qrender C modules; do not encode call cleanup or parameter order as a new
  MIR property.
- Keep WCC capture as the initial C semantic oracle and compare its generated
  MIR with hand-derived kernels.
- Add `legacy`, `source`, and `compare` build modes to qb-qrender without
  changing the source tree during comparison.
- Preserve public names and the exact Borland/VBDOS inter-language ABI at
  every BASIC/C/ASM boundary.

Gate: the source-mode build consumes 17 new QB objects, five WCC-frontend C
objects, and the existing two ASM objects; public/fixup audits agree with the
legacy link and the executable runs the same tracked map and scripted input.

## Phase 9: code-quality and runtime gate

- Compare canonical optimized MIR exactly for equivalent QB/C numeric
  kernels, separately from ABI scaffolding.
- Compare ABI-normalized emitted code against the local GCC and LLVM builds as
  structural references and against WCC as the link-compatible baseline.
- Require generated kernel cost no worse than 1.05x the best compatible WCC
  reference and explain every remaining spill; do not treat a call, interrupt,
  or runtime helper as free work.
- Measure six interleaved DOSBox runs of the same qb-qrender scene and input.
  Require the source build's median frame time within 2% of the current
  optimized-object build before calling the performance goal met.

Gate: raw listings, bytes, stage dumps, scoreboard inputs, and run artifacts
all support the result; no conclusion rests on a single aggregate number.

## Phase 10: finish dialect/runtime coverage and assess reuse

- Complete the measured QB 4.5 and PDS 7.1 deltas and runtime profiles.
- Run the same source/object-MIR differential suite across every valid
  compiler/runtime configuration.
- Audit which HIR concepts were actually language-neutral.
- Only then design a WCC-capture-to-HIR adapter, if it can use the model
  without weakening C semantics or changing MIR.

Gate: future-language reuse is demonstrated by an adapter, not claimed from
abstract interfaces. Concepts used only by QB remain in the QB layer.

## Test ladder for every implementation change

Each issue found during implementation leaves a regression test that fails
without the fix. The smallest applicable ladder is:

1. lexer/parser accept-reject fixture;
2. resolved HIR dump;
3. verified HIR-to-MIR unit test;
4. canonical diff with object-raised MIR;
5. linked runtime output or exact bytes where semantics require it; and
6. full existing llrm regression and real-program matrix.

FreeBASIC contributes only `.bas` scenarios to this ladder. Its ABI, runtime,
lowering, emitted code, diagnostics, harness behavior, and expected outputs are
not inputs to the compiler. Every adapted case is re-established against QB,
PDS, or VBDOS before it becomes an expectation.

Stage dumps are written separately and adjacent stages are diffed. A wrong
final program is not diagnosed by reasoning backward from assembly when the
first divergent HIR/MIR dump can identify the boundary directly.

The current integration frontier is beyond full level and texture loading. The isolated dynamic
STRING-array, STRING-function-result, non-addressable BYREF STRING, and static
numeric-array descriptor rungs pass. Fresh COMMON plus fresh D_SURF in the
otherwise-BC image renders the five-frame QRender benchmark; independently,
replacing SCREEN alone now passes font and map loading and renders the same
153 polygons and 365 triangles. Earlier all-source stopping-point measurements
used broken descriptor lifetime rules and are no longer evidence. Continue
ordered module substitution from these passing images; do not reopen the
isolated string or static-array conclusions without contradictory raw output.
Re-linking the all-fresh image with the corrected SCREEN object first moved its
trace from `ugl` to `clip_nodes`. The subsequent `$DYNAMIC` fix gives bounded
numeric arrays mutable runtime descriptors, and the measured split-huge fix
loads owned-array selector `AD+2` plus offset `AD+0Ah`. The trace now includes
`textures`, `mapclose`, `surfcache`, `backbuf`, and `colormap`, with a complete
`LOAD.TXT`. A frame-path BC substitution completes five frames. Comparing its
VBDOS listing with fresh `R_BSP` exposed numeric formals using `AD+0` instead
of selector `AD+2` plus adjusted offset `AD+0Ah`; the general semantic fix is
now applied and the corrected all-fresh object set links. Confirm that fix at
the first-frame boundary next; do not return to entity or texture loading
without contradictory raw output.

Every production round records both linked BASIC-owned `BC_CODE` and complete
linked executable code (`BC_CODE + CODE`) from a successful MAP file. The
current descriptor-field-reuse image is 120,181 versus 76,616 bytes in the
first scope, and 292,210 versus 255,047 bytes in the complete scope.
Descriptor-base CSE and placement are therefore part of the existing-MIR
optimization work, not optional cleanup after correctness.

## Completion criteria

The architecture is complete when:

- the selected dialect parser is faithful to measured QB/PDS/VBDOS behavior;
- supported source builds HIR without p-code;
- LONG, floating/math, and numeric array semantics lower to the same existing
  MIR concepts as the object raiser;
- other facilities remain explicit runtime calls or precise unsupported
  diagnostics;
- emitted objects link against the selected legacy runtime in 386 real mode;
- QB Quake/qb-qrender compiles and behaves correctly; and
- its supported numeric kernels meet the 1.05x code-quality and 2% runtime
  gates above; and
- no change to the MIR model or backend was required.
