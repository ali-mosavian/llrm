---
architecture-index: LP64 C front end, plan (no code)
architecture-order: 8
---
# LP64 C front end: plan

TL;DR: llrm's C front end is Open Watcom's `wccq` raising the code generator's stream; it cannot produce 64-bit or glibc C. The x86-64 and arm64 spikes use pinned clang's front end alone (`-O2 -Xclang -disable-llvm-passes -emit-llvm`) through `llrm-llc`. A real front end for llrm is a later item; this page is its plan, not work.

## What `wccq` to HIR assumes

- One tree per CPU (`i86`, `386`; `LLRM_WCCQ_DIR/<cpu>/wccq`), a patched Open Watcom front end. Types are Watcom's: `int` 2 or 4 bytes (`int_bytes` in `runtime/c/<target>/c.toml`), `long` 4, pointers 2, 4 or 2+2 far, `long double` x87; no LP64 model and no GNU C (`__attribute__`, `__extension__`, `__builtin_va_list`, `_Float128`), so no glibc headers.
- The product is a code-generator stream (`.cgs`, `stream.rs`), trees of the generator's own nodes (`hir.rs`, 735 lines); `translate.rs` (4.2 k lines) lowers it to HIR, then `llrm_hir::mir::emit` to MIR. Calls carry Watcom conventions (`call_class`, `REVERSE_PARMS`, `CALLER_POPS`).
- The target's `Profile` (`compile.rs`) names the CPU, the ABI switches and the header; nothing in it states an LP64 ABI.

## What a front end for 64-bit C needs

1. The C11 language with the GNU extensions glibc's headers use, a preprocessor, constant evaluation, sizes and alignments from the datalayout (`mode`, `p:64`), SysV AMD64 argument classification (INTEGER/SSE/MEMORY) for aggregates, `va_list` as the ABI's struct.
2. Output MIR (LLVM IR as `llrm_mir::parse` reads it) with clang's shapes: allocas for locals, `tbaa`, `lifetime` markers, `nsw`, `noundef`, the attributes `-O2` puts on functions.

## Routes

| route | effort | notes |
|---|---|---|
| A. clang's front end alone, IR to `llrm-llc` (this spike) | done for the spike; the IR reader's gaps are closed one attribute or intrinsic at a time | needs the pin's `clang` on the build host; not llrm's own front end; the oracle shares the front end, so front-end bugs cannot show |
| B. Port the front end to LP64: patch Open Watcom's | large, and a dead end | no GNU C, no 64-bit code generator in the tree; each extension is a patch to a 1990s front end |
| C. Own front end in Rust (lexer, preprocessor, parser, sema, lowering to MIR) | 30 to 50 k lines | clean ownership, offline builds; the work is the C library headers' extensions and the diagnostics |

## Order

1. Keep A as the milestone route for both spikes; widen the IR reader as programs need (`uwtable`, `captures`, lifetime markers without size, `dso_local`, `target triple` are done or stripped by `llrm-llc`).
2. Decide B or C after the spikes' report: C if the backend spikes are kept. Start with the preprocessor and declarations of the headers a hosted program includes (`stdio.h`, `stdlib.h`, `string.h`), then expressions and statements to MIR, checked against clang's IR for the same file (`tools/llc_check.sh` already runs both).
3. Retire `wccq` for flat targets once C reproduces the m32 rows; the 16-bit targets keep it until then.
