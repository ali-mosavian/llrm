---
architecture-index: x86-64 backend spike, plan and order of work
architecture-order: 7
---
# x86-64 spike

TL;DR: POC, not for feat/parity until decided. ELF64 x86-64 (SysV AMD64) on tinytim, translated from the LLVM X86 backend at `d1106deb71cc`; oracle is the pinned `clang -target x86_64-linux-gnu`. Status 2026-10-19: target `x86-m64`; 14 of 15 programs of `tests/llc` match the pinned clang's run (`tools/llc_check.sh`); aggregates in registers, SSE and the variadic `al` are open. The front end is clang's (`llrm-llc`), see [LP64 C front end](lp64-c-frontend-plan.md).

## What exists

- Objects: `llrm-object::Arch::X8664`, `llrm-elf64` (206 lines: RELA, `R_X86_64_{64,PC32,PLT32,32,...}`; `Machine` over `llrm-elf`'s container), wired in `objbuild.rs`. Untested against a real link beyond the crate's own tests.
- Shared x86 layer `llrm-x86` (`x86.instr` 124 lines, `family.isel` 466, `peephole.peep` 320, `asm.rs` on iced `Mode { bits: 64, .. }`, TTI, lowering) used by m16 and m32. `llrm-x86-m32` is a thin target: registers, `calling.toml`, `datalayout.toml`, `object.toml`, timings, 21 lines of `x86.instr` and 64 of peepholes. No x86-64 target crate, no AMD64 registers, no SysV64 calling convention (`sysv32` only).
- Pinned `clang`, `llc`, `opt` in `~/scratch/llvm-build/bin` (X86 target; no `llvm-objdump`: use binutils for the disassembly).

## What m32 assumes

1. Pointer is a dword (`p:32:32`, `[pointers] 4 = "near"`); i64 is two dwords: `isel/wide.rs` and `lower_int64.rs` split every i64, `calling.toml` has `wide_pairs`. x86-64 has native i64 and 8-byte pointers.
2. Registers: `registers.regs` names eax..esp and 16/8-bit views only; no r8-r15, no SIL/DIL/BPL/SPL (REX byte regs), no xmm. Float is x87 (`floatalloc`, `fpstack`); x86-64 passes and returns floats in SSE registers, which the backend has no class for (largest piece of work).
3. Calling: `sysv32` only; SysV AMD64 is new: six integer and eight SSE argument registers, red zone, 16-byte stack alignment, `al` = vector count on variadic calls, aggregates classified INTEGER/SSE/MEMORY.
4. Addressing: no RIP-relative form; `address_form` tables are 32-bit base+index only. Globals need `[rip+disp32]` and `R_X86_64_PC32`/`PLT32`/GOTPCREL(X).
5. Frame: `frame.rs`/`nativeframe.rs` assume `ebp`/`esp`, 4-byte slots, `return_address_bytes = 4`, push/pop of 32-bit registers, no red zone, no REX.
6. Encoding: iced handles 64-bit; `asm.rs` bitness filter and `x86.instr` forms must grow REX (`w`, r8+), 32-bit ops zero-extending the upper half, movsxd, `cqo`.
7. Generic-layer leaks shared with arm64: `RegId` is iced's `Register` (x86-64 names already exist there, so no change needed for this spike), `Flag` is iced's `RflagsBits` (fine). Both are arm64's to change; I take what lands.
8. TTI, lowering (`llrm-x86/src/tti.rs`, `lowering.rs`): `i64` cost and legality tables are 32-bit; `X86TTIImpl` at the pin has the 64-bit rows.

## Coordination with arm64-spike

The only shared-layer change I need is none for registers/flags: x86-64 fits the iced-based ids. I need native i64 as a legal type and 8-byte pointer from `datalayout.toml` (target data, not shared code) and `calling.toml` growing SSE classes and aggregate classification. I will tell arm64-spike before touching `calling.rs`, `registers.rs` or `layout.rs`, and keep the changes additive.

## Order (dates are targets)

1. 10-19..10-20: this page; probe pinned `clang -target x86_64-linux-gnu -O2 -S` and `-c` for the milestone programs; link with gcc/ld against glibc; store the reference outputs and object disassembly (premise: output matches `gcc -O2` run).
2. 10-21..10-23: `Arch`/`llrm-elf64` check: write a hand-built object with a call, a global (PC32, PLT32, GOTPCREL) and link it with the system linker; compare relocations with `readelf -r` of clang's object (fail-first tests).
3. 10-24..10-27: `llrm-x86-64` target crate: `registers.regs` (rax..r15, 8/16/32 views with REX byte regs), `datalayout.toml` (`p:64:64`, native i64), `object.toml` (ELF, `emulation = "elf_x86_64"`), `machine` and a `Target` impl; driver registers `-m64`.
4. 10-28..11-03: SysV AMD64 `calling.toml` and classification from `X86CallingConv.td`/`X86ISelLoweringCall.cpp`; native i64 in isel (retire `wide` splitting for this target); REX forms in `x86.instr`; RIP-relative addressing and symbol relocs. Integer-only programs first: return value, loops, calls, globals.
5. 11-04..11-10: frame lowering from `X86FrameLowering.cpp` (64-bit pushes, red zone, CFI, 16-byte alignment) and TTI rows for x86-64.
6. 11-11..11-20: SSE register classes and SysV float/double passing; variadic `al`; `printf` program.
7. 11-21..11-30: milestone: return, loops, calls, globals, printf compile, link with glibc, run, match clang's output; rows are not claimed (POC), compile-time and code-size vs clang reported.

## Risk

Step 6: the backend has no SSE class; if adding one touches the register allocator classes beyond data, the milestone's printf (doubles) shrinks to integer and `%s`/`%d` formats first. Step 4: native i64 must not regress m32's wide path; the target flag selects it.
