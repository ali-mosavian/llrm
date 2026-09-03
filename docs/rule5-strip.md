# Getting the machine out of MIR itself

The passes are the easy half. `mir.Op` is the hard half: four of its fields
are machine facts, and `MirBody.origin` is a fifth.

```
  Op.node    ir.Node -- the decoded x86 instruction
  Op.made    ir.Semantics over ir.Reg -- machine operands
  Op.covers  a range of BC's bytes
  Op.ref     the address of a fixup in BC's bytes
  MirBody.origin  Value -> Register_
```

## What replaces them

**`Op.id`.** An opaque token given at the raise and carried through every
`replace()`. It is what a side table keys on, and it survives an operation
being moved -- which is why these facts were on the op in the first place:
a table keyed by `at` breaks the moment the hoist re-seats something.

**`Provenance`.** `id -> (node, ref)`. Raise-time facts, so nothing a pass
does has to maintain them. Owned by the module, handed to layout.

**`Op.absorbed`.** The ids an operation stands for, replacing `covers`.
A pass says "this one is now those three", which is a statement about
operations; turning that into a byte range is layout's.

**`Op.args`.** MIR's own operands -- a value, a constant, a cell -- so a
pass that rewrites an operation can say what it now computes without
writing `ir.Semantics`. `made` goes when `lower()` can build it from `op`,
`args` and the allocation.

**`origin` last.** Lowering and the allocator's identity baseline are built
on it, and it is the one field `Value`'s docstring sanctions reading.

## Order

A. `Op.id` and the module's `refs` table.               done
D. `args`/`results`, and `lower.py` as the boundary.    done for fold,
   forward and decide; widen, absorb and the hoist still write `made`.
B. `node` into the side table.
C. `covers` -> `absorbed`.
E. `origin`.

## Measuring it

`wholeseg.rebuilt` is not the shipped optimiser. `rewrite.py` is, and it
runs the machine arm and the whole-segment arm into each other to a fixed
point -- so a corpus byte total taken through `rebuilt` can be identical
while the program prints the wrong number. It did: five commits measured
that way, and lngmix printed 110 for 142900 the whole time.

Each step: 485 of 485 rebuild, the corpus byte total, **and** twelve
configurations by five programs.
