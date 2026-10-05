# Running the tests

    cargo test --release <filter>                  the narrowest tests that answer the question

Unit tests sit beside their module (`*_tests.rs`); frontend tests are
`crates/*/src/test_*.rs`. `tests/toolchain.rs` builds programs with the
bootstrapped DOS toolchain and runs them. Release builds are incremental, so a
rebuild after an edit takes about 30 seconds.

## What belongs in the suite

Tests assert program behavior, representation invariants, or a named regression.
Exact corpus totals and coverage shares are measurements; keep those in the
reporting tools and documentation, not as assertions that fail when fixtures or
the pipeline change. Integration tests go through MIR optimization, instruction
selection, LIR allocation, and object writing.

## Every program compiles

`tests/test_programs_compile.py` compiles every program `tools/sizes.py` builds
(the suite, bench and examples) at -O2 with the release binaries, about ten
seconds. A program that fails and is not in `tools/sizes-known.txt` fails the
test, and so does a listed one that compiles: the list may only shrink. It is
skipped, loudly, when `target/release` has no binaries.

    uv run --no-project --with pytest --with iced-x86 python -m pytest tests/test_programs_compile.py -rs

## QCport compiles

QCport, the largest C program llrm-c compiles, is not in this repository,
so CI cannot build it. Before merging a change to llrm-c, HIR, its verifier
or MIR lowering, compile its 65 modules at -O2 and -Os; every one must
compile (#238 made the driver's verifier refuse five, unseen):

    QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc tools/qcport-compile.sh

## The loop corpus

`tools/loops` judges loop code: every case is one loop program in a neutral
spec, emitted as C, QB 4.5 and Nib.

    uv run --project tools python tools/loops/run.py --quick     under a minute, for every change
    uv run --project tools python tools/loops/run.py             every case, CPU and -O level

Correctness has no exceptions. A Python oracle computes what each case
reports in each language's own semantics; each run checks it against the host's
clang (C, in fixed-width types) and BC (BASIC; `--quick` leaves BC to the full
run, as it compiles in emulated DOS). llrm's MIR as the pipeline
received it and as it left it runs in llrm-mir's interpreter; the linked
programs run in DOSBox, each with its own time budget.

Quality is a ratchet. `tools/loops/shortfalls.txt` lists what falls short today,
one line per case, language and check, with its issue; only
`run.py --write-known` writes it. The run fails for a shortfall it does not list and
for a listed one that no longer falls short, so a pass may shrink the list,
never grow it. The checks: induction variables (counted from the decoded
bytes and by llrm-mir's ScalarEvolution) against a bound derived from the case
and the target's address forms (`tools/loops/expect.py`); no reload of a loop
invariant while the loop fits the registers; the one-counter reference shape
where it applies; C's inner loop no larger than Open Watcom's or gcc-ia16's;
and a metamorphic variant keeping its base's count.

`--dump DIR` keeps every program, object and MIR stage; `report.txt` there has
the per-case table, the sharing matrix and the coverage of each dimension.

## Measuring

`docs/measurement/readme.md` says what each kind of number means and which are
quotable; `docs/measurement/numbers.md` holds the results. `docs/machine/metal.md` is the protocol
for the one question a model cannot answer.
