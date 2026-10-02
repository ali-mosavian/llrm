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

## Beyond the gate

    bench.py --references    # Open Watcom (C) and BC 4.5 (BASIC) on the same programs; never gated
    bench.py --time          # the whole program's RDTSC time in DOSBox, never gated
    history.py record        # append this commit's measurements to results.jsonl on the bench-history branch
    dashboard.py             # target/bench/dashboard.html from that file

References that cannot be built say why in the history: Open Watcom needs its 16-bit libraries for the
long arithmetic and floating-point helpers (only its compiler is built here), and BC 4.5 refuses the
VBDOS-style `FUNCTION f (a AS INTEGER) AS LONG` the parity programs use.

`--time` runs `timeit.asm`, a .COM that reads RDTSC around the program, in a DOSBox with a fixed 75000
cycles per millisecond, so cycles / 75000 is milliseconds on any host. DOSBox charges about one cycle per
instruction and models no latency: the time follows the whole program's executed instructions, runtime
start-up and printing included, and says nothing the counts do not. It is for comparing one program
across commits.

`history.py` installs nothing and schedules nothing. It builds a commit in a scratch worktree, runs that
commit's own tools/bench, and appends one JSON line per commit x benchmark x language x level to
results.jsonl on an orphan branch that holds only that file. A commit already there is skipped.
`backfill N` records the last N first-parent commits of main.

When a benchmark's program changes its trend has a step, and its `expected.toml` reason says why: read a
step in the dashboard against the `git log` of that benchmark.
