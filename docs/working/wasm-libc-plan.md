# wasm: libc per platform and the shared .td generator

Owner llrm-target-wasm, plan due 2026-11-08. Goal (user 11-01): DOS uses Watcom's libc; every other platform its own libc;
the frontend stays clang; cards compare library bytes separately from program bytes.

## Today
- `runtime/wasm/libc.c` (68 lines: `memcpy`, `memmove`, `memset`, `memcmp`, `strlen`, `nanosleep`), `include/*.h` stubs
  (math, string, stdio, time, pthread, stdatomic) and `os.c` (280 lines, the JS host layer). The runtime builds with
  `-ffreestanding -nostdinc`. No real libc: a QB runtime routine that needs one more symbol needs one more stub.
- No C row for wasm in `llrm-cc` (`targets.toml`): C reaches wasm only through `runtime/wasm/build.sh` and the `.ll`
  goldens, so there is no wasm corpus against clang.

## Plan
1. Inventory (no build needed beyond the runtime): the libc symbols `runtime/qb` and `runtime/shared/posix` leave
   undefined, from the objects' undefined-symbol lists. Output: a table symbol / caller / WASI function it maps to.
2. The platform libc for wasm32 is wasi-libc (musl-derived, WASI preview1). Fetched 2026-11-02 with the user's yes
   (05:25): wasi-sdk-34's `wasi-sysroot-34.0.tar.gz`, 119,283,116 bytes, sha256
   `9d813544eeebe38b7b8f2244ed591de46b6db812c6dd1a257ff9f0d2a905a2be` (verified), extracted to
   a scratch directory outside the repo, `wasi-sysroot-34/sysroot/wasi-sysroot-34.0` (wasi-libc 2e6fb9d8ee0c, llvm 23.1.0 headers;
   targets wasm32-wasip1/-threads/wasip2/wasip3). The tarball carries no licence file; the wasi-libc repo has LICENSE,
   LICENSE-APACHE, LICENSE-APACHE-LLVM and LICENSE-MIT. Nothing else is cleared to fetch.
3. `llrm-cc` row `wasm32-wasi`: clang `--target=wasm32-wasi --sysroot=...`, `llc_program = "llrm-wasm-llc"`, object
   `wasm`, link `wasm-ld`, run `node`. The C-ABI lookup reads `llrm-NAME/src/machines/datalayout.toml`; wasm names it
   `datalayout32.toml`: add a `description_file` key to the row (one place) rather than copy the file.
4. Replace `libc.c` and the stub headers with the sysroot. `os.c` stays the host layer, its imports mapped onto
   `wasi_snapshot_preview1` where WASI has the call (`clock_time_get`, `fd_write`, `args_get`) and kept as `env` imports
   for the DAC, keyboard and screen (no WASI equivalent).
5. Corpus vs clang `--target=wasm32 -O2` (clang 20 is installed): per program code bytes of the program object, and
   instructions under node; the library's bytes in their own column. Fitted limits carry owner and date.
6. Shared .td generator: `regen.sh` already goes through `tools/dagisel/regen-target.sh` (as m68k and 6502); what is
   still wasm's own is the python that writes `ASM_STRINGS` (`regen.sh` lines 40-61). Move it into `llrm-dagisel-gen`'s
   `asm.rs` output, which m68k uses, so a target states its asm strings in its record only.

## Open
- Which of wasi-libc's `malloc` and stdio the QB runtime may use, against its own allocator (`runtime/qb/array.c`).
- Threads: `pthread.h` is a stub today; wasm32-wasi has no threads, so the stub stays.
