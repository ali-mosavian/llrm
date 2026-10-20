---
architecture-index: x86-64 backend spike, plan and order of work
architecture-order: 7
---
# x86-64 spike

TL;DR: POC, not for feat/parity until decided. Target `x86-m64` (ELF64, System V AMD64, run natively on tinytim), translated from the LLVM X86 backend at `d1106deb71cc`; oracle is the pinned `clang -target x86_64-linux-gnu`. Status 2026-10-19: all 27 programs of `tests/llc` (integers, pointers, structs of scalars, variadic calls and definitions, VLAs, external data) match the pinned clang's run (`tools/llc_check.sh`); floating point (SSE) is not started. The front end is clang's, through `llrm-llc`: see [LP64 C front end](lp64-c-frontend-plan.md).

## Route

`clang -O2 -Xclang -disable-llvm-passes -emit-llvm` (the pin) gives MIR text; `llrm-llc` (`llrm-driver/src/llc.rs`, `driver::compiled_module`) runs llrm's own whole pipeline and backend on it and writes an ELF64 object; the system `gcc` links it against glibc. The target is found by the module's `target triple` (`llvm_triple` of the target's `datalayout.toml`). Nothing in `compiled_module` names a target.

## What the target is

- `crates/target/llrm-x86-m64`: `registers.regs`, `calling.toml` (sysv64), `datalayout.toml` (`p:64`, native i64), `object.toml` (ELF, `bitness = 64`, `pc_relative`), `timings.times` (provisional: Skylake-class figures, to be replaced), selector crate `llrm-x86-m64-select`.
- Registers are rooted at their dword names, as m32's are (`rax` is a view of `eax`, `r8` of `r8d`): every 32-bit root convention in the backend holds, and `llrm_lir::root` maps long mode's registers to them. A root's upper four bytes are one lane, so the lane mask stays 128 bits.
- A symbol is `[rip + symbol]` (PC32, PLT32 for calls, `R_X86_64_64` in data). `pc_relative` makes the selector load a symbol's address into a register before it adds another; no jump tables yet (`jump_table 0`).
- i64 is a value (`is_wide` is false where the datalayout has a legal i64); `cqo`, quadword `push`/`div`, REX, `movsxd`.
- Frame: frame-free functions, the stack aligned for every call (`aligned_calls`), `al` set for variadic calls (`variadic_vector_register`), `va_start` with a register save area (`va_list = "sysv-amd64"`), dynamic `alloca` with `stacksave`/`stackrestore`.
- Symbols defined elsewhere are loaded through the GOT (`got_external`, `R_X86_64_REX_GOTPCRELX`).
- TTI: `X86::of_cpu("x86-64")` equals the pinned `opt -passes=print<cost-model>` on all 1592 oracle rows (`X64CostTbl`, free `zext i32 to i64`, 16 GPRs, PIC rip addressing).

## Open

1. SSE: `float`/`double` in `xmm` registers (a vector register class for the allocator, SysV classification of floats and aggregates, constants, compares, `va_arg` of doubles). x87 stays for `long double`.
2. `.eh_frame` and CFI from `X86FrameLowering`; `.rodata` for string constants (they sit in `.data`).
3. The mid-end finds that are not this target's: recursive functions inlined into themselves (fib is 8x clang's size; the clocks inliner's recursive peel, owner cycles-queue-quiet-host).
4. Jump tables (position independent: a table of offsets), timings from the scheduling model.

## Order (dates are targets)

1. 10-19: references and ELF64 link check. Done.
2. 10-19..10-20: target crate, IR front door, integer programs. Done.
3. 10-21..10-24: SSE.
4. 10-25..10-30: frame lowering and CFI from `X86FrameLowering`; GOT and `.eh_frame`; timings from the scheduling model.
5. Report: the same programs for both spikes, with the front end stated as clang's.
