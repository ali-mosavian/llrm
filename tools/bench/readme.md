# tools/bench

Executed instructions and memory operands of each benchmark's kernel, per language and optimization
level, against `bench/NAME/expected.toml`.

    uv run --project tools python tools/bench/bench.py                       # the gate
    uv run --project tools python tools/bench/bench.py sieve --opt O2        # one benchmark
    uv run --project tools python tools/bench/bench.py --bless --reason "why the counts moved"

Each variant is built (`-fno-inline-functions`, so the kernel stays a call), run in a real-mode emulator
(`icount.py`, unicorn) and counted from the kernel's entry to its return, runtime calls included.
The kernel is `bench.toml`'s `[region]`: found by name in the linker map (C, BASIC), or for Nib, which
publishes no function, as the one function `main` calls from its own segment.

The counts are deterministic: no clock, no DOSBox cycle setting. A count above its baseline fails; one
below fails too, until it is blessed, so the baseline only ratchets. `--bless` needs `--reason`, which
lands in the file.

## References

`expected.toml` also holds the reference compilers' counts, as `[bcc.O2]`, `[ow.O2]` and `[bc.O2]` beside
`[c.O2]`: BCC 3.1 (`-Ox` for O2, `-O1` for Os, medium, `-3 -f87`, linked with its own C0M, CM, FP87 and
MATHM), Turbo C 2.01 (`tc`: `-G -O -Z -r -1`, `-O -Z -r -1` for Os) and Turbo C++ 3.0 (`tcpp`: `-2 -G -O -Z -r`, no `-3` exists; its FP87.LIB does not link, BCC's is used), Open Watcom (`-ox -oe=0`, `-os -ol`, `-4 -fpi87`, linked with its own CLIBM, MATH87M, NOEMU87 and start-up from `OW_LIB`), and the BASIC compilers of QuickBASIC 4.5 (`bc`), PDS 7.1 (`pds71`) and VBDOS (`vbdos`), each `/O` with its own runtime (one level: both BASIC levels
meet it). They are deterministic, so a normal run reads them and builds none of them. Every run gates
llrm/reference per counter against the blessed ratio and prints the geomean per language and level.

    bench.py --references --time --bless --reason "why"   # measure the references again and record them
    bench.py --references                                  # they must equal the stored ones: the compiler or harness moved

Open Watcom builds only the programs without long arithmetic or floating point (its 16-bit libraries are not
built here), and BC refuses the VBDOS-style `FUNCTION f (a AS INTEGER) AS LONG` the parity programs use; a
program a reference cannot build has no row, and the geomean covers the programs both sides have (`n=`).
BCC runs at `-3`, llrm and Open Watcom at 486.

## Size

`code_bytes`, `data_bytes` and `bss_bytes` of each program's own object, from `tools/sizes.py`: the OMF object
the compiler wrote (BC's for the BASIC references), never the image, a map or the file's size, so the start-up,
`report`, the runtime and the libraries are out. Code is the SEGDEFs of CODE-class segments; data is the bytes
LEDATA/LIDATA records carry in every other segment, bss the rest of those segments. The stack and debug classes
are not counted. BC's runtime cells (`BC_DATA`, `BC_DS`, `BC_SA`) are in its object and are data. Sizes are gated
like the counts: growth fails, a shrink fails until blessed, and the llrm/reference ratio may not worsen. They are
of the build the counts use (`-fno-inline-functions`). BC leaves `BC_DATA` unloaded (bss) where llrm-qb stores it
as zeros (data): compare data+bss.

BASIC `huge` is built with `/AH` and builds, returns and prints right on all three BCs. Turbo C and C++ do not build the C `huge`
(`__huge`); VBDOS's BC rejects `parity/sum_three` with a syntax error.

## Time

    bench.py --time          # kernel time in DOSBox from RDTSC; recorded as kernel_ms, gated as a ratio to each reference
    history.py record        # append this commit's measurements to results.jsonl on the bench-history branch
    dashboard.py             # target/bench/dashboard.html from that file

`--time` runs `timeit.asm`, a .COM that reads RDTSC around the program, in a DOSBox with a fixed 75000
cycles per millisecond. The whole program includes its toolchain's start-up (C0M, BC's runtime, llrm's small
crt), which costs differently, so `startup/` (an empty kernel and one print) is built and timed with each
toolchain and level and taken out: `kernel_ms`. sieve -O2 read BCC 15% slower than llrm whole-program, and
2.4% slower by kernel. DOSBox charges about one cycle per instruction, so kernel time follows the counts; it
adds little: 1.6 points on the C -O2 geomean (-37.5% time, -35.9% instructions). A kernel under ~100 cycles is noise: a change must exceed 1% and 0.001 ms.

`history.py` installs nothing and schedules nothing. It builds a commit in a scratch worktree, runs that
commit's own tools/bench, and appends one JSON line per commit x benchmark x language x level to
results.jsonl on an orphan branch that holds only that file. A commit already there is skipped.
`backfill N` records the last N first-parent commits of main.

When a benchmark's program changes its trend has a step, and its `expected.toml` reason says why: read a
step in the dashboard against the `git log` of that benchmark.
