# C through clang: `llrm-cc`

TL;DR: `llrm-cc` is a `cc`-compatible driver. It runs the unmodified pinned clang as the front end, then `llrm-llc` for everything after. x86-64 and m32 DOS work end to end; m16 needs the i8086 clang (fork branch `llrm/clang-i8086`, not yet pushed).

## How it runs

```
llrm-cc [-target NAME] [cc flags] FILE.c...
  clang <table clang args> <user flags> -S -emit-llvm -Xclang -disable-llvm-passes -Xclang -disable-O0-optnone
  llrm-llc -O<n> <table llc args> x.ll -o x.o      (the -O level, -f[no-]unroll-loops, -S)
  link recipe from the table                        (no -c / -S)
```

Only phase flags are understood (`-c -S -o -O* -g0`, inputs, link flags). `-E`, `-M`, `-MM`, `-emit-llvm` and queries run clang as is. `.s` inputs go to clang's assembler. Everything else is clang's. `-fsanitize=*`, `-fprofile-*`, `-flto`, `-g` (not `-g0`) are refused until translated.

Per-target facts are in one table, [targets.toml](../../../crates/driver/llrm-cc/targets.toml): triple, clang arguments (headers, `-D`), llrm-llc arguments, object format, link and run recipes, and the data layout. Clang's layout must lay types out as the table's does; the driver then gives llrm-llc the table's spelling.

## The pin

`toolchain/clang/pin` names the commit. Upgrading: change the SHA, rebuild clang and opt, then run

1. every corpus `.ll` through `llrm_mir::parse` (an unknown token fails);
2. `tools/clang-oracle.sh` (clang split from `opt default<On>` prints the same IR as `clang -On`).

`tools/cc_check.sh` runs a directory of programs through clang and through `llrm-cc` and diffs stdout (`LLRM_CC_TARGET`, `LLRM_CC_DIR`, `LLRM_CC_RUN`).

## What `-disable-llvm-passes` skips

Clang's `BackendUtil` builds these only when passes are enabled. llrm must supply each:

| Piece | Where llrm stands |
|---|---|
| `buildPerModuleDefaultPipeline(O)`, including `AlwaysInliner` at O0 and `CoroEarly/Split/Cleanup/AnnotationElide` | the default pipeline is llrm's; coroutines are not translated (C has none) |
| ObjC ARC expand and opt | not translated; C and C++ only |
| Sanitizers (`-fsanitize`), bounds checking, KCFI, `allow.ubsan.check` lowering | refused |
| GCOV, instrumented PGO lowering, MemProf | refused |
| Fat/Thin LTO pre-link pipelines, type-test dropping | `-flto` refused |
| Assignment tracking (`-g`, O1+) | `-g` refused |
| Pipeline tuning that does not ride in IR: loop unrolling, interchange, fusion, interleaving, loop and SLP vectorization, merge-functions, call-graph profile | `-f[no-]unroll-loops` forwarded; llrm has no vectorizer |
| TLI preset: `-fveclib`, `-fno-builtin` (the latter rides as `nobuiltin`) | not forwarded; no vector library calls without a vectorizer |

## Status (2026-10-26)

| Target | State |
|---|---|
| `x86_64-linux` | `tests/llc`: 25 of 36 equal to clang's; the rest hit `llrm-llc` backend gaps (SSE floats, allocator, wide ops). Examples: 9 of 11 |
| `arm64-glibc` | `tests/llc` 36 of 36, examples 11 of 11 under qemu-aarch64 against the glibc sysroot (`LLRM_ARM64_SYSROOT`); Mach-O on the Mac untested |
| `x86_32-linux` | ELF32 on the host's glibc (`clang -m32` links and is the reference): `tests/llc` 36 of 36, examples 11 of 11 |
| `x86_32-wat` | Open Watcom libc built from source (`tools/build-ow-clib.sh`): `tests/llc` 36 of 36, examples 11 of 11 |
| `x86_16-wat` | same libc, 16-bit medium model: `tests/llc` 36 of 36, examples 11 of 11 (`sum_three` with far pointers); needs the patched clang (`tools/build-clang-m16.sh`, `LLRM_CLANG_M16`) |
| `x86_16-bcc` | Borland headers and medium libc |
| `x86_32-dos`, `x86_16-dos` | `tests/cc/dos` programs against the llrm OS layer |
| QCport m16 | 65 of 65 modules compile at -Os and draw the Borland build's frames, polys and BMP md5 (`tools/qcport-run-cc.py`) |

The i8086 patch (the fork branch `llrm/clang-i8086`, all in-tree, no patch files here):

- Target info with far/huge address spaces, far function pointers, Pascal as CC 1000, near/far/huge pointer conversions allowed, `far` dropped on function types.
- Keywords `__far__`, `__near__`, `__huge__` under `-fborland-extensions` (an error on other targets); `toolchain/clang/borland.h` maps Borland's `far`/`near`/`huge` onto them.
- `__emit__(bytes...)` is a builtin that puts the bytes in the code stream.
- Memory models: `-mcmodel=small|medium|compact|large|huge`. Code is far (`P1`) in medium, large and huge; data pointers are far by default in compact, large and huge, and `__near__` overrides. `ptrdiff_t` is `long` with far data. Default: medium.
- A huge object lives in a far segment (address space 1); the difference of two huge pointers is `llrm.ia16.ptrdiff`.
- Pragmas: `#pragma aux NAME "LINK"` is the link name of NAME (an asm label, `\01LINK`, which llrm-llc does not decorate); `data_seg`, `code_seg`, `bss_seg`, `const_seg` take Watcom's `("NAME", "CLASS")` and give globals and functions `section "NAME"`, which llrm-llc places in a data or code segment of that name. `#pragma aux NAME [parm [r]..] [value [r]] [modify [exact] [r]..] = "code" ..` makes a call of NAME the code as inline assembly in those registers (byte registers travel as their word; `ah`-style high bytes and segment registers are not carried). A register convention without code (`#pragma aux f parm [ax] value [ax]` on a real function) is dropped on purpose: no runtime or QCport source uses it, and clang and gcc have no per-function register lists (only `regparm`/`fastcall`-style conventions); the pragma is ignored with a warning.
- Huge pointers index by 32 bits and carry into the selector; loops over them are windowed. A static huge object past 64K is laid down as consecutive 64K segments (`NAME$0`, `NAME$1`, ..., one object file, `masm::split_data`), which a huge pointer steps through. MIR prints no `section`.
- Programs run in every model: small and compact link the start-up and externals near (`LLRM_NEAR_CODE`).
- Tests: `clang/test/CodeGen/i8086-*.c` in the fork (no `llvm-lit` in the local build; run their RUN lines by hand), `crates/frontends/llrm-cc/tests/{abi,models}.rs` here.
