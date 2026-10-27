> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/alias.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/alias.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/alias.rs sha256:ac387d69d6c47b8456c7ca017ec99bcc75df14c832ee746e9bba8266a61ea529 -->

# alias

Strong alias analysis: what each pointer may point to, what escapes and
when, and what each call reads and writes.

## How it works
One vocabulary serves every frontend. A pointer's fact is a `Provenance`: a
set of `Slice`s (a canonical object, a byte range of it, a stride and an
access width) plus `restrict` roots. Objects are the function's own
(`memory::object_of`: frame `alloca`s, globals), each pointer parameter's
`Parameter` object (offset 0; a `noalias` parameter is also a restrict
root, `seeds`) and, for a call whose callee returns `noalias`, a fresh
`Allocation`. Unknown stores kill spill facts; they never manufacture a
disjointness proof.

Value solve (`point_values`). Pointer facts flow over the values to a fixed
point, by gcc's worklist over constraints (`solve_graph`,
[tree-ssa-structalias.cc#L2711](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2711)):
an instruction is visited again only when a value it reads, or the fields of
an object it loads from, changed. The solve is flow-insensitive through
memory: one abstract location per object.

- `alloca`, `bitcast`, `addrspacecast`, `getelementptr` give a fact from
  their operand (`_direct`); a constant displacement shifts the slices, a
  variable index widens to the whole object.
- A `phi` or `select` joins its arms as a union; a pointer with an arm no
  fact follows is `UNKNOWN`.
- A store of a pointer adds it to the fields of the objects it may write; a
  load of one reads every pointer ever stored into the object, and
  `UNKNOWN`, unless the slot is one only this body writes: then it is the
  stores that reach it (`slotstores::reaching`, the memory SSA of one
  object), a spilled pointer read back as what was spilled.
- A loop-carried fact is widened to whole objects (`_widened`) at a
  natural-loop header so it settles, and a fact with more than
  `MAX_FIELDS_FOR_FIELD_SENSITIVE` slices is also collapsed, as gcc and LLVM
  do.
- Integers carry no provenance: a `ptrtoint` publishes its pointer, as LLVM
  counts it a capture, and an `inttoptr` has none.

```text
 %p = alloca [8 x i16]            %p -> {frame:%p [0,16)}
 %q = gep %p, i16 2               %q -> {frame:%p [4,6)}
 store ptr %q, ptr %slot          slot's stores: {this one}  (spill slot)
 %r = load ptr, ptr %slot         %r -> {frame:%p [4,6)}     (its reaching store)
 call @unknown(ptr %q)            %q's object escapes from here on
```

Escape solve (`points_to`, `escapes`). Over the value solve, what each
instruction publishes is found once for the instructions that can publish
(a call, a return, a stored pointer, one an instruction takes and does not
carry on: `could_publish`), what reaches each block is a pass over the
components of the CFG, and what had escaped before an instruction is
worked out when asked and kept (`EscapedBefore`): objects handed to a call
that keeps them (not `nocapture`; a summary's `captures` stand where the
callee has one), stored somewhere other than a frame, returned, or converted
to an integer something reads, closed over pointers stored in what was
published and over the initializers of published globals. A frame object not
escaped before an access cannot be reached by a pointer no fact follows
(`EscapedBefore::apart`, LLVM's `EarliestEscapeInfo`).

Summaries (`summaries`). Each body's `Summary` is the slices it reads and
writes, whether it reads or writes unknown memory, its captured parameters,
and the `!tbaa` types of its unplaced writes. It starts from the body's own
accesses (`_direct_summary`) and adds each callee's summary instantiated at
the call's actual arguments (`instantiated`), to a fixed point over the call
graph in SCC order; a cycle is solved from nothing, so a recursive function
that captures nothing proves so. A callee with a body that may be replaced
(weak or linkonce) describes no call. A call to something with no summary
reaches what its attributes allow (`_allowed`: `memory(...)`, `readnone`,
...): through each actual the objects it points to, elsewhere the nonlocal
memory, what escaped before the call, and, of the globals
[`crate::globalsaa`](globalsaa.md) tracks, those it names; unless `nocallback` it also
does what the module's entry points do (`callbacks`). `summaries_updating`
redoes only the bodies that changed and their readers.

Call effects (`calls_by_attributes`). Each call gets an `Effect`: the
`MemRef`s it loads and stores, instantiated through the actuals, and the
bytes it fills (`initialized`: bytes written before any is read). It is a
side table, because the rich MIR's call carries no memory operands.

Access annotation (`annotated`). Each load and store gets its provenance;
where a range bounds its index the slice narrows to the bytes the index can
reach, using strides from `congruences` (a value's residue modulo a
modulus). Far or fixed-address accesses name linear memory.

## Example
A pointer spilled to a slot, reloaded on one arm and joined with the
original:

```mir
define void @f(i1 %c) {
b0:
  %root = alloca [4 x i8]
  %slot = alloca ptr
  store ptr %root, ptr %slot
  br i1 %c, label %b10, label %b20

b10:
  %loaded = load ptr, ptr %slot
  br label %b30

b20:
  br label %b30

b30:
  %joined = phi ptr [ %loaded, %b10 ], [ %root, %b20 ]
  ret void
}
```

`points_to(..).values` gives `%loaded` and `%joined` the same `Provenance`
as `%root`, the frame object it allocates.

From unit test `test_points_to_flows_through_memory_and_a_phi` in
`alias_tests.rs`.

## Provides
`pointers`, `point_values`, `points_to` and `PointsTo` (`values`, `escaped`,
`escaped_before`, `nonnull`), `seeds`, `summaries`, `calls_by_attributes`,
`annotated`, `congruences`, `initialized`, `read_arguments`,
`nonnull_by_definition`. Manager entries in [`crate::manager`](manager.md): `Pointers`,
`PointValues`, `Annotated`, `Summaries`. Consumers:
[`crate::memory`](memory.md) (a `Unit`'s lazily built `annotated` and `points_to`),
[`crate::memoryssa`](memoryssa.md) (`calls_by_attributes`), [`crate::avail`](avail.md)
(`EscapedBefore`), [`crate::induction`](induction.md)
(`congruences`), [`crate::globalsaa`](globalsaa.md), [`crate::interprocedural`](interprocedural.md), and
`gvn`, `promote`, `loadjoins` and `decide` in `llrm-transforms`.

## Needs
[`crate::globalsaa`](globalsaa.md) (tracked globals), [`crate::cfg::Shape`](cfg.md),
[`crate::ranges`](ranges.md) and [`crate::consts`](consts.md) (index intervals, constants),
[`crate::cellmap`](cellmap.md) (the escape solve's per-block cells),
[`crate::slotstores`](slotstores.md), [`crate::induction`](induction.md) (strides), `llrm_mir::memory`
for what attributes state.

## See also
LLVM `BasicAA`, `GlobalsAA`, `EarliestEscapeInfo`, `FunctionAttrs`
`initializes`; GCC `tree-ssa-structalias.cc` (field sensitivity cap).
