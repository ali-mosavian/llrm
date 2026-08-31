# Where this is going

## The goal

Recompile BC's output rather than patch it. Not only the long arithmetic
that started this, but the ordinary integer code, the floats, array access
and loop invariants, strength reduction, register reallocation that keeps a
hot path in registers, constant propagation and folding, common
subexpression elimination and dead code elimination.

## The rule

**The MIR arm is the pass. The machine-code arm is legacy.**

Everything that emits today through `lift.py`, `calls.py`, `forward.py`,
`memory.py` and `fpu.py` works on decoded instructions and rewrites regions
of them in place. That was the right way to start and it is not the way to
finish: a peephole over emitted bytes cannot do CSE, cannot hoist, and
cannot reallocate, because it has no representation to do them in.

So work goes into the MIR arm. The machine arm is maintained -- it must
keep passing the gates, and a bug in it is still a bug -- but it does not
get new transforms. It is retired a piece at a time as MIR grows able to
express what each piece does, and the measure of MIR being finished is that
nothing is left in it.

## Where we are

Measured, not recalled. Re-measure before trusting any of it.

```
  qb-qrender   80,295 -> 82,029 bytes   26,290 -> 27,269 instructions
  arithmetic calls  157 -> 1
  x87 sites       1,766, none optimised
  bench/nbody     2.99x under DOSBox, 21 of 21 calls absorbed
  corpus redundant reads   553: 73 live provider, 48 dead, 432 none
```

What MIR can do, end to end:

- [x] raise a body to SSA, and lower it back byte-identical
- [x] variables that are not registers (`docs/variables.md` stage 1)
- [x] half and byte access, so a 32-bit value written as two halves is sayable (stage 2)
- [x] the stack as an address space, so a push links to the pop that reads it (stage 3)
- [x] prove an identity and delete what computes nothing (stage 4)
- [x] substitute a memory operand for a register, cross-block
- [x] emit one instruction the input did not contain (`select.move`)

What is built and has no consumer:

- [ ] `consts.known()` -- proves 1,971 values across the corpus, nothing emits from it
- [ ] `wide.py` -- re-derives 192 carry pairs and 871 comparison branches at MIR
      level while `lift.py` does the emitting
- [ ] `regalloc.colour()` -- correct and optimal, and identity is already
      optimal at pressure 6/6, so it cannot pay until something creates pressure

## The blocker

`mir.lower()` returns each op's `node` verbatim. MIR can delete an
instruction, and since `select.py` it can emit one particular new one, but
it cannot rebuild a body. Every unbuilt pass on the list needs that: CSE
must emit a copy, folding must emit an immediate, LICM must move code
between blocks.

Everything below is ordered by that dependency.

## Milestones

### M1 -- instruction selection

`lower()` can emit a body it was not handed.

- [ ] `mov reg,imm`
- [ ] the arithmetic forms: add, sub, and, or, xor, cmp, imul, in reg-reg and reg-imm
- [ ] load and store against each `Space`
- [ ] branches, with the displacement resolved after layout
- [ ] refuse anything not in the table, rather than approximate it

Proof: re-lower every body in the corpus through the selector with no
transform applied, and require the same program -- not the same bytes,
since the selector may choose differently, but the same output from every
configuration under `tools/matrix.py`.

### M2 -- placement

An emitted instruction goes where it is needed, not only where the old one
stood.

- [ ] a value's definition can move within its block, bounded by its own uses
- [ ] the target register's liveness decides, as `simplify._target_is_free` already does for one case

Proof: the two `ecx` rejoins in nbody stop being special-cased, and the
refusals recorded in `simplify.py` and `avail.py` for "the source is gone by
then" disappear.

### M3 -- the classic passes, on MIR

Each needs M1, and M2 for anything that moves code.

- [ ] constant folding -- `consts.known()` gets an emitter
- [ ] dead code elimination -- an op whose result no one reads
- [ ] common subexpression elimination -- measured at 0 sites over SSA values today,
      so measure again once loads are values rather than memory
- [ ] loop-invariant code motion -- `loops.py` has the structure, nothing uses it
- [ ] array access: the index computation is the invariant worth hoisting

### M4 -- registers

- [ ] `regalloc.colour()` reaches emission
- [ ] a reason to move: something that creates or relieves pressure, since
      identity is optimal while nothing does

### M5 -- retire the machine arm

Each of these is done when MIR expresses it and the old path is deleted,
not when MIR merely also does it.

- [ ] widening (`lift.py`) -- `wide.py` already has the analysis
- [ ] absorption and strength reduction (`calls.py`)
- [ ] load forwarding (`forward.py`) -- `avail.py` already has the cross-block half
- [ ] dead stores (`memory.py`)
- [ ] native x87 (`fpu.py`)

### M6 -- floats

The largest untouched surface, and newly testable: `tools/fuzzgen.py`
generates SINGLE and DOUBLE, and `87bhelp.asm`'s six helpers have
contracts.

- [ ] x87 stack positions as MIR values -- it is a stack machine, so `fld`
      renames every slot below it
- [ ] the shape worth looking at first: 690 `fld` against 351 `fstp` in
      qb-qrender, which is what values reloaded straight after being
      computed look like

## The gates, which do not change

Correctness is against the input, not against the old bytes. Byte-identity
only ever checked the path nothing transformed, and once MIR emits it stops
meaning much.

```
  uv run pytest -m "not e2e"     the host suite
  uv run pytest                  adds DOSBox
  uv run python tools/matrix.py  16 programs x 12 real compiler configurations
  uv run python tools/mutate.py  deliberate breakages, each must be caught
  uv run python tools/fuzzcheck.py --count 40   generated programs, BC as the oracle
```

`tools/matrix.py` is the one that catches semantics. Every real bug this
session was found by it or by `fuzzcheck`, and every one of them was
invisible to the host suite.

## Also stale

- [ ] `docs/architecture.md` predates `avail.py`, `simplify.py`, `select.py`
      and `fpu.py`, and its pass table is out of date
