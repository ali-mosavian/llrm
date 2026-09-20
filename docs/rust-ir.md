# Rust portable IR contract

## Purpose

This document defines the portable typed SSA representation used by the Rust
port.

The subsystem is named ir and its text format is .qir. It is constructed from
HIR, is consumed by analysis and transforms, and lowers into Machine IR owned
by codegen::machine. Machine IR serializes as .qmir.

The name MIR is reserved for Machine IR. It must not be used for the portable
SSA representation in new Rust code, diagnostics, or file formats.

The contract exists to make semantic optimization possible without importing
source bytes, object records, x86 registers, or instruction encodings into
the optimizer. It is a product contract, not a compatibility layer for the
old Python representation.

## Pipeline and ownership

The relevant pipeline is:

    frontend -> hir -> ir -> analysis + transforms
                                      |
                                      v
                              codegen::machine
                                      |
                                      v
                                  target::x86 -> mc -> object::omf

The arrows are dependency and data-flow directions, not permission for every
layer to inspect every earlier representation.

### Dependency rules

- ir may depend on support, but not on frontend, object, mc, codegen, or
  target.
- analysis reads ir and has no mutation API for it.
- transforms depends only on ir and analysis.
- codegen owns Machine IR, virtual registers, allocation, and frame
  construction.
- target::x86 owns x86 legality, register descriptions, ABI details,
  instruction selection hooks, and encoding constraints.
- mc owns symbols, expressions, fixups, fragments, layout, and encodings. It
  does not know SSA values or virtual registers.
- object::omf owns OMF parsing and serialization.
- driver is the only layer that assembles a complete compilation pipeline.

An architectural import test enforces these rules. Public types must also make
the boundary apparent: an ir instruction cannot contain a decoded instruction,
a register, a relocation, a machine address, an OMF record, or a target
description.

### What ir represents

ir represents portable program semantics:

- typed values and constants;
- functions, blocks, explicit control flow, and typed calls;
- source-semantic globals, arrays, places, and memory accesses;
- effects, observable traps, and behavior rules;
- optional diagnostic lineage.

ir does not represent:

- source byte ranges or decoded object instructions;
- OMF fixups, record positions, or archive membership;
- physical or virtual registers, register classes, frames, or spills;
- x86 addressing modes, instruction mnemonics, encodings, or branch ranges;
- target pointer widths, native aggregate layout, endianness, or ABI shuttles;
- allocation hints or post-allocation rewrite opportunities.

When a fact is needed by lowering but is not a portable semantic fact, it stays
in the appropriate frontend or target subsystem. It is not smuggled through an
untyped metadata field.

## Rust representation

ir uses owned Rust data, typed arena identifiers, exhaustive enums, and
deterministic storage.

~~~rust
struct Module {
    functions: IndexVec<FunctionId, Function>,
    globals: IndexVec<GlobalId, Global>,
    constants: IndexVec<ConstId, Constant>,
}

struct Function {
    signature: Signature,
    blocks: IndexVec<BlockId, Block>,
    instructions: IndexVec<InstId, Instruction>,
    values: IndexVec<ValueId, ValueData>,
}
~~~

The exact fields may change as the implementation learns, but these ownership
properties do not:

- FunctionId, GlobalId, BlockId, InstId, ValueId, and ConstId are distinct
  newtypes. A raw integer is never accepted where a typed identifier is known.
- A module owns its functions and globals. A function owns its blocks,
  instructions, and values. IDs are local to their documented owner unless an
  explicit module-level ID says otherwise.
- Instructions refer to operands through typed IDs and constants. They do not
  borrow one another or retain self-references.
- Deterministic index vectors are the default arena. Any map whose iteration
  can affect text, diagnostics, output, or pass choice is ordered or explicitly
  sorted.
- Names are optional diagnostic data. They are not identity, ordering, or
  semantic state.
- Values are owned records describing type and definition. Def-use links are
  derived analysis data, not mutable back pointers inside every value.

The implementation initially forbids unsafe. It also avoids Rc<RefCell<_>>,
global mutable contexts, pervasive cloning, and stringly typed opcodes. A
small interner or arena is permitted only when it establishes a real ownership
boundary and remains deterministic.

### Verified transactional editing

Published ir is verified. Analyses observe an immutable module or function.
Transforms may create a local mutable candidate through an editor, but the
candidate is not published until verification succeeds.

~~~rust
fn run_pass(
    input: &Function,
    analyses: &AnalysisManager,
) -> Result<EditedFunction, Diagnostic>;
~~~

An editor may mutate blocks, instructions, and value tables locally. Before it
commits, it validates the affected function and any required module invariants.
On an error, the original verified input remains the current artifact. This
gives transformations ordinary Rust ownership and local mutation without
shared mutable IR or partially committed invalid states.

The first implementation may use an owned replacement function instead of a
fine-grained undo log. It should optimize copying only after measurement shows
that it matters.

## Types, values, and constants

The first schema supports only types with an evidenced semantic consumer:

- signless fixed-width integers, including i1 for conditions;
- required floating formats;
- opaque pointers with a semantic address space;
- arrays, strings, and aggregate values when HIR lowering requires them;
- function signatures and typed results.

Integer signedness belongs to an operation or comparison predicate, not to the
integer type. A BASIC mask and an i1 condition are distinct values connected by
explicit conversion instructions.

Pointers have no portable numeric width. An address space records a semantic
region such as stack, global, or external only when that distinction affects
meaning or aliasing. Address spaces do not prove disjointness by themselves.
Alias rules must name the fact that proves disjointness.

Constants carry an explicit type. Integer constants normalize according to
their own integer type and bit width; floating constants preserve the required
semantic format. A symbolic address names an ir global or function, never an
object-file relocation location.

Every operand resolves to one of:

- a typed SSA value;
- a typed module constant;
- a typed global or function declaration where the instruction grammar permits
  it;
- a block or edge reference in a terminator or phi.

An unrecovered machine live-in is never an unknown value. Raising must make it
an explicit parameter, load, opaque-call result, or refusal. A non-parameter
value with no definition is a verifier error.

### Deliberately deferred type features

The initial contract does not reserve implementation work for vectors,
scalable vectors, atomics, or a universal token type. A later addition needs a
measured source semantic, a verifier rule, a text-format revision, interpreter
coverage where applicable, and a lowering plan.

There is no generic type or opcode plugin registry. Supported types and
instructions are closed Rust enums for a given .qir version. This keeps
construction, verification, printing, interpretation, and transformation
exhaustive.

## Instructions and behavior

Instructions use explicit enum variants with variant-specific data. They do
not use one all-purpose operation record containing many optional fields.

The initial families are introduced only when a lowering family needs them:

- constants, copies, phi, and select;
- integer arithmetic, bit operations, comparisons, and conversions;
- typed load, store, address calculation, and aggregate operations;
- direct calls, explicitly modeled intrinsics, and conservative opaque calls;
- branches, conditional branches, return, trap, and unreachable;
- required floating operations and conversions.

Multiple results are allowed when they state a real semantic relationship, for
example quotient and remainder. They are not a reason to introduce a general
tuple transport mechanism before a consumer exists.

Recognized runtime behavior is raised directly as semantic operations or a
closed intrinsic enum. A transform must not rediscover the meaning of a
runtime call by naming an external symbol, a register convention, or an x86
instruction pattern.

Calling conventions name concrete ABI behavior rather than a source language
or one runtime. The initial `far_pascal` convention means a far call with
left-to-right arguments and callee stack cleanup. Internal definitions and
external declarations use linkage to state which side supplies the body; IR
and Machine IR do not carry `basic`, `qb`, or `runtime` convention variants.

### No accidental undefined behavior

ir does not inherit undefined behavior, poison, or implicit target behavior
from another compiler IR. Each introduced operation specifies its observable
behavior:

- integer overflow, including wrapping, trapping, or an explicit language
  error;
- shift count behavior;
- signed and unsigned division rounding and exceptional cases;
- floating comparison and observable exception behavior;
- pointer conversion, comparison, and difference rules where supported;
- the failure behavior of trap and unreachable.

Optimization facts are distinct from behavior. A proved nonzero divisor or
non-overflowing addition may enable a rewrite. If a transform cannot preserve
the proof, it drops the fact; it may not turn an unproven case into arbitrary
behavior.

The raising frontend selects behavior from the source-language contract. For
example, a runtime operation with a named error remains modeled as such, while
a WCC operation is modeled only to the behavior that the supported capture path
can evidence. Unsupported behavior is diagnosed or refused explicitly.

Machine flags are not values in ir. A partial machine write becomes explicit
whole-value extraction, operation, and insertion unless raising proves the
source operations are one semantic operation.

## Effects, memory, and globals

Every instruction has an explicit effect description appropriate to its
variant. The initial effect vocabulary distinguishes at least:

- no observable effect;
- memory read, write, or conservative unknown access;
- control transfer;
- may trap or produce a named language error;
- external or input/output behavior;
- capture or escape facts when proved.

Effect queries such as removable, movable, duplicable, or speculatable derive
from the full description. A single barrier flag is not a substitute for
memory, control, trapping, and external behavior.

An externally called routine may have a complete evidenced summary or a
conservative summary. Empty effects never mean unknown effects. Imported
summaries identify the source of their evidence and are invalidated when that
evidence changes.

### Memory model

Address calculation and memory access are separate instructions. A source
memory arithmetic operation becomes load, arithmetic, and store in ir; x86
read-modify-write selection is a later codegen decision.

Objects, globals, places, and address paths represent source-semantic memory.
ir records typed access, required alignment, mutability, volatility when
semantically observable, and a semantic address space where needed. It does
not record base/index registers, legal addressing modes, frame offsets, or a
target-native pointer representation.

Conservative aliasing is correct by default. More precise alias results require
one named proof rule over objects, paths, capture, calls, or explicitly modeled
provenance. No family of pair-specific special cases is acceptable.

Byte-observable fixed encodings are exceptional. When a source format exposes
bytes, its encoding contract is explicit and includes byte order, scalar
encoding, pointer encoding where relevant, and invalid-decode behavior. The
portable interpreter must not silently model such an object as the host's
native byte layout.

Memory SSA, range analysis, and similar overlays are analyses. Ordinary ir
instructions do not carry a global memory token.

### Globals and calls

Globals have a type, linkage and visibility where semantic optimization needs
them, mutability, and an optional typed initializer. An absent initializer is
explicitly external or unknown according to the declaration; it is not assumed
to be zero.

Calls contain a typed callee, typed arguments and results, semantic calling
requirements, effects, and modeled exceptional behavior. ABI registers, stack
cleanup, sign-extension shuttles, clobbers, and frame setup are below ir in
target::x86 and codegen.

An unrecognized external operation has only two valid outcomes:

1. Raising reconstructs a complete typed interface and represents a
   conservative opaque call.
2. Raising cannot recover the interface and refuses the smallest closed unit
   that cannot be represented soundly.

Raw bytes, empty-argument calls, dangling values, and arbitrary opaque payloads
are not fallback representations.

## Control flow

Every block ends in exactly one terminator. Terminators own all successor
relationships. An ordinary returning call is not a terminator merely because
control transfers temporarily.

Phi inputs are explicit and typed. If two semantically distinct edges share a
source and target, the schema gives those edges stable local identities so a
phi can distinguish them. The simpler predecessor-block form remains
preferable when it represents the program without ambiguity.

An operation with a modeled normal and exceptional continuation is represented
as an invoking terminator. Values scoped to a continuation are imported through
that continuation's phi or block parameters, not by an implicit machine state.
A no-return call has no invented normal successor.

Exceptional, event, resume, and alternate-entry forms are added only when a
supported HIR family needs them. Once added, they receive the same verifier,
text, interpreter, and transformation treatment as ordinary control flow.

Executing a reachable trap or unreachable operation follows the explicit
behavior declared by the operation. It does not license arbitrary behavior.

## Textual .qir

.qir is a deterministic textual assembly of portable ir. It is intended both
for replay and for inspecting adjacent pass output.

Every file begins with a version directive:

    qir version 1

The grammar is defined by that version, not inferred from the implementation
revision. A format change that affects valid input, printed meaning, or public
tool behavior is versioned and receives the required compatibility decision.

The canonical printer provides:

- deterministic function, block, value, global, and declaration ordering;
- deterministic generated names and IDs;
- one readable instruction or CFG edge per line or indented logical unit;
- types at definitions and where an operand would otherwise be ambiguous;
- explicit non-default behavior and effect clauses;
- optional diagnostic lineage, omitted by default.

The parser and canonical printer round-trip a valid file deterministically.
Diagnostic-only metadata may be omitted from semantic comparisons, but semantic
attributes, effects, types, and behavior may not be lost. A normalized display
may alpha-rename internal values for debugging; it is a view, not a second
semantic representation.

The tools expose this through `llrm-opt`:

- --print-before and --print-after print adjacent pass stages;
- --stop-after stops at a named stage;
- --verify-each verifies after every pass;
- invalid text and semantic errors become diagnostic values printed by driver.

## Verifier

verify_ir is independent of frontend object side tables and target state. It
checks the features that the active schema version supports, including:

- every identifier resolves in its documented owner and namespace;
- every value, parameter, constant, global, instruction, and result is typed;
- each instruction variant has valid operands, results, behavior, and effects;
- every non-parameter SSA value has one definition;
- uses are dominated by definitions, including instruction ordering;
- phi inputs agree with predecessor relationships and types;
- every block has one terminator and terminators exclusively own CFG edges;
- function signatures, calls, results, and semantic calling requirements agree;
- memory accesses and globals obey their documented semantic contracts;
- effects are at least as conservative as the instruction requires;
- optional lineage references valid semantic entities but affects no behavior;
- no public ir data contains target facts, source bytes, decoded instructions,
  registers, relocations, object records, or untyped extension payloads.

Verification runs after HIR lowering and after every committed transformation.
The pass manager must not advertise a preserved property until its verifier has
established the property.

Invalid external interfaces and unsupported semantic forms are diagnostics or
explicit refusals. They are never silently lowered as an approximation.

## Focused interpreter

The interpreter is a small portable semantic oracle, introduced with the
initial standalone ir slice. It executes only supported .qir semantics and
returns an explicit unsupported diagnostic for all other forms.

It is not a replacement backend and does not model x86 registers, object
records, or host-native layouts. Its purposes are:

- validate arithmetic, control flow, memory, call, and failure semantics for
  supported operations;
- provide compact tests for transformations;
- compare HIR-to-IR lowering before Machine IR exists;
- make behavior differences visible before code generation obscures them.

Interpreter coverage grows with each introduced semantic family. It need not
execute the complete BASIC runtime before useful ir work begins.

## Analyses and transforms

Analyses are side-effect-free results over a verified ir revision. Initial
analyses are def-use, dominance, reachability, and loop information. The pass
manager owns caching and invalidation; analyses do not mutate functions to
cache their result.

A pass states the analyses and verified properties it requires, preserves, or
invalidates. A changed function receives fresh or correctly invalidated
results. Later analyses such as aliasing, memory dependence, value ranges, or
interprocedural summaries are introduced only when a consuming pass has a
measured need.

Transforms consume ir and analyses only. They do not receive a target, decoded
object node, byte range, allocator hint, emitter, or OMF record. They use the
transactional editor, verify before commit, and return diagnostics as values.

The intended pass order is delivered incrementally: simple scalar cleanup and
memory transformations first, then loop and interprocedural transformations
after their required analyses are stable. Pipeline ordering belongs to the pass
builder and driver, not to a dynamic plugin registry.

The only intentional extension points are:

- statically registered passes and analyses;
- target hooks below the ir boundary;
- object writers below the mc boundary.

This project does not build a plugin platform, a general table generator,
selectiondag, globalisel, or a required non-x86 backend.

## Boundary to Machine IR

codegen lowers verified portable ir to codegen::machine. Machine IR has its own
typed virtual registers, instructions, constraints, frames, and verifier. Its
text format is .qmir.

The lowering boundary may:

- map semantic operations to legal target::x86 operations;
- choose target data representations and addressing forms;
- lower semantic calling requirements to the x86 ABI;
- preserve semantic effects in machine-side memory and call records;
- report an explicit failure if a supported ir operation lacks a lowering.

It may not:

- reinterpret source bytes to decide ir meaning;
- treat an unsupported semantic behavior as a convenient machine instruction;
- invent undeclared inputs, outputs, effects, or control-flow edges;
- feed selected registers, encodings, or allocation outcomes back into ir
  transforms;
- mutate the already verified portable artifact.

Allocation, spilling, branch relaxation, x86 encoding, MC fixups, and OMF
writing are separate later steps. There is no general LIR optimization tier.
The one sanctioned exception is the explicit target peephole between allocation
and MC emission; it operates on physical Machine IR and does not reintroduce
portable-IR transforms after allocation.

## OMF provenance and rewrite accounting

OMF source-byte ownership is necessary for object rewriting but is not a
property of generic ir.

object::omf owns only OMF parsing and serialization. frontend::omf is the
BC-object semantic frontend and owns the rewrite ledger that accounts for
source bytes, fixups, symbols, entries, line records, relocation intent, and
refused units. The ledger is introduced with the BC-object frontend and is
audited transactionally against OMF input and output.

Diagnostic lineage may link an ir operation to a source occurrence for error
reporting. It is optional, many-to-many, and discardable. Removing lineage
cannot change behavior, byte ownership, relocation accounting, or emission.

Generic transforms do not return byte-disposition records and do not depend on
the rewrite ledger. QB and WCC source compilation use the same portable IR
without pretending that they own original object bytes.

For every pass edit, the transactional editor returns a small
target-independent change map. An empty map is valid when no source-derived
semantic identity is affected:

~~~text
old instruction or origin id -> retained | replaced(ids) | deleted | cloned(ids)
~~~

The map contains only stable semantic identities, never bytes, fixups, symbols,
addresses, or OMF records. Every retained, replaced, deleted, or cloned
source-derived identity is represented when present. Source compilation may
ignore the map. frontend::omf consumes it with its external ledger to account
for replacements, deletions, and clones while proving byte, fixup, symbol, and
entry ownership. Diagnostic lineage remains optional and cannot substitute for
this map.

No decoded instruction or arbitrary raw byte sequence may appear as an ir
instruction merely to keep an unsupported object region alive. The OMF path
refuses the smallest sound closed rewrite unit instead.

## Delivery and acceptance

The contract is delivered in palpable verified iterations.

### Initial HIR and IR milestones

1. Implement Rust HIR, its verifier, and deterministic .qhir replay.
2. Implement exact OMF parsing and untouched byte-identical round trips in
   object::omf.
3. Introduce the direct Rust ir core: typed IDs, a small scalar schema,
   verifier, versioned .qir parser/printer, and focused interpreter.
4. Lower supported HIR families into verified ir, adding behavior, effects,
   CFG, globals, arrays, strings, floating semantics, and exceptional forms
   only with their consumer and tests.
5. Add pass infrastructure, def-use, dominators, loops, and `llrm-opt`.

No temporary Python-MIR adapter is introduced. The Rust IR is constructed
directly, and Python optimizer development remains frozen except correctness
fixes during the port.

### Code generation and optimization milestones

6. Implement Machine IR, .qmir, target::x86 lowering, MC, and OMF emission.
7. Complete unoptimized Rust QB code generation, including allocation and
   required ABI, floating, pointer, string, and frame behavior.
8. Port scalar and memory transforms, then loop and interprocedural transforms,
   only after their analyses and verifier rules are ready.
9. Add the BC-object and WCC capture frontends, then complete driver cutover.

### Evidence required at each applicable milestone

- architectural import checks enforce subsystem direction;
- .qir parser/printer round trips are deterministic;
- invalid IR cases fail verification with useful diagnostics;
- interpreter tests establish supported semantic behavior;
- --verify-each validates transformed artifacts;
- stage dumps identify the first differing adjacent stage;
- focused tests are selected before broad suites, and verification execution is
  kept within the iteration time budget;
- untouched OMF input round-trips byte-identically;
- the OMF ledger accounts completely for bytes, symbols, entries, and
  relocations only in the object-rewrite path;
- behavior and refusal semantics match the established oracle;
- optimized generated code has no quality regression, even when byte identity
  is not expected.

The final cutover additionally requires green Rust tests and retained external
harnesses, no production Python dependency, no hidden unchanged fallback, and
the production driver and tools operating through the Rust pipeline.

## Deferred choices

The following are intentionally not designed before their first real consumer:

- exact aggregate representation and operations beyond HIR needs;
- fixed-layout byte contracts for object-derived data;
- floating environment state and observable deferred exceptions;
- event and resume entry forms;
- stronger provenance and alias refinements;
- Memory SSA and interprocedural effect summaries;
- vector, scalable-vector, atomic, and ordered-resource semantics;
- additional targets or target-independent backend implementations.

For each, the implementation proposal must state the source behavior it
preserves, the verifier rule, the .qir version impact, interpreter strategy,
lowering owner, focused regression, and any new architectural dependency.

Keeping these decisions deferred is deliberate. The contract should be small
enough to implement, strict enough to reject unsound fallback, and open only at
the pass, analysis, target-hook, and object-writer boundaries that the port
actually requires.
