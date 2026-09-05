# Architecture

An `.OBJ` in, an `.OBJ` out. Every module below either answers a question
about the bytes or changes them, and the split between those two is the
thing worth seeing: most of the analysis is finished and correct, and only
some of it is wired to emission.

## The pipeline it is becoming

```
      BC.EXE                                       LINK.EXE
        |  .OBJ                                 .OBJ  ^
        v                                             |
  +-----------+                                 +-----------+
  |  decode   |  omf declen blocks module ir    |   emit    |
  +-----+-----+                                 +-----+-----+
        |                                             ^
        v                                             |
  +-----------+                                 +-----------+
  |   raise   |  SSA over values, and every     | peephole  |
  |           |  idiom recognised here rather   +-----+-----+
  |           |  than in a pass: long pairs           ^
  |           |  named, an absorbable runtime         |
  |           |  call raised as the arithmetic  +-----------+
  |           |  it is                          | regalloc  |
  +-----+-----+                                 +-----+-----+
        |                                             ^
        v                                             |
  +-----------+                                 +-----------+
  |    opt    |  MIR in, MIR out, ten passes,   |   lower   |
  |           |  no machine anything            |  -> lir   |
  +-----+-----+ <--+                            +-----+-----+
        |          | to a fixed point                 ^
        +----------+                                  |
        |                                             |
        +---------------------------------------------+
```

Ten passes, in this order, each taking MIR and returning MIR:

```
  1  widen      two operations writing halves of one long -> one 32-bit
                operation. First, so everything after it works on whole
                values and the partial-write artifact stops mattering
  2  fold       constant propagation and folding
  3  decide     a branch whose condition is known, resolved
  4  cse        one value per computation, memory included
  5  dse        a store nothing reads
  6  licm       invariant work out of the loop
  7  scev       induction variables; an address strides, not recomputed
  8  algebraic  x * 8 -> x << 3, division by a constant -> reciprocal
  9  dead       last, so it clears what the rest orphaned
  10 place      sink a definition to its use; last, because it depends
                on what survived
```

Nothing between `opt` and `lower` and nothing after `regalloc` but
`peephole`. There is no LIR optimisation tier: if a pass has done its job
on MIR there is nothing left for one to do.

The order is the point, and each boundary is a rule:

- **The raise recognises, so no pass has to.** A long is one 32-bit value
  and `B$MUI4` is a `MULTIPLY` the moment the body is raised. Recognition
  done later is recognition done in a pass that must know about x86 --
  which is how `widen` and `absorb` came to hold 141 machine references
  between them.
- **mir is machine-independent, and so is every pass over it.** A value is
  `(id, at, kind)` and lives nowhere. Where BC kept it is `MirBody.origin`,
  a side map that lowering and the allocator's identity baseline read and
  *nothing else may* -- with one sanctioned exception, `widen` asking which
  register pair a long arrived in. A pass that names a register is doing
  the allocator's job with none of its information; five attempts at that
  are in the history and all produced wrong programs.
- **opt runs to a fixed point**, on one body. Not through emission: a pass
  that re-raises from bytes loses the SSA the one before it built, and
  every marker with it. That is why a live range split used to come back as
  an ordinary move and be hoisted again.
- **lower chooses encodings, not registers.** `lea` against `shl`, which
  form of `imul`. Its output is machine operations over abstract variables.
- **lir says what the machine requires** of an operand: a fixed register
  (`imul` multiplies by ax and names it nowhere), a register class (16-bit
  addressing reaches memory through bx, bp, si or di), a two-address
  instruction reading and writing one register. Checked against BC's own
  assignment over the corpus -- 431 fixed and 1,290 class requirements, and
  any that named a register BC had no value in would be a wrong
  requirement.
- **regalloc assigns, splits and spills.** The only thing that decides
  where a value lives. When an instruction requires a value somewhere it
  cannot live, the answer is a live range split -- a move putting it back
  just before the instruction that needs it.
- **peephole runs after allocation**, because what is worth rewriting
  depends on what ended up where. The segment reload, a redundant move, an
  addressing mode: all of them questions about what ended up in what.


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

Where each of today's twelve lands in the architecture above.

```
  today                 becomes             why
  --------------------  ------------------  ------------------------------
  fold                  fold                already MIR in, MIR out
  decide                decide              already
  dead                  dead                already, and runs last
  hoist                 licm                already; needs cse ahead of it
  place                 place               already, and runs last
  widen                 widen               stays a pass: only 169 of the
                                            corpus's 1,476 long values are
                                            joined by a carry, so most
                                            pairs are dataflow rather than
                                            an instruction pattern
  absorb                the raise           a B$MUI4 call is a MULTIPLY
                                            the moment it is raised
  forward               cse                 a read served from a value
                                            already held is not a separate
                                            question
  segments              cse + licm          the descriptor word is loaded
                                            once and does not change
  drop_loads            cse                 redundant load elimination
  drop_stores           dse
  strength              algebraic + lower   x * 8 -> x << 3 is a fact about
                                            values; the lea is an encoding
```

Not built: `cse`, `scev`, `algebraic` as a pass of its own, `peephole`.
Between them they are most of what stands between the suite and 1.5x, and
none of them may see a register.

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

## The flow, as passes

`qbopt/flow.py`. Six steps, five forms, each taking one and returning the
next:

    parse     bytes -> Module          omf.py, module.py
    raise     Module -> MIR            mir.py
    passes    MIR -> MIR               transform.py
    lower     MIR -> LIR               lower.py
    allocate  LIR -> LIR               allocate.py
    write     LIR -> bytes             objwrite.py

`lir.LirBody` is the form below MIR: one `Insn` per operation, every
operand a location, no values. `what` is None where the bytes are carried
rather than generated.

**Lowering's real work is the cell.** `mir.MemRef` says which bytes an
operand is and what its address depends on -- the alias question. `ir.Mem`
says how to encode it: which register reaches it, and how wide the
displacement field was, which is not how wide the number needs to be.
Neither derives from the other. Where the original instruction had a memory
operand in the same position its encoding is taken; where a pass put the
cell there, the encoding comes from the address, and only for the two
spaces that determine it -- a frame slot through bp, a segment-relative
cell through nothing, both with a two-byte displacement.

**Allocation reads LIR, and only LIR.** It took a `MirBody` at first, for
one reason: liveness and interference read `op.defines` and `op.uses`, and
a lowered instruction carried neither -- only `ir.Held(value_id)` inside an
encoded operand. So the last pass in the machine half reached back up a
form to ask a question about its own input. `lir.Insn` carries the two
lists now, by id, and `LirBlock.arrives` carries what a phi defines; LIR
has its own liveness and its own interference graph over those ids, and
`liveness.py` answers the same question over MIR values for the passes
above. Same algorithm, two forms, neither reading the other's.

**Allocation prices spilling and searches for the assignment.**

- The cost of keeping a value in memory is its references, each weighted
  `10 ** (loop nesting depth)`. One reference in a doubly nested loop
  outweighs a hundred outside, which is the right answer for this suite:
  the loop is where every program in it spends its time.
- Branch and bound over the values, most expensive first, pruning as soon
  as the spill bill reaches the best answer so far. Greedy colouring is
  optimal on a chordal graph with nothing pre-coloured, and neither half
  holds here -- a barrier pins every register it touches and an absorbed
  divide pins eax and edx.
- The search has a node budget. Where it runs out, the result says so
  rather than claiming an optimum it did not prove.

**No fallbacks.** An operand the allocation does not cover raises
`allocate.Unplaced` naming the value; a cell whose encoding cannot be
derived raises `lower.Unlowered` naming the address and the space. Quietly
putting back what the raise saw is how one wrong instruction reaches an
object with nothing reported.

**Measured, not shipped.** 487 of 487 objects write, 709,230 -> 631,957
bytes against 641,542 through `wholeseg`. Nothing has run that output, so
no correctness is claimed. `rewrite.py` still calls `wholeseg.rebuilt`, and
this exists so the seams are where the architecture says they are and can
be moved one at a time. `tools/flow.py` runs it.
