> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/regions.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/regions.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/regions.rs sha256:375ea50f52676f892f0c10eae3c0d6525d9d431c697c030803202fef7eb6efec -->

# regions

Decides whether a write through one memory reference can land on another
(`may_alias`, `overlapping`), and indexes cells so a write checks only those
that might meet it instead of every pair.

## How it works
`overlapping(one, other, known, other_known, program)` asks whether a write
through `other` could land on `one`, in this order:

1. `typed_apart`: accesses of different `!tbaa` types of one root, neither
   type an ancestor of the other, cannot alias, unless the address analysis
   already has them at overlapping bytes (`provably_overlap`) or at one
   start (`same_typed_start`). It is asked as LLVM's TypeBasedAA is, only to
   separate what addresses cannot.
2. `foreign_apart`: one reference is in an address space that holds program
   data and the other, by its selector's range, lands wholly in linear
   memory the target keeps no program data in (`foreign`).
3. Both with provenance: `_displaced` (two references off one root and one
   scaled index are disjoint by displacement, LLVM's constant-offset GEP
   compare) if they share a frame, else `may_alias`.
4. Otherwise a reference that only a named object reaches (`_unescaped`) is
   not reached by one through a pointer value. With range facts, an indexed
   reference is first widened to the hull of its range (`ranges::covering`),
   then `_displaced` again, then `may_alias`.

`may_alias` refines each reference to the bytes it may name under its own
interval facts, `foreign` linear bytes or its provenance `narrowed`: an
indexed access into its own object with a range fact on the index names
`[disp + scale * low, disp + scale * high]` plus the access width, when that
lies inside the object's extent. Refining that cannot be expressed in
`Slice` endpoints is a `RegionError`, and the caller treats it as overlap.
The two refined provenances then decide by `Provenance::intersects`. A
reference with no provenance is not proven apart from anything.

```text
   A: p + 4 (width 4)     B: p + 8 (width 4)     one frame (p, no index)
   A = [4, 8), B = [8, 12)  ->  _displaced: disjoint, no object reasoning
```

Finding candidates. A `CellMap` indexes cells by `OverlapBucket`: an
interned (object, frame, alias class) shape. `overlap_buckets` picks, for a
write, only the buckets that can hold a cell `overlapping` does not rule
out: objectless ones, the write's frame, its objects, and any class that may
alias. `displaced_buckets` and `displaced_span` serve a write at fixed
displacements in a frame; `hull_buckets` and `hull_span` serve a write and
cells named by provenance alone (no root), by the hull of their slices in
one object. `Index` does the same for lists of references:
`all_near` asks a test of each such entry once, without building the list
(a loop's loads asked its stores). `near` returns the entries that name an
object the reference may alias, every addressed one if the reference names
an addressed object, and every entry without provenance; more than needed,
never fewer.

Which linear memory holds no program data is the target's fact, and which
address spaces hold program data the segment layout's; both come from the
program the manager hands analyses as `Outer::program`.

## Example
Three 4-byte stores off `@g`:

```mir
@g = global [64 x i8] zeroinitializer

define void @f() {
b0:
  store i32 0, ptr getelementptr (i8, ptr @g, i16 10)
  store i32 0, ptr getelementptr (i8, ptr @g, i16 13)
  store i32 0, ptr getelementptr (i8, ptr @g, i16 14)
  ret void
}
```

| `overlapping(first, ..)` | bytes vs first's [10, 14) | answer      |
|--------------------------|---------------------------|-------------|
| second store             | [13, 17)                  | `Ok(true)`  |
| third store              | [14, 18)                  | `Ok(false)` |

From unit test `overlapping_same_base_statics_use_half_open_byte_ranges` in
`regions.rs`.

## Provides
`may_alias`, `overlapping`, `typed_apart`, `foreign_provenance`,
`fixed_provenance`, the bucket functions, `displaced_span`, `Index`.
Consumers: [`crate::alias`](alias.md), [`crate::memoryssa`](memoryssa.md), [`crate::avail`](avail.md),
[`crate::consts`](consts.md), [`crate::floatfacts`](floatfacts.md), [`crate::cellmap`](cellmap.md), and in
llrm-transforms `promote`, `transform`, `homes`, `loopmotion`, `loopidioms`,
`floatloop`.

## Needs
[`crate::memory`](memory.md) (`MemRef`, `Provenance`, alias classes) and
[`crate::ranges`](ranges.md) (`Interval`, `covering`).

## See also
LLVM BasicAA's constant-offset GEP comparison and TypeBasedAA.
