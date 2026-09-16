# Strong alias analysis

`qbopt/model/memory.py` is the one machine-independent vocabulary for memory.
The C frontend attaches it while raising HIR; every MemorySSA consumer reaches
it through `mir.overlapping`, so GVN, promotion, DSE and loop motion ask the
same question.

## 1. Canonical objects and subobjects

A memory object has a kind, stable identity, allocation generation and optional
extent. The kinds distinguish current-frame and stack storage, defined and
external globals, nonlocal storage, allocation sites, named far objects,
formal pointer parameters and genuinely unknown storage. Different concrete
objects are disjoint. `NONLOCAL` reaches globals, heap and escaped objects but
not an unescaped current frame; `UNKNOWN` reaches everything.

An access is a half-open byte `Slice`. Its object identity separates equal
offsets in different objects, and its byte range separates fields and array
subobjects in one object. A slice may also carry a stride and element width;
intersection uses the modular byte lanes, not merely their overlapping hull.

## 2. Flow-sensitive points-to and escape

`analysis/alias.py::points_to` propagates provenance through SSA copies,
pointer arithmetic and phis. A missing phi arm produces `UNKNOWN`; one known
arm can never erase it. Exact pointer spill slots are tracked as memory state:
an exact store is a strong update, an overlapping or unresolved store kills the
fact, and CFG joins retain a slot only when every incoming edge defines it.
Indirect references are resolved through those same value facts before a store
invalidates spill state. A write through a parameter can therefore kill cells
in that parameter object without discarding a pointer saved in a disjoint frame
object. The resolved reference also feeds escape and mod/ref analysis; the
three consumers cannot disagree about what an indirect operand reaches.

Escape is a second forward dataflow. Publishing a pointer outside the frame,
returning it, or passing it to a capturing callee exposes its object only from
that program point onward. Each call therefore sees the objects that escaped
before it, not every object that escapes somewhere in the procedure.

## 3. Interprocedural mod/ref and capture

Procedure summaries describe reads, writes and captured formal pointer
indices. They are solved to a fixed point, so recursion and mutually recursive
calls are conservative without losing effects already known. At a call site,
parameter slices are rebased onto each actual pointer's object and offset.
The direct summary resolves base-plus-displacement operands through the
procedure's points-to solution, rather than requiring provenance to have been
stamped statically on every dereference.

An unknown C callee has a complete, explicit footprint: all nonlocal storage,
the whole object behind each pointer actual, and objects already escaped on
that path. It cannot touch an unrelated current-frame object. Known callees
replace that footprint with their instantiated summary. Standard fresh
allocation routines create allocation-site objects with a distinct generation
and a constant extent when their size is known.

## 4. Fields, indices and dependences

Direct frame and global references carry the containing object and exact field
bytes. Pointer offsets preserve that provenance. Non-wrapping range facts
narrow indexed references to possible starting offsets, while induction
congruences retain their stride. This proves, for example, that adjacent fields
and interleaved even/odd lanes are disjoint without pretending their enclosing
objects are.

## 5. C alias contracts

The frontend records scalar alias classes on memory references. Incompatible
non-character classes are disjoint; character, aggregate and untyped accesses
remain conservative. The Open Watcom recording shim also preserves
`__restrict` as a private `CGAttr`, and the raise assigns each restricted
declaration a stable no-alias root. Roots flow with the pointer through copies,
arithmetic and phis.

References without canonical provenance retain the established RegionSet
query. That is a compatibility path for decoded BC objects, not a second
answer: when both operands have canonical provenance, the canonical query has
precedence and the legacy address-hull rewrite is bypassed.
