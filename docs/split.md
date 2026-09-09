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

- `qbopt/passes.py` -- `MIRTransform` has one method, `transform(mir) -> mir`.
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

**MIR itself still carries four machine facts.** `Op.node` (the decoded
instruction), `Op.made` (`ir.Semantics` over `ir.Reg`), `Op.covers` (a
range of BC's bytes) and `MirBody.origin`. The passes barely touch them;
the data structure holds them, so nothing stops a pass from starting.

**No LIR form.** `lir.py` is a requirements table -- `tied`, `reads`,
`writes` -- and lowering goes `mir.Op` → `ir.Semantics` → `select`. There
is no third representation and no peephole, so nothing runs after
allocation.

**Widening is not a pass** and is correctly outside the list: it recognises
an idiom and writes machine form. It runs after every pass and before
lowering, in `wholeseg`.

## What closes it

1. **Phase D** -- absorption at the raise, then the machine arm retires:
   `lift.py`, `calls.py`'s emission, `memory.py`, `forward.py`.
2. `node`, `made` and `covers` leave `mir.Op` for a side table keyed on
   `Op.id`; `origin` last, because lowering and the allocator's identity
   baseline are built on it.
3. LIR becomes a form, and peephole has somewhere to live.

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
| `transform`, `avail` and `simplify` asked `regalloc.live()` | `qbopt/liveness.py`. Liveness over SSA values names no register; it sat in regalloc because that is what first needed it |

Still open, and each is a change rather than a move:

- **`layout.rebuild` runs a MIR pass.** `wholeseg` hands it
  `settle=transform.widened`, and layout calls it on a body the allocator
  refused. The decision is layout's to report and wholeseg's to make.
- **`mir._folded` writes into the Module it is given** -- `found.absorbed`
  and `found.refs`. The raise may see machine form; it may not mutate its
  input. It should return the two maps.

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
