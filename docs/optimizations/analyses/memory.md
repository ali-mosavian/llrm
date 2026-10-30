> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/memory.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/memory.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/memory.rs sha256:ddebe503ca1a4e52c1fe0415cdf067f270a23ed4d55ea325fb88a306f4e9a8a1 -->

# memory

The memory model every alias query reads: objects, byte slices, provenance,
accesses (`MemRef`) and the `Unit` that carries a function's facts. It gives
every analysis one description of what an access touches, so none re-derives
it from the pointer.

## How it works
An object is what LLVM calls an underlying object (`MemoryObject`, interned
per module as `ObjectRef`). Its `MemoryKind` is one of:

- `Frame`: an `alloca`.
- `Global`: a global variable.
- `Parameter`: what a pointer parameter points to.
- `Unknown`: a pointer no fact follows.
- `Nonlocal`: what a callee may reach.
- `Absolute`: linear memory (`regions::linear`).
- `Stack`, `External`, `Named`, `Allocation`: kept for `classes_may_alias`;
  the rich MIR has no producer for them (an allocation is the `!tbaa` type
  "allocation" on the access).

Besides its identity an object carries three facts that equality ignores:
`addressed` (some code computes its address), `captured` (that address can
be found from outside this activation) and `constant` (nothing writes it).
`objects_may_alias` asks only the `AliasClass` (kind, addressed, captured):

```text
  an unaddressed object            -> reached only by its own name
  Unknown vs x                     -> x.captured
  Nonlocal vs x                    -> x.captured and x is not Frame/Stack
  Parameter vs x                   -> x.captured and x is not Frame
  Global/External vs Global/External -> only if one is External
  anything else (distinct)         -> no
```

A `Slice` is bytes of one object: `[low, high)` with `stride` and `width`,
so a stride above one selects byte lanes, not their hull. `WHOLE_LOW..
WHOLE_HIGH` is "any byte". `Slice::intersects` is exact for one object (gcd
of the strides) and, for two distinct objects that may alias, true. A
`Provenance` is a set of slices plus the `restrict` roots (`noalias`
parameters) a pointer is based on; two with disjoint nonempty restrict sets
do not intersect.

`MemRef` is an access, LLVM's `MemoryLocation`, with its pointer decomposed
as BasicAA's `DecomposeGEPExpression` does: a `root`, a constant `disp`
wrapped to the index width, and at most one variable index (`base`,
`scale`), plus the width, `!tbaa` type and its ancestors (`lineage`), and
`volatile`. `MemRef::of` builds it from a load or store, `filled` from a
constant-length `memset`, `reach` from provenance alone (what a call's
effect names). `named` says the bytes are fixed outright; `unwritable` that
every object it may reach is `constant`.

`Unit` is what the analyses read: the function, its module's types,
constants, layout, metadata and globals, the program's target and segment
layout, and optional facts handed in with `with_*` (shape, registers,
pointers, annotated references, assumptions, counted loops, edges,
exposed frames, GlobalsAA, callbacks). A fact not handed in is derived on
demand, which is correct but repeats work; [`crate::manager`](manager.md)'s
`Unit::within` fills them from cached analyses. `Standing` holds the
registers for a caller that edits the body as it goes. `constant_bits` and
`constant_bytes` read a `constant` global's initializer. `own_bytes` is the
one statement of what a load or store does to its bytes, and
`unmodeled_write` says whether any other instruction may write.

## Example
`MemRef::of` on the two loads below, under the DOS data layout (address
space 1 is far pointers):

```mir
@g = global [8 x i16] zeroinitializer

define i16 @f(i16 %sel, i16 %i) {
b0:
  %s = inttoptr i16 %sel to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %e = getelementptr inbounds i16, ptr addrspace(1) %far, i16 %i
  %f = getelementptr inbounds i8, ptr addrspace(1) %e, i16 -2
  %x = load i16, ptr addrspace(1) %f
  %y = load i16, ptr getelementptr (i8, ptr @g, i16 6)
  %z = add i16 %x, %y
  ret i16 %z
}
```

| `MemRef` of | `%x` load | `%y` load |
|------------|-----------|-----------|
| `root`     | `%far`    | -         |
| `disp`     | -2        | 6         |
| `base`     | `%i`      | none      |
| `scale`    | 2         | -         |
| `segment`  | `%sel`    | -         |
| `width`    | -         | 2         |
| `object`   | false     | true      |
| `addr()`   | none      | some      |

From unit test
`a_far_element_access_decomposes_to_its_segment_index_and_displacement`
in `memory.rs`.

## Provides
The types above; `exposed_frames`, `object_of`, `global_object`, `typed`,
`lineage`, `is_lifetime_marker`. Consumers: nearly every analysis in this
crate ([`crate::alias`](alias.md), [`crate::memoryssa`](memoryssa.md), [`crate::regions`](regions.md),
[`crate::avail`](avail.md), [`crate::consts`](consts.md), [`crate::effects`](../backend/effects.md)) and the memory
passes of llrm-transforms.

## See also
LLVM BasicAA `DecomposeGEPExpression` and `MemoryLocation`
(`llvm/lib/Analysis/BasicAliasAnalysis.cpp`).
