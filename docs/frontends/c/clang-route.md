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

Per-target facts are in one table, [targets.toml](../../../crates/frontends/llrm-cc/targets.toml): triple, clang arguments (headers, `-D`), llrm-llc arguments, object format, link and run recipes, and the data layout. Clang's layout must lay types out as the table's does; the driver then gives llrm-llc the table's spelling.

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
| `x86_64-linux` | `tests/llc` programs run equal to clang's, except those needing SSE floats or hitting the allocator (backend gaps: aggregates, fcall, fcmp, fconst, fconv, fnbody, fneg, fselect, fsum, strings) |
| `arm64-linux` | all 23 `llrm-arm64` test programs equal to clang's under qemu-aarch64; Mach-O on the Mac untested |
| `x86_32-dos` | `tests/cc/dos` programs equal to the host's under dosrun |
| `x86_16-dos` | `tests/cc/dos` and `sum_three` (far pointers, expected 1110) equal; needs the patched clang (`tools/build-clang-m16.sh`, `LLRM_CLANG_M16`) |
| QCport m16 | 14 of 65 modules compile. Open: `interrupt` functions, far automatics, unknown `QSurf` (typedef via a far-qualified tag), a few far-to-near argument cases, `_asm` blocks |

The i8086 patch (the fork branch `llrm/clang-i8086`): target info with far/huge address spaces and far function pointers, Pascal convention as CC 1000, near/far/huge pointer conversions allowed, `far` dropped on function types and void results.
