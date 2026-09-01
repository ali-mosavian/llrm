# Roadmap

Kept current. Tick a box when it lands, and move a number when it is
re-measured -- a roadmap that lags the code is worse than none, because it
is read as if it did not.

## Goal

Recompile BC's output rather than patch it: integer and long arithmetic,
floats, array access, loop invariants, strength reduction, register
reallocation, constant folding, CSE, dead code elimination.

## Rule

**MIR is the pass. The machine-code arm is legacy.**

`lift.py`, `calls.py`, `forward.py`, `memory.py` and `fpu.py` rewrite
regions of decoded instructions in place. They stay gated and their bugs
still matter, but they get no new transforms: a peephole over emitted bytes
has no representation to do CSE, hoisting or reallocation in. Each is
retired when MIR expresses it (M5). MIR is finished when none are left.

## Blocker — cleared

It read: `mir.lower()` returns each op's `node` verbatim, so MIR can delete
and can emit the one instruction `select.py` knows, but cannot rebuild a
body -- and CSE must emit a copy, folding an immediate, LICM a move between
blocks.

That is done. `rewrite.py` writes the code segment from MIR, an `Op` can
carry semantics no node produced (`Op.made`) and say which of the original
bytes it stands for (`Op.covers`), and `layout.py` places the result and
relaxes its branches. What orders the milestones below now is narrower:
**nothing moves code between blocks yet**, and `transform.placed()` moves it
within one but is switched off.

## State

Measured 2026-09-01. Re-measure before trusting it.

| | |
|---|---|
| corpus | 95,189 → 89,482 code bytes, **-6.00%** |
| qb-qrender | 74,979 → 75,535 code bytes, **+0.74%** — absorption costs the bytes, emission and the peephole give most back |
| **qb-qrender runs** | 291 frames against BC's 271, same geometry. It did not run at all before this session |
| its arithmetic calls | 157 → 1 |
| its x87 sites | 1,766, none optimised |
| bench/nbody | 2.99× under DOSBox, 21 of 21 calls absorbed, pressure 6/6 |
| corpus redundant reads | 751 — 73 live provider, 48 dead, 630 none |
| selector coverage | 23,794 of 23,838 corpus ops; the 44 are `movsw`, refused deliberately |
| segments MIR wrote | **every one**: 155 corpus objects and all 246 of qb-qrender's BC-built ones, absorbed or not |
| **optimised and MIR-written** | **the default**, 18 suite programs on all 12 configurations, the fuzz corpus clean, and 40 of 40 mutations |

MIR does, end to end:

- [x] raise to SSA and lower back byte-identical
- [x] variables that are not registers — `docs/variables.md` stage 1
- [x] half and byte access — stage 2
- [x] the stack as an address space — stage 3
- [x] prove an identity and delete what computes nothing — stage 4
- [x] substitute a memory operand for a register, cross-block
- [x] emit the register and immediate forms from `ir.Semantics` — `select.emit`

Built, no consumer:

- [ ] `consts.known()` — proves 1,971 corpus values, nothing emits from them
- [x] `wide.py` — `widened()` builds the 32-bit semantics the pair computes, and `transform.py` folds it
- [ ] `regalloc.colour()` — correct, and cannot pay while identity is optimal at pressure 6/6

## M1 — instruction selection

`lower()` emits a body it was not handed. Everything else waits on this.

- [x] `mov reg,reg` and `mov reg,imm`
- [x] add, adc, sub, sbb, and, or, xor, cmp — reg-reg and reg-imm
- [x] neg, not, inc, dec
- [x] push, register and immediate, at both widths
- [x] refuse what is not in the table rather than approximate it
- [x] load, store, push, accumulate and store-immediate against `Space.SEGMENT` and `Space.FRAME`
- [x] cmp, cwd, cdq, wait, nop, pop, ret and retf
- [x] near call, far call, jump and conditional branch
- [x] `imul` — `select.multiply_into`; `imul eax,ecx` is `66 0f af c1`.
      Absorption is what emits it, so BC's own output has none to count
- [x] the x87 instructions — 869 of them select across the corpus.
      Their *stack positions* are still not values, which is M6's
- [x] the addresses `operand_of` refused — none left: every operation in
      the corpus selects except `movsw`, which is refused deliberately
- [x] layout: a whole body emitted, with every branch pointing at where its
      target went — `layout.lay_out`, 50 of 171 corpus bodies, 8,353 ops and
      583 branches, all landing
- [x] pick the shorter encoding where it fits — short branches to a fixed
      point, the accumulator's moffs load and store, one-byte inc and dec,
      byte-sized immediates. 6.3% larger than BC became 4 bytes smaller
- [x] the five BC writes and this did not — the byte-immediate push, a byte
      displacement through a base register, the accumulator's own arithmetic
      opcode, the by-1 shift, and the byte immediate in a memory compare.
      Whole-segment emission cost qb-qrender +3.6% before these and saves
      bytes after
- [x] `cmp reg,0` as `test reg,reg` — a byte shorter and the same question,
      and the only peephole here on this pass's *own* output: BC writes no
      `cmp reg,0` at all, so every one is absorption's. Not when the
      immediate is relocated, where the zero is an address
- [x] an immediate a fixup names keeps its width — `add ax,offset X` arrives
      as `add ax,0`, and the byte form fits zero. `select.emit` had the flag
      and none of its callers passed it, so a two-byte relocation named a
      one-byte field and the linker wrote over the instruction after it
- [x] a comparison keeps its mnemonic — `test` and `cmp` are both
      Operation.COMPARE and `test reg,reg` came back as `cmp reg,reg`, which
      sets ZF unconditionally. Latent until the peephole above would have
      been the first thing to emit one

Proof: re-lower every corpus body through the selector with no transform
applied; `tools/matrix.py` green. Same program, not same bytes — the
selector may choose differently.

Held so far without running anything: everything emitted decodes to the
instruction it was asked for (`test_everything_selected_decodes_to_what_was_asked_for`),
and every laid-out body keeps its instruction order and lands every branch
(`tests/test_layout.py`).

### Emission is whole-segment, not per-body

Measured when wiring `layout` into `rewrite`, and it changes the shape of
the rest: splicing a laid-out body back into BC's own layout works for
**1 of 171 bodies**. 33 cross a LEDATA boundary, 16 are not contiguous, and
the rest are branched into or carry a line number. BC's chunking is not
something a body-sized edit can step around.

Regenerating the whole code segment does not have the problem, because
nothing is being spliced: chunk boundaries, line numbers and fixups are all
being written rather than preserved. **Every one of the corpus's 155 objects
now has every body layable**; it was 42 when this was written.

- [x] `layout.rebuild` — every body laid out into one image, cross-body
      targets resolved, and all 4,846 fixups in the span carried. 34 of 125
      objects; the rest refuse on an op `select.py` cannot emit, or on data
      BC put inline
- [x] turn that image into records — `relocate.as_records`, `qbopt/wholeseg.py`.
      The code block goes where the LAST code LEDATA stood: OMF numbers
      externals by EXTDEF order, and a block written at the first one names
      externals whose EXTDEF has not been read, which LINK rejects outright
- [x] **MIR-written code links and runs** — `arith`, `cmpord`, `flags` and
      `nots` on PDS /G2 and QuickBASIC /O, in `tests/test_e2e.py`
- [x] the ON GOTO tables BC puts between instructions — carried verbatim
      with their fixups, since the entries are relocated words
- [x] BC's trailing zero padding, carried rather than selected — every
      object ends with `00 00 00 00`, which reachability walks into and the
      decoder reads as `add [bx+si],al`
- [x] a cell whose address cannot be named — `ir.Mem` keeps the register it
      is reached through and the displacement, out of the comparison, so it
      can be encoded without changing what "the same cell" means
- [x] the last object that refused, on bytes reachability never reached —
      `jumps-q-evt`'s two calls to `B$EVCK` sitting after an unconditional
      jump, real relocated code that nothing can arrive at. Carried the way
      padding and tables already are: a `Table` remaps every fixup inside it
      by however far it moved, and what it cannot do is fix a branch landing
      in its middle, which is exactly what reachability rules out

**Whole-segment emission is on.** `rewrite.py` writes the code segment from
MIR: 155 of 155 corpus objects and all 15 of qb-qrender's BC-built modules
rebuild, all eighteen suite programs run right on all twelve
configurations, and the fuzz corpus finds no divergence. It is not a size
cost either -- 175 bytes saved over the corpus and 593 over qb-qrender
against the patched output.

Two bugs stood between building it and it being correct, and neither the
host suite nor the twelve-configuration matrix could see either. Both took
the fuzz corpus, and then reducing a generated program from 173 lines to 31
with the machine build as the oracle.

- [x] **the relocated immediate.** `add ax,offset X` arrives at the selector
      as `add ax,0`, the sign-extended byte form fits zero, and the two-byte
      fixup then named a one-byte field -- the linker patched two bytes
      regardless, over the immediate and the byte after it, and a generated
      program read 0 for an array element. `select.emit` had a `relocated`
      flag for exactly this and none of its callers passed it.

      Nothing that compared the emitted code could see it: before linking,
      both forms disassemble as `add ax,0`. It took reducing the program to
      thirty-one lines and diffing the two linked images, where one says
      `add ax,0DCh` and the other `add ax,0FFDCh`. No fixture has the
      instruction -- BC writes it only for an array reached by adding its
      own address into ax -- so the regression test checks the wiring
      instead: `layout` tells the selector, on every call it makes
- [x] **the accumulate read as a load.** Letting `loaded_into` allow the
      high half a narrow write preserves also let `sub ax,[x]` through:
      both are one use whose origin is the destination's own register, and
      values alone cannot separate them. `redundant()` deleted
      `sub ax,ds:[0]` and `adc dx,[si+2]`. The semantics can separate
      them -- a binary operation names its destination among its sources
      and a move does not -- which is what `forward._loads_only` exists to
      stop, arrived at from the other side. No fixture reaches the shape,
      so a unit test on `_preserved` is what discriminates

## M2 — placement

An emitted instruction goes where it is needed, not where the old one stood.

- [x] a definition moves within its block, bounded by its own uses —
      `transform.placed()`, sinking to just before the first use. Built and
      switched **off**: it is the only transform here that changes the order
      instructions run in, and its benefit is indirect
- [x] what stops a move: nothing between may write what the op reads, and
      nothing may touch the register it writes. Values are per-definition
      and registers are shared, which is the part SSA hides

Proof: nbody's two `ecx` rejoins stop being special-cased, and the "the
source is gone by then" refusals in `simplify.py` and `avail.py` disappear.

## M3 — the classic passes

Each needs M1; anything that moves code needs M2. **All of them measure
empty on BC's output**, which reorders the rest of this file: the wins are
in absorption, not in the textbook passes.

- [x] constant folding — measured, not built: 1,086 results are known
      constants and every one comes out *longer*, because `xor ax,ax` is
      two bytes and `mov ax,0` is three
- [x] dead code elimination — measured, not built: BC emits none
- [x] CSE — measured, not built: 0 sites over SSA values, because BC
      reloads from memory rather than recomputing, and the memory
      redundancy `avail.py` finds is the same thing by another name
- [ ] LICM — `loops.py` has the structure and no consumer. Unmeasured
- [ ] array access — the index computation is the invariant worth hoisting

## M4 — registers

- [ ] `regalloc.colour()` reaches emission
- [ ] something that creates or relieves pressure, since identity is optimal until it does

### Segment registers are a register class, not scenery

`mir.PHYSICAL` excludes them and `_memrefs` drops the segment of a
`Space.FAR` address on the floor -- "a segment register is physical, never
a value". That is wrong for far pointers and has to change: es holds a
`$DYNAMIC` array's base, and two live far pointers are two values that
need two registers, not one register reloaded between every access.

Measured: qb-qrender has 1,294 instructions that touch es and 397 carrying
an explicit override, against 2 in the whole fixture corpus. So nothing
here will surface it and only a real program will.

**And a real program says the allocation is not the win.** Of qb-qrender's
621 `mov es,<x>`, every block loads es from a single source -- zero
alternation between two far pointers, which is what a second segment
register would buy. 42 are provably redundant: the same source, its base
register untouched, no store and no call in between. A naive textual match
says 283, which is what measuring the easy way costs.

So the work is deleting a reload, not allocating a class:

- [x] the redundant reload — `qbopt/segments.py`, without making es a value.
      42 sites, about 170 bytes in 26,290 instructions. Making it an SSA
      value would change what every op uses and defines, what regalloc has
      to colour and what select has to emit, for an optimisation nothing
      has been found to need
- [x] carrying `Space.FAR`'s `segment` through emission — `lift.memory()`
      named Space.SEGMENT, FRAME and GROUP and let FAR fall into a bare
      operand, so a widened pair read and wrote `ds` where BC wrote `es`.
      **qb-qrender linked and then corrupted itself**, on one module of
      fifteen, and `select.py` had handled the same address correctly all
      along. The mutation that guarded this had lapsed -- see the Gates
- [ ] `Space.FAR`'s `segment` as a *value*, so two pointers through
      different segments stop being one aliasing pair. Carrying it is not
      the same as tracking it: nothing yet knows two far pointers apart
- [ ] a second segment register, only if a program is ever found that
      alternates. None here does

## M5 — retire the machine arm

Done when the old path is deleted, not when MIR also does it.

Three of the five are built and on, and **they find nothing**, which is what
parity means: `rewrite.py` runs `forward.py` and `memory.py` before a body
ever reaches `transform.py`, so there is nothing left for the MIR versions
to take. Their worth is that `lift.py`, `forward.py` and `memory.py` can be
deleted -- and that deletion is the milestone, not the building.

Two are left, and they are the two that matter: absorption is where every
measured win in this project comes from, and widening is the one that has
already been written wrong once.

- [ ] widening — `transform.widened()` is written and **off, because it was
      wrong**. It folded `add ax,[x]` with `adc dx,[x+2]` into
      `add eax,[x]`, and BC keeps a long in `dx:ax`, which is not `eax`:
      the carry landed in eax's high half and dx kept what it held.
      `suite/procs.bas` printed `0x02040C10` where it wants `0x04080C10`
      on all twelve configurations, with the host suite green. What is
      missing is the step before the rename -- proving the pair is one
      value and putting it in one register, which is the analysis
      `lift.py` already has and `wide.widened()` assumed away
- [x] recovering the arguments — `mir._stack_slot` keeps the depth across a
      recognised call now, which is what put 831 of the corpus's 923
      absorbable calls out of reach: an argument pushed before some *other*
      call runs is stranded under it, and its slot is only nameable if the
      depth crossed that call. `transform.arguments()` reads them off
- [ ] **emitting the absorbed call** — the half that is left, and the one
      piece of M5 that should not be written without running anything.
      `calls.py` does it in some five hundred lines: which operand goes in
      which register, the `cdq` before an `idiv`, where the result lands,
      and a scratch register for `B$CPI4` because that routine clobbers
      nothing and absorbing it must not either. A code generator is not
      something to write blind
- [x] load forwarding — `avail.redundant()`, 12 of 12. It also needed the
      register's current value tracked: `holders()` maps a cell to the value
      put there and says nothing about whether that value is still in its
      register, so after `mov ax,[x]` then `mov ax,[y]` the entry for `[x]`
      still names a value whose origin is eax. What blocked it first was
      `loaded_into` refusing a partial write: `mov ax,[x]` writes sixteen
      bits of a thirty-two bit variable, so the high half survives and MIR
      records a read of the old `eax`. A real read, and not the instruction
      consulting memory, which is the question being asked -- so all 36
      sites came back "not a plain load" and none was anything else.

      Allowing that partial write then let an *accumulate* through, because
      `sub ax,[x]` reads the old eax the same way: one use whose origin is
      the destination's own register. `redundant()` deleted
      `sub ax,ds:[0]` and `adc dx,[si+2]` before the semantics were asked
      instead of the values -- a binary operation names its destination
      among its sources and a move does not
- [x] dead stores — `avail.dead_stores()`, block-scoped and starting empty
      at each block's end, which costs a store that spans an edge and can
      never invent one that does not
- [x] native x87 — `layout` selects an emulator site from its own semantics
      under `native_fpu` rather than carrying its bytes

- [ ] **and `--native-fpu` does not work on qb-qrender.** It hangs, and both
      halves of the fifteen modules hang independently, so it is systematic
      rather than one module. `bench/fpbench.bas` runs correctly under the
      same flag and the same DOSBox (1.88x), so the flag is not simply
      broken.

      Unexplained, and left that way deliberately: the flag is opt-in, says
      REQUIRES A COPROCESSOR, and is not in the default path, so this is a
      limitation to know about rather than a bug blocking anything. The
      guess worth testing first is that BC's `/FPi` emulator patches its own
      `int 34h`..`3Dh` sites at load time when a coprocessor is present, and
      that pre-patching some of them interacts with whatever qb-qrender's
      uGL library does with the FP stack -- 1,766 sites there against
      fpbench's handful

## M6 — floats

Largest untouched surface, newly testable: `fuzzgen.py` generates SINGLE
and DOUBLE, and `87bhelp.asm`'s six helpers have contracts.

- [x] the x87 memory and popping forms select — `fld [x]`, `faddp st(i),st(0)`
- [x] x87 stack positions as MIR values — `qbopt/fpstack.py`. Entering slots
      are minted rather than assumed empty: BC leaves values on the stack
      across a branch
- [x] that shape, measured. qb-qrender's 191 bodies hold 743 `fld` against
      333 `fstp`, 426 in-place arithmetic and 76 popping. What the gap is
      *not* is a store immediately reloaded: **0 sites** of `fstp [x]`
      followed by `fld [x]`, in qb-qrender and in the corpus, adjacent
      either way. BC does not write the obvious x87 peephole any more than
      it writes the obvious integer ones

- [ ] the one shape that is there, and it is thin. Allowing a gap, a cell
      stored by `fstp` and later read by `fld` with nothing writing it in
      between happens **36 times in qb-qrender** and 6 in the corpus, at a
      mean distance of 6.4 operations. 19 sit at loop depth 0, 16 at depth
      1, 1 at depth 2.

      Not built, and the reason is the ratio rather than the count: keeping
      a value on the x87 stack across six operations means scheduling the
      stack, since every `fld` between them rotates it -- the hardest
      transform in this project for 36 static sites in 26,290 instructions.
      `fpstack.py` is what it would be built on and exists.

      What would change the answer is a cycle measurement showing those 17
      in-loop sites are hot. Static depth is not that: `docs/roadmap.md`'s
      own PITSNAP note is about a busy-wait that scores two loops deep and
      runs twice

## Gates

Correctness is against the input. Byte-identity only ever checked the path
nothing transformed.

```
uv run pytest -m "not e2e"                     host suite
uv run pytest                                  adds DOSBox
uv run python tools/matrix.py                  18 programs x 12 configurations
uv run python tools/mutate.py                  deliberate breakages, each caught
uv run python tools/fuzzcheck.py --count 40    generated programs, BC as oracle
```

`matrix.py` and `fuzzcheck.py` found every real bug this session. The host
suite was green for all of them.

A benchmark found the one they missed. `bench/fpbench.bas` printed
-2147483648 for every coordinate: `forward.py` read the base register of
`fld dword ptr [si]` as the load's destination, so two pushes of the same
address looked like a load and a redundant reload, and deleting the second
slid every x87 slot after it. Nothing in the suite could have caught it --
every other float program here is one operation deep, and `fuzzgen.py`'s
floats stay inside the exactly-representable integers. `suite/fpdeep.bas`
is the two-deep indexed shape, added so the corpus holds it now.

Widening the generator to cover it found a second one immediately. It made
two arrays and handed the widths out in the order INT, LNG, SNG, DBL, so a
float array had never once been generated; and `_ensure_valid_operand`
required a LONG among a node's operands, which quietly caught SINGLE and
DOUBLE too, so every float BinOp had its right operand replaced by a plain
variable. With one array of each width, a repeated operand, and that guard
narrowed to LONG, the first run turned up `simplify.py` deleting a round
trip whose register was overwritten between the pushes and the pop. That
one changed a real answer: `x MOD y MOD z` used the first divide's divisor
for the second.

**And the fuzz corpus is the only thing that has ever caught a
whole-segment bug.** Both of the ones that stood in the way of turning
emission on were invisible to the host suite and to all twelve
configurations, because the suite's programs do not contain the shapes: BC
writes `add ax,offset X` only for an array reached by adding its own
address into ax, and reaching `redundant()` with an accumulate needs the
memory map to hold that cell, which takes longer code than any suite
program. Neither has a fixture, so neither has a corpus test -- what guards
them is a test of the wiring in `layout` and a unit test on
`avail._preserved`.

Finding them needed one more tool than the gates: reducing a generated
program with the machine build as the oracle, which needs no authored
golden and shrank F004 from 173 lines to 31. Worth rebuilding as a script
if a third one turns up.

**Read the exit code, not the last line.** `mutate.py` gets this right: a
mutation whose pattern no longer matches is counted as survived and the run
returns 1, and its own docstring says so. What hid it was the invocation --
`uv run python tools/mutate.py | tail -5` reports `tail`'s status, so a
failing gate came back "exited with code 0" three times in one session while
printing `pattern appears 0 times` in plain sight.

The check that had lapsed was `segment-override-not-refused`, pointed at a
line in `lift.py` that was rewritten when an override became something
resolved rather than refused. The bug it existed to catch then shipped:
widening dropped the override off a far pointer and qb-qrender corrupted
itself.

Both lapsed mutations are repointed and the gate is 40 of 40. Pipe a gate
through anything and `set -o pipefail` first, or do not pipe it.

Known gap, and it predates this: on the three QuickBASIC 4.5
configurations one generated program in forty diverges between the
evaluator and BC's own build. `fuzzcheck.py` excludes those from the qbopt
comparison, so it costs coverage rather than correctness.

What it is not, measured on the pre-change generator's F001: an arithmetic
disagreement. Every label both printed carries the same value. The program
prints 8 lines where the evaluator expects 41 -- one contiguous run of
statements produces no output at all, and then execution resumes and runs
to DONE. Whatever 4.5 does there, the evaluator models VBDOS and PDS, which
both agree with it. Worth an interactive run under 4.5 to find; it has not
had one.

## Housekeeping

- [x] `docs/architecture.md` — brought up to date, including that the MIR
      tower emits now and that `rewrite.py` does not call it
