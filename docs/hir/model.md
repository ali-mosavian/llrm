# HIR model

This is the minimum HIR required by the QB source frontend. Names below are a
schema, not a commitment to a particular host-language class hierarchy.

## Ownership and identity

```text
Program
  Module*
    Type*
    Symbol*
    Global*
    Function*
      Block*
        Instruction*
        Terminator
```

Every entity has a stable integer identity local to its owner. Human-readable
names and source spans are annotations, not identity. Dumps and a future wire
format must be deterministic so that two frontend paths can be compared after
renumbering identities.

A `Program` also owns target and runtime profiles. For the QB frontend these
include the 386 real-mode target and one of the QB 4.5, PDS 7.1, or VBDOS
runtime ABIs. It also records array order because BC's `/R` changes address
semantics independently of both dialect and runtime. These facts are not
inferred from individual instructions.

## Types

The initial common type set is:

- signed integers of 16 and 32 bits;
- floating storage types `single` and `double`, with a separate evaluation
  semantics field where extended evaluation is required;
- booleans represented according to the source language's truth convention;
- fixed and dynamic strings as opaque runtime-managed types;
- records with named, laid-out fields;
- fixed and dynamic arrays with rank, element type, and bounds;
- procedures with parameter modes and a result type; and
- near, far, and huge pointer forms only where source or runtime semantics
  expose them.

The distinction between a stored `single`/`double` and its evaluation format
is mandatory. It maps to MIR's existing floating semantics rather than being
flattened into an imprecise `float` type.

String types are present so the frontend can resolve programs and call the
runtime correctly. This design does not add native string operations to MIR.
Stored dynamic strings are four-byte runtime descriptors. A string-valued
runtime expression is deliberately a different HIR value: a near pointer to
such a descriptor, matching the measured AX result of `B$LDFS`, `B$FMID`,
`B$FCHR`, and the trim/case functions. This prevents a width-based call-result
adapter from inventing an AX:DX return. When a fixed-string operation requires
the runtime's far descriptor spelling, the frontend derives DS from a static
data anchor, projects the near descriptor offset, and constructs a typed 16:16
pointer before ABI physicalization.

## Values and places

An instruction may produce an immutable `Value`. A `Place` identifies mutable
source storage and can be loaded, stored, addressed, or passed by reference.
Places include:

- local and parameter slots;
- static locals and module variables;
- `COMMON` or external objects;
- a record field of another place; and
- an array element described by an array object plus typed indices.

A place carries logical provenance and declared storage class. It does not
carry a chosen segment register, frame register, or x86 addressing mode.
Aliasing is derived from object identity, slices, escape, and call effects;
it is never guessed from a numeric address.

An indirect place may additionally be `volatile`. This means the pointed-to
source object is externally published and can change without an ordinary
store visible in the current function. It is an access property, not a
machine address-space or register property. HIR-to-MIR lowering preserves it
on both the `MemRef` and the operation, so value numbering cannot reuse the
read and loop motion cannot move it out of its source loop.

Initialized data relocations distinguish near offsets, complete far/huge
pointers, code identities, and a segment-selector word. The last is required
by real-mode runtimes: it is not an integer constant and cannot be recreated
from an offset after linking. VBDOS static strings use it to keep one selector
cell in near constant data while character payloads remain in private far
constant data.

This split lets semantic analysis represent BASIC assignment directly while
the adapter constructs the existing SSA MIR. HIR itself does not need phi
nodes or an SSA repair pass.

## Control flow

A function contains basic blocks. Instructions in a block execute in source
order. Each block ends in exactly one of:

- unconditional branch;
- conditional branch;
- integer switch;
- return; or
- unreachable/trap when the current MIR already has an equivalent path.

Calls are ordinary instructions and do not end a block merely because they
transfer control temporarily. Non-local BASIC behavior must be expressed by
the frontend using existing branches and calls; HIR does not introduce a
second exception or coroutine system.

Conditional successor order is semantic: target zero is taken when the HIR
condition is true and target one when it is false. A frontend must encode any
source-profile branch rule by choosing those successors before MIR. For
example, VBDOS `/O` lowers a top-level control `NOT` by testing its operand and
exchanging the successors; it does not first materialize the integer
complement. A `NOT` buried below another operator is materialized and triggers
VBDOS's separate successor-exchange quirk. The QB frontend records either rule
as explicit HIR edge order. Optimizers and machine lowering therefore see
ordinary control flow and need no source-language exception.

## Instructions

The initial instruction families are intentionally close to source semantics:

- constants and copies;
- load, store, and address-of a place;
- integer and floating conversions;
- signed integer arithmetic, bit operations, and comparisons;
- floating arithmetic, comparison, negation, absolute value, and square root
  where already represented by MIR;
- array bounds/descriptor queries and array element access; and
- direct or indirect calls with typed arguments, parameter modes, effects,
  and an optional result.

Evaluation order is explicit in instruction order. An instruction whose
result is unused may still remain because of trapping, floating-environment,
memory, or call effects.

### Native expansion allowlist

Only these QB semantic families are expanded into native MIR operations:

| Family | HIR meaning | Existing MIR destination |
|---|---|---|
| 16-bit and `LONG` math | whole typed arithmetic and comparison | existing integer `Kind` operations, including 32-bit operations and `DIVMOD` |
| floating-point math | operation plus evaluation and rounding semantics | existing floating `Kind` operations and `FloatingSemantics` |
| numeric arrays | descriptor/bounds/address computation and typed element access | existing `ArrayRequest`, `MemRef`, `ADDRESS`, `PTR_OFFSET`, `LOAD`, and `STORE` machinery |
| recognized numeric runtime helpers | the arithmetic meaning already recognized by the object raiser | the same native MIR form produced by that raiser |

“Math” includes named HIR intrinsics even where current MIR has no distinct
kind. Such an intrinsic remains a visible computation, never a runtime call.
The QB frontend carries its name conservatively through MIR and expands its
physical x87 sequence only after stack allocation; shared MIR and backend
vocabulary remain unchanged.

All other BASIC facilities, including strings, files, graphics, events, and
most library functions, are represented as calls with audited runtime
contracts when possible. An unaudited call is conservative about memory and
effects. If the current MIR/call representation cannot preserve a construct's
semantics, compilation stops at the adapter with a source diagnostic.

## HIR-to-MIR contract

The adapter consumes a verified HIR function and constructs the existing MIR
without changing its model:

| HIR fact | Current MIR representation |
|---|---|
| signed 16-bit or 32-bit value | existing constants/values with the corresponding width |
| integer arithmetic or comparison | existing integer `Kind` |
| floating operation | existing floating `Kind` and `FloatingSemantics` |
| local/global/field place | canonical object, slice, provenance, and `MemRef` from the current memory model |
| volatile indirect place | the same typed `MemRef`, marked volatile on the memory occurrence and operation |
| numeric array access | current array request/access raising result and `PTR_OFFSET` where required |
| direct runtime operation not expanded natively | `CALL` plus the existing runtime contract and call-memory facts |
| source block/terminator | existing MIR block, branch, switch, and return forms |
| source span and declared name | diagnostic/debug side table; never optimization semantics |
| module/runtime/link requirement | link-plan side table consumed after MIR; not a new MIR operation |

The adapter should reuse the policies already implemented by the object
raiser, notably:

- [`raising_calls.py`](../../qbopt/frontend/raising_calls.py) for recognized
  long arithmetic calls;
- [`raising_longs.py`](../../qbopt/frontend/raising_longs.py) for whole LONG
  values;
- [`raising_floats.py`](../../qbopt/frontend/raising_floats.py),
  [`raising_float_calls.py`](../../qbopt/frontend/raising_float_calls.py), and
  [`raising_float_values.py`](../../qbopt/frontend/raising_float_values.py)
  for floating semantics; and
- [`raising_arrays.py`](../../qbopt/frontend/raising_arrays.py) and
  [`raising_array_access.py`](../../qbopt/frontend/raising_array_access.py)
  for array identity and access.

“Reuse” first means factor or call the same semantic policy, not copy it into
a parallel implementation. The byte-pattern recognition portions remain
specific to the object frontend.

## Side tables

Information that is necessary for compilation but not for MIR optimization is
kept beside HIR/MIR:

- source map and expansion provenance;
- selected dialect, runtime, and compiler-option profile;
- public/external symbols and required runtime modules;
- procedure ABI and parameter passing;
- module header, startup, and OMF emission requirements; and
- diagnostic names and original spellings.

The implemented call side table names each call instruction, its source-to-
stack argument permutation, near/far distance, and caller/callee cleanup. A
procedure has the corresponding entry convention and parameter-byte count.
Typed operands stay on the semantic call while MIR optimization runs. The QB
adapter then replaces them with the existing MIR `ARG` operations and a call
contract immediately before machine lowering. Consequently optimizers see
argument values and alias provenance, while neither MIR nor the backend gains
a BASIC calling-convention field.

Keeping these facts outside instruction operands prevents link format and
runtime-family details from leaking into general optimization passes.

## Verification

HIR is verified before MIR construction. The verifier checks at least:

- all referenced identities belong to the enclosing program/function;
- every block has one terminator and every target exists;
- instruction/result and load/store types agree;
- conversions are explicit;
- places have complete storage and layout information;
- call arity, parameter modes, return types, and effects match the selected
  ABI contract;
- array rank, index types, and near/far/huge classification are known;
- floating evaluation semantics are explicit;
- no parser marks, unresolved names, p-code opcodes, registers, x86
  instructions, or OMF records occur; and
- every instruction has a defined mapping to current MIR.

The last check is the scope firewall: an attractive source feature cannot
silently force a MIR or backend extension.

## Serialization and dumps

The Rust QB frontend and Python MIR adapter use a deterministic, versioned JSON
document per compilation unit. JSON is chosen for the first implementation
because it is dependency-light, diffable, and replayable; it is not an API
promise that prevents a later encoding change.

The document contains an explicit major schema version and tagged entities
from this model. The Python consumer rejects an unknown version, tag, required
field, type, or operation. It never supplies a target-dependent default for an
omitted fact. Both the Rust producer and Python consumer run structural and
semantic verification so a serialization bug is localized at the process
boundary.

The canonical textual HIR dump is a projection of this document with stable
ordering and incidental identities renumbered. Each compiler stage writes its
own dump so source HIR and object-raised MIR can be compared at the first
boundary where they diverge.
