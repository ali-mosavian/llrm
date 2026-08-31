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
| corpus redundant reads | 553 — 73 live provider, 48 dead, 432 none |
| selector coverage | 19,646 of 20,245 corpus ops, 0 mismatched |

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
- [ ] `imul` — the form exists in iced, it is simply not wired
- [ ] the 409 x87 instructions — M6's
- [ ] the ~190 whose address is in a space `operand_of` refuses, or whose register this does not name
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
being written rather than preserved. **42 of the corpus's 125 objects
already have every body layable**, and that number grows with the selector.

- [x] `layout.rebuild` — every body laid out into one image, cross-body
      targets resolved, and all 4,846 fixups in the span carried. 34 of 125
      objects; the rest refuse on an op `select.py` cannot emit, or on data
      BC put inline
- [ ] turn that image into records: LEDATA at 1024 bytes or less, since a
      fixup's offset is ten bits, plus its fixups, the line numbers, publics
      and the entry point
- [ ] the ON GOTO tables BC puts between instructions — 8 objects, whose
      entries are code offsets needing the same remap
- [ ] the 91 objects with an op `select.py` refuses

## M2 — placement

An emitted instruction goes where it is needed, not where the old one stood.

- [ ] a definition moves within its block, bounded by its own uses
- [ ] target liveness decides, as `simplify._target_is_free` already does for one case

Proof: nbody's two `ecx` rejoins stop being special-cased, and the "the
source is gone by then" refusals in `simplify.py` and `avail.py` disappear.

## M3 — the classic passes

Each needs M1; anything that moves code needs M2.

- [ ] constant folding — give `consts.known()` an emitter
- [ ] dead code elimination — an op whose result nothing reads
- [ ] CSE — measured 0 sites over SSA values; re-measure once loads are values, not memory
- [ ] LICM — `loops.py` has the structure and no consumer
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

- [ ] a segment register is an allocatable value, with its own class
- [ ] `Space.FAR`'s `segment` is that value, so two pointers through
      different segments are two addresses rather than one aliasing pair
- [ ] pressure and spilling per class -- four segment registers on a 386,
      of which ds and ss are effectively pinned, so es and fs/gs are what
      there is to allocate
- [ ] spill the lowest-priority pointer when the class is full, rather than
      reloading whichever was touched last

## M5 — retire the machine arm

Done when the old path is deleted, not when MIR also does it.

- [ ] widening — `lift.py`; `wide.py` has the analysis
- [ ] absorption and strength reduction — `calls.py`
- [ ] load forwarding — `forward.py`; `avail.py` has the cross-block half
- [ ] dead stores — `memory.py`
- [ ] native x87 — `fpu.py`

## M6 — floats

Largest untouched surface, newly testable: `fuzzgen.py` generates SINGLE
and DOUBLE, and `87bhelp.asm`'s six helpers have contracts.

- [ ] x87 stack positions as MIR values — a `fld` renames every slot below it
- [ ] first shape to look at: 690 `fld` against 351 `fstp` in qb-qrender

## Gates

Correctness is against the input. Byte-identity only ever checked the path
nothing transformed.

```
uv run pytest -m "not e2e"                     host suite
uv run pytest                                  adds DOSBox
uv run python tools/matrix.py                  16 programs x 12 configurations
uv run python tools/mutate.py                  deliberate breakages, each caught
uv run python tools/fuzzcheck.py --count 40    generated programs, BC as oracle
```

`matrix.py` and `fuzzcheck.py` found every real bug this session. The host
suite was green for all of them.

## Housekeeping

- [ ] `docs/architecture.md` predates `avail.py`, `simplify.py`, `select.py` and `fpu.py`
