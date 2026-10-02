# tools/bench

Executed instructions and memory operands of each benchmark's kernel, per language and optimization
level, against `bench/NAME/expected.toml`.

    uv run --project tools python tools/bench/bench.py                       # the gate
    uv run --project tools python tools/bench/bench.py sieve --opt O2        # one benchmark
    uv run --project tools python tools/bench/bench.py --bless --reason "why the counts moved"

Each variant is built (`-fno-inline-functions`, so the kernel stays a call), run in a real-mode emulator
(`icount.py`, unicorn) and counted from the kernel's entry to its return, runtime calls included.
The kernel is `bench.toml`'s `[region]`: found by name in the linker map (C, BASIC), or for Nib, which
publishes no function, as the one function `main` calls from its own segment. `"*"` counts the whole
program, for a kernel the compiler folds away.

The counts are deterministic: no clock, no DOSBox cycle setting. A count above its baseline fails; one
below fails too, until it is blessed, so the baseline only ratchets. `--bless` needs `--reason`, which
lands in the file.
