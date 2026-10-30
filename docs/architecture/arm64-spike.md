---
architecture-index: arm64 backend spike, plan and order of work
architecture-order: 6
---
# arm64 spike

TL;DR: POC, not for feat/parity until decided. Mach-O arm64 (Darwin AAPCS64) first, AArch64 ELF under qemu second, translated from the LLVM AArch64 backend at `d1106deb71cc`.

## What exists

- `crates/target/llrm-target`: the generic layer (`Target` trait, `Machine`, `calling`, `registers`, `layout`, `CpuPrices`); data-driven by toml/text descriptions.
- x86 only: `llrm-x86` (`x86.instr` forms, `family.isel`, `peephole.peep`, TTI, lowering), `-m16`, `-m32`, `-bc`. isel, assembly and LIR forms in `codegen/llrm-codegen` are driven by those tables; `llrm-lir` carries x86 notions (flags, two-address).
- Objects: `llrm-object::Arch` is `I8086 | I386 | X8664`. `llrm-macho` and `llrm-elf64` write x86-64 only; `llrm-macho` is on this tree (650 lines), wired in `objbuild.rs` for `X8664`.

## A new target implements

`llrm_target::Target` (names, layout, registers, results, stack pointer, callee-saved, frame register, forms text), `llrm_mir::tti::Tti`, `llrm_mir::lowering::Lowering`, a `calling` description, forms and selection tables, an encoder, and an object `Arch`.

## Status

Works, checked against the pinned `clang -O2` (output and exit status, under `qemu-aarch64` for ELF; 22 programs in `crates/target/llrm-arm64/tests/programs`, run by `tests/run.rs`): integer and float arithmetic of every width, division by constants, pointers and arrays, structs and arrays by value and by `sret`, variadic functions (AAPCS64 register save areas; Darwin stack slots), calls with stack arguments, function pointers, `switch`, variable-length arrays, 5 KB frames, `malloc`ed lists and trees, `qsort` callbacks, global initializers with relocations. Mach-O arm64 and ELF aarch64 objects; the Darwin fixtures disassemble clean under the pinned `llvm-objdump`; `tools/targets/arm64-mac-check.sh` runs them on a Mac.

Not yet: `__int128` (and i128 in the mid-end's SCEV), vectors, atomics, exceptions, `long double`, TLS, jump tables, `-fPIC` data access on ELF.

Measured with `tools/targets/arm64_compare.py` (modes `whole`, `backend`, `compile`; the program's own code only, runtime excluded):

| comparison | code bytes | instructions executed | note |
|---|---|---|---|
| backend alone, same clang -O2 IR as `llc -O2` | 1.08x | 1.14x (geomean) | no LSR/post-index, no `ldp`/`stp` |
| whole: this mid-end + backend against `clang -O2` | 1.60x | 1.62x | mid-end gaps: no vectorizer, recursion peeling |
| compile time against `opt -O2` + `llc -O2` | | 4.35x | backend alone is 5.8 M instructions against `llc`'s 25.5 M |

## Numbered DEVIATEs

1. `regalloc.rs`: ranges with holes, copy coalescing, no splitting or eviction; stands in for `RAGreedy`.
2. `driver.rs`: Apple's `va_arg` instruction is written out as loads in the text; MIR has no `va_arg` (exit: the instruction, owner lir-text-prs-1893).
3. `prepass.rs`: CSE and loop hoisting of constants and addresses, a small version of `MachineCSE`/`MachineLICM`.
4. `driver.rs`: newer clang's `ptrtoaddr` is read as `ptrtoint`; MIR refuses it by design.

## Numbered gaps

- ARM64-EIC-1: no `MachineEarlyIfConversion`/`MachineTraceMetrics` (pin: `aarch64-enable-early-ifcvt` on). `div_constants_long`: pin IR has 28 selects, the asm 121 `csel`/`cset`/`cinc`-family instructions, ours 35 and 35; the extra ones are made by the machine pass from `br` chains the pin IR still has. Programs with more in the pin than ours: 9 of 159 built (7 FAIL rows excluded); `div_constants_{long,short,ulong}` hold 284 of the 298 pin instructions, the other six 14. Exit: step 6 (~3,000 pin lines, not worth writing twice).

## Order of work

1. Pin build with AArch64. Done.
2. Leaks of the x86 backend (below). Done; decision: own backend from MIR.
3. Object writers (Mach-O, ELF). Done, checked against `llvm-mc`.
4. Register file, A64 encoder (byte-identical to `llvm-mc`), constant expansion (identical to `llc`). Done.
5. Selector, allocator, frame, calling convention, cost model (5064 oracle rows equal the pinned `opt`). Done.
6. The deciding experiment: the core machine passes on arm64 LIR. Open.

## x86 leaks (step 2 probe, first pass)

1. `RegId` is a newtype over iced's `Register`; constants are generated from iced's names (`llrm-lir/build.rs`), `index()` is iced's number. AArch64 registers have no id. Fix: ids generated from the target's `registers.regs`, per target.
2. `Flag` is iced's `RflagsBits`; LIR flag reads/writes are CF/PF/AF/ZF/SF/OF. AArch64 has NZCV. Fix: flag set per target.
3. Selection tables are x86's: `x86.instr` (two-address `^0` ties, fixed cx shifts, rm/r shapes) and `family.isel`, `peephole.peep`. AArch64 needs its own tables; the format must carry three-address forms and condition-select.
4. Encoding goes through iced (`llrm-x86/src/asm.rs`, block encoder). AArch64 needs its own fixed-width encoder, checked against `llvm-mc`.
5. `Mem`/`Addr`/`Space` come from `llrm_omf::module` and carry segment/frame-BP notions (`FRAME`, `in_frame`). Flat targets use a subset; AArch64 addressing (base+imm, base+index<<s, pre/post-index, pairs) needs `AddressForm` entries.
6. Backend files naming iced: 12 (`verify`, `registerinfo`, `peephole`, `valuetrack`, `target`, `cfi`, ...); `RegId::` appears in 78 files (1320 hits), `Space::` in 52.
7. `Arch` and `objbuild.rs` know three x86 arches; `llrm-macho`, `llrm-elf64` write x86-64 relocations only.

Cost reading: items 1, 2 and 6 touch the shared backend and are the real price; 3 and 4 are new target crates. Decision after the first pass: do item 1 as a generic-layer PR before any AArch64 code.

## Decision: arm64 gets its own backend from MIR (2026-10-19)

The shared backend (`llrm-codegen`: 146 K lines, isel tables, masm, assembler) is x86 end to end: selection patterns and hooks, 16/32-bit `LirBody`, an iced encoder behind `masm`. Generalizing it first would touch the files 30 sessions are landing in. The spike therefore takes MIR (after the shared mid-end) through arm64 modules of its own in `llrm-arm64`: isel, register allocation, frame lowering, MC encoding, object. Reuse is by interface: MIR and the mid-end, `Target`/`Machine` descriptions, `RegId` ids, `Object` and its writers. Whether the machine passes (greedy allocator, copy propagation) can later run on arm64 LIR is a separate question, answered by trying it on this selector's output. Front end: clang's IR (the shared `llrm-llc` door of the x86-64 spike).

This private path duplicates what LLVM shares (one CodeGen over any target). It is a spike-only shortcut, not the merge design: the mirror is the shared machine passes (greedy allocator, copy propagation, sink) running on target LIR with target descriptions. The experiment that decides the merge, run as soon as the first three programs run: feed this selector's output to `llrm-codegen`'s machine passes, and list what breaks and what it costs to fix. Its result goes here, with the end-to-end numbers against the pinned `clang -O2` build.

## Mac (Mach-O arm64 runs)

The Mac needs: a clone or worktree of `feat/arm64-spike`; rustup (toolchain per `rust-toolchain.toml`); Xcode command line tools (`clang`, `ld`). Then:

```
cargo build --release -p llrm-c --bin llrm-c
target/release/llrm-c -O2 -march=<arm64 name> prog.c -o prog.o   # flags provisional: fixed when the target lands (today: -O2 -march=i486 -S -o)
clang -arch arm64 prog.o -o prog && ./prog       # system clang links; the compiler is llrm
clang -arch arm64 -O2 prog.c -o ref && ./ref     # reference output
```

Until step 5 the Mac builds and runs the existing test suite only; the first useful Mac run is the writers check (step 3: `otool -rv`, `ld` accepts the object).

## Risk

Step 2 decides cost: if LIR cannot carry 3-address and NZCV, the generic layer changes first.

`llrm-arm64-llc` takes `-O0`..`-O3`, `-Os` and `-Oz` (default `-O2`); `llrm-cc` passes the level of the `arm64-macos` target row, and `tools/targets/qb-runtime-check.sh` builds the QB host runtime through it.
