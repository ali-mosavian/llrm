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

Structured against LLVM's backend, whose source is at
`~/work/other/llvm-project` and whose `TargetPassConfig::addOptimizedRegAlloc()`
is the order the machine half mirrors.

    parse      bytes  -> Module     omf.py, module.py
    raise      Module -> MIR        mir.py
    opt        MIR    -> MIR        transform.pipeline()
    lower      MIR    -> LIR        lower.py
    machine    LIR    -> LIR        flow.machine()
    write      LIR    -> bytes      objwrite.py

`qbopt/flow.py` is the pass config and holds the order and nothing else.
The machine phases, against LLVM's own:

| ours | LLVM |
| --- | --- |
| `phielim.PhiElimination` | `PHIElimination` |
| `twoaddr.TwoAddress` | `TwoAddressInstructionPass` |
| `coalesce.Coalescer` | `RegisterCoalescer` |
| `splitkit.Splitter` | `SplitKit` |
| `allocate.RegAlloc` | `RegAllocBase` + `VirtRegRewriter` |
| `spiller.Spiller` | `InlineSpiller` (inside RegAlloc's loop) |
| `prologue.Prologue` | `PrologEpilogInserter` |

And beside them: `verify.py` is `MachineVerifier`, `frame.py` is
`MachineFrameInfo`, `target.py` is `TargetRegisterInfo` + `TargetInstrInfo`.

### Spilling, and the frame it needs

`RegAlloc.transform` is LLVM's `RegAllocBase::allocatePhysRegs` loop:
assign, and where that spills, make the spill real and assign again. The
corpus settles in one round.

`frame.py` hands out a slot per spilled value, below the deepest
displacement BC's own code already reaches. `spiller.py` turns every
definition into a store and every use into a load out of that slot, each
through a fresh value that lives across one instruction -- which is what
makes the spilled value's range vanish and frees the register it wanted.

`prologue.py` is the part BC makes awkward. **Its bodies have no prologue
of their own to grow**: the runtime sets the frame up before the body runs,
so a spill slot is not in the frame BC declared and taking it means
lowering sp ourselves. `sub sp,N` at entry, `add sp,N` before every return
-- and a body with no return is refused, unless it calls `B$CENP`, the
runtime's exit, which never comes back. Every main body ends there, which
is the difference between spilling being available in one and not.

Asked of the whole body rather than of its last instruction: BC pads the
end of a code segment with zeros, `00 00` decodes as `add [bx+si],al`, and
reachability walks in -- so the last instruction is routinely not a
terminator at all.

### Splitting

`splitkit.py` makes the one cut that pays on this corpus: a value defined
before a loop, read after it, and touched nowhere inside. Its range crosses
the loop and holds a register through every iteration; cut into "before"
and "after", the loop body sees neither. LLVM prices every candidate cut;
this makes the one whose shape `docs/hoist-blocker.md`'s programs wanted.

### Subregisters

`target.LANES` says which bytes of its root each register is -- LLVM's lane
masks, with four lanes, which can simply be written down. `ir.ROOT` folds
every name to its 32-bit parent, which answers "same register file entry"
and not "same bytes", and al and ah are where those differ.
`target.overlaps()` is the exact question.

Two pass bases rather than one generic over the form: `MIRTransform` and
`LIRTransform`. A MIR pass may name no register and a LIR pass may name
nothing else, and a shared base would be a place for a pass to be written
that does not know which half it is in.

**Analyses are not phases.** `liveness.py` (over MIR values), `intervals.py`
(over LIR ids) and `loops.py` answer questions and change nothing, which is
why the pass config does not list them.

### The target

`target.py` is LLVM's `TargetRegisterInfo` and `TargetInstrInfo` in one
module -- one target, so one module. It holds the register file, the widths
each register is named at, the register classes, the allocation order, and
what each instruction requires of a register whether or not it names it.

It is written down once because it was written down six times. `AVAILABLE`
and a narrow-name table lived in `regalloc.py`, a second copy of that table
in `select.py`, the addressing class in `lir.py`, the root map in `ir.py`,
and a pass that wanted any of them reached for whichever module it already
imported.

The two copies were **not** the same table, which is the part worth
recording: `select.py` built its from three explicit rows and covered width
1; `regalloc.py` built its from `ir.ROOT`, which has no byte entries.
Reading them as duplicates and keeping the narrower one broke every object
in the corpus -- an `ir.Held` of width 1 then resolved to its root, and
`mov [k],al` became `mov [k],eax`. `lir.py` now holds only the form.

**A register class is the unit an allocator works in.** LLVM allocates
within a `TargetRegisterClass` and orders the candidates with an
`AllocationOrder`. Asking "any of the six" is only right when every operand
can take any of the six, and 16-bit addressing reaches memory through bx,
bp, si and di and nothing else. `allocate.classes()` confines a value some
instruction reaches a cell by; the fixed requirements -- `imul`'s dx:ax,
`cwd`'s eax, a shift's cl -- arrive as pins from the raise.

### Live intervals, and what a spill costs

`intervals.py` is LLVM's `LiveIntervals` and `CalcSpillWeights` in one
module. A **slot index** numbers every point a value can start or stop
being live, a **segment** is a half-open run of them, and an **interval**
is the segments one value occupies. Two slots per instruction rather than
LLVM's four -- read and write -- because nothing here needs early-clobber
yet, and inventing the distinction before a pass asks for it would be four
times the indices for none of the answers.

The weight is LLVM's `normalizeSpillWeight`:

    sum(references, each weighted 10 ** loop depth) / (live slots + grace)

The division is the half a plain count misses. Two values referenced
equally often are not equally worth keeping if one is live for three
instructions and the other for the whole body: spilling the long one frees
a register for longer, so it is the cheaper one to spill.

### What lowering has to do about a cell

`mir.MemRef` says which bytes an operand is and what its address depends on
-- the alias question. `ir.Mem` says how to encode it: which register
reaches it, and how wide the displacement field was, which is not how wide
the number needs to be. Neither derives from the other. Where the original
instruction had a memory operand in the same position its encoding is
taken; where a pass put the cell there the encoding comes from the address,
and only for the two spaces that determine it -- a frame slot through bp, a
segment-relative cell through nothing, both with a two-byte displacement.

### Allocation

`RegAllocGreedy`. Largest range first, out of a priority queue -- "assigning
larger ranges first" is LLVM's own reason, and it is that a long range has
the most ways to conflict, so it wants placing while the file is empty. For
each range:

    assign   a register nothing live at the same time is using
    evict    take one from ranges that cost less, and put those back on
             the queue to find another
    split    give up on one register for the whole range
    spill    give up on a register

A range that fails one stage comes back at the next, and `Stage` only moves
forward, which is what makes the loop terminate. What is assigned to each
register is kept as a list of intervals -- LLVM's `LiveIntervalUnion` --
because the question asked a thousand times is "does this range overlap
anything already in there".

**Eviction replaced a branch-and-bound search.** That search was exactly
optimal and exponential. It went for the reason LLVM reached greedy: the
cost model is what decides, a cheap range moving aside for an expensive one
is the whole of the decision, and a search that finds the same answer by
trying everything has only proved the cost model right at a price that
grows with the body.

**A call carries a register mask, not a value per register.** LLVM's
`MO_RegisterMask`: the call says which registers survive it, and the
allocator refuses a destroyed one to any range live across the call --
`checkRegMaskInterference`. `runtime.py`'s contracts already knew this per
routine, measured against the runtime's own source; nothing asked.

Before it, a call *defined* a value for every register it clobbered. **114
of nbody's 162 call defines were read by nothing** and each still got an
interval, competed for a register and was spilled -- a store for a value
nobody wanted. Dropping them is only sound because the mask is honoured:
without `_clobbered` it would be a miscompile, not an optimisation.

Worth **5,615 bytes** over the corpus.

**A reload cannot be spilled again.** Its value is live across one
instruction, so its weight -- references over live range -- is tiny, and
under a cost model it never wins a register. Spilled again, it puts a load
in front of a load: three values spilled every round and three
instructions added every round, for ever, measured on fpcsex-p-g2-zd.
`LiveInterval::markNotSpillable` is LLVM's name for the answer; here the
spiller hands its reloads back and they weigh infinity -- while they are
still short. LLVM asks `isZeroLength()` for the same reason: a value that
reached emission with a long range is not a reload any more, whatever made
it, and refusing to spill one that crosses fourteen calls clobbering every
register is refusing to compile the program.

### No fallbacks

An operand the allocation does not cover raises `allocate.Unplaced` naming
the value. A cell whose encoding cannot be derived raises `lower.Unlowered`
naming the address and the space. A value the allocator chose to spill
raises `allocate.Spilled` naming the values and the cost. Quietly putting
back what the raise saw is how one wrong instruction reaches an object with
nothing reported.

### The MC layer

`asm.py` is `MCAssembler`: how long each instruction is, where each
therefore lands, which branches can shrink now that everything is closer,
and where each fixup ended up. `select.py` is the `MCCodeEmitter` above it
and `relocate.py` the `MCObjectWriter` below. `layout.py` is what is left:
ordering the bodies, the tables and padding it carries verbatim, and the
byte-preservation check -- which is ours, not LLVM's, because ours emits
into an image BC laid out.

**An assembler does not allocate.** `layout.rebuild` used to colour a body
when handed no assignment, which made it a phase nothing downstream could
be told had already run: `objwrite.py` runs after a real allocator and was
allocated over a second time. `layout.allocated()` is that work, called by
`wholeseg.py` before the assembler, and `rebuild` now remaps only what it
is handed.

### An inserted instruction has no node

`node` is the instruction an operation was raised from, and every question
answered by reading the original bytes goes through it. A phi's copy, a
two-address move and a spill's store carry the *address* of the
instruction they stand beside, so a far call's `9a` was read at that
address and the inserted instruction claimed the call's own fixup.
Nineteen objects said `call has 1 fixups and 0 fields to put them in`,
naming the call, which was not the operation asking.

### Where it stands

**487 of 487 objects write**, 709,230 -> 640,122 bytes. Nothing has run
that output, so no correctness is claimed.

**What LLVM has and this still does not:**

- `RegAllocGreedy`'s eviction and its split candidates. `splitkit.py`
  makes one cut, in response to a failure to assign; LLVM prices many.
  Run on every crossing range instead of on the ones that failed, it cost
  12,329 bytes over the corpus and freed nothing.
- `BranchFolding`, `MachineCopyPropagation`, `MachineLICM`,
  `DeadMachineInstructionElim`, `MachineCSE`, `PeepholeOptimizer` -- the
  late optimisations, cheap now that the frame exists.
- Register classes beyond addressing: the segment registers are named in
  `target.py` and are still not a class the allocator works in.

Not shipped. `rewrite.py` still calls `wholeseg.rebuilt`, and nothing has
run this output. `tools/flow.py` runs it.

## The machine arm is gone

It patched BC's own bytes in place: an idiom matched by address and
adjacency, rewritten where it stood. Measured over `fixtures/omf` before it
went:

    machine arm alone   682,978    -26,252
    MIR arm alone       642,807    -66,423
    both                641,542    -67,688

So it was worth **1,265 bytes on top of the MIR arm** -- 1.9% of the gain --
for 3,256 lines whose every matcher is an address and an adjacency, which
is exactly what stops any pass above from moving anything. The suite links
and runs on the MIR arm alone.

`forward.py`, `memory.py`, `registers.py` and `price.py` went with it, along
with `rewrite.py`'s region planner. `calls.py` stays: `mir.py` asks it what
a runtime call *is* at the raise, and `select.py` what an absorbed one
becomes. `lift.py` stays for `ir.py` and `calls.py`.

`--take`, `--max-regions` and the region report bisected the arm by region
index. What replaces them is `transform.applied(only=...)`, which runs one
MIR pass -- a better question anyway: a pass has a name, a region had a
number.

**A long divide now leaves its loop.** That xfail passed the moment the arm
was out: it had been holding the loop in place.

## Aliasing, after LLVM

`MemRef` names the *object* a reference is in even where it cannot name the
byte. LLVM's `PseudoSourceValue`: a machine memory operand whose exact
address is unknown still says whether it is a stack slot, a constant pool
or the GOT, and two different kinds never alias.

It is what a push needs. The raise names the slot a push lands in while it
knows the stack depth and gives up on the address when it does not -- but a
push is still a push, and a push cannot land on a global whatever the
depth. Measured before: **two thirds of the corpus's pushes (354 of 520)
had no nameable slot**, and each one aliased every named cell in its body.

    963 references name the byte
    413 name only the object
    793 name neither

### Sequential execution buys nothing; the segment layout might

Everything here already assumes one path at a time. `may_alias`, `avail`
and `induction` walk in program order and nothing anywhere assumes
concurrency. The blocker is not a parallel writer -- it is a callee running
*sequentially* between a store and a load.

What is adjacent and real: **DGROUP holds more than one segment**, and the
object says which is which.

    seg 1   MATRIX_CODE  class BC_CODE
    seg 2   BR_DATA      class BLANK      the runtime's data
    seg 5   BC_DATA      class BC_DATA    this program's variables
    seg 9   BC_CN, BC_DS, BC_SA           BC's own tables

`B$PRINT` writes through its own buffers, which are in BR_DATA. It cannot
touch BC_DATA unless the program hands it a pointer. `module.program_data`
names the segment.

**And the guarantee is not earned.** Asked per body -- "does this body hand
out an address at all" -- it grants 47 cells and is wrong: all 32 `-p-g2`
fixtures push a relocated immediate, which is what handing over an address
looks like here. `lea` is not how BC does it, and testing for
`Operation.ADDRESS` found none of them. That version was written, measured
at 47 cells, and reverted.

Asked **per cell** -- "is *this* variable's address ever taken" -- 59 cells
have one nothing takes, 42 do, and the analysis is sound for a scalar. It
is not written. What it still owes is transitive escape: a descriptor whose
address is passed may point at another cell.

### Why promotion is not the pass to write

Measured, and it changes the answer given a day earlier. Of 312 candidate
cells the corpus offers, **none is promotable**, and after the push fix the
reason is a single honest fact rather than a modelling gap:

    704 of 778 blind references are calls whose contract says writes=ANY

Twenty of the twenty-one routines with that contract are `established`,
measured against `rt/prnval.asm`: `B$PRINT` and everything falling into it
write through the runtime's own buffers, which live in DGROUP beside the
program's variables. A global genuinely cannot be promoted across a PRINT,
and every program in the suite prints.

Frame slots escape that -- a callee cannot see an un-escaped one, LLVM's
alloca rule -- but BC puts QuickBASIC's variables in DGROUP, not on the
frame: the whole corpus has **24 frame cells**, 15 of which a named
reference may alias.

So the pass that pays is not mem2reg. It is forwarding *between* clobber
points, and copy propagation, which memory aliasing does not touch at all.

## Induction variables

`induction.py` is LLVM's `ScalarEvolution` in the one shape this needs: a
value is `start + step * iteration` -- an affine recurrence, `{start,+,step}`
in LLVM's notation -- or it is not one. Everything a loop does to an array
index is that.

Split from any transform on purpose, the way LLVM splits it:
`LoopStrengthReduce` asks and rewrites, `IndVarSimplify` asks and does
something else, and a measurement asks and does nothing.

**It is the largest item left.** `docs/targets.md` names what closes each
program's gap, and induction variables come up in six of the thirteen --
more than any other item, ahead of LICM at four. `stride` is "a division
that is really a counter", `matrix` wants a stride of 2(w+1) on the
address, `harr` wants "one add, not a multiply and two segment loads".

Measured over the 32 `-p-g2` fixtures: **29 loops with a counter, 29
counters, 21 multiplies derived from one** -- 18 `imul word [w]` and 3
shifts, every one of them per iteration.

Three things had to be right and each was wrong first:

- **A cell can be loop-invariant.** BC reads the multiplier straight out of
  memory, so 23 of the 28 candidate multiplies had a `Cell` where an
  invariance test that only tracked values looked for one. None qualified.
- **Proving it needs the object bounds.** An indexed store into an array
  may land anywhere in the segment unless something says how big the array
  is, and then every scalar in DGROUP reads as written by it.
  `module.landmarks()` knows where each object ends.
- **The multiply loads.** `imul word [w]` reads its own operand, so
  excluding anything that touches memory excluded every site the analysis
  exists for.

## Strength reduction, and why it is off

`strength.py` is LLVM's `LoopStrengthReduce` in its classic form. Where
`induction.py` says `j = i * m` with `i = {start,+,step}` and `m`
invariant, `j` is `{start*m,+,step*m}` -- so the multiply need not be in
the loop at all. One multiply in the preheader, an add of `step*m` at the
latch, and where BC wrote `imul word [w]` every iteration there is an add.

No phi is written. A fresh variable assigned in the preheader and again at
the latch *is* one, and `mir.resolved()` puts it at the header, because it
renames per variable and that is what a variable written on two paths
means. Writing one by hand would be saying the same thing twice.

**It fires on 7 of the 32 fixtures and it is off, because it costs.**
Measured through the flow path, where the two-address fixup handles the
form it invents:

    harr    8.6x -> 9.8x        segld  6.4x -> 8.0x
    split   4.3x -> 6.2x        matrix 3.4x -> 3.5x

Every program it touches gets worse. The multiply it removes reads memory
and the add it inserts reads the same memory, so the loop body is no
cheaper -- and the new counter holds a register for the whole loop, which
is what the two worst results are.

**LLVM's LSR is mostly the cost model this does not have.** It enumerates
formulas for every use, prices them against register pressure, and picks;
that machinery exists because the choice is not obvious, and here it is
obviously wrong four times out of four. The analysis is sound and tested;
what is missing is the pricing.

Two other things it needs and has:

- **Only what can be removed.** `_without` refuses a deletion no neighbour
  can account for and leaves the operation standing -- which here would
  compute the value twice, once in the loop and once in the counter its
  readers now name.
- **Once each.** A multiply inside a nest is derived in every loop that
  contains it.

## What the runtime actually writes

`runtime.py` said `writes=ANY` for twenty-one routines, `established` and
cited to the runtime source. True, and useless: it says "writes memory",
which every one of them does, and a caller needs to know *whose*.

`tools/runtime_writes.py` answers it from the linked image, which is on
disk. Over `B_NBODY`, `B_ARRIDX` and `B_MATRIX`:

    writes to a fixed address    150 _DATA  105 _BSS  31 BR_DATA  1 BR_SKYS
                                   0 BC_DATA
    not to a fixed address       191 through a pointer  28 its own frame

**No runtime write names a cell in BC_DATA**, the segment BC puts a
program's variables in -- it cannot, since that segment's position is
per-program and the runtime is pre-compiled. Nor does any data-segment
fixup in the corpus hand it a pointer into one: 151 are `BC_CN -> BC_CN`
and one per program is `BC_SA -> its own code`.

So a runtime call reaches a program's variable only through a pointer the
program pushed. `Memory.OWN` says that, and it is GCC's `ipa-modref` split:
writes at a fixed address, and writes through parameter N.

**Three instrument bugs on the way to it**, each the same mistake:

- Six apparent BC_DATA writes were `mov word [es:7Ch],5D6h` and
  `mov [es:7Eh],ds` -- the runtime installing an INT 1Fh vector at 0:7C.
  The classifier assumed `ds` for every displacement.
- Fifteen more were in EMULATOR_TEXT, whose `ds` is EMULATOR_DATA, class
  FAR_DATA.
- Every far call counted as a write, because a call writes the stack.

**And three on the consumer side:**

- `module.escaped` scanned `mov` as well as `push`/`lea`, so every store
  through a relocated displacement marked its own cell as escaped -- which
  is every cell.
- Five routines *do* write anything: `B$CENP`, `B$EVCK`, `B$OEGA`,
  `B$RESN` and `B$FCMD` hand control back to the program, and what they
  write is what that code writes. GCC's modref gives up on an indirect
  call for the same reason.
- `B$CENP` ends every program, so its `ANY` aliased every variable in
  every body -- until `Control.NEVER` was consulted. A call that does not
  come back writes nothing anybody can observe, which is what `noreturn`
  means.

**26 cells promotable, 64 accesses**, against zero this morning.
