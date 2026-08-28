# Working on qbopt

A post-compilation pass over the `.OBJ` BC produces, between BC and LINK.
Most of what is here was established the hard way by a runtime version of the
same idea that still lives in uGL; it was paid for once already, and the
parts that survive the move are below.

## The goal

Replace long runs of long operations with correct and optimised 386 code.
"Optimised" means real liveness and register allocation, so the

    load, load, call, store, store

BC emits for *every single* long operation collapses into

    32-bit load, 32-bit op, 32-bit op, ..., 32-bit store

The prize is the store/reload/call traffic between operations, not the width
of any one of them. A pass that only widens pairs in place leaves most of it
on the table -- and in place is exactly what the runtime version could do,
which is why this one exists.

## Rules

- **Check BC's output on all three compilers, and with the flags that
  matter.** They do not agree, and building against one is how you get a
  pass that fires on a third of what it should. This is not hypothetical:
  the comparison operand order was read off VBDOS `/G3` alone and looked
  settled until the other three were checked.
- **Round trip first.** Read an object and write it back; the bytes must be
  identical. Nothing that rewrites is trustworthy until nothing it does not
  touch is disturbed. `tests/test_omf.py` asserts it on all four
  configurations.
- **Measure, do not reason, about what things cost.** DOSBox charges per
  instruction and models no latency. Use `qbopt/price.py` for 486, P5, P6,
  K5, K6, K7 and Core, and treat those as a ranking -- they are published
  latencies, not measurements.
- **No type suffixes in BASIC.** `As Long`, `As Integer`, spelled out.
- **Fixtures are real BC output.** Generated ones agree with whatever the
  generator believed.

## What BC actually emits

Measured across QB 4.5, PDS 7.1 and VBDOS. `docs/inherited-plan.md` has the
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

## What widening changes, exactly

`flageq.py` put both forms side by side over 1125 cases:

    ZF differs on 18.7% of them,  PF on 37.2%,  AF on 12.4%
    CF, SF, OF and the computed values never differ

BC leaves the **high half's** flags; one 32-bit operation leaves the whole
result's. So a region whose flags something reads afterwards must be refused
unless nothing in it writes flags. This gate was designed into the runtime
pass, then lost when its peephole matcher became a value graph -- set,
never tested -- and a `jz` after a widened `AND` could go the other way. It
is the kind of thing that stays green in every test until it does not.

## Things that will bite

- **"Transfers control" is not "ends a basic block".** Calls and software
  interrupts come back. 49 of 102 blocks once ended at a far call that was
  not a block end at all, which halves what liveness can see and puts a
  boundary exactly where the store/reload worth removing lives.
- **`FF /4` and `/5` are indirect jumps and do end a block** -- that is what
  `ON GOTO` and `SELECT CASE` compile to -- and the opcode tables give `FF`
  no QFLOW at all. Their targets are not computable from the instruction.
- **A call destroys the flags**, and `B$CPI4` returns its answer in them
  through `lahf`/`sahf`. A `jng` after a long operation is usually reading
  the *call's* flags, not the operation's.
- **BC leans on FIXUPP THREADs.** 34 of the 40 fixups in a module with an
  `ON GOTO` and a `SELECT CASE` name a thread rather than a target. Resolve
  them or you cannot see most of the relocations at all.
- **The FP emulator patches its own call sites** at run time -- `int
  34h`..`3Dh`, 58 of them in 15.8K of BC code. The runtime pass could not
  move code out from under an already-patched one. Whether that still
  matters when the moving happens before the program has ever run has not
  been established; check before assuming either way.

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

Moving code means updating all of, and the list is finite:

    FIXUPP  the offset the fixup patches, and the addend stored there
    PUBDEF  public symbol offsets
    LINNUM  line number to offset tables
    SEGDEF  segment length
    MODEND  the entry point
    code    self-relative branches -- NOT fixups, so they must be
            recomputed by decoding, which is the one part with no
            record to lean on

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

- **Python 3.13+, and `uv` for everything.** `uv run`, `uv sync`, `uv add`. No
  `pip`, no hand-rolled venv.
- **Functions over classes.** A class earns its place only when behaviour and
  state travel together. Data is a `@dataclass(slots=True)`, and `frozen=True`
  unless something has to mutate it.
- **Type annotations on every parameter and every return.** `ty` checks them.
- `ruff check` and `ruff format`: line length 120, double quotes.
- **No docstrings. No comments unless something is not trivial** -- and here
  that means a comment carries a *fact that is not in the code*: a
  measurement, the reason a case is refused, something BC does that nobody
  would guess. Narrating the next line does not qualify. Module-level facts
  live in this file or in `docs/`, not in a docstring.
- **pytest.** No test classes. Fixtures in `conftest.py` for the OMF corpus.
  `parametrize` wherever one assertion runs over the fixtures, the twelve
  configurations, or an opcode table -- which is most of this suite.
- `pre-commit` runs ruff, ruff-format, the whitespace hooks, `ty`, and the
  hermetic test tier.

## Writing

- **Commit messages are conventional commits**: `type(scope): description`.
  Short, to the point, no rambling.
- The same goes for every other description -- PR bodies, docs, comments,
  replies. Say the thing and stop.

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
- Report what was measured, not what was expected. Several figures here were
  corrected after the fact: a parity number that was timer quantisation, a
  "no absorption" reading that was the relocation artifact above, and a
  flag-safety count that was wrong in the safe direction.
