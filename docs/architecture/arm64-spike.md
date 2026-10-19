---
architecture-index: arm64 backend spike, plan and order of work
architecture-order: 6
---
# arm64 spike

TL;DR: POC, not for feat/parity until decided. Mach-O arm64 (Darwin AAPCS64) first, AArch64 ELF under qemu second, translated from the LLVM AArch64 backend at `d1106deb71cc`.

## What exists

- `crates/target/llrm-target`: the generic layer (`Target` trait, `Machine`, `calling`, `registers`, `layout`, `CpuPrices`); data-driven by toml/text descriptions.
- x86 only: `llrm-x86` (`x86.instr` forms, `family.isel`, `peephole.peep`, TTI, lowering), `-m16`, `-m32`, `-bc`. isel, assembly and LIR forms in `backend/llrm-core` are driven by those tables; `llrm-lir` carries x86 notions (flags, two-address).
- Objects: `llrm-object::Arch` is `I8086 | I386 | X8664`. `llrm-macho` and `llrm-elf64` write x86-64 only; `llrm-macho` is on this tree (650 lines), wired in `objbuild.rs` for `X8664`.

## A new target implements

`llrm_target::Target` (names, layout, registers, results, stack pointer, callee-saved, frame register, forms text), `llrm_mir::tti::Tti`, `llrm_mir::lowering::Lowering`, a `calling` description, forms and selection tables, an encoder, and an object `Arch`.

## Order (dates are targets)

1. 10-19..10-21: AArch64+X86 pin build in a separate dir (Release, no assertions, buildslot, low -j); `llc`, `llvm-mc`, `llvm-objdump`, `clang`. Oracle = pinned `clang -target arm64-apple-macos`.
2. 10-22..10-24: probe which x86 assumptions the LIR and isel carry (flags, two-address, forms). Output: list of leaks, each fixed in the generic layer or recorded as a numbered DEVIATE.
3. 10-25..10-29: `Arch::Aarch64`; Mach-O arm64 and ELF aarch64 writers (relocs `ARM64_RELOC_*`, `R_AARCH64_*`); checked with `llvm-objdump`/`llvm-readobj` against pinned clang output.
4. 10-30..11-06: `llrm-aarch64`: registers, Darwin CC, forms, encoder (checked against `llvm-mc`).
5. 11-07..11-20: TTI, isel, frame lowering from `AArch64ISelLowering`, `AArch64FrameLowering`.
6. 11-21..11-30: milestone programs (return, loops, calls, globals, printf) run under qemu; on the Mac when a route exists.

## Risk

Step 2 decides cost: if LIR cannot carry 3-address and NZCV, the generic layer changes first.
