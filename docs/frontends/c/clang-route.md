# C through clang: `llrm-cc`

TL;DR: `llrm-cc` is a `cc`-compatible driver. It runs the unmodified pinned clang as the front end, then `llrm-llc` for everything after. x86-64 and m32 DOS work end to end; m16 needs the carried clang patch.

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
