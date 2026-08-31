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
        v                            +-- reencode.py  with_registers()
  calls.py    absorb, reduce               |
  forward.py  drop a load                  v
  memory.py   dead stores            mir.lower() -- proven lossless,
        |                            2,060,168 bytes byte-identical
        v                                  |
  rewrite.py --> .OBJ out                  v
                                     tools/dump.py
                                     ...and nothing else.
```

The right-hand tower is real, verified work. `raise_body()` builds proper
SSA -- iterated dominance frontiers, phi placement, a dominator-tree
renaming walk -- and `lower()` round-trips it back to byte-identical
machine code across the whole corpus. `regalloc.colour()` is an optimal
SSA allocator. None of it reaches `rewrite.py`.

Where the two towers overlap, the left one wins by being wired:
`lift.py`'s widening is what emits; `wide.py` re-derives the same 192
carry pairs and 871 comparison-branches at MIR level and feeds a dump.

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
  const prop + fold     MIR        no      known(): values and arithmetic
  register allocation   MIR        no      colour(): where values live
  re-encoding           MIR        no      one instruction, remapped

  not built yet:  CSE, LICM, dead code elimination proper
```

The gap between the two halves of that table is the project. The MIR
passes are correct and idle; the machine passes emit but are limited to
what one basic block can see.

## Where the join has to happen

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
                        forward.py
                   makes this join today
                   BLOCK-SCOPED ONLY
```

That is the open work, and it is one join, not a rewrite of either side.

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
