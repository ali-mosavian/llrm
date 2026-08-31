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

## Blocker

`mir.lower()` returns each op's `node` verbatim. MIR can delete, and can
emit the one instruction `select.py` knows, but cannot rebuild a body. CSE
must emit a copy, folding an immediate, LICM a move between blocks. That
dependency orders every milestone below.

## State

Measured 2026-08-31. Re-measure before trusting it.

| | |
|---|---|
| qb-qrender | 80,295 → 82,029 bytes, 26,290 → 27,269 instructions |
| its arithmetic calls | 157 → 1 |
| its x87 sites | 1,766, none optimised |
| bench/nbody | 2.99× under DOSBox, 21 of 21 calls absorbed, pressure 6/6 |
| corpus redundant reads | 751 — 73 live provider, 48 dead, 630 none |
| selector coverage | 23,794 of 23,838 corpus ops; the 44 are `movsw`, refused deliberately |
| segments MIR wrote | **all 155 corpus objects and all 246 of qb-qrender's BC-built ones**, absorbed or not |
| **optimised and MIR-written** | **six programs on all 12 configurations, 18/18 each** |

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
- [ ] `wide.py` — re-derives 192 carry pairs and 871 comparison branches while `lift.py` emits
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

**Every object in the corpus rebuilds whole-segment**, 155 of 155, and all
six programs in `REBUILDS` link and run on all twelve configurations. What
is left is not reach but the wiring: `rewrite.py` still does not call
`wholeseg.rebuilt`.

## M2 — placement

An emitted instruction goes where it is needed, not where the old one stood.

- [ ] a definition moves within its block, bounded by its own uses
- [ ] target liveness decides, as `simplify._target_is_free` already does for one case

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

- [ ] es as a tracked value, so `avail.py` can see the redundant reload --
      42 sites, about 170 bytes in 26,290 instructions
- [ ] `Space.FAR`'s `segment` as that value, so two pointers through
      different segments stop being one aliasing pair
- [ ] a second segment register, only if a program is ever found that
      alternates. None here does

## M5 — retire the machine arm

Done when the old path is deleted, not when MIR also does it.

- [ ] widening — `lift.py`; `wide.py` has the analysis
- [ ] absorption and strength reduction — `calls.py`. Measured: 831 of the
      corpus's 923 absorbable calls have something other than a push
      immediately before them, so recovering the arguments needs the
      block-scoped depth model `stack.py` already has, restated over
      `Space.STACK`. Parity work: the same optimisation either way
- [ ] load forwarding — `forward.py`; `avail.py` has the cross-block half.
      Measured: `avail.loaded_into` models 0 of the 36 sites `forward.py`
      deletes, and always for the same reason. MIR is right and the gate is
      too blunt: `mov ax,[x]` writes 16 bits of a 32-bit variable, so the
      high half survives and MIR records a use of the old `eax`. That is a
      real read, so "a plain load reads nothing but its address" refuses it.
      The machine pass is sound anyway, because the site it deletes is one
      where the register already holds those bytes -- a no-op leaves the
      high half alone whether it runs or not. Saying that in MIR means
      letting `loaded_into` accept a use that is the destination's own
      previous value under a partial write, which is the `HALF_TO_LOW` and
      `CONCAT_LOW` machinery `mir.py` already has for pairs
- [ ] dead stores — `memory.py`
- [ ] native x87 — `fpu.py`

## M6 — floats

Largest untouched surface, newly testable: `fuzzgen.py` generates SINGLE
and DOUBLE, and `87bhelp.asm`'s six helpers have contracts.

- [x] the x87 memory and popping forms select — `fld [x]`, `faddp st(i),st(0)`
- [ ] x87 stack positions as MIR values — a `fld` renames every slot below it
- [ ] first shape to look at: 690 `fld` against 351 `fstp` in qb-qrender

## Gates

Correctness is against the input. Byte-identity only ever checked the path
nothing transformed.

```
uv run pytest -m "not e2e"                     host suite
uv run pytest                                  adds DOSBox
uv run python tools/matrix.py                  17 programs x 12 configurations
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
