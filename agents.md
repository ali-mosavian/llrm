# Working on llrm

llrm, the Low Level Real Machine, is a compiler suite for 16-bit real-mode
DOS aiming at GCC/LLVM-quality code. Its frontends are QuickBASIC-family
source, C, its own language and BC-produced OMF objects; all share one MIR,
optimizer, x86 backend and OMF writer.

## Layout

- The crate is `llrm`; every command is `llrm-*`.
- Frontends live under `src/frontends/<lang>` (`c`, `qb`, `nib`, `bc`); all of
  them share the MIR, optimizer, x86 backend and OMF writer.
  `docs/architecture/source-layout.md` has the rest.

**Writing rules -- repeat them after every iteration.** Keep it short, to
the point and coherent. No rambling. This applies to all writing: replies,
docs, comments, tests, and git commit messages in Conventional Commit form.

## Measurements — the second rule

**Doubt the measurement before the subject.** A result that contradicts what
is known about the thing being measured is evidence the instrument is wrong,
not a finding. Never conclude "there is nothing here" from a number that
common sense says should be large.

- Several measures agreeing on zero is one broken assumption, not several facts.
- A question shaped so the subject's own style cannot answer it always returns
  zero. Ask what the subject actually does, not what the textbook name is.
- Verify against the raw thing -- read the assembly, the bytes, the output --
  before writing a conclusion down. One look beats any amount of analysis.
- Derive a target by hand on both sides. An estimated one flatters whatever
  it measures.
- Cost hidden behind a call, an interrupt or a helper reads as free unless the
  measure is told about it.
- Never explain an implausible result with a story. That is how it survives.

## Regression tests — the third rule

**Every issue found and fixed gets a regression test, in the same commit as
the fix.** Not "should"; the fix is not finished without it.

The reason is measured rather than assumed: every miscompile this project
has found passed 55,000 host tests and was caught by `tools/matrix.py`
running actual programs. A bug that reached the gate got there because
nothing smaller was looking, and nothing smaller will be looking next time
either unless the fix leaves something behind.

- **Write the test so that it fails first.** Revert the fix, watch it fail,
  restore the fix. A test written after the fix and never seen to fail is
  evidence of nothing -- two in this project's history were written so they
  could not fail, and both hid the defect they were named for.
- **Test the symptom, not the patch.** hotlop printing 585 for 630 is the
  fact; which branch of which helper returned early is this week's shape of
  it. Assert on the emitted code or the program's answer where you can.
- **Instruments count.** A broken measurement is an issue like any other and
  gets a test like any other -- see the second rule. The scoreboard costed
  BC's code instead of ours for a while and every ratio sat still; nothing
  would have said so. `tests/test_scoreboard.py` exists for that reason.
- **Say what it cost.** The docstring names the program that printed the
  wrong number and what it printed. That is what makes the test readable in
  a year, and it is cheaper to write while it is still understood.

## Debugging — the fourth rule

**Dump every step and every pass to a file, and diff them.** Do not reason
about where the thing went wrong. `tools/stages.py` knows; ask it.

Measured against the alternative, repeatedly. pressx's accumulator chain
took four attempts at reading disassembly and reasoning about which prune
had cut it -- the answer was that nothing had been cut, and a re-seat had
rewritten a two-address operation's source along with its destination. One
MIR dump either side of the hoist showed it immediately.

- **One file per stage, then diff.** A wrong answer at the end says nothing
  about which stage produced it. Two adjacent dumps say exactly.
- **Diff the MIR, not the emitted code.** Reading the object means inferring
  backwards through layout, allocation and emission. The op list either side
  of a pass is the answer rather than a clue to it.
- **Project away the noise.** `resolved()` renumbers every value, so a raw
  diff is unreadable; dump address and name alone and the structural change
  is one line.
- **Build the dumper before you need it.** stages.py was written mid-task
  and found the hoist bug it was written for in one line.

## The MIR boundary — the fifth rule

**Every pass takes MIR and returns MIR, and names nothing about the
machine.** Only lower, regalloc and peephole see machine form. There is no
LIR optimisation tier: if a pass has done its job on MIR there is nothing
left for one to do.

[docs/architecture/split.md](docs/architecture/split.md) is the whole of this rule: where the line
is, what each side may say, why it is not negotiable, and which parts of it
are broken today with a name against each.

- **Recognition is the raise's job.** What an idiom *is* -- a long pair, an
  absorbable runtime call -- is answered where the body is built. Answered
  later it is answered by a pass that must know about x86, which is how
  `widen` and `absorb` came to hold 141 machine references between them.
- **A pass may not name a register.** `MirBody.origin` says where BC kept a
  value, and lowering plus the allocator's identity baseline are the only
  readers -- with one sanctioned exception, `widen` asking which pair a
  long arrived in. Five attempts at doing the allocator's job inside a pass
  are in the history and every one produced a wrong program.
- **opt runs on one body, not through emission.** A pass that re-raises
  from bytes loses the SSA the one before it built and every marker with
  it. rewrite.py did that for a day: a live range split came back as an
  ordinary move and was hoisted again, and the fix was read as a fact about
  the world rather than as the bug it was.
- **A machine detail that survives into a pass is a defect in the raise**,
  not a thing to work around there. The partial-write artifact blocked five
  separate things and was worked around five times before anyone asked why
  MIR was carrying 16-bit halves at all.

## The general mechanism — the sixth rule

**Only general solutions. Never an edge-case patch.** A fix that names one
combination -- one pair of `Space` values, one runtime routine, one demo --
is a symptom of the mechanism being wrong, not a fix for it.

The alias model is what this rule is made of. It grew one hand-written arm
per category pair -- `(FRAME, SEGMENT)`, `(STACK, _)`, `(FRAME, EXTERNAL)` --
so every combination nobody had written yet was a bug waiting, and `MemRef`
ended up carrying five partial spellings of one idea: `allocation`, `beyond`,
`excludes`, `space`, `frame_bounded`. Each arrived as a reasonable local fix.
Together they are why a constant could not reach the segment of the store
that needed it, and they cost more than the mechanism would have.

- **Name the general rule the fix is an instance of, before writing it.**
  If there is no such rule, the work is to build it. Say so and cost it out
  rather than adding the arm.
- **Prefer collapsing special cases to adding one.** Five partial answers to
  the same question are worse than one whole answer, however reasonable each
  was on the day.
- **Deleting code is the evidence.** A refactor that removes
  `_allocation_disjoint` or `optimize/segments.py` landed the general form.
  One that only adds did not.
- **An assumption gets stated once, by name.** Locals-vs-globals is sound and
  load-bearing; what makes it safe is that it is written down in one place
  with its justification, not spread implicitly across a dozen match arms.

## Shared proofs — the seventh rule

**A fact about the program is proved once, by one analysis, and every pass
asks it.** A pass that derives its own proof -- its own stride walk, its own
escape scan, its own trip bound -- disagrees with the next pass's version on
the first case either missed. Design by SRP, DRY, Open/Closed and separation
of concerns:

- **One owner per fact.** Escape, provenance, trip counts, strides: each has
  one analysis. A pass consumes it; it never re-derives it.
- **A language's promise is stated by its frontend, in the IR.** That an
  access stays in its object, or a value is in range, is marked where the
  source says it -- not assumed by an analysis for every frontend.
- **Machine facts come from the target.** An analysis that needs an object
  size or address width reads it from the IR or `Where`, never a literal.
- **New cases extend the analysis, not its callers.** When a pass needs a
  fact the analysis cannot give, the analysis grows; the pass does not.

It began as a post-compilation pass over the `.OBJ` BC produces, between BC
and LINK, and much of what is here was learned there.

## The goal

**Produce what a modern optimising compiler would have produced.**

Not "make longs cheaper", which is where this started and is now one case
of the goal rather than the goal itself. Integers, longs, floats, array
addressing, loops -- all of it, judged against what a good backend would
emit for the same source, not against what BC emitted.

`docs/measurement/targets.md` carries a hand-derived optimal listing for each suite
program and `tools/opportunity.py --targets` scores against it. Done means
**every program within 1.5x**. Today the spread is 1.3x to 8.8x with a
median above 4x, so most of the distance is still ahead.

For BC's objects, one fact underlies every symptom: **BC compiles a statement at a time, so
no value outlives a statement.** Every reload, every recomputed address,
every invariant left inside a loop, two of eight x87 slots and two of six
registers -- all of it follows from that. A pass that fixes one instance
fixes a symptom; what closes the gap is keeping values in registers across
statements, which is why the register allocator is upstream of nearly
everything else on the list.

### What is actually done

Measured over the 32 `-p-g2` fixtures, bytes removed per pass:

    absorb     -269    a call to the runtime becomes inline instructions
    widen      -121    two 16-bit operations become one 32-bit one
    forward     -16    a read served from a register
    drop_loads  -14
    drop_stores -21
    segments     -6
    hoist       +39    removes memory reads, adds a split move
    place         0    fires on 4 of 32; it frees a call for absorption
    strength     +1

Absorption and widening are BC-specific -- they undo a decision BC made
because it targets an 8086 -- and they carry the whole result. Of the
classic passes only LICM fires at all, and there is no CSE, no constant
folding or propagation, no dead code elimination, no induction variables,
no addressing-mode folding. Say so plainly rather than describing the
project as if the list in the roadmap were implemented.

## The architecture

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

### Against what is there today

`fold`, `decide`, `dead`, `widen`, `hoist` (which becomes `licm`) and
`place` stay. The rest are not passes in the architecture above:

```
  absorb                  moves into the raise
  forward                 a read served from a value already held: cse
  segments                a descriptor word loaded once: cse and licm
  drop_loads              cse
  drop_stores             dse
  strength                its value half is algebraic, its lea is lower's
```

`cse`, `scev` and `peephole` do not exist yet, and no pass may see a
register once the first of them is written.

### Where the code actually is

`lir` exists and is right. `regalloc` has liveness, interference,
congruence classes and splitting. What it does not have is spilling, and
`opt` passes still name registers because the migration that stops them is
unfinished -- see `docs/architecture/variables.md` and the strict xfails in
`tests/test_allocation.py`, which name the remaining defects and the
programs that exposed them.

There is also a second, older representation: a machine-code arm
(`lift.py`, `calls.py`, `forward.py`, `memory.py`) that still ships and
still does the absorption. `docs/architecture/readme.md` has both towers. MIR is
the pass; the machine arm is legacy and is retired last.

## Rules

- **Check BC's output on all three compilers, and with the flags that
  matter.** They do not agree, and building against one is how you get a pass
  that fires on a third of what it should. The comparison operand order was
  read off VBDOS `/G3` alone and looked settled until the other three were
  checked.
- **Round trip first.** Read an object and write it back; the bytes must be
  identical. Nothing that rewrites is trustworthy until nothing it does not
  touch is disturbed. `omf.rs`'s `test_round_trip_is_byte_identical`
  asserts it.
- **Measure, do not reason, about what things cost.** DOSBox charges per
  instruction and models no latency. Use `src/cycles` for 486, P5, P6,
  K5, K6, K7 and Core, and treat those as a ranking -- they are published
  latencies, not measurements.
- **BASIC style.** Names are camelCase and
  descriptive in as few words as do that -- `posX` and `stepCount`, not `p` and
  not `theNumberOfStepsToRun`. Only a user-defined type's own name is
  PascalCase. Keywords are lowercase, always. No type suffixes: `as long` and
  `as integer` spelled out, and a function declares its return type rather than
  wearing a sigil. Every variable is declared before it is used, with `dim` or
  `redim`, never brought into being by an assignment.
- **snake_case is not available, and that is measured.** QuickBASIC 4.5 and
  PDS 7.1 both reject an underscore in an identifier -- `dim pos_x as long` is
  `Simple or array variable expected` -- and only VBDOS accepts one. A period is
  legal in all three, but `pos.x` reads as a field of a UDT, which is why the
  convention here is camelCase rather than BASIC's traditional separator.
- **Watch DOSBox, do not wait for it.** The MCP debugger drives it directly:
  `dosbox_text_screen` and `dosbox_screenshot` say what is on the screen now,
  `dosbox_run_to` and `dosbox_regs` say where the program is. Launching with a
  timeout and reading the artifacts afterwards is how a run that stopped at a
  prompt gets reported as a program that produced no output, and how a
  diagnosis ends up resting on a file that was never written.
- **Fixtures are real BC output.** Generated ones agree with whatever the
  generator believed.

## What BC actually emits

Measured across QB 4.5, PDS 7.1 and VBDOS. `docs/history/inherited-plan.md` has the
full matrix.

- A long lives in a register pair, `ax:dx` or `cx:bx`, and every operation is
  done twice. `cx:bx` is a temporary: across four programs it is **never**
  stored, only consumed register to register.
- `*`, `\`, `MOD` and every comparison are calls into the runtime.
  **Comparison pushes its left operand first; multiply and divide push it
  second.** Uniform across all four configurations, opposite between the two
  routines. Backwards is silently a different answer, not a crash.
- `/G3` (VBDOS only) pushes a long argument as one dword, 15 bytes with the
  call. Everything else pushes two words, high first, 21 bytes. PDS and
  QB 4.5 reject `/G3`; QB 4.5 rejects `/G2` as well and says `Option
  unknown` where the others say `Ignored unknown command line option`.
- A constant is split across the halves, each as short as it fits:
  `sub ax,1234h` then `sbb dx,10h`.
- Procedures open with the frame size in `cx` and a far call into the
  runtime, and close with a far call and `retf n`. Module-level code has no
  prologue at all -- it starts straight after a 0x30-byte header.
- **A LEDATA record is flushed roughly every 128 bytes**, on QB 4.5 at least,
  regardless of what statement is mid-emission when the threshold is crossed.
  It is not a per-statement or per-line boundary; a single call's own pushes
  and its call instruction can land in different records this way, and that
  region is refused rather than risked.

## What the runtime does that an instruction does not

Measured with `tests/suite/divmod.bas`, which traps under `/X` and prints `ERR`:

- `x \ 0` raises BASIC error 11. `idiv` traps with `#DE`.
- `-2147483648 \ -1` raises **nothing at all** -- `B$DVI4` returns. `idiv`
  traps here too.
- `305419896 * 252645135` overflows and raises **nothing**: `B$MUI4` wraps,
  which is exactly what `imul` does.

Multiply is absorbed as a bare `imul`, which matches.

**Divide and remainder are C's**: `mov eax,[a] / mov ecx,[b] / cdq / idiv ecx`,
and the remainder out of `edx`. No test of the divisor, because C does not make
one -- `x / 0` and `-2147483648 / -1` are undefined there, and on this machine
they fault. So BC's error 11 on a zero divisor does not survive, and neither
does its silent return from the second. That is deliberate, and it is the one
place a rewritten program can behave worse than the one BC built.

## A BC compiler behavior, not an llrm one

Found by `tools/fuzzcheck.py`'s generated corpus, not by hand: on VBDOS under
`/O`, `IF <expr> THEN ... ELSE ...` where `NOT` appears anywhere in `<expr>`
takes the branch as if testing `<expr>`'s own truthiness with the branches
swapped, not the arithmetic `NOT`'s. Measured directly --

    v = 5   : IF (NOT v) THEN A ELSE B         -> B
    v = 5   : IF ((NOT v) + 0) THEN A ELSE B   -> B   (still swapped, not buried)
    PRINT NOT v                                -> -6  (the correct two's-complement value)

-- so it survives `NOT` being a level below the top of the condition, and it
is not that `NOT` computes wrong: printed directly, it is exactly `~v`. Only
the branch BC's optimizer takes is wrong for a `NOT` of anything other than a
canonical `-1`/`0` value, which is why ordinary code never surfaces it --
`NOT` on a genuine boolean gives the same branch either way `<expr>` is read.
The object rewriter did not touch INTEGER control flow when this was found,
so a rewritten object could neither fix nor break it; `tools/fuzzgen.py` keeps `NOT` out of
every generated `IF` condition instead of chasing PDS 7.1 and QB 4.5 for the
same rule.

## What widening changes, exactly

`flageq.py` put both forms side by side over 1125 cases:

    ZF differs on 18.7% of them,  PF on 37.2%,  AF on 12.4%
    CF, SF, OF and the computed values never differ

BC leaves the **high half's** flags; one 32-bit operation leaves the whole
result's. So a region whose flags something reads afterwards must be refused
unless nothing in it writes flags. This gate was designed into the runtime
pass and then lost when its peephole matcher became a value graph -- set,
never tested -- and a `jz` after a widened `AND` could go the other way.

## Things that will bite

- **"Transfers control" is not "ends a basic block".** Calls and software
  interrupts come back. 49 of 102 blocks once ended at a far call that was
  not a block end at all, which halves what liveness can see and puts a
  boundary exactly where the store/reload worth removing lives.
- **`FF /4` and `/5` are indirect jumps and do end a block**, and the opcode
  tables give `FF` no QFLOW at all. Their targets are not computable from the
  instruction.
- **But `ON GOTO` does not compile to one.** Measured over the whole fixture
  corpus: not a single `FF /4` or `/5` appears in any module. What BC emits is
  `call far B$OGTA` followed by *inline data* -- a count byte, then that many
  `offset16` words, each one a fixup into this segment. The runtime reads the
  return address to find the table and jumps from there, so the indirect jump
  is inside `B$OGTA`, which is where the runtime pass would have seen it and
  the reason the inherited note says otherwise.
  So **`B$OGTA` does not come back to the byte after the call.** A block
  builder applying the "a call comes back" rule -- which is right everywhere
  else, and worth 49 of 102 blocks -- decodes the table as instructions.
  Knowing which call this is, is an EXTDEF lookup.
- **BC writes an .OBJ even when it reports severe errors.** "Did an object
  appear" is not a compile check, the same way "did an .EXE appear" is not a
  link check -- read the `Severe Error(s)` count out of BC's own output. An
  identifier test that trusted the file's existence reported every compiler as
  accepting underscores when three of its four cases had actually failed.
- **A call destroys the flags**, and `B$CPI4` returns its answer in them
  through `lahf`/`sahf`. A `jng` after a long operation is usually reading
  the *call's* flags, not the operation's.
- **BC leans on FIXUPP THREADs.** 34 of the 40 fixups in a module with an
  `ON GOTO` and a `SELECT CASE` name a thread rather than a target. Resolve
  them or you cannot see most of the relocations at all.
- **`int 34h`..`3Bh` is not an interrupt, it is an x87 instruction.** Under
  `/FPi` BC emits the emulator's interrupt where the ESC opcode would go, and
  the operand follows inline exactly as it would after the real opcode:
  `CD 35 46 C8` is `D9 46 C8`, `fld dword [bp-38h]`. Reading the int as two
  bytes and carrying on lands in the middle of the operand. There are 2130 in
  qb-qrender, and they are why reachability explained two of its fifteen
  modules before the decoder knew this and all fifteen after. ndisasm does not
  know it either, so the two decoders differ there on purpose.
- **The emulator's range is `int 34h`..`3Dh`, and only `34h`..`3Bh` carry an
  inline operand.** Open Watcom's `bld/watcom/h/fppatche.h` names the whole
  protocol: `FIDRQQ` is `34h`..`3Bh`, `FIERQQ`/`FICRQQ`/`FISRQQ`/`FIARQQ` are
  `3Ch` with a segment override, `FIWRQQ` is `3Dh`, the `WAIT`. `3Ch` is a
  two-byte stand-in for the *prefix* only, and the real `D8`..`DF` opcode
  follows it as ordinary bytes; `3Dh` stands in for the whole of `9B` and has
  nothing after it. Both therefore decode at the right length as plain
  interrupts, which is why qb-qrender's 201 `3Ch` and 507 `3Dh` sites mapped
  even before `declen` knew them. **The three shapes are not interchangeable**:
  widening `EMULATED` to `3Eh` and treating them alike makes `3Ch` swallow the
  ESC opcode as its operand and `3Dh` swallow two bytes of real code. `declen`
  decodes each for what it is, and refuses a `3Ch` with no `D8`..`DF` after it
  -- measured on qb-qrender, every one of its 201 has one.
- **The emulator patch is driven by a linker symbol, not only at run time.**
  An object that does floating point carries `FIDRQQ` as an EXTDEF -- 13
  references across qb-qrender, one in `tests/suite/fpemu.bas`. The runtime pass
  could not move code out from under an already-patched site; here the moving
  happens before LINK has resolved that symbol, and `tests/suite/fpemu.bas`
  establishes that it is safe -- it moves all 27 of its sites and gets BC's
  own answers in all twelve configurations.

## Where the code begins

Nothing in the OMF records says: MODEND's start-address bit is clear on every
object here. **The BASIC runtime says instead.** `MODULE_CODE` in QuickBASIC
4.5's `runtime/inc/addr.inc` is a fixed structure -- a signature word, an
eight-byte module name, nineteen more words -- summing to 48, and the offset
past it is named `O_ENT`. `rtinit.asm` spells out the contract: the first
module in the link is the entry point, and "the actual beginning of the users
code is at a fixed offset (O_ENT) from this address". So the entry is 0x30, and
the signature word makes it checkable rather than assumed: `bl` for a BCOM
module, `bm` or `br` for a BRUN one. All 110 objects here carry one, as do all
15 modules of qb-qrender.

Searching for it instead is what this replaced, and the search was wrong on 22
of the 110. A `/G3` module opens with a 66-prefixed 32-bit store, and starting
one byte into it puts the displacement field at the same offset -- so 0x30 and
0x31 explain exactly the same fixups, and a tie-break has nothing to separate
them. The five `-zd` objects prove it independently: their own LINNUM records
say 0x30 where the search said 0x31.

**`U_FLAG`, the header's last word, records the switches BC was given** --
`u_sw_v` is 0x400, `u_sw_w` 0x800, `u_sw_i` 0x4, `u_sw_x` 0x20. Under `/V` or
`/W`, PDS and VBDOS open the module with `jmp short` over a sixteen-byte
event-poll routine that only the runtime enters, so nothing falls into it and
reachability cannot find it unaided. Seeding it is why every module maps now
where fourteen did not. QuickBASIC 4.5 sets the same bits and emits no stub.

## Moving code

The runtime pass could not, and that one constraint bounded everything: a
widened region had to fit the bytes the original occupied, and the bytes it
saved became slack jumped over rather than given to a neighbour. Running it
twice provably buys nothing -- the second pass takes 0 regions and leaves
*more* blocks than it found, because the jump over the slack is itself a
terminator.

Here code may be moved. That had to be established rather than assumed: an
intra-segment offset baked into the code without a relocation would be
invisible to a rewriter and would break silently. It is not -- `ON k GOTO
L1, L2, L3` emits its three labels as three consecutive `offset16` fixups
into the module's own code segment, and `tests/test_omf.py` asserts it.

**LINK adds whatever is in the code to the fixup's target.** Measured: poking 2
into a relocated `offset16` field moved the linked address by exactly 2. BC
writes zero there and so must anything that emits a relocated operand -- a
widened instruction holds zero in its displacement and the address comes from
the fixup, or the two are added and the program reads the wrong place.

Moving code means updating all of, and the list is finite:

    FIXUPP  the offset the fixup patches, and the addend stored there
    PUBDEF  public symbol offsets
    LINNUM  line number to offset tables
    SEGDEF  segment length
    MODEND  the entry point
    code    self-relative branches -- NOT fixups, so they must be
            recomputed by decoding, which is the one part with no
            record to lean on

## Moving code across a LEDATA boundary

BC splits a code segment across several LEDATA records -- a flush roughly
every 128 bytes, on QuickBASIC 4.5 at least, with no relationship to
statement or instruction boundaries. A region whose bytes span two of them
was refused outright for a long time: merging two records looked like it
would have to reconcile BC's own backpatch records, which arrive later in
the file at an earlier offset to fill in a forward jump, and untangling that
looked like more than a concatenation. Measured, over every region the
corpus actually wants to take: the two records a crossing spans are always in
plain file order, never a backpatch pair, in all 73 cases. That measurement
is what the fix rests on -- it does not attempt the general case.

**The fix moves the shared boundary rather than merging the records.** A
region crossing from record A into record B extends A's own span to the
edit's own end and shrinks B's to start there; neither is removed. That
matters because a FIXUPP's offset is relative to whichever LEDATA precedes it
in the file -- `src/objectfile/omf.rs`'s `fixups()` tracks `base` exactly that way, and
`relocate()` mirrors it with `covered`, reset on every LEDATA it passes. An
earlier design that dropped the fully-absorbed record instead re-parented
every FIXUPP that used to follow it: measured, 72 of 73 corpus crossings have
a *data*-segment LEDATA between the leader and the record it would have
merged in, so dropping it left the FIXUPP with nothing to attach to and
refused the whole module. `qbopt/objectfile/relocate.py`'s `crossed_pair` and
`_boundary_overrides` do the moving; a record an edit swallows whole is
dropped rather than shrunk to nothing, safe only because every fixup that
would have followed it falls inside the edit and is already refused a home.

**Two edits may move two different boundaries of the same chunk.** One
region's own crossing can shrink a chunk from the left while a second,
unrelated region's crossing shrinks it from the right -- both are safe, and
have to compose into one span rather than either overwriting the other.
Keying a conflict check by *chunk touched* rather than *boundary moved* was
tried first, and refused a real pair of regions that did not conflict at
all -- correct by its own rule, wrong in fact, and caught because a run-twice
idempotence check found the region on the second pass that the first had
refused for nothing. Only two edits wanting to move the *same* boundary
actually conflict, and that needs every taken edit at once to see, which a
single region's own check cannot -- `src/rewrite.rs`'s
`drop_chained_crossings` runs after every region has been decided
independently, refusing the later of the two rather
than the runtime pass's blunter option of costing the whole module.

## What the OBJ gives that the loaded image did not

- A call site is a FIXUPP naming an EXTDEF. At run time it was a relocated
  segment:offset to compare, and the host tools could not see an absorbed
  call at all: the segment word is not relocated in the file, so the
  comparison failed on disk and succeeded in memory. Every host coverage
  number was a lower bound wherever calls were involved. That is gone.
- The module's extent is exact. `qbeRun`'s `nb` had to stay inside the
  caller's own module and nothing could find that boundary at run time --
  asking for 30000 bytes of a module with 1662 of code hung PDS 7.1 and
  quietly survived on the other two.
- Nothing ships, and nothing is self-modifying.

## Code

- **Names say what the thing is, in as few words as do that.** `value`, not
  `v`; `from_memory`, not `m`. Not `the_value_being_encoded` either.
- **The code explains itself, or it is rewritten until it does.** A comment
  that has to say *what* the code does is a naming or structure problem.
- **DRY, SRP, open/closed.** One fact in one place -- two parallel switch
  statements over the same cases will drift, and a test asserting they agree
  is a patch over a structure that should not allow the disagreement. One
  function, one job. Extending the pass with a new idiom should mean adding a
  case, not editing the ones already there.
- **Functions are pure.** No mutating shared or global state, and no
  module-level code that does work. Module-level *constants* are fine and
  wanted -- but a table that has to be computed is computed by a function that
  returns it and bound once, and a script's body lives in `main()`.
- **Functions over types.** A type earns methods only when behaviour and
  state travel together.
- **Prefer an iterator chain where it fits.** A loop that only pushes one value
  per iteration is a `map`/`filter`/`collect` not yet written as one.
- **Match the surrounding formatting.** `cargo fmt` is not run on this tree.
- **No docstrings. No comments unless something is not trivial** -- and here
  that means a comment carries a *fact that is not in the code*: a
  measurement, the reason a case is refused, something BC does that nobody
  would guess. Narrating the next line does not qualify. Module-level facts
  live in this file or in `docs/`, not in a docstring.
- **Tests sit beside their module** (`*_tests.rs`, or `#[cfg(test)]` in it).
- **While working, run only the tests for the files being changed**:
  `cargo test --release <filter>`. See `docs/testing.md`.

## Writing

- **Commit messages are conventional commits**: `type(scope): description`,
  and a body. The subject says what changed; the body says what it was and why
  it had to. Both short and to the point, neither rambling. No
  `Co-Authored-By` trailer.
- **Replies in the session are held to the same rule**, and it is the one most
  often missed. Say the thing and stop: the finding first, no preamble, and no
  closing paragraph recommending what to do next unless a decision is open.
- PR bodies, docs and comments, likewise.

## Method

- **A fix starts with a failing test.** Reproduce the bug, watch the test
  fail, then fix it -- a test only ever seen passing proves nothing. This is
  about fixes. It is not TDD in general: new code may be written first and
  covered afterwards, as long as the cover is comprehensive.
- **Test behaviour, not implementation.** Tests should survive a refactor
  that changed no behaviour. One that does not is a test to rewrite.
- **Mutation-check every fix.** Put the bug back, confirm a test notices,
  and confirm the mutation actually applied before believing the result.
- **When careful measurements of the source all come back clean and the
  program still misbehaves, stop measuring the source and check what was
  actually built.**
- **Consult an Opus-model agent to review a design or diff, and whenever
  stuck.** A review is big-picture, not adversarial. Ask it:
  - Is this at the right abstraction level? Does it generalize to cases not
    yet seen?
  - Does it follow DRY, SRP, Open/Closed and separation of concerns?
  - How does it fit the bigger picture? Does something need re-architecting?
  - What will a skilled engineer think of it in three years?

  Give it the plan or diff and the files it is grounded in. Resume the same
  agent across rounds of one question. If Opus is unavailable, fall back to
  Fable. Opus reviews and unblocks; it does not write the diff.
- Report what was measured, not what was expected. Several figures here were
  corrected after the fact: a parity number that was timer quantisation, a
  "no absorption" reading that was the relocation artifact above, and a
  flag-safety count that was wrong in the safe direction.
