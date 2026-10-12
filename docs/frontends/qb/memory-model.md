# 386 real-mode memory and runtime model

Audience: contributors to `qbfront` and the QB HIR-to-MIR adapter. The address
spaces, storage classes, floating-point rules and frame layouts the QB frontend
relies on.

The target is Intel 386 or newer executing 16-bit real-mode DOS code. The 386
minimum permits 32-bit integer instructions and operand/address-size
overrides. It does not turn DOS memory into a flat address space.

Ordinary data segments and DGROUP remain 64 KiB resources. A 32-bit `LONG`
or address-size override does not widen a near pointer, increase a segment
limit, or authorize unreal-mode addressing. Module statics, descriptors,
runtime bookkeeping, and near allocations therefore need an explicit group
budget; data that cannot fit uses the runtime's far/huge allocation path.

## Address spaces

| Form | Meaning |
|---|---|
| near | 16-bit offset interpreted in a known segment or group |
| far | 16:16 segment:offset pointer naming one object/access path |
| huge | 16:16 pointer whose arithmetic normalizes and wraps for objects crossing segment windows |
| code | far procedure identity or code relocation used by the runtime/linker |

A far or huge pointer is not a flat 32-bit integer. Huge pointer arithmetic
does not become packed `ADD`: the adapter emits the whole-pointer and
`PTR_OFFSET` MIR form, and the backend performs the selector/offset lowering
described in [pointer-lowering](../../machine/pointer-lowering.md).

No frontend operation selects `DS`, `SS`, `ES`, `BP`, `SI`, or any other
register. Segment selection and addressing modes are backend work.

Field projection through a dynamic array keeps the whole pointer and byte
displacement as HIR `ptr_offset`; it is not the address of an indirect
whole-pointer cell. Dynamic string descriptors are the exception. The QB string
arena is near, so `B$SASS`, `B$FLEN`, and `B$SCMP` receive the projected 16-bit
descriptor offset.

## Logical storage objects

Source storage is classified before HIR emission as one of: automatic
procedure local or parameter; static procedure local; module variable or
constant; `COMMON`/shared or external object; record object or field slice;
array descriptor; array allocation; opaque runtime-owned storage.

An object has stable identity, extent when known, alignment, mutability,
escape, and an address form. Fields and array elements are slices of that
object, not unrelated numeric addresses. The adapter maps these facts into the
memory model in `crates/ir/llrm-mir/src/memory.rs`.

Near-versus-far by itself does not prove two accesses disjoint, and equal
offsets in different objects are not the same storage. Raw segmented memory
(`DEF SEG`, `PEEK`, `POKE`), and addresses passed to unknown code, escape the
object and force conservative alias and call effects. Unequal segment:offset
spellings are never proof that two physical addresses differ.

## Runtime profiles

QB 4.5, PDS 7.1, and VBDOS are distinct ABI profiles; `llrm` is QB 4.5's
frames and descriptors called by the target's own convention
([dialects](dialects.md)). A profile owns procedure entry/exit and frame
layout, near/far and callback conventions, argument order and `BYREF`/`BYVAL`,
return locations, dynamic string and array descriptor layout, module header,
startup, event and error integration, external symbol spelling, and the
floating-point library linkage.

Facts are measured per profile. Layouts are shared only after the bytes and
calls agree.

Static STRING literals show the difference. QB 4.5 and PDS 7.1 use one near
descriptor-plus-payload object in `BC_CN`. VBDOS uses a private `FSL_CONST`
far descriptor and payload plus a near `BC_CN` bridge and a shared selector
word. HIR records the profile's typed data objects and relocations; MIR sees
only the descriptor address.

HIR holds typed calls and logical storage. The adapter applies the ABI
contract and produces MIR calls and memory effects. OMF records, module
headers, fixups and library requirements are a link plan beside MIR.

## LONG and integer math

A `LONG` is a signed 32-bit value throughout HIR and MIR. At a public
call/return boundary only, ABI physicalization splits or joins two words and
assigns `DX:AX`. Long multiply, divide, remainder and compare are whole
semantic operations. Overflow and division policy is the compiler's shared
one (`-fsanitize=signed-integer-overflow`, `integer-divide-by-zero`); the
frontend does not infer semantics from the convenient 386 instruction.

## Floating point

- `SINGLE` storage is binary32 and `DOUBLE` binary64.
- Evaluation may use extended x87 precision until a store or conversion rounds.
- Rounding, exceptions and unordered comparisons use the MIR floating
  semantics, not folding of host floats during parsing.

Arithmetic, comparison, negation, absolute value, square root and conversions
are native MIR operations. Other math intrinsics stay named in HIR through
optimization. After x87 stack allocation the QB finalizer replaces each
remaining `st(0) -> st(0)` intrinsic with its 80387 byte sequence: `SIN` and
`COS` are single instructions, `ATN` is `fld1; fpatan`, and a positive-base
power is `log2(base) * exponent` followed by `exp2`, both expanded inline.
Substituting after allocation keeps MIR and the machine passes free of
calls that do not exist.

Inline floating operations are native x87, so a program that uses floats
needs an x87-compatible coprocessor or execution environment. A new module has
no patch-site anchors for the legacy floating-emulator byte protocol, and
there is no soft-float backend.

## Arrays

The frontend records element type and size, rank and source bounds, static or
dynamic allocation, near/far/huge address behavior, descriptor and allocation
identity, and whether a bounds check is required. Dimensions stay in source
order in HIR; the ABI adapter applies descriptor ordering, adjusted base and
stride once, in an explicit width, never host `usize`.

Numeric arrays are addressed inline. String arrays and unsupported
descriptor operations are runtime calls.

A dynamic element of a frame allocation is `ADDRESS(fixed frame base)` plus an
ordinary integer `ADD(dynamic byte offset)`, for near offsets and for the
offset half of far/huge pointers, so the allocator may use any general register
for the offset.

### Descriptor layout

The `AD` descriptor of a numeric array, from the measured `ARRAY.INC` layout:

![AD descriptor of a numeric array](../../assets/figures/docs-frontends-qb-memory-model-1.png)

An array of unspecified rank reserves BASCOM's eight dimension records: a
14-byte header plus eight four-byte records, 46 bytes. That is an ABI storage
rule, distinct from the parser's subscript ceiling.

Module `$STATIC` numeric arrays split storage from metadata. Elements live in
`BC_DATA`; the descriptor lives in read-only `BC_CN` and holds a relocated far
pointer to the elements. BASIC startup clears `BC_DATA`, so a descriptor
emitted there would be zero when a procedure passed it to the runtime.

`$DYNAMIC` arrays own a mutable descriptor in `BC_DATA`; a bounded declaration
executes `B$DDIM`, and a later `REDIM` passes the same descriptor to
`B$RDIM`. The element allocation is not in DGROUP. VBDOS `MOD_TEX` and
`R_BSP` load the selector from `AD+2` and the adjusted offset from `AD+0Ah`,
then add the scaled subscript to that 16-bit offset. `AD+0Ah` already accounts
for every declared lower bound, so indexing must not subtract the first lower
bound again. HIR forms the 16-bit byte offset and concatenates it with the
selector; it does not send the pointer through huge-pointer normalization.
`/AH` arrays are not covered.

## Procedure calls and frames

HIR procedure types keep the parameter mode, array/descriptor passing, result
type and whether a call can escape an address. HIR keeps source parameter
order. Pascal pushes left to right, so the first formal is furthest from the
return address and the adapter alone maps it to `6 +` the widths of all later
formals; `CDECL` pushes right to left and uses ascending offsets.

Runtime calls use the audited contracts of `llrm-qbruntime`. An unknown call
reads and writes escaped memory and preserves no fact the ABI does not
guarantee.

### Runtime frames

![Frame of a QB procedure (far call, Pascal order)](../../assets/figures/docs-frontends-qb-memory-model-2.png)

`B$ENRA` pushes BP, installs the BASIC frame chain, saves SI and DI, and
allocates and zeroes the locals; `B$EXSA` reverses it before the far return.
Locals and spills are rebased below the header
([sizes](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/backend/llrm-core/src/driver/basic.rs#L63-L77)).
A procedure frames itself instead when the runtime needs none
([dialects](dialects.md#procedure-frames)); `--runtime-frames` keeps the
runtime's frame everywhere.

Compiler-created cells follow their enclosing storage class: a module-level
`FOR` end or step is module data, a `STATIC` procedure's is a private data
object, an automatic procedure's is a frame cell. Identical floating literals
are pooled per module, as BC does.

### STRING temporaries and descriptors

`BX` at `B$ENRA` counts the frame-owned dynamic-STRING descriptors. Runtime
call results (`COMMAND$`, `LTRIM$`, `RTRIM$`, `LDFS`, `SCAT`) stay on the
runtime's temporary chain and add none. Fixed-bound variable-length STRING
arrays are descriptor-backed and use `B$DDIM` with `B$ERS1` at exit.

A `BYREF STRING` formal needs a descriptor that lives through the whole callee.
A dynamic STRING variable already has one. A literal, fixed string,
concatenation or string-function result is only a runtime temporary, so it is
first assigned with `B$SASS` to an owned descriptor, because `B$FLEN` may
consume a temporary: `LEN` then `ASC` on a literal passed directly raises
error 5 on the second call. In a procedure the descriptor is a managed local
and counts in `BX`; at module level it is module data.

A dynamic STRING function result has two representations. The result variable
is an owned four-byte descriptor in the runtime frame. The expression the
caller sees is a near descriptor address: before `B$EXSA` releases the frame
the callee passes the owned descriptor to `B$SCPF`, which copies it to the
temporary chain and returns the surviving near address in AX. HIR types the
callable's result `near*string` and the local result place `string`.

### Result conventions

- A public `LONG` result is returned in `DX:AX`, also under `/G3`.
- A Pascal function returning `SINGLE` or `DOUBLE` takes a hidden near
  destination pointer after its source formals, stores the rounded result
  there, returns the pointer in AX, and counts its two bytes in `retf n`. HIR
  keeps a floating result; only the adapter introduces the pointer.
- `TIMER` is a zero-argument `B$TIMR` call whose AX result is a near pointer
  to a runtime-owned `SINGLE`.

## Responsibility boundary

| Layer | Responsible for | Must not do |
|---|---|---|
| QB semantic frontend | source types, storage class, parameter mode, bounds, evaluation order | choose registers or encodings |
| HIR | resolved typed meaning and logical effects | model p-code, OMF, or legacy register pairs |
| QB HIR-to-MIR adapter | runtime ABI, memory/array/call facts, allowed semantic expansion | add MIR kinds or machine operations |
| MIR and optimizer | SSA and machine-independent optimization | know QB grammar or x86 registers |
| lowering and backend | real-mode encodings, register allocation, pointer lowering, OMF emission | reinterpret source semantics |
