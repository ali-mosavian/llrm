# Tools

Every script and directory under `tools/`, grouped by the task it serves: build and test the compiler, measure the code and the compile, and the smaller routes for BC objects and the QuickBASIC runtime. For contributors who need a measuring,
comparing or building tool and want to know whether one exists. Each entry's header or `--help` has the options.

Python tools run as `uv run --project tools python tools/NAME.py` (`tools/pyproject.toml` pins `iced-x86` and
`unicorn`; Python 3.13). Tools find the built compilers through `tools/llrmbin.py`: `LLRM_BIN`, else
`$CARGO_TARGET_DIR/release`, else `target/release`. Tools that need sources or toolchains outside the repository
name them in an environment variable, listed in the tool's header.

## Build, format, test infrastructure

| Tool | Does |
|---|---|
| `fmt.sh` | The formatter: nightly rustfmt, then `rfmt-post`. `--check` lists what would change; `--stdin FILE` formats for editors |
| `rfmt-post/` | The Rust program `fmt.sh` runs after rustfmt: lays out method chains and long `matches!`, moves or breaks comments past column 80 |
| `wrapcomments.py` | Wraps the comments of C files at 80 columns |
| `gate/gate.py` | The gate in tiers: `plan` picks the steps a diff needs from `gate/tiers.toml`, `run` runs them concurrently |
| `gate/tiers.toml` | The path map: which paths select which tests |
| `gate/doctests.py` | Exit 0 when a doc comment holds a fence or an indented block rustdoc would run, so the gate runs `cargo test --doc` only then |
| `gate/pytest_shards.py` | the pytest step: one pytest process per test file, N at a time, longest first (`PYTEST_SHARDS`, default min(16, cpus)); a SPLITTABLE slow file is dealt over several; times kept in `$CARGO_TARGET_DIR/pytest-durations.json` |
| `gate/proctree.py`, `gate/pytest_cpulimit.py` | CPU of a process tree; a pytest plugin that fails a test by CPU seconds, not wall time |
| `ci/nib_lsp_paths.py` | The paths whose change can change the `nib-lsp` binary; `--write` puts them in the release workflow |
| `ci/restore_mtime.py` | Sets each file's mtime to its last commit's so a restored build cache stays valid |
| `debug-gate.env` | Source it before the tests so a missing debugger tool fails a debug-information test instead of skipping it |
| `llrmbin.py` | Where the built binaries are |
| `linkrecipe.py` | A target's link recipe, read from its `object.toml` |
| `dosbatch/dosbatch.py` | Many programs, one DOSBox-X launch: build, run each under a time budget, read what it printed |
| `dosbatch/cpulimit.py`, `dosbatch/conftest.py` | A host command stopped by CPU seconds; the slow DOS tier (`-m slow_dos` or `LLRM_DOS_SLOW=1`) |
| `dosbatch/corpus.py` | Corpora too big for the tree: fetched once, cached, verified against a pinned SHA-256 |
| `dosbatch/dirty.asm` | `DIRTY.COM`: fills free memory with `0A5h`, then runs a program, to expose reads of memory it never wrote |
| `intrinsics/gen.py`, `intrinsics/attrdump.cpp` | Writes `intrinsic_attrs.rs`, the attributes LLVM gives each intrinsic, from `Intrinsics.td`; `attrdump.cpp` prints them from a real LLVM, for the drift test |
| `nib-build.sh` | Builds a Nib module into a DOS `.EXE` with `jwasm` and `jwlink` |
| `test_*.py`, `*/test_*.py` | Tests of the tools themselves: `uv run --project tools python -m pytest tools/NAME` |

## Running programs and suites

| Tool | Does |
|---|---|
| `dosbatch/run_tests.py` | `tests/run`: compile, run in one DOSBox launch, diff with `NAME.out` |
| `dosbatch/run_differential.py` | `tests/differential/qb-bc`: each program through Microsoft's BC and `llrm-qb`, outputs compared |
| `dosbatch/run_conformance.py` | `tests/differential/conformance`: BASIC compatibility cases per dialect against `known.toml` |
| `torture/torture.py` | GCC's `gcc.c-torture/execute` through `llrm-c` at -O0, -O2 and -Os, run on the emulator; `expected.toml` names each refusal |
| `torture/libc.c`, `torture/include/` | The little C library and headers the torture programs build against |
| `returns_twice/run.sh` | A `setjmp`/`longjmp` program built by `llrm-c` and run under `dosrun`, at -O2 and -Os |
| `qcport/strlib.c` | `strcmp` and `strncpy`, which Borland C inlines, for building the QCport modules |
| `qcport-compile.sh` | Compiles QCport's C modules at -O2 and -Os; lists each refusal |
| `qcport-run.py` | Runs QCport built by `llrm-c` beside the all-Borland build and compares frames and `BENCH.BMP` |
| `qcport-swap.py` | Whole-run executed instructions of QCport arms that differ in one module (BENCH run, dosrun emulator); refuses a noisy repeat or arms that draw different work; flags a per-function change only above the worst of 3 repeats of identical objects, and not a rise paired with a matching fall |

## Identity gates

| Tool | Does |
|---|---|
| `identity.sh` | Two builds of `llrm-nib` or `llrm-c` make byte-identical objects for a corpus; see [tools/readme.md](../../tools/readme.md) |
| `identical.py` | The same for `llrm-c` alone: every program, every `tests/run/c` program (each flag variant), the bench harness's own compiles (its flags, every language, -O2/-Os/-O3) and every QCport module; aborts on a binary older than its head's commit |
| `g-identical.sh`, `qcport-g-identical.sh` | The assembly of every bench C program, or QCport module, with and without `-g`; lists the ones that differ |
| `baseline.sh` | Every MIR stage dump of the OMF corpus and the Nib examples; `diff -r` two trees |

## Measuring the compiled code

| Tool | Does |
|---|---|
| `bench/bench.py` | The benchmark gate: executed instructions and memory operands per kernel against the base's counts (and the references' in `bench/NAME/expected.toml`); see [tools/bench](../../tools/bench/readme.md) |
| `bench/history.py`, `bench/dashboard.py` | Per-commit benchmark history on the `bench-history` branch, and the HTML dashboard drawn from it |
| `bench/maps.py`, `bench/timeit.asm`, `bench/startup/` | Public symbols from a linker map; the RDTSC timer `.COM`; the empty program whose start-up time is subtracted |
| `sizes.py` | Object bytes and the backend's expected instructions and memory operands of every program, two builds side by side |
| `sizes-known.txt` | Programs `sizes.py` builds that do not compile yet; it may only shrink |
| `innerloops.py`, `msp430.py` | Each innermost loop's instructions and memory operands, from an object's bytes (OMF or ELF, x86 or MSP430) |
| `loops/run.py` | The loop corpus: cases written once in `loops/spec.py`, emitted as C, BASIC and Nib, checked by an oracle and measured against bounds `loops/expect.py` derives from the target; `--quick` for a short run |
| `loops/shortfalls.txt`, `loops/known.toml` | What falls short today, each with its issue; a run fails on a new shortfall and on a listed one that is gone, and a run of everything fails on a `known.toml` bug no failing line matches. `invariant-loads` (a loop reloading what it never changes) is a shortfall only where Open Watcom or gcc compiled the same loop with fewer instructions or fewer memory operands, the bar of `size-vs-ref`; where no reference gave a bar (gcc built none, or Open Watcom's loop calls a helper) the class is `invariant-loads-ideal`, as `ivs-ideal` is, and it stays a shortfall; with no reference built it is not judged. `--dump` refuses a tmpfs: a run writes hundreds of thousands of files |
| `scoreboard.py` | `llrm-c` against gcc: instructions per level, and where llrm's go, by family |
| `pressure.py` | The spill model's forecast beside the allocator's spill code, per function of the bench programs |
| `dwarf-coverage.sh` | `llvm-dwarfdump --statistics` of every bench C program built by `llrm-c` and by gcc |
| `debug-matrix.py` | For each frontend, width, object format and debug format: whether `-g` output is accepted by an independent reader |
| `jpeg_image.py`, `jpeg_reference.py` | The jpeg benches' input picture and their decoder in Python, for the expected checksum |

## Measuring the compiler

| Tool | Does |
|---|---|
| `compile-cost.py` | What it costs `llrm-c` to compile: user-space instructions per file at each level |
| `measure.py` | `compile-cost` and its growth against the merge-base's measurement; `check` exits 1 on a rise past tolerance |
| `pass-profile.py` | Where `llrm-c`'s compile instructions go, summed per step over the files `compile-cost.py` measures |
| `callgrind-check.py` | Callgrind under `--read-inline-info`, and a check of its call edges: a pass's inclusive total is reliable, a callee split only for a profile that passes `check` (fat LTO folds callees under callers that never call them) |
| `pipeline-runs.py` | How many times each body goes through the MIR pipeline and the allocators, and how much of that changes nothing |

## The QuickBASIC runtime

| Tool | Does |
|---|---|
| `dosbatch/qbruntime.py` | Builds and inspects the real-mode QB runtime archive |
| `dosbatch/qb32.py`, `qb32_fixtures.py`, `qb32demos.py` | The runtime on the flat 32-bit target (DOS/32A): build, run the bench programs, fixtures for symbol loading, demos played |
| `dosbatch/qbplay.py` | Plays a DOS program under DOSBox-X through its debug socket: typed keys, screenshots |
| `dosbatch/qbsizes.py`, `qbcodedata.py` | The runtime's size and each program's linked size against BCOM45's; code and data per program from linker maps |
| `dosbatch/inventory_qbruntime.py`, `os_objects.py` | The QB runtime symbols a corpus links; the OS layer's objects for a program |
| `dosbatch/gfxbench/` | Graphics primitives and text per screen mode on BCOM45, the llrm runtime in real mode, and flat, on virtual time |
| `dosbatch/gorilla_aim.py`, `signal_gen.py` | Where a GORILLA banana lands, from a screenshot; the input of the `stft_*` benches |

## MIR corpora

| Tool | Does |
|---|---|
| `mir-corpus.sh` | Regenerates `crates/opt/llrm-analysis/corpus`, the MIR the corpus tests read |
| `mir-oracle.sh` | LLVM 20 as MIR's oracle: each `.ll` verified, compiled and run by LLVM and by `llrm-mir`, and read back by both |
| `mir-clang-fixtures.sh` | Regenerates `crates/ir/llrm-mir/tests/clang` from `bench/parity` with clang |
| `hir-mir-corpus.sh` | Every QB suite program's and Nib fixture's HIR emitted as MIR, which the verifier and opt must accept |

## BC objects and calling conventions

| Tool | Does |
|---|---|
| `e2e/e2e.py` | BC compiles, `llrm-omf` recompiles, LINK links, DOSBox runs both, output compared, for one BC configuration |
| `e2e/matrix.py`, `e2e/configs.py` | `e2e` over every BC configuration, in parallel, as one table; the switch sets |
| `e2e/fuzzgen.py`, `e2e/fuzzcheck.py` | Random BASIC programs with a reference evaluator, run through BC's build and `llrm-omf`'s |
| `e2e/mkgolden.py`, `e2e/qbprint.py` | The suite's expected outputs computed from what each program means; BASIC's wraparound and `PRINT` formatting |
| `e2e/mkfixtures.py` | Rebuilds `tests/inputs/omf` with the BC toolchains; `--check` recompiles and compares |
| `e2e/dosbox.py`, `e2e/cache.py` | The headless DOSBox-X runner; its launch cache |
| `analysis/qbfootprint.py` | Linked BASIC code bytes per module from two LINK maps |
| `analysis/runtime_writes.py` | What the QuickBASIC runtime writes, read off a linked image |
| `callconv/bas.sh`, `basref.sh` | The BASIC calling-convention matrix: BC and `llrm-qb` callers and callees; rebuilds BC's reference listings |
| `callconv/c.sh`, `bcc.sh`, `reference.sh` | The C calling-convention matrix: BCC and `llrm-c` in all four pairings; the BCC 3.1 compile; rebuilds BCC's reference |
| `callconv/matrix.py` | Random callee and caller units of N signatures, and what a host compiler prints for them |
