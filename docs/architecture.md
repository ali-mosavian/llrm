# Architecture

An `.OBJ` in, an `.OBJ` out. Every module below either answers a question
about the bytes or changes them, and the split between those two is the
thing worth seeing: most of the analysis is finished and correct, and only
some of it is wired to emission.

## The pipeline

```
  BC.EXE                                                       LINK.EXE
     |                                                             ^
     v                                                             |
 +--------+     +-----------+     +----------+     +---------+     |
 | .OBJ   |---->|  decode   |---->| analyse  |---->| rewrite |-----+
 | bytes  |     |           |     |          |     |         |
 +--------+     +-----------+     +----------+     +---------+
                 omf module        the middle       rewrite
                 declen blocks     of this file     relocate
```

`rewrite.py` is the only module that writes bytes. Everything else answers
a question, and answering wrongly is silent -- which is why the gates
(`tools/matrix.py`, `tools/mutate.py`) run real compilers rather than
trusting the host suite.

## Two towers

The single most important thing about this codebase's shape: there are two
representations reaching up from the bytes, and only one of them is
connected to the output.

```
              .OBJ bytes -- omf.py, declen.py
                        |
              blocks.py, module.py, ir.py
                        |
        +---------------+----------------+
        |                                |
        v                                v
  MACHINE-CODE TOWER               MIR / SSA TOWER
  (emits)                          (analysis only)
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
  forward.py  drop a load                  |
  memory.py   dead stores                  |
        |                                  |
        +---------------+------------------+
                        |
                        v
                  rewrite.py --> .OBJ out
```

Both towers now reach the output, but by different amounts. The left one
carries every transform that changes real programs. The right one carries
exactly one: `avail.py` + `regalloc.py` + `reencode.py`, joined in
`rewrite._substituted()`, which serves 60 redundant memory reads from a
register instead.

The rest of the right-hand tower is still verified work with no consumer.
`lower()` round-trips SSA back to byte-identical machine code across the
corpus and nothing routes emission through it; `consts.known()` folds and
nothing reads the result; `wide.py` re-derives at MIR level the same 192
carry pairs `lift.py` already widens on the machine side.

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
  uv run pytest -m "not e2e"     8941 host tests, seconds
  uv run pytest                  adds DOSBox, the real thing running
  uv run python tools/matrix.py  16 programs x 12 real compiler configs
  uv run python tools/mutate.py  37 deliberate breakages, each must be caught
```

`tools/matrix.py` is the one that catches semantics. It found the inverted
kill direction in `memory.py`'s backward pass, and it found `forward.py`
treating `and cx,[x]` as a load -- both with every host test passing.
