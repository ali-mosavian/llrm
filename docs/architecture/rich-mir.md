# Rich portable MIR

Status: steps 0 and 1 landed (`tools/baseline.sh`, `crates/llrm-mir`); the
compiler still runs on the old MIR.

This document defines a replacement for llrm's public MIR.  It is closely
modelled on LLVM IR while remaining faithful to llrm's measured semantics and
object-rewriting requirements.

The design is based on qbopt revision
`03d444841715d9b4c3f342f4ab0e170e053fa960` and the local LLVM checkout at
`~/work/other/llvm-project`, revision
`338e0c94943a6fb917c276bbbd9ff4b6cd6dd71e`.

This is a MIR design.  LIR appears only where necessary to define the lowering
contract.  It is not a proposal to redesign the allocator or reproduce all of
LLVM GlobalISel.

## Decision

llrm MIR becomes a self-contained, typed, portable SSA representation of the
program:

- a module owns declarations, globals, functions, semantic runtime intrinsics,
  and externally fixed object layouts;
- every value has exactly one type at its definition;
- every instruction has one opcode, one ordered operand list, typed results,
  semantic attributes, effects, and diagnostic lineage;
- every function has explicit parameters, returns, blocks, terminators, and
  exceptional/event edges;
- memory is expressed through pointer SSA, explicit loads/stores, canonical
  objects, access paths, and provenance;
- integer overflow, shifts, division, floating evaluation, failures, calls,
  and volatile behaviour have explicit contracts;
- source byte ownership, relocations, allocation history, and decoded machine
  instructions remain outside MIR.

The design goal is stronger than making the dataclasses resemble LLVM.  A MIR
module must be sufficient to execute or lower the supported program without
consulting the original instruction stream.

## Portability contract

A completed MIR module can be handed unchanged to any backend whose capability
set implements the module's types, address spaces, effects, and semantic
intrinsics.  A backend may reject an unsupported capability; it may not
reinterpret it.

MIR contains no:

- target triple, CPU, feature bit, instruction, mnemonic, or addressing mode;
- physical or virtual register, register class/bank, flags register, or
  calling-convention location;
- pointer width or near/far/segmented physical representation;
- target legality, instruction cost, schedule, encoding, or relocation kind;
- byte offset into the input object or decoded instruction object;
- target-specific intrinsic disguised as a generic operation.

The compilation request supplies a target only when MIR is lowered.  No MIR
analysis or optimization receives the target description.

Some input objects have externally fixed memory layouts.  MIR may describe
field offsets, extents, and alignment when those are observable program facts.
That source-object layout is distinct from an output target data layout and
does not prescribe an addressing mode.

The boundary is mechanical:

- core MIR modules cannot import decoder, object-address, target, or backend
  packages;
- public MIR datatypes contain no untyped `object` escape hatch;
- serialized MIR has no target section;
- a boundary test rejects nested machine objects, not merely forbidden names
  spelled directly in a pass;
- the same normalized MIR must execute under a MIR interpreter and at least
  one deliberately non-x86 test backend for the capabilities it supports.

The current strict rule in `docs/architecture/split.md` remains in force until the new MIR,
lowering boundary, and verifiers exist.  The eventual replacement rule is
“portable facts only in MIR,” not “there can be no useful work after MIR.”

## Why the current MIR is too small

The present representation contains much of the information needed by the
optimizer, but it represents one fact several ways and still relies on the
source machine form for answers.

- `Value` has identity and version information but no type.  Width is repeated
  by `Held`, `Const`, `MemRef`, and LIR.
- `Op` describes dataflow through overlapping `args/results`, `uses/defines`,
  `loads/stores`, `merges`, and `exits` fields.
- `Op.kind` coexists with a decoded `ir.Operation`, an instruction name, and
  selected-machine fallback semantics.
- `MemRef.addr` can contain `objectfile.module.Addr`, whose base and segment
  are physical registers.
- `Opaque.what` can hold arbitrary machine state.
- partial writes, flags, call completeness, floating origins, and volatility
  are separate exceptions rather than instances of types and effects.
- an undefined value may be treated as a caller input instead of being rejected
  as a lost definition.
- modules, declarations, globals, signatures, attributes, and semantic
  intrinsics are not one coherent IR object graph.

Adding more optional fields to `Op` would make these contradictions worse.  The
replacement needs a type and instruction system, not a larger record.

## LLVM mechanisms to use

| LLVM mechanism | llrm decision |
| --- | --- |
| `LLVMContext`, `Module`, `Function`, `BasicBlock` | Adopt the ownership hierarchy with immutable llrm objects. |
| Every `Value` has a `Type` | Adopt.  Types belong to definitions, never independently to uses. |
| Signless integers and signed/unsigned opcodes | Adopt. |
| Opaque pointers distinguished by address space | Adopt.  Physical pointer representation is chosen below MIR. |
| Typed constants, globals, function declarations, and attributes | Adopt with llrm semantic contracts. |
| Uniform instruction operand traversal | Adopt.  One ordered operand list and typed result list are authoritative. |
| SSA, phi nodes, explicit terminators, dominance verification | Adopt. |
| Intrinsics with semantic declarations | Adopt for recognized BASIC runtime behaviour. |
| Alias analysis, MemorySSA, dominators, loops, SCEV, known bits | Adopt as analyses over MIR, not fields copied into every instruction. |
| Function/call attributes such as readonly, nocapture, noreturn | Adopt only as proved or conservatively imported facts with evidence. |
| Metadata and debug lineage | Adapt.  LLVM metadata is not sufficient for llrm's byte-conservation ledger. |
| Intrusive mutable `Value`/`User`/`Use` lists and RAUW | Reject.  Bodies stay immutable; def-use is a rebuilt analysis. |
| Poison, general `undef`, and UB-producing `nsw`/`nuw` | Reject.  llrm uses explicit behaviour and proved optimization facts. |
| Target triple and `DataLayout` embedded in semantic MIR | Reject.  The lowering request owns target layout. |
| `ConstantExpr` as a second hidden instruction language | Reject.  Nonliteral computation is an ordinary instruction. |
| LLVM's single-result restriction | Adapt.  llrm keeps first-class multiple results where they express the operation directly. |
| LLVM exception-handling personalities and landing pads | Adapt to explicit BASIC error, event, resume, and normal edges. |

## Ownership hierarchy

```python
@dataclass(frozen=True)
class MirContext:
    types: TypeInterner
    constants: ConstantInterner
    intrinsic_registry: IntrinsicRegistry

@dataclass(frozen=True)
class MirModule:
    id: ModuleId
    declarations: FrozenMap[FunctionId, FunctionDeclaration]
    globals: FrozenMap[GlobalId, GlobalObject]
    functions: FrozenMap[FunctionId, MirFunction]
    fixed_layouts: FrozenMap[LayoutId, ObjectLayout]
    semantic_policies: ModulePolicies

@dataclass(frozen=True)
class MirFunction:
    id: FunctionId
    name: str
    type: FunctionType
    parameters: tuple[Value, ...]
    entries: tuple[FunctionEntry, ...]
    blocks: FrozenMap[BlockId, MirBlock]
    local_objects: FrozenMap[ObjectId, MemoryObject]
    attributes: FunctionAttributes
    properties: frozenset[MirProperty]

@dataclass(frozen=True)
class FunctionEntry:
    id: FunctionEntryId
    kind: EntryKind
    target: BlockId
    arguments: tuple[Value, ...]
    resources: tuple[Value, ...]
    primary: bool = False

@dataclass(frozen=True)
class MirBlock:
    id: BlockId
    instructions: tuple[MirInstruction, ...]
```

Each block contains zero or more phi/resource-phi instructions, ordinary
instructions, and exactly one final terminator.  Successors are derived from
the terminator rather than stored as another editable truth.

`FunctionEntry` names a normal, event, resume, or alternate entry block and the
typed values/resource states supplied on its synthetic entry edge.  Exactly one
entry is primary.  Multiple entries are therefore visible to dominance and SSA
rather than being hidden layout seeds.

`MirContext` interns immutable structural objects for compactness and stable
printing.  Pointer identity is not part of semantics.

## Type system

```python
type MirType = (
    VoidType | IntType | FloatType | PointerType | VectorType |
    ArrayType | StructType | FunctionType | TokenType
)

@dataclass(frozen=True)
class IntType:
    bits: int

@dataclass(frozen=True)
class FloatType:
    format: FloatFormat

@dataclass(frozen=True)
class PointerType:
    address_space: AddressSpace

@dataclass(frozen=True)
class VectorType:
    element: IntType | FloatType | PointerType
    elements: int
    scalable: bool = False

@dataclass(frozen=True)
class ArrayType:
    element: MirType
    elements: int

@dataclass(frozen=True)
class StructType:
    fields: tuple[MirType, ...]
    name: str | None = None

@dataclass(frozen=True)
class FunctionType:
    parameters: tuple[MirType, ...]
    returns: tuple[MirType, ...]
    variadic: bool = False

@dataclass(frozen=True)
class TokenType:
    resource: ResourceKind
```

### Integer types

Integers are signless.  Signedness belongs to operations such as `sdiv`,
`udiv`, `sext`, `zext`, and signed or unsigned comparison predicates.  `i1` is
the logical condition type.  BASIC's `-1`/`0` mask is produced by an explicit
`bool_to_mask`, not by pretending a branch condition is a 16-bit integer.

There are no implicit width changes.  Truncation, sign extension, zero
extension, bitcast, extraction, insertion, concatenation, and split/merge are
distinct operations with checked type rules.

### Floating types

Floating type names exact semantic formats: binary32, binary64, extended80,
and any later evidenced format.  Equal storage size does not make two formats
equivalent.

The type is distinct from evaluation policy, but the value's type always names
the format actually carried between instructions.  A binary32 operand may be
extended and evaluated in extended80, but the arithmetic result is then an
extended80 SSA value.  An explicit rounding conversion produces binary32 at a
materialization point.  An ordinary binary32 value never secretly contains an
unrounded wider result.

### Pointer types

Pointers are opaque and carry only an address space.  They have no MIR bit
width, segment register, pair layout, or near/far encoding.  Address spaces are
semantic domains: for example, an externally fixed object space may obey
different observable pointer rules from a transient local allocation space.

Address space is not alias identity.  Two spaces are disjoint only when one
named rule proves that property.

### Aggregates and vectors

Arrays, structs, and vectors are part of the schema from the start.  Initial
migration may construct only scalar SSA values, but SROA, ABI values,
initializers, and future vectorization do not need an opaque payload or a new
type system later.

Aggregate operations are explicit: `extract_value`, `insert_value`,
`extract_element`, `insert_element`, `shuffle`, and aggregate constants.
Aggregate memory layout is an `ObjectLayout`, not an accidental consequence of
the output target.

## Values, constants, and uses

```python
@dataclass(frozen=True)
class Value:
    id: ValueId
    type: MirType
    variable: VariableId | None = None
    version: int | None = None

type Operand = (
    ValueRef | ConstantRef | BlockRef | EdgeRef | FunctionRef | GlobalRef
)

type Constant = (
    IntegerConstant | FloatConstant | NullPointer | AggregateConstant |
    SymbolAddressConstant
)
```

Every use refers to a definition, a function parameter, or an explicit module
constant.  “No definition found” is always a verifier failure.  ABI live-ins
are translated into explicit parameters while raising; they are not inferred
from the original register in which BC happened to leave a value.

Integer constants normalize modulo their bit width.  Floating constants retain
exact bits and format.  Aggregate constants recursively contain typed
constants.  Symbol addresses name semantic globals or functions, never OMF
fixup locations.

Unknown external values are explicit function parameters, load results, or
opaque-call results.  There is no unknown/undef constant which can stand in for
an unrecovered machine input.

Def-use links are not stored inside `Value`.  `DefUse` is an immutable analysis
indexed by `ValueId`, rebuilt or incrementally derived for a new body.

## Instruction model

```python
@dataclass(frozen=True)
class MirInstruction:
    id: InstructionId
    opcode: MirOpcode
    results: tuple[Value, ...]
    operands: tuple[Operand, ...]
    attributes: AttributeSet
    effects: EffectSummary
    lineage: tuple[OccurrenceId, ...]

@dataclass(frozen=True)
class OpcodeSpec:
    name: str
    operand_schema: tuple[OperandSpec, ...]
    result_schema: tuple[ResultSpec, ...]
    properties: frozenset[OpcodeProperty]
    type_rule: TypeRule
    default_effects: EffectRule
```

The ordered operand list is the only use list.  The typed result list is the
only definition list.  Calls, loads, stores, terminators, and multi-result
operations use the same model.

Opcode definitions are the authority for construction, verification, printing,
operand traversal, constant folding, value numbering, and default effects.
There is no all-purpose `Op` with dozens of optional fields and no decoded
instruction fallback.

First-class multiple results are intentional.  `divrem`, overflow-reporting
arithmetic, compare-and-value operations, and resource-token updates can state
their result directly rather than packing an artificial struct or using a
hidden field.

### Core opcode families

The schema vocabulary contains:

- values: `constant`, `copy`, `phi`, and `select`;
- integer: add/sub/mul, signed/unsigned div/rem, divrem, negation, bitwise
  operations, shifts, rotates, min/max, carry/borrow and overflow forms;
- comparison: integer predicates returning `i1`, boolean operations, and
  `bool_to_mask`;
- conversion: trunc/zext/sext, integer/float conversions, float extend/truncate,
  bitcast, extract/insert bits, concatenate and split;
- aggregates/vectors: extract/insert value or element, shuffle, splat;
- pointer: object address, global address, function address, pointer offset,
  field/element address, pointer difference, address-space conversion, checked
  integer/pointer conversion;
- memory: load, store, memset, memcpy, memmove, lifetime start/end, allocation
  and deallocation;
- floating: arithmetic, comparison, conversion, rounding, transcendental
  intrinsic, environment read/write, and check;
- calls: direct, indirect, intrinsic, checked/invoking call;
- control: branch, conditional branch, switch, return, unreachable, trap;
- resources: resource entry, resource phi, resource barrier.

Recognizing a BC runtime idiom still belongs in the raise.  `B$MUI4` is a
semantic `mul` or multiply intrinsic the moment MIR is produced, not an opaque
call which an optimization pass rediscovers.

## Arithmetic semantics

MIR never imports LLVM undefined behaviour accidentally.  Operations state
their actual behaviour:

```python
@dataclass(frozen=True)
class IntegerBehavior:
    overflow: OverflowBehavior       # wrap, trap, saturate
    shift_count: ShiftCountBehavior  # checked, masked, modulo-width
    division: DivisionBehavior       # rounding and exceptional cases

@dataclass(frozen=True)
class OptimizationFacts:
    no_signed_overflow: bool = False
    no_unsigned_overflow: bool = False
    exact: bool = False
    nonzero: bool = False
```

Behaviour is semantic.  Optimization facts are separately proved annotations.
Using a fact may justify a rewrite; violating it never creates poison or makes
the whole program arbitrary.  A transform either proves and preserves a fact
or drops it.

This is necessary for:

- BC division by zero and minimum-integer divided by `-1`;
- C division where those cases are outside the preserved contract;
- shift instructions which mask their count;
- wrapping multiplication;
- widened operations whose value agrees while their source flag observations
  do not.

Partial writes are whole-value operations.  A genuine low-half update is
`extract_bits`, arithmetic on the extracted value, and `insert_bits` into the
preserved whole value.  If the raise proves two word operations form one LONG
operation, MIR contains the single `i32` operation instead.

Carry and borrow are ordinary `i1` results and operands.  No MIR value means
“whatever the machine flags currently contain.”

### Required semantic policy tables

An opcode is supported only when its policy is one of the interpreter-tested
cases in the module's versioned semantic registry.  The first implementation
must define, rather than leave implicit:

- shift counts as `EXACT`, `MODULO(n)`, `MASK(mask)`, or
  `TRAP_IF_OUT_OF_RANGE`;
- division quotient rounding and separate outcomes for zero divisor and
  minimum-signed-value divided by `-1`: a named runtime error, named hardware
  trap, specified return value, or refusal to raise;
- overflow as wrap, trap, saturate, or a separately proved impossible case;
- pointer difference as same-object element/byte difference with an explicit
  failure outcome for unrelated objects;
- integer/pointer conversion width, round-trip guarantee, provenance result,
  and invalid fixed-representation decode behaviour;
- floating comparison behaviour for NaNs and every observable exception.

`unreachable` is not LLVM undefined behaviour.  Executing it produces the
named `ImpossibleControlReached` trap.  A pass may create it after proving an
edge impossible, but the proof does not license arbitrary behaviour if the
assumption is later violated.

The schema reserves aggregate, vector, scalable-vector, atomic, and advanced
floating opcodes.  A separate support matrix lists which type/opcode/policy
combinations the raiser, interpreter, optimizer, verifier, and lowering all
implement.  “Present in the enum” is not the same as supported.

## Memory, objects, and pointers

Pointer SSA is the address language.  An arithmetic operation never hides a
memory operand: a source memory add becomes load, add, store.  Lowering can
select a read-modify-write instruction later.

```python
@dataclass(frozen=True)
class MemoryObject:
    id: ObjectId
    kind: ObjectKind
    layout: ObjectLayout
    generation: GenerationId | None
    extent: ObjectExtent
    mutability: Mutability
    linkage: Linkage | None

@dataclass(frozen=True)
class MemoryAccess:
    kind: AccessKind
    value_type: MirType
    extent: TypedExtent | FixedByteExtent
    alignment: Alignment
    volatility: Volatility
    atomicity: Atomicity
    provenance: ProvenanceId | UnknownProvenance
    access_path: AccessPath | None
```

`TypedExtent` derives storage size during lowering from the portable type and
target layout.  `FixedByteExtent` is allowed only when an externally observable
source-object layout fixes the byte count.  A fixed-layout pointer field is
decoded or encoded explicitly; its bytes are never assumed to be the output
target's native pointer representation.

A target-layout object is accessible only through symbolic typed field or
element paths.  Its native padding, byte order, pointer bytes, and field offsets
are unobservable in MIR.  Byte offsets, bytewise copies, type punning, pointer
differences involving representation size, or character views require an
explicit `FixedLayout`/encoding contract.  Such a contract states byte order,
padding, integer encoding, pointer-field encoding, and the failure behaviour of
decoding an invalid representation.

The MIR interpreter models target-layout objects as typed cells keyed by
symbolic access path, not as an arbitrarily sized native byte array.  Alias
analysis treats two typed paths in the same object according to their
structural relationship; if layout-independent disjointness cannot be proved,
they may alias.  Fixed-layout objects use their declared byte intervals.  This
prevents a pointer store and byte-offset load from becoming disjoint on one
backend but overlapping on another without MIR recording that distinction.

`Provenance` retains and consolidates the current object/slice/restrict model:

```python
@dataclass(frozen=True)
class Provenance:
    slices: frozenset[ObjectSlice]
    restrict: Restriction
```

An unknown provenance is explicitly unknown.  Object kind, address space,
generation, access path, and range are separate facts with one definition
each.  This replaces `MemRef.addr`, `space`, `allocation`, `symbolic`,
`beyond`, `excludes`, `within`, `typed`, and `pointer` as overlapping answers.

Pointer operations have semantic contracts for wrapping, bounds, comparison,
and address-space conversion.  They do not describe base/index registers or
legal x86 addressing forms.

MemorySSA is a cached analysis overlay, as in LLVM.  It provides MemoryUse,
MemoryDef, and MemoryPhi relationships without forcing every ordinary
instruction to carry a global memory token.

## Effects

Every instruction has one effect summary, supplied by its opcode and refined
by its occurrence:

```python
@dataclass(frozen=True)
class EffectSummary:
    memory: tuple[MemoryEffect, ...]
    resources_read: frozenset[ResourceKind]
    resources_written: frozenset[ResourceKind]
    control: ControlEffect
    traps: TrapSet
    captures: tuple[CaptureEffect, ...]
    completeness: EffectCompleteness
    evidence: EffectEvidence | None
```

Queries such as pure, removable, speculatable, duplicable, and movable are
derived from this contract.  They are not aliases for one `barrier` flag.

`COMPLETE` is a trusted statement produced by raising or evidenced
interprocedural analysis.  Structural verification cannot discover an omitted
effect in an external routine.  Imported summaries therefore record the
runtime/library/object identity from which they came and are invalidated when
that identity changes.

`CONSERVATIVE` means required unknown memory, capture, trap, control, and
resource effects are present.  An empty tuple never means “unknown.”

Volatility, atomic ordering, synchronization scope, trapping, and external
visibility are distinct.  The design does not make `volatile` carry all of
those meanings.

## Functions, calls, and intrinsics

A function declaration contains:

```python
@dataclass(frozen=True)
class FunctionDeclaration:
    id: FunctionId
    name: str
    type: FunctionType
    semantic_convention: SemanticCallingConvention
    attributes: FunctionAttributes
    effects: EffectSummary
```

`SemanticCallingConvention` describes portable language/runtime obligations,
not registers, stack offsets, push order, or cleanup instructions.  Lowering
maps it to a target ABI.

For an existing object, `TargetQualifiedAbiBindings` is a separate hard
interface map keyed by function, external symbol, and entry point.  It records
the evidenced target convention for arguments, returns, preserved state, frame
establishment, and cleanup.  It is not an allocation hint.  A semantic integer
widening is an explicit MIR conversion; ABI-only `signext`/`zeroext` placement
belongs in this binding map.

A call contains a callee operand, complete typed arguments, typed results,
semantic convention, attributes, and occurrence effects.  ABI shuttles and
clobber registers do not exist in MIR.

Recognized runtime routines use a versioned intrinsic registry.  Each intrinsic
declares its signature, effects, exceptional behaviour, and semantic policy.
The original external symbol remains diagnostic/source provenance, not the
operation's meaning.

Raising an unrecognized external operation has exactly two outcomes:

1. A complete typed value interface is recoverable.  MIR represents a portable
   opaque external call with every input and result explicit and all unknown
   effects conservative.
2. The interface is incomplete.  The enclosing function is atomically refused
   and preserved outside optimizable MIR.  If alternate entries, cross-function
   transfers, shared inline data, or relocations make the function an open
   ownership unit, refusal expands to the smallest closed unit containing all
   such transfers.

The second case never leaves an unknown byte region embedded between optimized
blocks: without a complete typed value/control boundary that would not be a
valid MIR function.  It also never enters MIR as an empty-argument call,
dangling value, arbitrary `Opaque` payload, or raw instruction.

Useful function/call attributes include `noreturn`, `readonly`, `readnone`,
`nocapture`, `noalias`, `nonnull`, `dereferenceable`, alignment, parameter
extension, and tail-position eligibility.  They are accepted only when proved
or tied to invalidatable evidence.

## Control flow and observable failures

Every block has exactly one terminator.  Terminators own all successors and
edge kinds:

```python
class EdgeKind(Enum):
    NORMAL = auto()
    ERROR = auto()
    EVENT = auto()
    RESUME = auto()

@dataclass(frozen=True)
class MirEdge:
    id: EdgeId
    source: BlockId | FunctionEntryId
    target: BlockId
    kind: EdgeKind
    arguments: tuple[ValueRef, ...]
    resources: tuple[ValueRef, ...]
```

Each terminator owns immutable `MirEdge` records.  Edge identity matters:
distinct switch or exceptional edges may have the same source and destination.
Phi is an ordinary instruction placed first in a block; each incoming operand
is `(EdgeRef, ValueRef)` and has exactly the result type.  Resource phis use the
same edge identities.  A block's phi inputs and incoming edges agree exactly.

An ordinary call does not end a block merely because it calls and returns.  A
call or arithmetic operation with modeled exceptional successors is instead
an invoking/checked terminator, exactly as an LLVM invoke is both a returning
call and a terminator.  Its normal result values exist only on the normal edge;
error values and resource state are declared separately on each exceptional
edge.  Successor phis are the only way to import those edge-scoped values into
ordinary block SSA.  A no-return call has no invented normal edge.

Normal, event, resume, and alternate `FunctionEntry` records create synthetic
entry edges with explicit parameter/resource arguments.  These edges make
dominance and phi construction well-defined even for a block which execution
can enter without passing through the primary entry.

Unhandled or deliberately unpreserved behaviour is an explicit `trap` or an
operation whose trap contract states the limitation.  It is not implicit
machine behaviour discovered after lowering.

Dominance, post-dominance, reachability, loops, and control dependence derive
from this CFG.  Calls and software interrupts are not terminators merely
because they transfer control temporarily.

## Floating point and ordered resources

```python
@dataclass(frozen=True)
class FloatBehavior:
    evaluation: FixedEvaluation | EnvironmentControlledEvaluation
    rounding: RoundingMode
    exceptions: ExceptionBehavior
    contraction: ContractionMode
    storage_rounding: StorageRounding
```

`EnvironmentControlledEvaluation` preserves the existing dynamic-precision
case.  Its result still has an explicit carrier format—normally extended80—
while the environment controls the significand precision used to produce that
value.  Constant folding and interpretation therefore need both the result
type and current environment.  Storage rounding is explicit at stores,
conversions, calls, and other semantic materialization points.

Where the floating environment or deferred exception is observable, an
operation consumes and produces an `FP_ENV` resource token.  Calls which can
inspect or change it have explicit token inputs/results; `fp_check` consumes
the state it observes.  No token is created where order is unobservable.

Resource joins use a dedicated `resource_phi`.  This is a llrm extension,
not a claim that LLVM permits token phi nodes.  Normal, error, event, resume,
loop-back, call, and return edges carry their resource state explicitly.
Resource tokens cannot be stored, spilled, converted to integers, or returned
as source-language values.

The same mechanism can represent another small, genuinely ordered runtime
resource later.  It is not used as a universal memory dependency; MemorySSA
does that job.

## Globals, initializers, and linkage

Globals are first-class module objects:

```python
@dataclass(frozen=True)
class GlobalObject:
    id: GlobalId
    type: MirType
    address_space: AddressSpace
    linkage: Linkage
    visibility: Visibility
    mutability: Mutability
    initializer: ConstantRef | None
    alignment: Alignment
    fixed_layout: LayoutId | None
```

Initializers are typed constant trees.  Relocatable addresses name functions
or globals symbolically.  Linkage and visibility support whole-module proofs
without asking OMF records repeatedly.

Aliases and externally supplied storage are explicit declarations.  A missing
initializer means externally initialized or unknown according to the
declaration; it does not silently mean zero.

## Metadata, lineage, and source-byte ownership

MIR metadata has two categories:

1. **Semantic attributes** affect correctness and are typed fields verified as
   part of the instruction or declaration.
2. **Discardable metadata** supports diagnostics, profiling, source lines, and
   optimization remarks.  Removing it cannot change program behaviour.

Diagnostic lineage is many-to-many:

```python
@dataclass(frozen=True)
class DiagnosticLineage:
    operation: SemanticOperationId
    occurrences: tuple[OccurrenceId, ...]
```

Folding unions lineage; every clone may retain it; deletion needs no surviving
instruction.  MIR cannot inspect an occurrence to recover bytes, addresses,
registers, or decoded operations.

Correctness never depends on lineage.  Every transform returns a mandatory
`TransformChange` mapping stable old semantic operation IDs to replacements,
deletions, clones, and moved landing points.  The rewrite ledger consumes that
record transactionally.  Lineage may annotate a change for diagnostics but may
be stripped without affecting byte ownership or emission.

Exclusive original-byte ownership is a separate rewrite ledger:

```python
@dataclass(frozen=True)
class OccurrenceDisposition:
    occurrence: OccurrenceId
    state: Disposition       # retained, replaced, deleted, data, refused
    replacement: ReplacementId | None
```

The ledger also maps external labels, public symbols, alternate/event entries,
LINNUM entries, and fixup targets to their valid replacements.  Relocation
intent is separate again.  No live MIR instruction must survive merely to own
deleted bytes.

Unsupported raw regions live in the ledger/source map and bypass optimizable
MIR atomically.  Arbitrary source bytes never enter a MIR instruction.

## Analyses enabled by the richer MIR

The initial analysis layer should expose LLVM-like reusable results:

- `DefUse`: definitions, users, replacement maps;
- dominators, post-dominators, reachability, control dependence;
- loop forest, natural-loop form, induction recognition, trip counts;
- scalar evolution over integer and pointer recurrences;
- value ranges, known bits, demanded bits, congruences;
- alias analysis from objects, provenance, access paths, capture, and calls;
- MemorySSA and memory dependence;
- object escape/capture and observer analysis;
- call graph, SCCs, function summaries, mod/ref and return facts;
- liveness and pressure estimates over abstract values;
- profile/block-frequency information as discardable metadata;
- semantic equivalence/value numbering keys derived from opcode schemas.

Analyses are immutable products keyed by body identity.  A transformed body
does not mutate or partially preserve an old result.  A pass declares which
analyses and verified properties it requires, preserves, or invalidates.

## Transform contracts

Every transform receives MIR and returns MIR plus a mandatory change summary.
It may not
receive the target, decoded nodes, allocation hints, raw byte ranges, or the
emitter.

```python
@dataclass(frozen=True)
class TransformResult:
    module: MirModule
    changes: tuple[TransformChange, ...]

@dataclass(frozen=True)
class TransformChange:
    old: SemanticOperationId
    replacements: tuple[SemanticOperationId, ...]
    disposition: ChangeDisposition   # retained, rewritten, cloned, deleted
```

Stable semantic IDs and this record update the rewrite ledger.  They are not
discardable metadata.

A transform declares:

- required and preserved `MirProperty` values;
- required and preserved analyses;
- whether it changes CFG, types, effects, objects, or diagnostic lineage;
- its proof rule for replacing operations;
- whether it can clone or delete source-derived computation.

Useful properties include:

- `TYPED`;
- `SSA`;
- `CFG_COMPLETE`;
- `EFFECTS_CONSISTENT`;
- `LINEAGE_VALID`;
- `LOOPS_SIMPLIFIED`;
- `LCSSA`;
- `NO_UNSUPPORTED_REGIONS` where applicable.

The rich MIR is the home of constant folding/propagation, branch decision,
GVN/CSE, DSE, LICM, SROA and promotion, algebraic simplification, induction
variables, loop transforms, inlining, IPSCCP, private-object elimination, and
semantic call/intrinsic optimization.

Target-width legalization, instruction idioms, register banks/classes,
addressing modes, ABI moves, spills, scheduling, and encodings remain below
MIR.  A later generic-machine tier may still perform GlobalISel-style
legalization and low-level combines over virtual registers.  This document
does not constrain that tier beyond requiring it to consume the complete MIR
contract rather than reconstruct semantics from x86 or source bytes.

## Textual MIR

MIR text is designed for reading stage diffs first and serialization second.
It does not copy LLVM's `%`, `@`, `^`, repeated type, and metadata punctuation.

The normal form follows these rules:

- values, functions, globals, blocks, and edges use ordinary names; grammar
  position identifies their namespace;
- a value's type is printed at its definition and not repeated at every use;
- constants inherit the expected operand type, with `0:i16` syntax only when
  inference would be ambiguous;
- comparisons and control flow use readable words rather than encoded
  predicates such as `icmp.sgt` and `br`;
- default attributes and empty effects are omitted;
- short non-default attributes use one bracketed clause;
- longer effects or policies use an indented `with` block;
- diagnostic lineage, internal IDs, analysis results, and source-byte ledger
  state are hidden unless explicitly requested.

The same example becomes:

```text
module example

declare printI32(value: i32)
    effects io

global count: i32 = 0 [internal, mutable]

function step(amount: i32, address: ptr(frame)) -> i32
entry:
    old: i32 = load address [align 2, memory frame.x]
    next = add.wrap old, amount
    store next to address [align 2, memory frame.x]
    positive = compare.signed next > 0
    if positive goto yes else no

yes:
    call printI32(next)
    return next

no:
    return 0
end
```

The syntax still has a one-to-one instruction model.  `compare.signed` defines
the `i1` value `positive`; the `if` consumes it.  The printer does not combine
those instructions merely to look like source code.

Phi inputs name edges in a vertically readable form:

```text
join:
    result: i32 = phi
        from leftEdge: leftValue
        from rightEdge: rightValue
    return result
```

When two edges share the same source and target, the terminator names them:

```text
dispatch:
    switch key
        case 1 goto shared as firstCase
        case 2 goto shared as secondCase
        default goto failed

shared:
    selected: i32 = phi
        from firstCase: firstValue
        from secondCase: secondValue
```

Exceptional control flow uses the same named-edge form:

```text
entry:
    invoke quotient: i32 = divide.checked numerator, denominator
        normal goto continued as divided
        error divideByZero goto failed as zeroError

continued:
    value: i32 = phi from divided: quotient
    return value
```

Detailed semantic policies remain readable without crowding the operation:

```text
quotient: i32 = divide numerator, denominator
    with
        signed
        rounding towardZero
        divideByZero runtimeError(11)
        overflow return(minSigned)
```

The printer provides three views over the same MIR:

| View | Purpose | Includes |
| --- | --- | --- |
| `normal` | stage dumps and code review | semantic instructions and non-default contracts |
| `full` | serialization and forensic debugging | stable IDs, all semantic attributes, lineage references, and discardable metadata |
| `normalized` | structural comparison | normal form with deterministic alpha-renaming and no discardable metadata |

`normal` is still parseable and lossless for program semantics.  `full` is
lossless for the complete in-memory artifact.  Printing a parsed `full` file is
byte-stable; printing a parsed `normal` file is semantically stable after
canonical formatting.

Generated values use a short deterministic fallback such as `value7` only
when no semantic name is available.  Names never encode source offsets or
physical registers.  The full view prints internal IDs in a separate trailing
annotation rather than making them the value's visible name.

`tools/stages.py` writes one MIR file after every pass.  The first differing
adjacent pair remains the primary debugging evidence.  It uses `normal` by
default and can emit `full` beside it when a provenance or ledger investigation
needs the extra detail.

## MIR verifier

The verifier checks at least:

- all IDs are unique in their namespace;
- every parameter, value, constant, global, and instruction is typed;
- opcode signatures and attribute schemas hold;
- every non-parameter value has exactly one definition;
- every use is dominated by its definition, including instruction order;
- phi/resource-phi inputs exactly match predecessor edge identities and types;
- every block ends in one terminator, terminators exclusively own CFG edges,
  and edge-scoped results are used only through their edge and target phis;
- every normal/event/resume/alternate entry has one synthetic entry edge with
  complete typed value and resource arguments;
- normal, error, event, resume, and alternate-entry rules hold;
- function declarations, calls, arguments, results, and semantic conventions
  agree;
- integer, pointer, aggregate, vector, and floating operations obey their
  policies;
- typed/fixed memory extent, object layout, access path, alignment, address
  space, volatility, and atomicity are internally consistent;
- every byte-observable object declares byte order, padding, scalar and pointer
  encoding, and invalid-decode behaviour;
- effect summaries contain opcode-required effects; conservative summaries
  contain explicit unknowns; imported complete summaries identify evidence;
- resource tokens have valid single-threaded flow and joins;
- diagnostic lineage references valid occurrences but owns no bytes;
- MIR contains no register, register class/bank, mnemonic, encoding,
  relocation, source byte address, decoded instruction, `objectfile.Addr`, or
  target-only intrinsic;
- no general `object` field can smuggle one of those into a public datatype.

Verification runs after raising and every transform in production.  A property
is not attached until its complete verifier checks pass.

`verify_mir(module)` is independent of source byte side tables.  A separate
`audit_rewrite(module_ids, source_map, rewrite_ledger)` checks transform-change
coverage, occurrence disposition, landing points, relocations, and transaction
atomicity.  This lets portable MIR be parsed, interpreted, and optimized with
all diagnostic metadata removed.

## Lowering contract

The entire backend-facing contract is:

```python
def lower(
    module: MirModule,
    target: TargetDescription,
    source_map: SourceMap,
    rewrite_ledger: RewriteLedger,
    abi_bindings: TargetQualifiedAbiBindings,
    allocation_hints: TargetQualifiedAllocationHints,
) -> LirModule:
    ...
```

Lowering may:

- map portable types and address spaces to low-level target representations;
- legalize operations into supported widths and forms;
- implement semantic intrinsics with instructions or compatible library calls;
- lower semantic conventions to ABI locations and clobbers;
- select addressing modes and machine instructions;
- transfer MIR effects and provenance to machine memory/effect records;
- publish stable emitted replacement IDs so the rewrite ledger can resolve
  mandatory transform-change records independently of diagnostic lineage;
- use allocation hints only when their target identity matches.

ABI bindings are mandatory compatibility obligations.  Lowering refuses a
symbol whose target binding is missing or inconsistent; it never substitutes
the default convention.

Lowering may not:

- consult original bytes to decide what a MIR operation means;
- reinterpret an unsupported semantic policy as a convenient target operation;
- invent an undeclared input, result, effect, or fall-through edge;
- mutate MIR or feed selected machine facts back into a MIR pass.

The exact internal shape and phase split of LIR is deliberately outside this
design.  The acceptance test is that the rich MIR contains enough information
for lowering to proceed without its old `ir.Operation`, `node`, `name`, or
machine-address fallbacks.

## Migration

The migration keeps the compiler runnable and output-stable.  Each issue found
gets a fail-first symptom regression in the same commit as its fix.

### 0. Freeze the baseline

- Record normalized MIR after every existing pass and final allocated output.
- Preserve byte-identical untouched OMF round trips separately from semantic
  reconstruction.
- Record linked answers, final assembly, structural metrics, and per-CPU costs.

`tools/baseline.sh` records every MIR, LIR and assembly stage of the OMF
corpus and the Nib examples; while final assembly is unchanged, so are costs.
The round trip is `test_round_trip_is_byte_identical` and
`test_every_mapped_fixture_round_trips_byte_identical`; answers are
`tools/e2e/e2e.py`.

### 1. Land the typed shell and its instruments

- Introduce `MirContext`, the first scalar types, typed constants/values,
  signatures, explicit parameters and returns, stable semantic IDs, and one
  small opcode family.
- Land the verifier, deterministic printer/parser, normalized stage dump, and
  a focused interpreter for that slice immediately.
- Make an undefined non-parameter use a verifier error.
- Add a temporary adapter from new MIR operations into the existing backend so
  the compiler remains runnable before lowering is ported.
- Derive compatibility widths from types temporarily; never infer a type from a
  consumer's guessed width.

No old field is deleted in this step.

Landed as `crates/llrm-mir`, which has no dependencies: MIR cannot name a
decoder, object file or target, and a test keeps it that way. The adapter and
compatibility widths wait for the first producer in step 3, since nothing
emits the new MIR yet. The `full` view waits for lineage in step 2.

### 2. Separate lineage from byte ownership

- Make diagnostic lineage many-to-many and discardable.
- Introduce mandatory `TransformChange` records.
- Move retention, replacement, deletion, refusal, landing points, and
  relocation intent into an independent rewrite ledger.
- Prove that cloning and DCE no longer require live instruction anchors.

This happens before opcode migration changes transformation identity and before
new DCE/CSE can duplicate the old ownership coupling.

In the old MIR, `Op.id` is each operation's own identity: unique in its body,
assigned by the pass manager (`mir::identified`) after the raise and after
every pass, and excluded from equality. `Op.source` is the provenance a copy
shares: the raise-time operation whose relocations, node and site it
re-emits. `mir::transformed` identifies a pass's output and diffs it against
its input into `TransformChange` records (rewritten, cloned, deleted,
inserted); `transform::recorded` returns them per stage with the body, the
unroll, peel and unswitch drivers included.

No pass sees a tombstone. At each pass boundary `mir::transformed` strips
them and records where their bytes land: before the next operation still in
the block, chained as that operation is deleted or moves, or where a vanished
block's control or last operation went (`mir::Ledger`). `transform::applied`
puts them back before the backend runs.

In LIR the byte markers are meta instructions (`Insn::is_meta`), as LLVM's
debug instructions are: they take no slot, and no pass counts, windows or
stops on them. `LLRM_STRIP_META` drops them at every LIR phase boundary;
the code must come out the same, as LLVM's must with and without `-g`.

### 3. Add modules, declarations, globals, and intrinsics

- Build `MirModule` for both object and C paths.
- Publish complete declarations, signatures, entries, and ABI binding keys.
- Convert recognized runtime calls to versioned semantic intrinsics during
  raising.
- Add globals, typed initializer trees, linkage, mutability, and fixed layouts.

### 4. Replace `Op` family by family

- Introduce `MirInstruction`, `OpcodeSpec`, ordered operands, typed results,
  attributes, and default effects.
- For each family, update raising, the verifier/interpreter, every optimizer
  consumer, the temporary old-backend adapter, and focused tests together.
- Make phi, edges, entries, and terminators schema-checked instructions/data.
- Replace partial writes and flags with explicit values.

`lower.named`, `lower.semantics`, `lower.rewritten`, `lower.current`, and every
pass which reads an old field are explicit cutover checklist items.  Delete
`uses/defines`, `args/results`, `loads/stores`, `merges`, `flags`, `name`, or
decoded `op` for one family only after both optimizer and backend consumers of
that family have migrated.  `Held.width` and `Const.width` follow the same
rule.

### 5. Replace `MemRef`

- Publish pointer SSA, memory objects, layouts, access descriptors, paths, and
  canonical provenance.
- Migrate alias analysis, MemorySSA, GVN, DSE, LICM, promotion, and SROA.
- Remove `objectfile.module.Addr` and every physical address spelling from MIR.

The evidence that this stage landed is deletion of special-case memory fields,
not another combination arm.

### 6. Complete effects and control flow

- Replace barrier/completeness booleans with `EffectSummary`.
- Add explicit error, event, resume, alternate-entry, and no-return edges.
- Add semantic call attributes and invalidatable summary evidence.
- Refuse incomplete external interfaces before MIR construction.

### 7. Complete floating and resource semantics

- Move current floating format, precision, rounding, and exception facts into
  types and `FloatBehavior`.
- Add explicit storage rounding, FP environment tokens, checks, calls, and
  resource joins.
- Verify dynamic-precision and exceptional paths before enabling movement.

### 8. Finish pass and analysis cutover

- Port the remaining passes one at a time against verifier-backed textual
  fixtures and interpreter oracles.
- Add the full reusable analysis layer only as its consumers migrate.
- Keep representation-only changes separate from optimization-policy changes
  unless the old form cannot express the correct rule.

### 9. Cut lowering over

- Make lowering consume only the new MIR plus explicit side tables.
- Remove the temporary adapter only after complete output parity.
- Remove MIR backreferences to decoded operations and source addresses.
- Delete compatibility constructors and the old MIR in one bounded cleanup.
- Update `docs/architecture/split.md`, `docs/architecture/mir-vocabulary.md`, and architecture diagrams.

## Acceptance gates

Every migration step must pass all applicable gates:

1. Untouched OMF read/write remains byte-identical for every compiler and flag
   configuration.
2. Optimization-disabled decode/raise/lower/allocate/emit/link/run preserves
   program answers independently of the untouched round trip.
3. Until an optimization is separately proposed and measured, optimized final
   code and answers remain unchanged.
4. Each corrected symptom has a test observed failing before the fix.
5. Each corrected instrument has its own fail-first regression.
6. The first changed adjacent MIR stage identifies the responsible phase.
7. MIR verifies after raising and every pass over the complete corpus.
8. A MIR interpreter and non-x86 test backend execute the same supported MIR
   against the same oracle.
9. Focused matrices cover checked failures, zero/overflow division, event and
    resume paths, unknown calls, dynamic floating state, resource joins,
    aggregates, non-default address spaces, and fixed-layout pointer encoding;
    each cell records raiser, verifier, interpreter, optimizer, and lowering
    support or an explicit refusal.
10. Boundary mutation tests inject registers, machine addresses, mnemonics,
    target intrinsics, source offsets, and decoded nodes into MIR and observe
    every rejection.
11. The rewrite ledger accounts for every source byte, fixup, external landing
    point, and refused transaction exactly once.
12. Cost and structural reports are independently derived and representative
    final assembly is inspected before claiming improvement.
13. The final quality gate remains `docs/measurement/targets.md`; richer MIR is enabling
    infrastructure, not evidence of better generated code by itself.
14. `full` MIR text round-trips byte-stably; `normal` round-trips semantically;
    normalized before/after fixtures keep one operation or edge per readable
    line and omit diagnostic noise.  Representative complex dumps are reviewed
    directly rather than accepting parser tests as evidence of readability.

## Resolved choices

- MIR is the rich portable IR; LIR redesign is not the subject of this plan.
- Integers are signless; operations state signedness and behaviour.
- Conditions are `i1`; BASIC masks are explicit conversions.
- Pointers are opaque address-space values; representation begins below MIR.
- Aggregates and vectors exist in the type system from the start.
- Multiple instruction results are first-class.
- There is no LLVM poison or general `undef`.
- Unknown external interfaces refuse the smallest closed function/ownership
  unit rather than leaving an opaque region inside MIR.
- Terminators own identified edges; phi and resource-phi inputs name edges,
  not only predecessor blocks.
- Target-layout objects have symbolic typed storage; byte observation requires
  an explicit fixed encoding.
- MemorySSA is an analysis; resource tokens model only genuinely ordered
  non-memory state.
- MIR is immutable; def-use and other analyses are body-scoped products.
- Semantic attributes and discardable metadata are different systems.
- Diagnostic lineage and original-byte ownership are different systems.
- Hard ABI bindings and soft allocation hints are different side tables.
- The target is supplied only to lowering, never to MIR optimization.

The result is an LLVM-like MIR in the sense that matters: a typed module of
values, instructions, memory, effects, calls, globals, and control flow with
strong verification and reusable analyses.  It remains llrm's IR because its
integer, floating, runtime, event, provenance, and object-rewriting contracts
come from measured program behaviour rather than LLVM's language assumptions.
