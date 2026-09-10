# Running the tests

    uv run pytest                                  everything
    uv run pytest -m "not e2e"                     host only, no emulator
    uv run pytest -m corpus                        the rewriter over the fixtures

`pre-commit` runs the whole suite, end-to-end tier included, and runs it in
parallel (`pytest-xdist`, `-n auto` in `addopts`). Every `tools/cache.py`
compile/link/run is memoized on disk under `build/launch-cache/`, keyed off
the exact bytes DOSBox is about to see -- a warm commit (nothing a `.bas`
fixture, a switch, or the pass itself touches has changed) takes single-digit
seconds; a cold one still has to boot DOSBox for real and takes about a
minute. `rm -rf build/launch-cache` forces everything cold again, and
`QBOPT_NO_LAUNCH_CACHE=1 uv run pytest` bypasses the cache for one run without
deleting it -- reach for that before trusting a green warm run after a
toolchain reinstall or a dosbox-x upgrade that didn't change any file the key
already covers (see `tools/cache.py`'s own docstring for exactly what the key
does and does not cover).

## Tiers

**Hermetic** -- `tests/test_omf.py`, `test_declen.py`, `test_lift.py`. Bytes
built in the test. Milliseconds, no dependencies.

**Corpus** (`corpus`) -- `tests/test_rewrite.py`, the rewriter over
`fixtures/omf/*.obj`, which are real BC output. Needs nothing but the repo.

**End to end** (`e2e`) -- `tests/test_e2e.py`. BC compiles `suite/*.bas`, qbopt
rewrites the object, LINK links it, the program runs, and its output is
compared. Needs `dosbox-x` and the three DOS toolchains; skips without them.

## What belongs in the suite

Tests assert program behavior, representation invariants, or a named regression.
Exact corpus totals and coverage shares are measurements; keep those in the
reporting tools and documentation, not as assertions that fail when fixtures or
the pipeline change. The legacy machine arm remains tested while it ships, but
tests must not feed raised MIR directly to its layout/allocator and call that the
production path. Current integration tests go through MIR optimization, lowering,
LIR allocation, and object writing.

## What the toolchains are, and where

| tag | compiler | expected at |
|---|---|---|
| `v-*` | VBDOS 1.0 | `~/work/other/d32x/toolchains/vbdos` |
| `p-*` | PDS 7.1 | `~/work/other/d32x/toolchains/pds71` |
| `q-*` | QuickBASIC 4.5 | `~/work/42-labs/mini-qb/dosbox/qb45` |

`tools/configs.py` holds the twelve switch combinations. Only the codegen pair
(`/G2` vs `/G3`) and event polling (`/V /W`) change what the pass sees, but all
twelve run, because a switch that does not change the long shapes today is
exactly the kind of thing that changes them tomorrow.

Without the toolchains the first two tiers still cover most of the pass's
failure surface. What they cannot do is run anything.

## The three-way differential

`golden` is what the program means, computed by `tools/mkgolden.py` in Python
rather than captured from a compiler. `base` is BC's own object, linked and run.
`opt` is the rewritten one. The verdicts name different culprits:

| verdict | means |
|---|---|
| `BASEDIFF` | not the pass -- a compiler difference or a wrong expectation |
| `DIFF` | the pass |
| `NODONE` | the program stopped early; a crash, not a diff |
| `BCFAIL` / `LINKFAIL` / `RUNFAIL` | it never got as far as answering |

`BASEDIFF` fails the run. An unexplained base is not a base.

## Measuring

`docs/measurement.md` says what each kind of number means and which are
quotable; `docs/numbers.md` holds the results. `docs/metal.md` is the protocol
for the one question a model cannot answer.

## Driving it by hand

    uv run python tools/matrix.py                  all twelve, in parallel
    uv run python tools/matrix.py --dry-run        the harness proving itself
    uv run python tools/e2e.py v-g3 --prog arith   one configuration
    uv run python -m qbopt.rewrite F.OBJ --report  the region census for one object
    uv run python -m qbopt.price F.OBJ             what it costs, per architecture
    uv run python tools/mutate.py                  put each bug back, check something notices

A failing run leaves `build/e2e/<tag>/` complete -- sources, objects, maps,
both executables, both outputs, the batch files and the generated
`dosbox.conf`. It reproduces by hand.
