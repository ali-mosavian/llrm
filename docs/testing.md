# Running the tests

    cargo test --release <filter>                  the narrowest tests that answer the question

Unit tests sit beside their module (`*_tests.rs`); frontend tests are
`crates/*/src/test_*.rs`. `tests/toolchain.rs` builds programs with the
bootstrapped DOS toolchain and runs them. Release builds are incremental, so a
rebuild after an edit takes about 30 seconds.

## The gate in tiers

    python3 tools/gate/gate.py plan            what the diff against origin/main selects
    python3 tools/gate/gate.py run             run it (CARGO_TARGET_DIR set; JOBS=6 steps at once)
    python3 tools/gate/gate.py main            scheduled: full tier over origin/main every 5 merges or hour, bisects a red

The path map is `tools/gate/tiers.toml`, the only place a path is tied to a test. Fast runs every cheap step plus
each heavy step (debugger sessions, identity gate, QCport, compile-everything) whose owner paths the diff touches;
full runs everything and is chosen when the diff touches IR, support, target descriptions or the workspace manifests,
or a path no table knows. Backend and transform changes take fast plus every codegen step.

Measured on the loaded host (load 55-90, `JOBS=6`, build warm): fast 3m50 for a backend diff, full 3m35; the gate
before ran about 20 minutes, one step after another. Steps run at once, so the wall is the longest step (QCport,
the Python tests, the compile-everything test: 2-2.5 minutes); the tiers cut the work a diff starts (a frontend diff
skips ~190 s of debugger and identity steps) and the load it puts on the host. Replaying the 45 red steps in the 194
gate logs, the fast tier selects the failing step every time. Of the last 80 merges, 61 pick fast, 17 full, 2 nothing.


### Compile cost and its growth

`tools/measure.py check` (step `measure`, run when the backend, transforms, front end or x86 targets change) compares this tree's
`llrm-c` with the same measurement of the commit it branches from. Three measurements, all user-space instructions
(`perf stat -e instructions:u`), so host load does not move them:

- the compile of the 66 vsgcc programs and QCport's 65 modules at -O1, -O2 and -Os (`tools/compile-cost.py`);
- the cost at N/2, N and 2N on generated programs, per axis and level (`crates/target/*/vsgcc/scaling_gate.py`): the gate holds the second difference c(2N) − 3c(N) + 2c(N/2), which is 1.5kN² of a cost a + bN + kN²: nil for fixed and linear work, so a saving of either does not move it and a pass gone quadratic does (neither the ratio 2N/N nor c(2N) − 2c(N) is free of both);
- the same for each step of the compile with 1.5% or more of its work (`LLRM_DEBUG=time`'s `[instr]` rows).

A measurement is stored per commit in `~/.cache/llrm/measure` (`LLRM_MEASURE_DIR`), never in the repository, so two branches
share no file and nothing conflicts. The base's is read from there; if it is missing the base is built in a tree and target
directory of its own with the gate's build command (`gate.BUILD`: `cargo build --bins` alone makes another llrm-c) and
measured with this tree's tools and inputs, then stored (3-8 minutes, once per base). A stored measurement of another
method (tools, programs, QCport modules) is not used. A rise past the tolerances in `tools/gate/tiers.toml` `[measure]` fails: a
level's geomean 1.003, one file 1.02, an axis' second difference 1% of its cost at 2N and that cost 1.02, a step's 0.5% of the compile and its cost 1.05 (a step needs 2.5% of the work to fail, so one on the edge of the
share floor does not flip). A drop is recorded nowhere; the next branch's base has it. Sessions that miss one base at once wait on its lock and read what the first stored. Against the parent alone, ten commits of +0.2% each pass; the scheduled run (`gate.py main`) also compares each main commit with the one 50 merges or a week back (`measure.py creep`), at the same tolerances, and lists the commits between with their steps. Tolerances come from the same build
measured twice (`tools/compile-cost.py --noise`: geomean within 0.0001, worst file 0.0047 of 393). The QCport files need `QCPORT`
and `QCPORT_INC` (`~/scratch/qcport-env.sh`); without them they are not measured. Without a working counter the step exits 77
(SKIPPED). Wall time per step is not usable (a linear step read 4-6x at 2N under load).

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

## QCport runs

Compiling is not running: `tools/qcport-run.py` links QCport's 65 C modules as llrm-c builds them (-O2) with the rest of a
Borland build of it, runs it headless (`start.qmp -ticks 300`) beside the all-Borland build, and fails on any difference in
frames, polygons or the md5 of `BENCH.BMP`. About 13 s when the objects exist (`QCPORT_OBJECTS`, the gate's qcport-cmp.sh
output), 70 s when it compiles them; each run has 30 s (`QCPORT_RUN_SECONDS`).

    QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc QCPORT_BORLAND=~/scratch/qcbcc \
        JWLINK=~/scratch/pr-jwlink/GccUnixR/jwlink tools/qcport-run.py

## gcc.c-torture

`tools/torture/torture.py` builds GCC's `gcc.c-torture/execute` (1698 self-checking programs, `TORTURE_CORPUS`, default
~/work/personal/gcc/gcc/testsuite/gcc.c-torture/execute) at -O0, -O2 and -Os for m32 and runs each on the emulator: a program passes
by exiting 0. Every program ends in one class: pass, refused by design (`tools/torture/expected.toml` `[[refused]]`: a regular
expression on the compiler's complaint and why), differs by design (`[[differs]]`: a program that runs and exits non-zero, with its
reason), or a finding: compile failure, link failure, wrong result. None is skipped quietly. The full run is 2 minutes and is not
in the gate; the gate runs `torture.py --gate`, the fixed sample of `sample.txt` (15 s).

    uv run --project tools python tools/torture/torture.py [names...] [--levels O0,O2,Os] [--stage compile] [--out results.json]

## Debug information

`tests/dwarf.rs` and `crates/target/llrm-dwarf` run llvm-dwarfdump, gdb, ld and as over the DWARF llrm
writes. A test whose tool is missing prints `SKIPPED: why` and passes; with `LLRM_REQUIRE_DWARF=1` it
fails instead. A gate that has the tools sets it: `. tools/debug-gate.env`.

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
