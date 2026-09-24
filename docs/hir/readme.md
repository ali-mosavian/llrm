# HIR architecture

Status: initial implementation. `src/hir/` contains the typed model, strict
versioned JSON codec, verifier, canonical MIR projection, and adapter to
existing MIR. `frontends/qb/` is now a real producer for numeric, control-flow,
string, file, array, and procedure slices; source reaches existing MIR without
p-code or a BC object. The QB adapter emits fresh BASIC-envelope OMF,
including procedure scaffolding, module headers, BASIC segments, and measured
error registration. All qrender source modules reach final staging, and fresh
`SYS.OBJ` and fresh `SCREEN.OBJ` mixed builds each link, render five frames,
and exit against the VBDOS/uGL runtime set. Whole-program substitution remains
frontend work.
Nothing here replaces MIR or changes the backend.

## Purpose

HIR is the narrow semantic boundary between a source-language frontend and
llrm's MIR. It exists to keep parsing, name resolution, source type
rules, and runtime ABI selection out of MIR while retaining enough information
for the existing optimization passes to see whole computations.

Its producers are the QB frontend and the llrm language frontend. The C
frontend raises the Open Watcom code generator's trees to MIR directly, and
the BC frontend raises machine code.

```text
QB/PDS/VBDOS source
        |
        v
  lexer and parser            dialect-specific syntax
        |
        v
  semantic analysis           names, types, storage, control flow
        |
        v
       HIR                    typed source semantics; no p-code
        |
        v
  QB HIR-to-MIR adapter       runtime ABI and target policy
        |
        v
  existing MIR               unchanged
        |
        v
  existing opt/lower/LIR/regalloc/emit
```

The object frontend remains alongside this path:

```text
BC-generated OMF -> decode -> raise -> existing MIR
```

That raiser is both a supported input path and the executable reference for
the new source frontend. The source path must produce equivalent MIR for the
same supported computation; it must not route source through BC, p-code, or
OMF.

## Fixed boundaries

HIR obeys these rules:

1. It is typed and name-resolved. Parser tokens and implicit-name lookup do
   not escape into HIR.
   `SUB` and `FUNCTION` calls name stable callable-table IDs; the table owns
   the linkage name, result type, parameter types/modes, array/SEG flags, and
   declaration-versus-definition state. HIR-to-MIR never repeats BASIC name
   lookup or reconstructs a procedure signature. Runtime intrinsics occupy a
   separate frontend registry because their measured effects are not source
   procedure declarations. That registry maps the spelling to one exact
   semantic-lowering variant during resolution; lowering dispatches on the
   variant and does not compare the intrinsic name again.
2. It preserves source evaluation order, storage identity, aliasing facts,
   conversions, and observable effects.
3. It is a control-flow graph, but it is not SSA. The existing MIR builder is
   responsible for SSA construction.
4. It names semantic operations, never x86 registers, instructions, calling
   sequences, OMF records, or byte encodings.
5. It may state target facts that affect meaning, such as near, far, or huge
   storage. Those are source/runtime properties, not selected machine
   instructions.
   Data symbols distinguish internal definitions from external declarations.
   An external place therefore retains both its storage identity and linkage
   name through HIR lowering; it is not disguised as absolute address zero or
   allocated in the source module.
6. Every HIR operation must lower to the current MIR vocabulary and metadata.
   If it cannot, the source frontend reports that construct as unsupported.
   Adding a MIR kind or changing lowering is outside this project.
7. It is not p-code. There is no postfix operand stack, scanner rewrite, or
   bytecode patching phase between the parser and HIR.

The current MIR boundary remains the one specified by
[split.md](../split.md): recognition belongs before optimization, every
optimization pass consumes and produces MIR, and machine requirements begin
only after MIR.

## LLVM is a reference, not an intermediate layer

The local LLVM checkout under `~/work/other/llvm-project` is the reference for
several proven separations:

- LLVM's data layout keeps sizes and alignment in one target description;
  HIR likewise refers to one layout/profile rather than scattering host-size
  assumptions through the frontend.
- LLVM address spaces distinguish pointers whose accesses have different
  rules; HIR preserves near, far, huge, and code address forms. It does not
  copy LLVM's default flat-pointer assumptions onto real mode.
- `getelementptr` separates address computation from memory access; HIR array
  and field addressing is likewise distinct from `LOAD` and `STORE`, then
  maps to the current MIR memory and `PTR_OFFSET` forms.
- LLVM intrinsics attach stable semantics to operations without exposing a
  selected instruction; the HIR native allowlist uses the same principle.
  Operations without a current MIR semantic form remain runtime calls.
- LLVM call memory effects and alias scopes show why effects must be explicit;
  HIR calls carry the existing llrm runtime contracts and logical storage
  provenance.
- LLVM's `alloca`-then-promotion architecture separates mutable source places
  from SSA values. HIR keeps source places, while the existing MIR builder
  performs the corresponding SSA construction.

The useful references are `llvm/docs/LangRef.md` (data layout, address spaces,
`getelementptr`, intrinsics, and `memory(...)`) and
`llvm/docs/GetElementPtr.rst`. LLVM IR itself is not introduced into the
pipeline: it has no native description of the BASIC runtime ABI or this
project's segmented 16:16 pointer semantics, and llrm already has the MIR and
backend that must remain fixed.

## Deliberately small common core

The common model contains only the concepts needed by the QB implementation:

- modules, procedures, symbols, types, constants, and source locations;
- basic blocks with explicit terminators;
- immutable expression results and mutable source places;
- loads, stores, conversions, numeric operations, calls, and array access;
- explicit call effects and runtime identities; and
- side tables for diagnostics, linking, and ABI facts.

There is no language plug-in registry, universal syntax tree, user-defined
rewrite language, or second optimization framework. Language-specific parse
trees and semantic bookkeeping stay in their frontend. A concept becomes
common only when a second frontend needs the same semantic fact without
changing its meaning.

## Reuse rule

Reuse means that another frontend can construct the same small set of typed
operations and lower through the same boundary. It does not mean erasing
language semantics. QB array descriptors, BASIC strings, and runtime calls
remain QB frontend or ABI-adapter concerns. C qualifiers and C object layout
would likewise remain C concerns.

The current WCC-capture frontend is not changed as part of this work. Once the
QB path has proved the HIR model, a separate project may translate its typed
capture into HIR. We do not make the unproved C adapter a prerequisite for QB.

### Relationship to the current WCC path

[`src/frontends/c/hir.rs`](../../src/frontends/c/hir.rs) is accurately named within that
frontend, but it is a capture of WCC code-generator calls: nodes retain names
such as `CGBinary`, WCC type codes, target flags, handles, and call classes.
Those are excellent evidence for the C adapter and should not become the
common language contract.

The QB design does reuse three successful WCC-path mechanisms:

- a frontend process is separated from the Python MIR/backend process;
- its complete output is inspectable and can be replayed without rerunning
  the source compiler; and
- the consumer refuses unknown records or semantics instead of guessing.

Unlike the `.cgs` capture, the QB process emits resolved common HIR rather
than a transcript of parser or code-generator calls. A later WCC adapter would
normalize `.cgs` into that same HIR; neither producer gets to expose its
private vocabulary as common operations.

## Replay tool

The process boundary is executable independently of a parser producer:

```text
python -m qbopt.hir unit.hir.json              # validate only
python -m qbopt.hir unit.hir.json --canonical  # normalized wire document
python -m qbopt.hir unit.hir.json --mir        # canonical semantic MIR dump
```

The decoder rejects unknown schema versions, fields, enum values, operand
tags, targets, types, and ill-typed memory operations. The adapter currently
exercises whole 32-bit arithmetic, explicit floating semantics, frame/global
places, fixed and dynamic numeric-array indexing, segmented pointer offsets,
typed calls, and CFG terminators. Its tests lower these paths through the
unchanged LIR boundary. `tools/qbstages.py` also writes every implemented
boundary adjacently so representation changes can be diffed rather than
inferred from final code. Its MIR projection is three-address text of the form
`c <- a op b`; LIR and all machine-pass projections are MASM/Intel listings,
using virtual `vN` operands until register allocation and physical register
names after it.

Runtime-specific array descriptor fields are resolved before this boundary.
For example, the QB frontend loads a numeric dynamic descriptor's split
selector and adjusted offset whether the descriptor is owned or received as a
formal, then emits the ordinary typed HIR `concat` and `ptr_offset` operations.
HIR therefore preserves the optimizer-visible huge pointer without acquiring
a Microsoft BASIC descriptor opcode or changing MIR.

## Documents

- [model.md](model.md) defines the entities, operation subset, verifier, and
  exact mapping to the existing MIR.
- [QB frontend](../frontend/qb/readme.md) defines the first producer.
- [QB real-mode memory model](../frontend/qb/memory-model.md) defines the
  target and runtime facts carried through the adapter.
- [QB implementation plan](../frontend/qb/plan.md) gives staged gates.
