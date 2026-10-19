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

## x86 leaks (step 2 probe, first pass)

1. `RegId` is a newtype over iced's `Register`; constants are generated from iced's names (`llrm-lir/build.rs`), `index()` is iced's number. AArch64 registers have no id. Fix: ids generated from the target's `registers.regs`, per target.
2. `Flag` is iced's `RflagsBits`; LIR flag reads/writes are CF/PF/AF/ZF/SF/OF. AArch64 has NZCV. Fix: flag set per target.
3. Selection tables are x86's: `x86.instr` (two-address `^0` ties, fixed cx shifts, rm/r shapes) and `family.isel`, `peephole.peep`. AArch64 needs its own tables; the format must carry three-address forms and condition-select.
4. Encoding goes through iced (`llrm-x86/src/asm.rs`, block encoder). AArch64 needs its own fixed-width encoder, checked against `llvm-mc`.
5. `Mem`/`Addr`/`Space` come from `llrm_omf::module` and carry segment/frame-BP notions (`FRAME`, `in_frame`). Flat targets use a subset; AArch64 addressing (base+imm, base+index<<s, pre/post-index, pairs) needs `AddressForm` entries.
6. Backend files naming iced: 12 (`verify`, `registerinfo`, `peephole`, `valuetrack`, `target`, `cfi`, ...); `RegId::` appears in 78 files (1320 hits), `Space::` in 52.
7. `Arch` and `objbuild.rs` know three x86 arches; `llrm-macho`, `llrm-elf64` write x86-64 relocations only.

Cost reading: items 1, 2 and 6 touch the shared backend and are the real price; 3 and 4 are new target crates. Decision after the first pass: do item 1 as a generic-layer PR before any AArch64 code.

## Mac (Mach-O arm64 runs)

The Mac needs: a clone or worktree of `feat/arm64-spike`; rustup (toolchain per `rust-toolchain.toml`); Xcode command line tools (`clang`, `ld`). Then:

```
cargo build --release -p llrm-c --bin llrm-c
target/release/llrm-c -O2 -c prog.c -o prog.o   # arm64 Mach-O, once step 5 lands
clang -arch arm64 prog.o -o prog && ./prog       # system clang links; the compiler is llrm
clang -arch arm64 -O2 prog.c -o ref && ./ref     # reference output
```

Until step 5 the Mac builds and runs the existing test suite only; the first useful Mac run is the writers check (step 3: `otool -rv`, `ld` accepts the object).

## Risk

Step 2 decides cost: if LIR cannot carry 3-address and NZCV, the generic layer changes first.
