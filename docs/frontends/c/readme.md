# C frontend

Audience: users and contributors of `llrm-cc`. How C source is compiled.

`llrm-cc` is a `cc`-compatible driver: the pinned clang is the front end, `llrm-llc` everything after. How it runs, the
pin and the per-target table are in [clang-route](clang-route.md); [its calling conventions](calling-conventions.md).

## Targets and ABI

A target is a row of [`targets.toml`](../../../crates/driver/llrm-cc/targets.toml), named by `--target=ROW` or
`LLRM_CC_TARGET`: `x86_16-wat` (real-mode DOS), `x86_32-wat` (flat 32-bit DOS), `x86_32-linux`, `x86_64-linux`, `arm64-glibc`.

On real mode the default is the medium model: far code, near data, regparm3. `-mabi=ABI` selects another ABI of the
target:

| Target | ABIs |
|---|---|
| `x86-m16` | `cdecl` (Borland), `watcom`, `ia16`, `regparm3` (the default) |
| `x86-m32` | `watcom`, `sysv` |

The C runtime and its start-up are in `runtime/c`, on top of the OS layer in `runtime/shared/dos`. A program that
includes `llrm_os.h` finds it without `-I`.

## Usage

```
llrm-cc [--target=ROW] SOURCE [-o OUT] [-I DIR]... [options]
```

`-o OUT.obj` (or `.o`) writes an object; `-S` writes assembly. Clang's options apply, and the shared options of the
llrm compilers: `-O0` to `-O3`, `-Os`, `-Oz`, `-f[no-]PASS`, `-march`, `-mtune`, `-m[no-]stack-is-data`,
`-m[no-]far-bss`, `-m[no-]bss-begin`, `-mcode-segment=NAME`, `--machine`, `-fstack-usage`, `-Wstack-usage`, `--dump DIR`.
`-fsanitize=stack` checks the stack on entry. `llrm-llc --help` prints the full list.

## Tests

Programs in `tests/run/c` are compiled, run in DOSBox-X and compared with their `.out` by
`tools/dosbatch/run_tests.py c`; see [testing](../../working/testing.md). `tests/check/c` holds
`// RUN: llrm-cc ...` assembly checks.
