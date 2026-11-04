# tools/bench

For contributors measuring a change's effect on code. Executed instructions and memory operands of each benchmark's kernel, per language and optimization
level, against the base's counts (the compiler's own rows) and `bench/NAME/expected.toml` (the references').

    uv run --project tools python tools/bench/bench.py                       # the gate
    uv run --project tools python tools/bench/bench.py sieve --opt O2        # one benchmark

Each variant is built (BASIC and Nib with `-fno-inline-functions`, so the kernel stays a call; C with its inlining on, the
kernel called through a volatile pointer as the m32 table does, since the flag also kept a recursive function from inlining itself),
run in a real-mode emulator
(`crates/target/llrm-x86-m16/bench/icount.py`, unicorn) and counted from the kernel's entry to its return, runtime calls included.
The kernel is `bench.toml`'s `[region]`: found by name in the linker map (C, BASIC), or for Nib, which
publishes no function, as the one function `main` calls from its own segment.

The counts are deterministic: no clock, no DOSBox cycle setting. Beside them the run prices the same instruction stream by the 486's clocks (vsgcc's table, `crates/target/llrm-x86-m32/vsgcc/harness.py`, shared with the m32 comparison): `clocks` in `--json`, and a line per benchmark when benchmarks are named on the command line. Reported, not gated or blessed: a change that trades instructions for fewer clocks is judged by it. A count above its baseline fails; one
A count above the base's fails; a count below passes and is the next base: no file holds the compiler's counts, so two PRs that change one program's count never conflict.

## References

`expected.toml` holds the reference compilers' counts, as `[bcc.O2]`, `[ow.O2]` and `[bc.O2]` beside
`[c.O2]`: BCC 3.1 (`-Ox` for O2, `-O1` for Os, medium, `-3 -f87`, linked with its own C0M, CM, FP87 and
MATHM), Turbo C 2.01 (`tc`: `-G -O -Z -r -1`, `-O -Z -r -1` for Os) and Turbo C++ 3.0 (`tcpp`: `-2 -G -O -Z -r`, no `-3` exists; its FP87.LIB does not link, BCC's is used), Open Watcom (`-ox -oe=0`, `-os -ol`, `-4 -fpi87`, linked with its own CLIBM, MATH87M, NOEMU87 and start-up from `OW_LIB`), and the BASIC compilers of QuickBASIC 4.5 (`bc`), PDS 7.1 (`pds71`) and VBDOS (`vbdos`), each `/O` with its own runtime (one level: both BASIC levels
meet it). They are deterministic, so a normal run reads them and builds none of them. Every run gates
llrm/reference per counter against the base's ratio and prints the geomean per language and level.

    bench.py --references --time --bless --reason "why"   # measure the references again and record them
    bench.py --references                                  # they must equal the stored ones: the compiler or harness moved

Open Watcom builds only the programs without long arithmetic or floating point (its 16-bit libraries are not
built here), and BC refuses the VBDOS-style `FUNCTION f (a AS INTEGER) AS LONG` the parity programs use; a
program a reference cannot build has no row, and the geomean covers the programs both sides have (`n=`).
BCC runs at `-3`, llrm and Open Watcom at 486.

The flat target (`bq32`) is counted by DOSBox, and its clocks come from `flatcount.py`: the same objects linked again as an ELF (`dosbatch.link_target(image=)`), run in unicorn from the program's entry with the DPMI and DOS calls its start-up makes answered, and priced by vsgcc's table over the kernel's instructions. The row has clocks only where the emulator's instruction count is DOSBox's (within 0.5% or 64: DOSBox counts a long REP in slices of its cycle budget, `scroll` reads 58,321 there and 21,015 here).

BASIC is built on `llrm-qb`'s own frames, the default: no `B$ENRA`/`B$EXSA` where the runtime needs no frame. `--runtime-frames` is not benchmarked.

## Against the base

The compiler's rows (`c`, `nib`, `bq16`, `bq32`) are judged against the base commit's: the merge-base with `origin/main`
(`LLRM_BENCH_BASE_REF`), built and measured by its own tree's `bench.py` and programs (so a flag only the head's harness knows never reaches
the base's compiler) and kept by its git tree (`~/.cache/llrm/bench-base`, `LLRM_BENCH_BASE_DIR`): a landed tree is measured once, and by what the counts depend on (`basecounts.py key`: the tree but docs and editor files). CI: the bench step is `[bench] shards` jobs (`bench.py --shard I/N`; the split is `shards.toml`, re-measured by `python tools/bench/shards.py measure`), each measuring its slice of the head, and `[bench] shards` `bench-base-I` jobs (`--shard base:I/N`, the same split) that measure (or restore) the base, a slice each; a `bench-merge` job (`bench.py --merge DIR N`) prints the verdict no slice has alone. A push to main records its counts there (`--record`) into the actions cache `bench-base-KEY`, and a pull request restores its base's, so a miss costs the `bench-base-I` jobs (each a base build and a slice of its measurement), not one per head shard; the merge fails, naming it, when a slice's file is missing or its programs are not the split's. The bench step's stage lines (with the programs and cores of the first) are printed to the job log as they come. Counts, sizes
and, with `--time`, a timed-only program's kernel time may not exceed the base's. A row the base lacks (a new program), or of a program whose
files differ between the base and the head, is not judged. The line `BASE sha: n rows` says which base. A base that cannot be built or measured
fails the step with the reason (`FAIL BASE: ...`).

A deliberate worse count is a waiver, one file per PR, `tools/bench/waivers/NAME.toml`: `reason`, and `[[row]]` entries of `label`
(`"fib bq32 -O2"`), `counter` and `to` (the count it may reach); it holds only for that count and is listed `BASE-STALE` when it excuses
nothing (delete it once its PR has landed).

## Timed-only: grep

`bench/grep` (`timed_only = true` in its bench.toml) is Aho-Corasick over the Silesia `dickens` text (10,192,446 bytes), read in 1K chunks
through DOS (`input_read` in ext.asm for C, `OPEN`/`GET` for BASIC, `std.io` for Nib). The emulator would step about 10^8 instructions,
so it has no counts: it is sized, and timed in DOSBox under `--time` over the whole file. Its output is checked there, since nothing else does. Its kernel time gates on its own, both ways,
and as a ratio to each reference.

The file is `tools/dosbatch/corpus.py`'s: fetched once from the Silesia site (the GitHub mirror is the fallback), cached in
`~/.cache/llrm-bench`, and checked against its pinned SHA-256 on every use. Missing and offline, grep is skipped with a message.

`bench/jpeg_single` and `bench/jpeg_fixed` are timed-only too (the 256x192 `photo.jpg`, from `tools/jpeg_image.py`, decoded to
`OUT.BMP` through DOS; the printed Adler-32 is `tools/jpeg_reference.py`'s). C reads and writes through `bench/bench_io.h`, which maps
to the OS layer under llrm and to `<io.h>` under the references; `bench.py` copies it beside the Borland builds. Open Watcom has no row:
its `clibm.lib` cannot link `open` here (`__doserror_`).

## Size

`code_bytes`, `data_bytes` and `bss_bytes` of each program's own object, from `tools/sizes.py`: the OMF object
the compiler wrote (BC's for the BASIC references), never the image, a map or the file's size, so the start-up,
`report`, the runtime and the libraries are out. Code is the SEGDEFs of CODE-class segments; data is the bytes
LEDATA/LIDATA records carry in every other segment, bss the rest of those segments. The stack and debug classes
are not counted. BC's runtime cells (`BC_DATA`, `BC_DS`, `BC_SA`) are in its object and are data. Sizes are gated
like the counts: growth fails, a shrink fails until blessed, and the llrm/reference ratio may not worsen. They are
of the build the counts use, except that a C program's size is of the object with the kernel `noinline` and `main` as written, not the
one with the pointer (18 bytes of code, 4 of data). BC leaves `BC_DATA` unloaded (bss) where llrm-qb stores it
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
toolchain and level and taken out: `kernel_ms`. DOSBox charges about one cycle per instruction, so kernel time follows the counts. A kernel under about
100 cycles is noise: a change must exceed 1% and 0.001 ms.

`history.py` installs nothing and schedules nothing. It builds a commit in a scratch worktree, runs that
commit's own tools/bench, and appends one JSON line per commit x benchmark x language x level to
results.jsonl on an orphan branch that holds only that file. A commit already there is skipped.
`backfill N` records the last N first-parent commits of main.

When a benchmark's program changes its trend has a step, and its `expected.toml` reason says why: read a
step in the dashboard against the `git log` of that benchmark.

## BASIC languages

`bas` is BASIC on Microsoft's runtime (BC and LINK in DOSBox); it is skipped, with a `SKIP` line naming the directory and the variable (`QB45_DIR`, `PDS71_DIR`, `VBDOS_DIR`), where that install is absent. `bq16` and `bq32` are the same source on our runtime, linked by jwlink, and need no install: `bq16` is real mode, counted by the emulator between the kernel's entry and return; `bq32` is flat under DOS/32A, counted by DOSBox on virtual time from the program's link map: the instructions the kernel's function ran, itself and what it called, as `bq16`'s are (the extender's load and the runtime's start-up are not in it, nor is any code the debug info and map do not name that ran inside the kernel, DOS and the extender); the kernel ends where its image's CodeView procedure record says, because the map gives no length and code after the kernel that no symbol names would be counted in it (bq32 is built with `-g` and linked `debug codeview` for this; the code is the same, and a kernel without a length prints a WARNING). Both are deterministic.
