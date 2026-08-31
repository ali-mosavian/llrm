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
| selector coverage | 5,839 of 20,245 corpus ops, 0 mismatched |

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
- [ ] `imul` — the form exists in iced, it is simply not wired
- [ ] load and store against each `Space` — 3,126 moves, 3,709 pushes, 787 binaries
- [ ] call, branch and jump, target resolved after layout — 6,104 ops
- [ ] layout: an address's fixup and a branch's displacement move with the code

Proof: re-lower every corpus body through the selector with no transform
applied; `tools/matrix.py` green. Same program, not same bytes — the
selector may choose differently.

Held so far by `test_everything_selected_decodes_to_what_was_asked_for`:
everything emitted decodes to the instruction it was asked for, over the
whole corpus. That is the same claim at the size the table currently
reaches, and it found both bugs in the first version.

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
