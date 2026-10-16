> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/regclass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/regclass.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/regclass.rs sha256:d03eb17e08660f01864cd25b0c7c10a6363a3fb4a595037013367a2671530184 -->

# regclass

The register class of each value: the set of registers an instruction's
operands allow it to be in.

## How it works
A value is confined when an instruction names it where only some registers
will do: a byte operand (the byte-addressable registers), a word address
base or index (BX/BP, SI/DI, or any of them, as the cell's shape needs), the
selector of a far access (a segment register), or the register a far load
writes. Each instruction states this by `contribution`, a list of `Item`s: a
`Restrict` (value, `Kind`, `Role`), or a note that the value is read as a
`Numeric` or `Selecting`, as half of an address `Pair`, or as a `Far`
selector. The sets are the target's (`RegisterClasses`,
`Segments::selectors`); the instruction only says which one applies.

`classes` intersects every restriction on a value. Then four steps refine
it:

1. A value read as a selector and never as a number takes the selector
registers. 2. The selector a far load writes takes them as well. 3. A
`[base+index]` pair with scale 1 can be spelled either way round, so each
connected component of such pairs is two-coloured (base side and index
side). The colouring is chosen by fewest base-register clobbers, then by
preferred indexes, then fewest bases; a component that is not bipartite, or
cannot take either colouring, keeps the spelling in the source. The
clobbers are read from the caller's intervals and masks, or, where it holds
none, from the intervals of the pairs' values alone, found when a component
has two colourings to choose between. 4. After
coalescing, a phi's result and arguments are one web and take the web's
class where its members agree. A member with a class of its own keeps it.

```text
 mov al, [bx+si]       al : Bytes          bx : base, si : index
 mov es, x ; mov [es:di], 0                x  : selectors
 mov ax, [p+q]         p, q a Pair: p base / q index, or the reverse
```

`classes_with(.., optimistic = true)` is the checker's reading of a body the
coalescer has not merged: the two sides of a phi edge's copy also join, and
address classes are read through webs. `classes_given` takes intervals and
masks a caller already has. `confining_uses` lists each confining read
(`Use`), for the pressure sink. `Scan` keeps per-value counts of what
instructions say, so a rewrite takes the counts of the instructions it lost
and gained and recomputes only the values they name.

`violations` checks a body against its classes, per register file (general,
selector): at each point it asks whether the live values fit the file
(`Crowded`) and whether the values an instruction acts on can have distinct
registers of their classes, a bipartite matching (`Unmatched`). A body with
none can be coloured with copies; one with some cannot, whatever the
allocator does. `LLRM_DEBUG=regclass` runs it after the phases from
`ssaspill` to `coalesce`.

## Example
A far pointer's segment `t2` enters a loop through the phi `t6`, which a far
load reads as the selector of `[t6:t5]`:

```lir
     #3  t2         = load.i16    [arg0+8]
; ...
         #8  t5         = phi         B0: t1, B3: t16
         #9  t6         = phi         B0: t2, B3: t6
; ...
        #15  t16        = add.i16     t5, 2
        #16  t17        = load.i16    [t6:t5]
```

| value | class            | why                       |
|-------|------------------|---------------------------|
| t2    | {ES, FS, GS}     | the web of the phi `t6`   |
| t6    | {ES, FS, GS}     | read only as a selector   |
| t5    | {BX, BP, SI, DI} | address base              |
| t17   | none             | a plain loaded word       |

From `ISEL_DUMP=1 llrm-c -m16 -O2` of a loop `while (n--) s += *p++;` over
`int far *p`, before `PhiElimination`. The classes are those unit test
`test_a_segment_only_a_phi_reads_has_the_class_of_its_web` asserts for this
shape, from the m16 register sets.

## Runs
Not a pass: queried on demand, by `coalesce`, `constrain`, `allocate` and
the pressure sink, at every -O level. `LLRM_DEBUG=regclass` also checks
each body after the phases from `ssaspill` to `coalesce`.

## Provides
One answer to "which registers may this value be in", owned here (#441).
Read by [`crate::backend::ssaspill`](ssaspill.md), [`crate::backend::allocate`](allocate.md),
[`crate::backend::coalesce`](coalesce.md), [`crate::backend::constrain`](constrain.md),
[`crate::backend::pressuresink`](pressuresink.md) and `target.rs`.

## Needs
The target's register sets ([`crate::backend::classes`](classes.md),
[`crate::backend::target`](target.md)) and, for the pair colouring, the body's
intervals and clobber masks (`allocate`).
