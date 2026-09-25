# Values as variables, not as registers

`mir.py` names an SSA value after the machine register BC kept it in:
`Value.of` is a `Register_`, and `eax#155` reads as "the 155th version of
eax". That was the right first step -- it made the raise/lower round trip
checkable, and it is byte-identical over 2,060,168 bytes -- but it is the
thing now blocking every remaining transform, and this document is the
staged plan for removing it.

## What it blocks, measured

Absorption leaves a split/rejoin idiom at every site boundary, because
`calls.py` emits each site in isolation and has no view past its own call.
In `bench/nbody.bas` there are 24 of these triples, 23 of them inside
loops and 8 at loop depth 3 -- the innermost hot loop:

```
  0x0177 restore  def=(eax#156, edx#157)  use=(eax#155, edx#147)
  0x0181 push     use=(edx#157)   stores=[(None, 2)]
  0x0182 push     use=(eax#156)   stores=[(None, 2)]
  0x0183 pop      def=(eax#158)   loads=[(None, 4)]
```

`eax#158` is `eax#155`. Nothing in MIR can say so, for two independent
reasons, and neither alone is enough:

**Half-register values are inexpressible.** A variable is a 32-bit root, so
`Restore` reports `defs = uses = {EAX, EDX}` and lands as an opaque op. MIR
cannot say `eax#156 = low16(eax#155)`, which is the fact that makes the
rejoin an identity. `wide.Pair` is the nearest existing concept, but it is
a pattern match over two ops -- it recognises pairs, it cannot carry "this
value *is* those halves" through a dataflow.

**The stack is unnameable memory.** Every push and pop carries
`MemRef(addr=None)`, the address that aliases everything, so nothing links
`eax#158` to the two pushes above. `avail.py` refuses such cells outright,
correctly -- they can never match, and would sit in the map as entries no
lookup can use. `stack.py` does not cover this either: it answers which
pushes feed which *call*, and this idiom has no call in it.

## The shape it should have

Variables, memory and stack. A variable is `v0`, `v1`, with no location at
all; where BC kept it is a fact about lowering, not about the value.

    v3 = low16(v1)
    v4 = high16(v1)
    v5 = concat(v4, v3)      -- and v5 is v1, by algebra

Half and byte access are *operations*, not annotations on a storage class.
That is what turns the churn from a byte pattern a peephole matches into an
identity a simplifier proves, and it is more precise than the model it
replaces: `mov al,5` becomes `v2 = insert8(v1, 0, 5)` rather than an opaque
write of a whole root.

The stack is an address space inside the one memory model, not a third
thing. `module.Space` already carries GROUP and FAR; STACK joins them, sp
is tracked symbolically, and push/pop become store and load at a slot. The
reason not to give it its own model is aliasing: two models means two alias
analyses that have to agree, and `may_alias` should be asked once.

## Stages

Each stage keeps the round trip green. That property is the whole safety
net here, and it is worth restating why it survives a change this large:
`Op.node` is already `ir.Node | None`, so an op that nothing transformed
lowers by emitting its own origin bytes and never has to re-derive BC's
register choice. Only changed ops go through a selector. Without that,
losslessness would become contingent on an allocator reproducing BC
exactly, which is a far weaker claim than the one this pass has today.

**1 -- the variable stops being a register.** `Value` becomes `(id, at,
kind)`, kind being DATA or FLAGS. Where BC kept it moves to a side map on
`MirBody`, so an analysis cannot key on a register by accident rather than
by choice. Lowering and `regalloc`'s identity baseline read the side map;
nothing else may. 25 `.of` uses across 8 files.

**2 -- half and byte access.** `low16`, `high16`, `concat`, `insert8` as
real ops, with `Restore` raised as the first two and the push/push/pop
rejoin as the third. The gate is that `concat(high16(x), low16(x))`
simplifies to `x` and the nbody churn becomes visible as an identity.

**3 -- the stack as an address space.** `Space.STACK`, sp symbolic, push
and pop as store and load. Scoped at first to slots the code itself pushes
and pops within one block: BC's `SS == DS` means a stack slot and an array
element can genuinely alias, so the general claim needs an analysis that
does not exist yet, while a push/pop pair nothing takes the address of is
private and provable. The gate is `avail.py` linking a pop to its pushes.

**4 -- simplify and emit.** The identity proven, the ops dropped from
`lower()`, addresses fixed by `relocate.py`. Deletion needs no instruction
selector: `ir.emit` slices each surviving node's own span, so omitting a
node is already expressible. Synthesis is what would need one, and this
stage does not synthesise.

## What it buys beyond the churn

`regalloc.colour()` stops being a no-op. Today identity is trivially
optimal *because* values are pinned to the registers they came from --
`moved()` measured every move as pointless over 27,680 values, which is not
a result about allocation but about the representation.

CSE becomes expressible at all: two computations match by what they
compute, which cannot be asked while a value's name contains where it
landed.
