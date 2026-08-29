# Handover

State of qbopt as of 2026-08-29, `main` at `b66747d`.

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
quoted in `docs/` and of the reasoning behind the refusals in `AGENTS.md`, which
is worth having when a decision here looks arbitrary. The plan it was built to
is `~/.claude/plans/make-a-end-to-delightful-goose.md`; all ten phases are done.

Earlier sessions in this repo are under
`~/.claude/projects/-Users-alim-work-personal-qbopt/`.

## What the pass does

    110 objects, 71204 bytes    102 mapped, 8 refused
    1166 regions, 1080 taken    21131 -> 13530 bytes, 35 per cent smaller

Twelve configurations across QB 4.5, PDS 7.1 and VBDOS build, link and run, each
compared against both BC's own object and a golden authored from what the
program means. `tools/mutate.py` catches 14 of 14 seeded bugs.

## Verification

    uv run pytest -m "not e2e"          2341 tests, host only, 80 seconds
    uv run pytest                       adds DOSBox; what pre-commit runs
    uv run python tools/matrix.py       twelve configurations, end to end
    uv run python tools/mutate.py       every seeded bug must be caught
    uv run python tools/census.py       what the pass makes of the corpus

`docs/testing.md` says which tier needs what.

## Open, in the order they are worth doing

**No dynamic number exists.** `tools/bench.py` and the 8253 timer are unwritten,
deliberately: `TIMER` quantises to 54.9 ms and would put 0.2 of error into every
ratio. `conf/pinned.conf` is committed and ready.

**`docs/metal.md` is a protocol with an empty results table.** Whether the 66h
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
generalise past. `suite/fixmul.bas` no longer needs the padding statement it
once did to dodge a call site landing on a flush point. See AGENTS.md's
"Moving code across a LEDATA boundary".

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
`suite/fpemu.bas` is back in `MOVABLE`, and the nop-motion test passes on all
three compilers with it.

**`suite/fpemu.bas` is written, and the FP emulator question is answered: yes,
moving is safe.** The program does its floating point after a long divide and a
long remainder, each of which grows its call site under `/G3`, so the pass moves
all 27 of its `int 34h`..`3Bh` sites itself -- the first from 0x5d to 0x60, the
last from 0x216 to 0x21f, the segment from 578 bytes to 587. It gets the same
answers as BC's own build in all twelve configurations. Moving code out from
under a site the emulator has not patched yet is fine, which is what the
post-compilation position buys.

The guarded-divide sentence was stale in five places, not one -- `numbers.md`,
`price.py`, `rewrite.py`, `test_rewrite.py`, and a README that still said divide
was not absorbed at all and carried a pre-absorption census line. Measured:
eighteen bytes for the divide, twenty-one for the remainder, against fifteen
under `/G3` and twenty-one elsewhere. `calls.py`'s emitter is `dividing` now.

## The one deliberate behaviour change

Divide is C's. `x \ 0` and `-2147483648 \ -1` fault, where BC's runtime raised
error 11 for the first and returned silently from the second. It is the only
place a rewritten program can do worse than the one BC built, and it was chosen
rather than discovered.
