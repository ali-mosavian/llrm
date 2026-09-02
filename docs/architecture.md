# Architecture

An `.OBJ` in, an `.OBJ` out. Every module below either answers a question
about the bytes or changes them, and the split between those two is the
thing worth seeing: most of the analysis is finished and correct, and only
some of it is wired to emission.

## The pipeline it is becoming

```
     BC.EXE                                             LINK.EXE
       |  .OBJ                                       .OBJ  ^
       v                                                   |
 +-----------+                                       +-----------+
 |  decode   |  omf declen blocks module ir          |   emit    |
 +-----+-----+                                       +-----+-----+
       |                                                   ^
       v                                                   |
 +-----------+                                       +-----------+
 |   raise   |  mir.raise_body                       | peephole  |
 +-----+-----+  SSA; values, not registers           +-----+-----+
       |                                                   ^
       v                                                   |
 +------------------------+                          +-----------+
 |          opt           |                          | regalloc  |
 |  machine-independent   | <---+                    +-----+-----+
 |  hoist forward fold    |     | to a fixed point         ^
 |  CSE DCE strength      | ----+                          |
 +-----------+------------+                          +-----------+
             |                                       |    lir    |
             +-------------------------------------> +-----------+
```

The order is the architecture, and each boundary is a rule.

**mir is machine-independent.** A value is `(id, at, kind)` and lives
nowhere. Where BC kept it is `MirBody.origin` -- a side map that lowering
and the allocator's identity baseline read, and nothing else may. This is
`docs/variables.md`'s stage 1 and it is done.

**opt runs to a fixed point.** Many passes, repeated, none choosing
registers. A pass that names a register is doing the allocator's job with
none of its information; the history has five attempts at it and every one
produced a wrong program. What a pass may say is "this value is live here";
where it goes is not its question.

**lir says what the machine requires.** Three kinds, all in `lir.py`:

```
  fixed     imul word [k]   multiplies by ax and names it nowhere
            idiv            reads dx:ax, writes dx:ax
            cwd / cdq       extends ax into dx
            shl ax,cl       takes its count in cl

  class     [bx+si]         16-bit addressing reaches memory through
                            bx, bp, si or di and nothing else

  tied      add ax,[c]      one register at two moments, not two places
```

Checked against BC's own assignment across the corpus: 431 fixed and 1,290
class requirements, and any naming a register BC had no value in would be a
wrong requirement. Two were, and the check found them.

**regalloc assigns, splits and spills.** The only thing that decides where a
value lives. When an instruction requires a value somewhere it cannot live
-- `imul` wants ax, the loop clobbers ax -- both facts are true and the
answer is neither: a live range split, a move putting the value back just
before the instruction that needs it.

```
   before                          after
   ------                          -----
   loop:                           mov cx,[r]        <- hoisted, once
     mov ax,[r]   <- every pass    loop:
     imul word [w]                   mov ax,cx       <- the split
                                     imul word [w]
```

**peephole runs after allocation**, because what is worth rewriting depends
on what ended up where.

## Where the code actually is

`lir` exists and is right. `regalloc` has liveness, interference,
congruence classes (phi members and tied operands move together or not at
all) and live range splitting. It cannot spill, and `opt` passes still name
registers because the migration that stops them is unfinished --
`tests/test_allocation.py` carries strict xfails naming what is left and
the program each one broke.

## Two towers

The single most important thing about this codebase's shape: there are two
representations reaching up from the bytes. Both can emit now. Only one of
them emits in a shipped build.

```
              .OBJ bytes -- omf.py, declen.py
                        |
              blocks.py, module.py, ir.py
                        |
        +---------------+----------------+
        |                                |
        v                                v
  MACHINE-CODE TOWER               MIR / SSA TOWER
  (what rewrite.py ships)          (emits; e2e only)
        |                                |
  lift.py    pairs, widened        mir.py     raise_body()
  flags.py   which are live          |        every value named once,
  memory.py  cells and aliasing      |        phis at the joins
  stack.py   where args are          |
  runtime.py what a call breaks      +-- consts.py    known() -- propagate + fold
  registers  backward liveness       +-- wide.py      pairs(), tests()
        |                            +-- regalloc.py  colour() -- SSA, chordal
        v                            +-- avail.py     MemRef -> Value, x-block
  calls.py    absorb, reduce         +-- reencode.py  with_operand()
  forward.py  drop a load            +-- simplify.py  round trips
  memory.py   dead stores            +-- select.py    an Op -> bytes
        |                            +-- layout.py    place, relax branches
        |                            +-- wholeseg.py  a whole segment
        |                                  |
        +---------------+------------------+
                        |
                        v
                  rewrite.py --> .OBJ out
```

Both towers reach the output, but by very different amounts.

The left one carries every transform that changes a shipped program. So
does one piece of the right: `avail.py` + `regalloc.py` + `reencode.py`,
joined in `rewrite._substituted()`, serving 60 redundant memory reads from
a register instead.

The right-hand tower can now also emit whole segments -- `select.py` turns
an Op back into bytes, `layout.py` places them and relaxes branches to a
fixed point, `wholeseg.py` composes the two and hands `relocate.py` a new
code block. It runs end to end and the programs it builds run correctly on
all twelve configurations, but `rewrite.py` does not call it: today it is
reachable only from `tests/test_e2e.py`. That is the milestone the roadmap
calls retiring the machine arm, and it is not done until `rewrite.py` goes
through here instead.

The rest of the right-hand tower is verified work with no consumer.
`consts.known()` folds and nothing reads the result; `wide.py` re-derives
at MIR level the same 192 carry pairs `lift.py` already widens on the
machine side.

## The optimisation passes

What exists, what it runs on, and whether it changes the program.

```
  pass                  level      emits   what it does
  --------------------  ---------  ------  ----------------------------
  absorption            machine    yes     B$MUI4 -> imul, inline
  strength reduction    machine    yes     x2^n -> shl, x3/5/9 -> lea,
                                           /2^n -> sar with sign bias
  widening              machine    yes     add/adc pair -> one 32-bit op
  dead store removal    machine    yes     a store nothing reads
  load forwarding       machine    yes     a load whose register has it
                                           (block-scoped only)
  operand substitution  MIR        yes     `add ax,[y]` -> `add ax,si`,
                                           cross-block; 60 corpus reads
  const prop + fold     MIR        no      known(): values and arithmetic
  register allocation   MIR        part    colour() built; live() is what
                                           operand substitution asks
  re-encoding           MIR        yes     with_operand(); with_registers()
                                           has no caller
  native x87            machine    opt-in  fpu.py: the emulator's int 34h
                                           back to the ESC it stands for.
                                           1.88x on bench/fpbench.bas, and
                                           the only float win there is
  whole-segment emit    MIR        tests   select + layout + wholeseg; runs
                                           right on all twelve, not yet
                                           called by rewrite.py

  not built yet:  CSE, LICM, dead code elimination proper
```

Operand substitution is the one pass that crosses. It changes where the
second operand is *read from* and nothing else -- the destination is
untouched, so nothing downstream is rewritten, which is what makes it work
on a two-address machine where forwarding a use does not. It is also why
an accumulate is safe: `and cx,[x]` keeps its `and`.

## Where the join happens

A load is removable when memory says the cell's content is known *and* a
register still holds it. `memory.py` answers the first question
cross-block. Nothing answers the second cross-block.

```
   memory.py                            mir.py + regalloc.py
   "what does this address hold?"       "what value is this, and where
          |                              does it live?"
          |  cells, aliasing,                   |  SSA, phis, dominance,
          |  forward + backward                 |  chordal colouring
          |  dataflow to a fixed point          |
          +------------------+------------------+
                             |
                             v
                         avail.py
              MemRef -> Value, forward, cross-block
                             |
                  + regalloc.live() -- is it still there?
                             |
                             v
                  rewrite._substituted()
```

Both halves are needed and neither is enough. The cell's content being
known says the read is redundant; the value being live says a register
still has it. Where they disagree the read stays, and they disagree often:
of 481 redundant reads in the corpus, 73 have a live value holding their
bytes, 42 have a dead one, and 366 have none at all.

That gap is BC spilling around calls, and it is the whole reason
`bench/nbody.bas` gets nothing from this pass. All 27 of its redundant
reads are spills:

```
  mov [bp-18h],ax     the spill
  call B$MUI4         ax is gone -- the call clobbers it
  add ax,[bp-18h]     the reload, and it is real work
```

No dataflow removes that. Absorbing the call does, because an absorbed
`imul eax,ecx` clobbers far less than `B$MUI4` -- which is why absorption,
not forwarding, is what moves that benchmark.

## The gates

Nothing here is trusted because the host suite is green; the host suite
being green is how the two worst bugs so far got in.

```
  uv run pytest -m "not e2e"     15666 host tests, seconds
  uv run pytest                  adds DOSBox, the real thing running
  uv run python tools/matrix.py  17 programs x 12 real compiler configs
  uv run python tools/mutate.py  40 deliberate breakages, each must be caught
  uv run python tools/bench.py   a real program, timed on the 8253
```

`tools/matrix.py` is the one that catches semantics. It found the inverted
kill direction in `memory.py`'s backward pass, and it found `forward.py`
treating `and cx,[x]` as a load -- both with every host test passing.

`tools/bench.py` is a gate too, and not only a number. It found the one
bug matrix could not: `forward.py` reading the base register of
`fld dword ptr [si]` as the load's destination, which deleted the second
of two pushes and slid the x87 stack. No program in the suite had a
two-deep float expression, so nothing there could have caught it, and
`fuzzgen.py` could not generate one either -- it made two arrays, handed
the widths out in the order INT, LNG, SNG, DBL, and so had never once
produced a float array. `suite/fpdeep.bas` covers the shape now and the
generator makes one array of each width.
