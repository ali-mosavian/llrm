---
architecture-index: machine descriptions: ISA, tuning, platform files
architecture-order: 560
---
# Machine descriptions

TL;DR: a target is five independent choices, each tied to a gcc/clang switch. Each choice has its own small set of TOML files under `machine/`.

## The five choices

| Choice | Switch | Values | Where |
|---|---|---|---|
| family | `--target` arch | `x86`, `arm`, `m68k` | the target crate (`llrm-x86`, `llrm-arm64`, ...): registers, encodings, calling, data layout, object format |
| instruction set | `-march`, `-m<feature>`, `-mno-<feature>` | a named bundle of features; the x86 minimum is `i386`, also in real mode | `machine/isa/<family>/` |
| tuning | `-mtune` | a CPU name | `machine/tune/<family>/<cpu>.toml` |
| memory model | `--memory-model`, or `-m16` `-m32` `-m64` | `real16`, `flat32`, `flat64` | picks the platform file |
| platform | `--target` os | `dos`, `linux`, `darwin`, `amiga`, `mac` | `machine/platform/<os>/<model>.toml` |

Bitness is not part of the family name.

`--memory-model=real16|flat32|flat64` is the long form. `-m16`, `-m32`, `-m64` are the gcc-compatible shorthand for the same three. `real16` is segmented; its sub-model (small, medium, large) is set by `-mcmodel=`, medium by default.

Thumb is the `-mthumb` state of `arm`. `arm` 32-bit and `thumb` are reserved names: no code generation, no files.

## Features

An instruction-set file is a bundle of features and extends another bundle:

```
machine/isa/x86/features.toml    the catalog
machine/isa/x86/i386.toml        features = []
machine/isa/x86/i486.toml        extends i386
machine/isa/x86/pentium.toml     extends i486
machine/isa/x86/i686.toml        extends pentium
machine/isa/x86/pentium3.toml    extends i686
```

Each feature in `features.toml` has:
- `enables`: the instructions it makes legal (`bswap`, `cmpxchg`, `xadd` from the 486; `cx8`, `tsc` from the P5; `cmov`, `fcmov`, `fcomi`, `nopl` from the P6; `mmx`, `fxsr`, `sse`, `sse2`).
- `requires`: other features (`sse2` requires `sse`).
- `selects`: a strategy it switches on. `sse` puts `float` in xmm registers; `sse2` does the same for `double`; with neither, the FP unit is x87. This is derived as the pin does; there is no `-mfpmath`. Results can differ in the last bits from an x87 build, which keeps extended-precision intermediates.
- `os_enables`: the feature is usable only if the platform allows it (`sse` needs the OS, or the program in real mode, to enable state saving).

SIMD level is part of the feature set. There is no vector MIR until 32-bit parity; the catalog leaves room for it.

## Platform files

`machine/platform/<os>/<model>.toml` holds what the OS and memory model fix: addressing, segment faults, ports, alignment, and which features the OS enables.

| OS | Models |
|---|---|
| `dos` | `real16`, `flat32` |
| `linux` | `flat32`, `flat64` |
| `darwin` | `flat64` |
| `amiga` | `flat32` |
| `mac` | `flat32` |

Files shared by all models of one OS sit in `machine/platform/<os>/` (`dos/ports.toml`).

## How a compile resolves

`--target=i386-pc-dos -m16 -march=i486 -mtune=pentium -msse` reads `isa/x86/i486.toml` plus the `sse` feature, `tune/x86/pentium.toml`, and `platform/dos/real16.toml` with `platform/dos/ports.toml`. `sse` is checked against `os_enables` in the platform file.

`--target=x86_64-linux-gnu -m32` reads `platform/linux/flat32.toml`.

## Current files and their new places

| Today | New |
|---|---|
| `llrm-x86-m16/src/machines/dos.toml` | `machine/platform/dos/real16.toml` |
| `llrm-x86-m32/src/machines/dos32.toml` | `machine/platform/dos/flat32.toml` |
| `llrm-x86-m64/src/machines/linux64.toml` | `machine/platform/linux/flat64.toml` |
| `llrm-target/src/machines/pc-ports.toml` | `machine/platform/dos/ports.toml` |

New: `machine/isa/x86/` (read by `llrm-target`'s `isa`), `machine/tune/x86/` (a CPU's prices, read where a target's `timings.times` cell is `=`), `platform/linux/flat32.toml`, and `platform/darwin/flat64.toml`, written with the Mach-O platform work. Calling, data layout and object files stay in the target crate: the `-darwin` files in `llrm-arm64/src/machines/` are data layout and object content, not a platform file, and stay there.

## Copies of a datalayout

A target's datalayout string is stated once, in its `datalayout.toml`. Tests, fixtures and the cc driver's table spell it too; `tools/datalayout_sync.py --check` names each copy that differs and `--sync` rewrites them (`tools/test_datalayout_sync.py` runs the check). A change to the string is one edit and one `--sync`.

## Adding to it

- A new family: its crate, `isa/<family>/`, `tune/<family>/`.
- A new OS: one `platform/<os>/<model>.toml` per memory model it supports.
- A new CPU: an `isa` file that extends its parent, and a `tune` file.
- Every path named here exists; a test checks it.
