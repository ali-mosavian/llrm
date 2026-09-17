# The split

There is one line in this compiler and everything else is arranged around
it. On one side a program is bytes an 8086 executes; on the other it is
values and operations. **Nothing crosses except at the two doors.**

```
  ─────────────── machine ───────────────┼──────── abstract ────────┼──────────── machine ────────────
                                         │                          │
  omf ──▶ decode ──▶ blocks ──▶ raise ───┼──▶ mir passes ───────────┼──▶ lower ──▶ regalloc ──▶
          declen     extent     mir.py   │    fold decide dead      │    lir      colour
          ir.py      loops               │    segments hoist        │             untangle
                                         │    forward drop_loads    │
                                         │    drop_stores           │
                                         │                          │
                                         │                          │  ──▶ peephole ──▶ layout ──▶ omf
                                         │                          │      peephole.py select
                                         │                          │                   relocate
                                    the raise                   lowering
                                    (one door)                  (the other)
```

## The rule

**Every pass between the raise and lowering takes MIR and returns MIR, and
names nothing about the machine.** No register, no mnemonic, no encoding,
no byte offset into BC's object. Only `lower`, `regalloc` and `peephole`
see machine form.

That is AGENTS.md's fifth rule and this file is why it is not negotiable.

## Why

**A pass that knows the machine does the allocator's job badly.** The hoist
picked a spare register out of `regalloc.AVAILABLE`, rewrote every reader
to name it, and emitted the copies itself -- ninety machine references and
the source of every hoist bug for a year, including one that hung `harr` on
four configurations while the gate said clean. It had none of the
allocator's information and could not have had it.

**A pass that reads the emitted form asks the wrong question.** Twenty
callers asked `op.made or op.node.semantics` -- "what does this compute, in
x86" -- and for an operation a pass had already rewritten they got the
*original instruction*. A fold kept the fixup of the read it replaced, and
`suite/jumps.bas` took the CASE ELSE arm for `k = 1`.

**An idiom recognised in a pass is recognised in the wrong place.** What a
long pair *is*, what an absorbable call *is*: those are questions about the
bytes BC wrote, and only the raise is looking at them. A pass that asks
gets a different answer each round, because by then the bytes have moved.

**And the abstract side is where the optimisation is.** Value numbering,
loop-invariant motion, promotion and induction variables are all statements
about values. Every one of them is impossible to state -- not merely harder
-- while a value is a register and an operation is an instruction.

## What each side may say

|                      | machine side | abstract side |
| -------------------- | ------------ | ------------- |
| a place              | `Register.EAX` | a `Value`, and `Value.variable` says which variable it versions |
| an operation         | `ir.Operation`, a mnemonic | `mir.Kind` -- `ADD`, `LOAD`, `BRANCH` |
| an operand           | `ir.Reg`, `ir.Mem`, `ir.Imm` | `mir.Held`, `mir.Cell`, `mir.Const` |
| a comparison         | flags, and six spellings of `jl` | `Op.test` -- `LT`, `LE`, … |
| a width              | the instruction's encoding | on the operand, `Held.width` / `MemRef.width` |
| a partial write      | a read-modify-write of a 32-bit register | `Op.merges` -- which use is only what a result preserves |
| a machine resource   | `Register.ES`, `st(0)` | `mir.Opaque`, carrying the name and nothing else |
| where something is   | an address, a byte span | a block, and `Op.target` for a branch |

The abstract side may *carry* a machine fact it never reads --
`MirBody.origin` is where BC kept each value, which lowering needs and
`Value`'s own docstring sanctions four readers of. Carrying is not the same
as reasoning: a pass that reads `origin` to decide something has crossed
the line, and `tests/test_rule5.py` is the fence.

## How it is enforced

- `qbopt/model/passes.py` -- `MIRTransform` has one method, `transform(mir) -> mir`.
  Module facts go in at construction, so the signature cannot grow a way to
  ask the machine a question.
- `tests/test_rule5.py` -- walks the AST of every pass for `Register`,
  `ROOT`, `AVAILABLE`, `ir.Reg` and the rest, with an allow-list that only
  ever shrinks. A name joining it needs a reason in writing.
- `tools/stages.py` -- dumps MIR between passes and the machine form once,
  where lowering happens. It used to print an `asm` and an `lir` view beside
  every stage, which said a pass has a machine form. It does not.

## Where the line is broken today

Measured, not remembered. Re-measure before trusting this section.

**Two arms, not one pipeline.** The largest deviation, and it is not
inside MIR:

```
  rewrite.rewrite:
      repeat:  data = machine(data)      a whole compiler -- decode, lift,
                                         calls, memory, forward, simplify,
                                         emit omf
               data = written(data)      another -- raise, mir passes,
                                         widen, lower, regalloc, layout,
                                         emit omf
```

Each emits an object the other re-reads, and they iterate to a fixed point.
The machine arm is not "x86 passes before the raise": it is a second
optimiser that has already done the work by the time MIR sees anything,
which is why the MIR arm never meets an absorbable call. Phase D retires
it and collapses the two into the diagram at the top.

**Twelve machine references in the passes**, all named:

```
  transform._leaving   8   what the caller sees is a statement about
                           registers, and Value's docstring sanctions it
  avail.redundant      4   a register-level map, waiting on the allocator
                           honouring a substituted value
```

**MIR itself still carries two source-machine facts.** `Op.covers` (a range
of source bytes) and `MirBody.origin`. Decoded instructions now leave the
raise in `SourceMap.nodes`, keyed by the operation's stable id, and lowering
transfers each required node to LIR. Byte ownership is also represented by
opaque `Op.absorbed` identities whose immutable raw ranges live in
`SourceMap.occurrences`; the existing `coverage` map keeps its legacy folded-
operation meaning for MIR compatibility. Lowering now resolves the opaque
identities into concrete LIR `covers` and disjoint `spread` ranges, and layout
consumes those LIR ranges directly. Optimization modules are mechanically
forbidden from naming the old MIR ranges: deletion leaves an inert owner and
semantic combination transfers only opaque identities. The compatibility
fields now remain solely at the raise/lower edges until they are deleted from
`Op`. Selected machine semantics live only on LIR.

**LIR is now the backend form.** It owns selected machine semantics and
allocation requirements (`tied`, `reads`, `writes`), and layout plus fresh
OMF emission consume allocated LIR directly. There is still no peephole or
post-allocation optimization pass.

**Long-pair recognition is not a pass.** `raising_longs` turns BC's adjacent
word operations into whole scalar MIR before the fixed point. The old
post-optimization `transform.widened()` path and its restore machinery have
been deleted.

## What closes it

1. **Phase D** -- absorption at the raise, then the machine arm retires:
   `lift.py`, `calls.py`'s emission, `memory.py`, `forward.py`.
2. `covers` leaves `mir.Op` for a side table keyed on `Op.id`; `origin` last,
   because lowering and the allocator's identity baseline are built on it.
   (`made` has already moved to LIR and `node` is already in `SourceMap`.)
3. Add the post-allocation peephole now that LIR has become a real form.

Until then, every one of those is a debt with a name, and none of them is a
licence to add another.

## The two rules, and where they were broken

**No MIR pass may call a machine pass, and no pass may have a side effect.**
A pass takes MIR and returns MIR; that is the whole of its contract. The
same holds at every tier: the raise does not call a pass, and layout does
not call one either.

Audited 2026-09-05. What was broken, and what it is now:

| was | now |
| --- | --- |
| `transform` asked `layout.selectable(op)` | `mir.rewritable(op)` -- what an operation *is* is the raise's answer |
| `transform` asked `lower.current(op)` | `mir.instruction(op)`, same reason |
| `transform`, `avail` and `simplify` asked `regalloc.live()` | `qbopt/analysis/liveness.py`. Liveness over SSA values names no register; it sat in regalloc because that is what first needed it |

`mir.bodies` now returns `RaisedBodies`: the MIR sequence plus an external
`SourceMap` containing decoded nodes, relocations, floating protocols,
absorbed sites and disjoint byte coverage. `_folded` returns its maps, and the
other recognition steps write only that fresh result. The parsed `Module`
remains unchanged; optimization coverage, lowering, layout and fresh OMF
emission receive that same side table explicitly. Lowering is the only stage
that joins decoded nodes back to operations, and it places them directly on
LIR. `SourceMap.applied()` remains only as a compatibility helper for focused
low-level tests; no production route reconstructs a fused module view.

## What honouring a pass's order would take

`layout.rebuild` sorts on `op.at` before emitting, so an operation a pass
moved goes straight back where it was -- which is why the old `place`
"bought nothing measured", and why the rebuilt one buys nothing either.

Sorting on the order the lists give does not work. Two attempts failed the
same way, and `nots` caught both: the right low word of a long and the
wrong high one.

- The raise's own lists are **not always in byte order**, so a key that
  reads any out-of-order operation as one a pass moved reorders real
  instructions.
- `covers[0]` is the honest answer to where an operation's bytes are and is
  still not a sort key here.

What is missing is that **an operation does not say whether a pass moved
it**. Until one does, layout cannot tell a move from the raise's own order,
and `place` cannot pay.
