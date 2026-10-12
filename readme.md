# llrm -- Low Level Real Machine

`llrm` is a compiler suite for 16-bit real-mode DOS (and a flat 32-bit DOS target) that aims at GCC/LLVM-quality
code. Every frontend raises to one SSA IR (MIR), shares one optimizer and x86 backend, and writes linkable OMF
`.OBJ` files. For people who build llrm, use its compilers or work on it; documentation starts at [docs](docs/readme.md), and
[comparison](docs/comparison/readme.md) sets its output beside other compilers'.

![llrm pipeline: QuickBASIC, Nib and C raise through HIR and BC objects straight to MIR; one target-neutral optimizer; one x86 real-mode backend](docs/architecture/pipeline.svg)

MIR passes are machine-independent; only lowering, allocation and peephole see registers or instructions
([the MIR boundary](docs/architecture/split.md)). The goal is output within 1.5x of a hand-derived modern-compiler
listing for every suite program; [measurement](docs/working/measurement/readme.md) says how it is measured.

## Programs

Each is built from `src/bin`. `llrm-qb` is the default `cargo run` target.

| Program | Does |
|---|---|
| `llrm-qb` | QuickBASIC-family source (QB 4.5, QBasic 1.1, PDS 7.1, VBDOS, QuickrBASIC) to an object; see [the dialects](docs/frontends/qb/dialects.md) |
| `llrm-c` | C, through a patched Open Watcom front end (`toolchain/owshim/`), to an object; see [the front end](docs/frontends/c/ow-contract.md) |
| `llrm-nib` | Nib, llrm's own language, to an object; see [the language](docs/frontends/nib/readme.md) |
| `llrm-omf` | A small input route: objects written by QuickBASIC's BC, raised to MIR and compiled afresh; see [BC objects](docs/frontends/bc.md) |
| `llrm-mir` | Reads and verifies MIR text and writes it back; `--run [--entry NAME] [--fuel N]` runs `@main` on the interpreter; `--ivs` prints each innermost loop's induction-variable count |
| `llrm-run` | Runs a Nib module's entry on the host HIR interpreter: `llrm-run [-m16\|-m32] SOURCE.nib [ENTRY] [INTEGER...]` |
| `nibfront` | Nib's front end alone: `--tokens`, `--syntax`, or `--declare h\|bi\|inc` for C, QuickBASIC and assembler declarations of a module |
| `nib-lsp` | Nib's language server; see [the server](docs/frontends/nib/lsp.md) |

Floating point is native x87, so a coprocessor is required. The crates behind these programs are in
[crates](docs/reference/crates.md).

## Build

```sh
cargo build --release
```

The default feature `toolchain` bootstraps what the compilers and tests need:

| Feature | Builds |
|---|---|
| `wccq` | Open Watcom's front end for `llrm-c` (`toolchain/owshim/`) |
| `jw` | `jwasm` and `jwlink` |
| `dosrun` | the headless DOSBox-X the DOS tests run on |

Their binaries land in `target/<profile>`. Sources are cloned under `$XDG_CACHE_HOME/llrm` (`~/.cache/llrm` when
unset), one tree per pinned commit, made whole or not at all, so worktrees and parallel builds share them safely.
`$OWROOT` names an existing Open Watcom tree to use instead of cloning one. The scripts need a Unix host.

`--no-default-features --features wccq` builds only the front end the library tests need. Without any feature the
build bootstraps nothing; `llrm-c` then has no Open Watcom front end.

On Debian or Ubuntu the toolchain needs:

```sh
sudo apt install build-essential git autoconf automake libtool libpng-dev libpcap-dev libncurses-dev
```

The Rust toolchain is pinned by `rust-toolchain.toml`. Python tools run through `uv`; see
[the tools](docs/reference/tools.md).

## Use

```sh
target/release/llrm-qb PROGRAM.BAS --dialect qb45 --runtime qb45 -o PROGRAM.OBJ
target/release/llrm-c program.c -o PROGRAM.OBJ
target/release/llrm-nib program.nib -o PROGRAM.OBJ
target/release/llrm-omf MAIN.OBJ DRAW.OBJ -o build/optimized -march=i486
```

`llrm-qb`'s default runtime is llrm's own; `--runtime qb45|pds71|vbdos` selects a Microsoft one, and the default
dialect is `vbdos`. `llrm-omf` takes every object of a program in LINK order, ignores `.LIB` arguments, writes
`-o` as the object (one input) or a directory (several), and writes nothing unless every object succeeds.

`llrm-qb`, `llrm-c`, `llrm-nib` and `llrm-omf` share gcc-named options:

| Option | Does |
|---|---|
| `-O0` `-O1` `-O2` `-O3` `-Omax` `-Os` `-Oz` `-Og` | Optimization level; `-O2` is the default. What each level turns on: [levels](docs/reference/levels.md) |
| `-f[no-]PASS` | One pass on or off by gcc's name, for example `unroll-loops`, `inline-functions`, `strength-reduce`, `gcse`, `tree-dse`; the names are the `PASSES` table in `crates/backend/llrm-core/src/driver/flags.rs` |
| `-march=CPU`, `-mtune=CPU` | The processor, gcc's name for it (`i386` through `core2`) |
| `-m16`, `-m32` | The target; `-fobject-format=omf\|elf\|macho\|coff` its object format |
| `-fsanitize=CHECKS`, `-ftrapv`, `-fwrapv` | Run-time checks: `bounds`, `integer-divide-by-zero`, `signed-integer-overflow`, `stack`, or `undefined` (the first three); see [semantic switches](docs/reference/semantics/readme.md) |
| `-g` | Debug information; see [formats](docs/formats/readme.md) |
| `-S` | Writes the assembly listing instead of an object |
| `--dump DIR` | Writes every stage to `DIR`, for diffing; `llrm-qb`, `llrm-c` and `llrm-nib` take it, and `LLRM_MIR_STAGES=DIR` does the same for any of them |

`llrm-qb --runtime-frames` calls the runtime's frame entry and exit in every procedure, so its frame chain, stack check
and event poll are there to debug against. `--own-frames` is accepted and does nothing.

One loop in three languages, compiled with `-S` and compared: [examples/dot](examples/dot).
[Loop strength reduction](docs/optimizations/lsr.md) says what the backend does with it.

## Validate

```sh
cargo test --release <name>
uv run --project tools python tools/loops/run.py --quick
```

The second command runs the loop corpus: one loop program per case, emitted as C, QuickBASIC and Nib and checked
against an independent oracle and against hand-derived quality bounds. [Testing](docs/working/testing.md) has the
rest; [debugging](docs/working/debugging.md) finds a miscompile in a running DOS program.

Format with `tools/fmt.sh`.
