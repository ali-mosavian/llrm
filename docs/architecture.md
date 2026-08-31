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

## The layers

Read bottom-up. An arrow is an import; a module never imports from a layer
above it.

```
  L5  driver        rewrite ................ the only writer of bytes
                       |    \
                       |     `--- bodyedit ..... a whole body replaced
                       |     `--- price ........ what it cost, in cycles
                       v
  L4  transforms    calls ....... a runtime call, absorbed or strength-reduced
                    forward ..... a load that need not happen
                       |
      analyses      memory  mir  consts  wide  regalloc  reencode
                       |     |     |      |       |         |
                       |     |     `--- known values, folded
                       |     |     `--- 32-bit ops BC had to write as two
                       |     |     `--- which register a value lives in
                       |     |     `--- one instruction, re-encoded
                       v     v
  L3  total decode  ir ......... every instruction in a body, as a Node
                       |
                       v
  L2  facts         lift  flags  loops  extent  runtime  stack  registers
                       |     |      |      |       |       |
                       |     |      |      |       |       `- where args are
                       |     |      |      |       `--------- what a call clobbers
                       |     |      |      `----------------- whose code a byte is
                       |     |      `------------------------ what nests in what
                       |     `------------------------------- which flags are live
                       `------------------------------------- pairs, widened
                       v
  L1  structure     blocks ...... which bytes are code, and where control goes
                    module ...... one BC module, as the analysis sees it
                       |
                       v
  L0  bytes         omf ......... the object format
                    declen ...... instructions, via iced-x86
```

`relocate.py` sits beside `rewrite.py` rather than under it: when code
moves, the fixups move with it, and that is the driver's problem alone.

## Two ways up from the bytes

There are two representations, built for different questions, and neither
subsumes the other.

```
     blocks + ir                          blocks + ir
          |                                    |
          v                                    v
      memory.py                             mir.py
   "what does this               "what value is this, whatever
    address hold?"                 register it happens to be in"
          |                                    |
          |  cells, aliasing,                  |  SSA: phis, dominance,
          |  forward + backward                |  one value per definition
          |  dataflow to a fixed point         |
          v                                    v
   redundant_loads()                      regalloc.py
   dead_stores()                          "and which register
                                           should it live in"
```

`memory.py` reasons about addresses and is cross-block. `mir.py` reasons
about values and is cross-block. `forward.py` connects them -- a load is
removable when memory says the cell's content is known *and* a register
still holds it -- and today it makes that connection block-scoped only.
That gap is the open work.

## Wired, and not

The distinction that matters most, because a finished analysis with no
consumer looks exactly like a finished feature.

```
   emits bytes today          analysis only, no consumer
   -----------------          --------------------------
   calls.py    absorb         mir.py      SSA over a body
   calls.py    reduce         regalloc.py where values live
   wide.py     widen          reencode.py one instruction, remapped
   forward.py  drop a load    consts.py   known values
   memory.py   dead stores    memory.py   the rest of what it knows
```

`reencode.py` and `regalloc.py` were built for a transform that measured
out at zero: forwarding a load into a *different* register than it names.
BC does not emit that shape -- it keeps values in memory and reaches for a
register only to compute -- so the machinery is sound, gated, and idle.

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
