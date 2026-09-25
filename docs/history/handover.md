# Handover

State of qbopt as of 2026-08-29, `main` at `9c02db9`.

## Where the work is

All 34 commits are on `main` and every one is signed: the history was rebased
from the root to sign the 31 the 1Password agent had refused mid-session, and
the SHAs below are the post-rewrite ones. Nothing is pushed -- the repo has no
remote configured.

The pre-rewrite history is still reachable from the tag `pre-signing-backup`
and from `alim/mgl-peek-poke-inline-dacaee`, which is also the branch of the
redundant worktree. Drop all three once the rewrite is trusted, from a session
not running inside that worktree:

    git worktree remove .claude/worktrees/mgl-peek-poke-inline-dacaee
    git branch -D alim/mgl-peek-poke-inline-dacaee
    git tag -d pre-signing-backup

## Transcript

The whole build, from the empty plan to the merge, is one session:

    ~/.claude/projects/-Users-alim-work-personal-qbopt--claude-worktrees-mgl-peek-poke-inline-dacaee/29b55cdf-b54c-4c9d-9dfd-c7d736645029.jsonl

7.2 MB of JSONL, one object per line. It is the record of every measurement
quoted in `docs/` and of the reasoning behind the refusals in `agents.md`, which
is worth having when a decision here looks arbitrary. The plan it was built to
is `~/.claude/plans/make-a-end-to-delightful-goose.md`; all ten phases are done.

Earlier sessions in this repo are under
`~/.claude/projects/-Users-alim-work-personal-qbopt/`.

## What the pass does

    110 objects, 71204 bytes    110 mapped, 0 refused
    1404 regions, 1382 taken    26233 -> 21210 bytes, 19 per cent smaller

Twelve configurations across QB 4.5, PDS 7.1 and VBDOS build, link and run, each
compared against both BC's own object and a golden authored from what the
program means. `tools/mutate.py` catches 14 of 14 seeded bugs.

## Verification

    uv run pytest -m "not e2e"          2641 tests, host only, 8 seconds
    uv run pytest                       adds DOSBox; what pre-commit runs
    uv run python tools/matrix.py       twelve configurations, end to end
    uv run python tools/mutate.py       every seeded bug must be caught
    uv run python tools/census.py       what the pass makes of the corpus

`docs/testing.md` says which tier needs what.

## Open, in the order they are worth doing

**`docs/optimizations/residue.md` catalogs what survives absorption and widening.** A
manual liveness trace of `bench/nbody.bas`'s rewritten object, not its tests:
the object is 137 bytes larger than BC's own, and eight distinct, addressed
patterns account for essentially all of it, from a genuinely trivial one-line
fix (`popped_into()` recombines two words that were already contiguous) to
one that needs real dependence analysis (an interleaved instruction splits a
region neither pass can currently step over). Five of the eight need no new
architecture at all.

**`docs/machine/metal.md` is a protocol with an empty results table.** Whether the 66h
prefix cancels the widening win on a 486 or P5 cannot be answered by DOSBox,
which charges per instruction and will report widening as a win at exactly the
instruction-count ratio on every machine forever. `--no-widen` exists so the
answer arrives as a configuration change rather than a rewrite.

**22 regions are still refused**: 16 are single pairs that widen to more bytes
than BC wrote, 6 would swallow a line number. In qb-qrender the proportion is
worse -- 98 of 99 refusals are the single-pair case, which is the shape a
whole-procedure rewrite fixes and a per-region one cannot.

## Fixed since

**No region is refused for crossing a LEDATA boundary any more.** It was the
largest refusal category by far, 73 of 1337 regions across the corpus.
`relocate()` moves the shared boundary between the two records a crossing
spans to the edit's own edge, rather than merging them: an earlier design
that dropped the fully-absorbed record instead re-parented every FIXUPP that
used to follow it to whatever LEDATA happened to precede it after the drop,
which failed on 72 of the 73 cases. Measured first, not assumed: every one of
those 73 crossings spans records in plain file order, never one of BC's own
backpatch records, which is what the fix leans on and does not attempt to
generalise past. See agents.md's "Moving code across a LEDATA boundary".

**qbopt has been run on qb-qrender, end to end.** All 15 modules rewritten
between BC and LINK, linked against the patched uGL, run under the pinned
profile with `-campath -ticks 200`: the rendered frame is byte-for-byte
identical to the baseline's and every simulation and geometry field matches.
No measurable speed change, for the reason the census already gave -- the
program declares no LONG, so the pass touches under 1,000 bytes of 74,873.
The build used the `build/vbd-aa7dc172` snapshot, whose objects and uGL match;
a fresh `tools/dosbox.sh build vbd` does not link today, because
`build/native-mgl/UGLV.LIB` is stale against `src/` and lacks `UGLZSCALE` and
the `UGLARR*` family.

**The entry point is not searched for any more, and every module maps.** The
QuickBASIC 4.5 runtime source in `~/work/ms/msdos_60/45` settles it: `MODULE_CODE`
in `runtime/inc/addr.inc` is 48 bytes with `O_ENT` named as the offset past it,
`rtinit.asm` says the user's code begins at that fixed offset, and the header's
first field is a signature word so the layout is checked rather than assumed.
The old search was one byte late on 22 of 110 objects. `U_FLAG`, the header's
last word, carries the compile switches, which is what identifies the `/V /W`
event stub -- seeding it maps the 14 objects that could not be mapped before.
Corpus: 110 of 110 mapped, 1237 of 1332 regions, 24009 -> 15321 bytes.
`tests/suite/fpemu.bas` is back in `MOVABLE`, and the nop-motion test passes on all
three compilers with it.

**`tests/suite/fpemu.bas` is written, and the FP emulator question is answered: yes,
moving is safe.** The program does its floating point after a long divide and a
long remainder, each of which grows its call site under `/G3`, so the pass moves
all 27 of its `int 34h`..`3Bh` sites itself -- the first from 0x5d to 0x60, the
last from 0x216 to 0x21f, the segment from 578 bytes to 587. It gets the same
answers as BC's own build in all twelve configurations. Moving code out from
under a site the emulator has not patched yet is fine, which is what the
post-compilation position buys.

The guarded-divide sentence was stale in five places, not one -- `../measurement/numbers.md`,
`price.py`, `rewrite.py`, `test_rewrite.py`, and a README that still said divide
was not absorbed at all and carried a pre-absorption census line. Measured:
eighteen bytes for the divide, twenty-one for the remainder, against fifteen
under `/G3` and twenty-one elsewhere. `calls.py`'s emitter is `dividing` now.

**A real dynamic number exists.** `bench/nbody.bas` is `tests/suite/nbody.bas`'s
Q23.9 integrator, and BC alone builds the base half of the comparison.
`tools/bench.py` reads
the 8253 the way `docs/measurement/readme.md` prescribes; getting a repeatable
reading out of it took an IRQ0 mask and a guard band, and even then DOSBox-X's
own tick bookkeeping keeps an absolute noise floor of about one 18.2 Hz period
regardless -- see `docs/measurement/readme.md`. See `docs/measurement/numbers.md` for the final
figure and the two intermediate ones that preceded it.

**`calls.py` absorbs a call whose operands were never a named address at
all.** The gap `bench/nbody.bas` exposed: BC's optimizer routinely pushes a
value straight from the register it was just computed in rather than
reloading it, sometimes with a backing store and sometimes without one, and
sometimes leaves one argument stranded on the stack under an entirely
separate, self-contained nested call before pushing the second and calling.
`match()`'s backward scan only ever saw a push immediately, contiguously
before the call, and missed all three shapes. `qbopt/frontend/stack.py` tracks stack
depth in raw bytes, block-scoped, to name what fed a call regardless of
where the pushes sit; `consume()` in `calls.py` pops every one of them
unconditionally rather than reloading any -- reloading a classifiable operand
while its own push stays on the stack would leak four bytes per call,
forever, which is what an earlier, unreviewed draft of this would have done.
Verified byte-identical on every site already absorbed; `tests/suite/arrays.bas`
(added because the corpus has zero indexed or stranded-call examples) goes
from 1 of 8 call sites absorbed to 8 of 8. `bench/nbody.bas` goes from 3
regions taken to 26 of 28, and the timing number above moved with it.

## The one deliberate behaviour change

Divide is C's. `x \ 0` and `-2147483648 \ -1` fault, where BC's runtime raised
error 11 for the first and returned silently from the second. It is the only
place a rewritten program can do worse than the one BC built, and it was chosen
rather than discovered.
