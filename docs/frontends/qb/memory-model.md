# 386 real-mode memory and runtime model

The target is fixed: Intel 386 or newer, executing 16-bit real-mode DOS code.
The 386 minimum permits 32-bit integer instructions and operand/address-size
overrides. It does not turn DOS memory into a flat address space.

Ordinary data segments and DGROUP remain 64 KiB resources. A 32-bit `LONG`
or address-size override does not widen a near pointer, increase a segment
limit, or authorize unreal-mode addressing. Module statics, descriptors,
runtime bookkeeping, and near allocations therefore need an explicit group
budget; data that cannot fit must use the runtime's established far/huge
allocation path.

## Address spaces

The frontend and HIR adapter preserve these semantic address forms:

| Form | Meaning |
|---|---|
| near | 16-bit offset interpreted in a known segment or group |
| far | 16:16 segment:offset pointer naming one object/access path |
| huge | 16:16 pointer whose arithmetic has normalization/wrapping semantics required for objects crossing segment windows |
| code | far procedure identity or code relocation used by the runtime/linker |

A far or huge pointer is not a flat 32-bit integer. In particular, huge
pointer arithmetic does not become ordinary packed `ADD`. The adapter emits
the current whole-pointer and `PTR_OFFSET` MIR form, and the existing backend
performs the established selector/offset lowering described in
[pointer-lowering.md](../../machine/pointer-lowering.md).

This is where the design intentionally diverges from LLVM's usual integral,
flat pointer model. LLVM's address-space and `getelementptr` separation is the
reference for preserving pointer kind and separating address formation from
access, but it does not define the normalization rule for a BASIC huge pointer.
That rule remains an explicit runtime/target profile fact.

No frontend operation selects `DS`, `SS`, `ES`, `BP`, `SI`, or any other
register. Segment selection and legal addressing modes remain backend work.

Field projection through a dynamic array keeps the whole pointer and byte
displacement as HIR `ptr_offset`. It must not become address-of an indirect
whole-pointer cell: that loses the separation between address formation and
access. Dynamic string descriptors are the deliberate runtime-boundary
exception. The QB string arena is near, so `B$SASS`, `B$FLEN`, and `B$SCMP`
receive the projected 16-bit descriptor offset.

## Logical storage objects

Source storage is classified before HIR emission as one of:

- automatic procedure local or parameter;
- static procedure local;
- module variable or constant;
- `COMMON`/shared or external object;
- record object or field slice;
- array descriptor;
- array allocation; or
- opaque runtime-owned storage.

An object has stable identity, extent when known, alignment, mutability,
escape, and an address form. Fields and array elements are slices/accesses of
that object rather than unrelated numeric addresses. The HIR-to-MIR adapter
maps these facts into the current canonical memory model in
[`memory.rs`](../../../src/model/memory.rs).

This is the basis for alias analysis. Near-versus-far by itself does not prove
two accesses disjoint, and two numerically equal offsets in different objects
do not make them the same storage.

Source-known objects are disjoint only when the language and runtime allocation
contract establish it. Facilities that expose raw segmented memory, change a
default segment, or pass an address to unknown code escape the relevant object
and force conservative alias/call effects. HIR never treats the arithmetic
non-equality of two segment:offset spellings as proof that their physical
addresses cannot alias.

## Runtime profiles

QB 4.5, PDS 7.1, and VBDOS are distinct ABI profiles. A profile owns:

- procedure entry/exit and BASIC frame layout;
- near/far procedure and callback conventions;
- argument order, width, and `BYREF`/`BYVAL` behavior;
- return locations and caller/callee effects;
- dynamic string and array descriptor layout;
- module header, startup, event, and error integration;
- external symbol spelling and runtime library selection; and
- native-x87 and floating-runtime linkage requirements.

Facts are measured per profile. Similar layouts are shared only after byte
layouts and calls agree; one runtime is never used as an undocumented proxy
for another.

Static STRING literals are a measured example. QB 4.5 and PDS 7.1 use one
near descriptor-plus-payload object in `BC_CN`. VBDOS uses a private
`FSL_CONST` far descriptor/payload plus a near `BC_CN` bridge and shared
selector word. HIR records the selected profile's typed data objects and
relocations; MIR sees only the resulting descriptor address. No FreeBASIC
layout or ABI participates in this choice.

The HIR contains typed calls and logical storage. The adapter applies the
selected ABI contract and produces existing MIR calls and memory effects.
OMF records, module headers, fixups, and library requirements live in a link
plan beside MIR rather than in numeric operations.

## LONG and integer math

A BASIC `LONG` is a signed 32-bit value throughout HIR and semantic/optimized
MIR. It is never an AX:DX- or CX:BX-shaped arithmetic value there. At a public
call/return boundary only, frontend ABI physicalization splits or joins two
anonymous words; lowering assigns the measured `DX:AX` locations. Arithmetic
helper calls such as the measured long multiply/divide/remainder/compare family
are created as whole semantic operations when current MIR can express them, matching
[`raising_calls.rs`](../../../src/frontends/bc/raising_calls.rs) and
[`raising_longs.rs`](../../../src/frontends/bc/raising_longs.rs).

The source frontend supplies the existing overflow and division policies. It
does not infer semantics from the convenient 386 instruction. Where llrm has
already made an explicit compatibility tradeoff, the source path uses that
same policy and tests it; it does not silently invent another one.

The backend is free to use 32-bit real-mode instructions because 386 is the
minimum target. There is no alternate 8086 word-pair lowering to maintain.

## Floating point and math

The frontend distinguishes storage format from evaluation semantics:

- `SINGLE` storage is binary32;
- `DOUBLE` storage is binary64; and
- evaluation may use the existing extended/x87 semantics until an explicit
  store or conversion rounds the value.

Rounding, exceptions, unordered comparisons, and observable environment
effects are carried using current `FloatingSemantics`. The adapter follows the
same policy as the existing float raiser rather than folding host-language
floats during parsing.

"Inline floating/math" includes all numeric BASIC math intrinsics. Arithmetic,
comparison, negation, absolute value, square root, and conversions already
supported there become native operations. The remaining math intrinsics stay
named in HIR until QB-owned late physicalization rather than becoming runtime
calls or expanding the shared MIR vocabulary.

Named HIR intrinsics which have no distinct MIR kind are physicalized by the
QB frontend after MIR optimization, using the same x87 meanings recovered by
the object frontend; they never become BASIC runtime calls and do not add
source-language cases to shared MIR or backend code.

After x87 stack allocation, the QB-owned finalizer replaces each remaining
`st(0) -> st(0)` intrinsic with its 80387 byte sequence. `SIN` and `COS` are
single instructions; `ATN` is `fld1; fpatan`; positive-base power is retained
as `log2(base) * exponent` followed by `exp2`, with both helpers expanded
inline. This byte substitution is deliberately after allocation: it cannot
hide a call or machine constraint from MIR because there is no call.

The source compiler emits native x87 for inline floating operations. A 386 CPU
does not itself contain an FPU, so a float-using program additionally requires
an x87-compatible coprocessor or execution environment. The legacy
floating-emulator byte protocol is recoverable when rewriting a BC object
because that object supplies patch-site anchors; a newly generated module has
no such anchors, and the current whole-segment backend is native-only. This
design adds neither an emulator-protocol generator nor a soft-float backend.

## Arrays

The native array scope is numeric arrays. The frontend records:

- element type and byte size;
- rank and source bounds;
- fixed/static versus dynamic allocation;
- near, far, or huge allocation/address behavior;
- descriptor identity and allocation identity; and
- whether a bounds proof or runtime check is required.

The adapter produces the same current MIR facts as
[`raising_arrays.rs`](../../../src/frontends/bc/raising_arrays.rs),
[`raising_array_bounds.rs`](../../../src/frontends/bc/raising_array_bounds.rs),
and
[`raising_array_access.rs`](../../../src/frontends/bc/raising_array_access.rs).
Static numeric allocations and established far/huge numeric access patterns
may be lowered natively. String arrays and unsupported descriptor operations
remain runtime calls or receive a precise unsupported-lowering diagnostic.

Array dimensions are kept in source order in HIR. Any runtime-specific
descriptor ordering, adjusted base, or stride convention is applied once by
the ABI adapter. Bounds and stride arithmetic use an explicitly chosen width;
host `usize` is never a substitute for target arithmetic.

A dynamic element of a frame allocation is not kept as one machine-shaped
`[bp + dynamic]` address. HIR lowering emits `ADDRESS(fixed frame base)` and
an ordinary integer `ADD(dynamic byte offset)`, for both near offsets and the
offset half of far/huge pointers. This is semantic pointer arithmetic and lets
the allocator place the dynamic value in any general register; it cannot
accidentally request the nonexistent 16-bit `[bp+bx]` encoding. The production
optimizer currently leaves address-form recombination disabled because the
shared folder does not expose the legal `BP+SI`/`BP+DI` pair constraint to
allocation. That is a target-selection limitation, not a QB special case in
MIR or the backend.

## Procedure calls and frames

HIR procedure types preserve parameter mode (`BYREF` or `BYVAL` where the
dialect permits it), array/descriptor passing, declared result type, and
whether a call can escape an address. The selected runtime profile maps that
signature to the established BASIC frame and far-call conventions.

Runtime calls use audited contracts from the existing runtime contract model.
Unknown calls conservatively read/write escaped memory and preserve no facts
that the ABI does not guarantee. Optimization never treats the call instruction
itself as cost-free; its semantic operation is expanded only when it is on the
native allowlist.

Fresh procedures use the frontend-owned BASIC runtime frame directly, without
the shared backend's native BP shell. `B$ENRA` pushes BP, installs the BASIC
frame chain, saves SI/DI, and zeroes the local extent; `B$EXSA` reverses that
work before the far return. Source locals and spills are rebased below the
runtime-owned header: 10 bytes for QB, 18 for PDS, and the measured 20 for
VBDOS. Before that rebase, a local array descriptor occupied VBDOS's frame-link
words and the reduced executable failed immediately after `B$EXSA`.

Compiler-created cells follow their enclosing storage class. A module-level
`FOR` end/step value is module data, a STATIC procedure's value is a private
data object, and an automatic procedure's value is a frame cell. Using the
module-data cursor as a negative BP displacement made qb-qrender's module entry
reserve 7.5 KiB again on the stack; `B$DDIM` then allocated string storage over
a live `SYS_PARSE_ARGS` local. The corrected module frame dropped from `1DA2h`
bytes to allocator spill space, and the string corruption disappeared.

An unspecified-rank dynamic array descriptor reserves BASCOM's eight-
dimension envelope: a 14-byte header plus eight four-byte dimension records,
46 bytes total. The language parser's larger subscript ceiling is not an ABI
descriptor size. Identical floating literals are pooled per module, matching
BC and avoiding needless pressure on VBDOS's near constant/string space.

`BX` at `B$ENRA` counts frame-owned dynamic-STRING descriptors. Runtime call
results such as `COMMAND$`, `LTRIM$`, `RTRIM$`, `LDFS`, and `SCAT` stay on the
runtime temporary chain and do not add handle-block entries. Raw VBDOS objects
measure `BX=0` for `COM_TOKENIZE` and an isolated array append, and `BX=1` for
one local STRING even around nested `RTRIM$(LTRIM$(COMMAND$))`. Fixed-bound
variable-length STRING arrays remain
descriptor-backed and use `B$DDIM` plus the measured `B$ERS1` exit cleanup. The reduced
array-plus-command executable now prints `HELLO WORLD` and exits normally.

A user `BYREF STRING` formal needs a descriptor whose lifetime covers every
operation in the callee. A genuine dynamic STRING variable already supplies
one and aliases directly. A literal, fixed string, concatenation, or
string-function result supplies only an expression/runtime temporary; it is
first assigned with `B$SASS` to an owned descriptor. The distinction is
observable because `B$FLEN` may consume a temporary: passing a literal
directly to a function that calls `LEN` and then `ASC` raises error 5 on the
second call. In a procedure the materialized descriptor is a managed local and
contributes to `B$ENRA`'s `BX`; in module-level code it is module data because
there is no procedure runtime frame.

The subsequent real-program rounds corrected four independent ABI facts:
local `B$DDIM` descriptors carry a near data offset at `+0Ah`, BASIC procedure
calls push in Pascal left-to-right order, and positive BP displacements remain
parameters when the runtime header is inserted. Bare declared zero-argument
functions resolve as calls rather than implicit locals, and public LONG
results cross the legacy boundary in `DX:AX` even under `/G3`. `TIMER` is a
zero-argument `B$TIMR` call whose AX result is a near pointer to a
runtime-owned SINGLE; it is neither an implicit local nor inline math. A
Pascal user function returning SINGLE or DOUBLE receives a hidden near
destination pointer after its source formals, stores the rounded result there,
returns that pointer in AX, and includes its two bytes in `retf n`. HIR keeps a
semantic floating result; only the QB ABI adapter introduces the pointer,
store/load, and cleanup.

A dynamic STRING function result has two representations at one source-level
boundary. The function's result variable is an owned four-byte descriptor in
its runtime frame. The expression returned to its caller is a near descriptor
address. Before `B$EXSA` releases the frame, the callee passes the owned
descriptor address to `B$SCPF`; the runtime copies it to its temporary chain
and returns the surviving near address in AX. Accordingly, HIR represents a
callable dynamic-STRING result as `near*string`, while the local result place
remains `string`. This is a QB runtime ownership rule, not a new MIR value kind
or backend calling convention.

The former `ents.bin missing` gate was caused by a stale uGL archive lacking
ZIP support. With the corrected archive and complete link response, both the
all-BC control and a fresh-`SYS` mixed build render five frames and exit. Their
structural counters agree. Timing-dependent camera state differs between runs,
so raw BMP identity is not used as evidence for this timed scenario.

Module `$STATIC` numeric arrays split storage from metadata. Their mutable
elements live in `BC_DATA`, but their Microsoft `AD` descriptor lives in
`BC_CN` and contains a relocated far pointer back to the element storage.
This is a lifetime requirement, not cosmetic segment selection: BASIC startup
clears `BC_DATA`. A descriptor emitted there is present in the linked EXE but
is all zero by the time a procedure passes it to an external runtime.

The implemented rank-one layout is the measured `ARRAY.INC` layout: far data
pointer, next link, total size, rank byte, `FADF_STATIC` (`40h`), adjusted
offset, element width, then `(count, lower-bound)` for each dimension. The HIR
place names the separate read-only data object, while the relocation names the
ordinary mutable array object. No array ABI fact is introduced into MIR or the
backend.

`'$DYNAMIC` is different in two independent ways. A bounded declaration owns
a mutable descriptor in `BC_DATA` and executes `B$DDIM`; a later `REDIM`
passes that same descriptor to `B$RDIM`. Its numeric/UDT element allocation is
not in DGROUP. The VBDOS `MOD_TEX` listing loads its selector from `AD+2` and
its adjusted offset from `AD+0Ah`, then adds the scaled subscript to that
16-bit offset.
`R_BSP` shows the same two fields for a numeric array formal; descriptor
offset zero is not the adjusted data offset. QB semantics therefore resolves
both owned and incoming numeric/UDT descriptors to the same split far-pointer
view. `AD+0Ah` has already accounted for all declared lower bounds, so indexing
must not read `AD+10h` and subtract the first lower bound again. HIR forms the
16-bit byte offset first and concatenates it with the selector; it does not
send the packed pointer through huge-pointer normalization. `/AH` arrays are a
different, future descriptor/address policy and must not change this default
memory-model rule.

Left-to-right Pascal pushes also reverse physical formal layout: the first
source parameter is furthest from the return address. HIR retains source order;
the QB ABI adapter alone maps a Pascal formal to `6 +` the widths of all later
formals (before the frontend's native-shell adjustment). CDECL remains
right-to-left and therefore uses ascending offsets. The isolated
`runtime-call-basic.bas` subtraction distinguishes the two orders and links
against VBDOS successfully.

## Responsibility boundary

| Layer | Responsible for | Must not do |
|---|---|---|
| QB semantic frontend | source types, storage class, parameter mode, bounds, evaluation order | choose registers or encodings |
| HIR | preserve resolved typed meaning and logical effects | model p-code, OMF, or legacy register pairs |
| QB HIR-to-MIR adapter | apply runtime ABI, build current memory/array/call facts, select allowed semantic expansion | add MIR kinds or machine operations |
| existing MIR/optimizer | SSA and machine-independent optimization | know QB grammar or x86 registers |
| existing lower/backend | real-mode encodings, register allocation, pointer lowering, OMF emission | reinterpret source semantics |

Any required fact that cannot cross these existing boundaries is a frontend
scope issue to diagnose, not permission to alter MIR or the backend.
