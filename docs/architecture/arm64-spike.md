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

1. `regalloc.rs`: the fallback allocator (ranges with holes, copy coalescing, no splitting or eviction), used under `LLRM_ARM64_LINEAR` or where the shared greedy refuses a function. The default is the shared greedy through `lir_view`; the machine passes around it are arm64's own (exit: arm64 on `flow::machine_with`).
2. (closed 2026-11-09) Apple's `va_arg` was written out as loads in `driver.rs`; MIR now has the `va_arg` opcode and the DAG selects `VAARG` (#7265, 11-05); no text rewrite is left in `driver.rs`.
3. `prepass.rs`: CSE and loop hoisting of constants and addresses, a small version of `MachineCSE`/`MachineLICM`.
4. `driver.rs`: newer clang's `ptrtoaddr` is read as `ptrtoint`; MIR refuses it by design.
5. (2026-11-08) `lir_body`: a load or store's `MemOperand`, a symbol's name and a debug note's variable ride in side tables keyed by instruction number (`Lowered`), not on the `Insn`. A stage that clones, inserts or reorders instructions loses them (a cloned load has no memory operand and is refused). Owner llrm-arm64-lir; exit: gap 3, the memory operand on the `Insn` as LLVM keeps it, symbols in the body, debug notes in `LirBody::notes`.
6. (2026-11-08) `lir_body`: a physical register is a `Loc::Reg`, a virtual one a `Loc::Held` whose class is the name of its `COPY` (`COPY_FPR`); `LirBody::class_like` does not carry it yet. Effects are `Insn::declared` in the allocator view's model (one status-flag lane, one memory lane, MS6), not rows of an `arm64.instr`. Owner llrm-arm64-lir; exit: gap 2 (`RegisterClasses`, flag model).
7. MS6 (2026-11-08) `lir_body` `declared`: memory is one lane (a load reads it, a store writes it), so no access moves past a store; the pin asks alias analysis. NZCV as one flag lane is the pin's (one physical register), not a deviation. Owner llrm-arm64-lir; exit: gap 3 (2026-11-17, gap 3 follows gap 2; coordinator-dated), alias queries on the memory operand replace the lane.
8. (2026-11-09) `flow::machine_with` asks a profile for clocks by x86 form names (`cost("movzx")`, `cost("alu_ri")` for MaskByAnd, `mov_rm` for FixupBW). arm64 states a `cpu_table` with issue width and order from the pin (A510, Cyclone) and no clocks, so MaskByAnd is not made and FixupBW does not widen (an unstated clock is not a free one). Owner llrm-arm64-lir; exit 2026-11-16: the target is asked through a neutral query instead of form names.
9. ARM64-PHI-AT-ISEL (2026-11-09, oversight ruling) both selectors lower phis to edge copies while selecting, so the MFunc is not SSA and 13 shared stages (EarlyTailDuplicate..LiveRangeShrink) cannot run. Owner llrm-arm64-lir, implementation a second arm64 session; design `arm64-phi-at-isel.md`; exit 2026-11-15 if the arm64-phi session starts by 2026-11-10 06:00Z, else 2026-11-17: the selectors emit `Op::Phi`, `lower_phis` keeps the bytes, the SSA stages run. Status 11-09 (arm64-phi): both selectors emit `Op::Phi`, `mfunc::lower_phis` (DEVIATE ARM64-PHI-SHAPE, exit 11-17: every critical edge split, the pin splits by liveness) runs right after selection; the stage harness runs all phases from the selector's body. Writeback tie still open.
10. X86-LANES (2026-11-09) `machinecp.rs:450` asserts 72 byte-lane units (x86 roots); CopyPropagation panics on 43 of 43 functions, Peephole on 18. Owner llrm-arm64-lir; exit 2026-11-12: units from the register file.
11. X86-VERIFY-FPR (2026-11-09) `verify.rs:493` `described()` knows integer, segment and XMM only, so arm64's `d`/`s`/`h` views are `not a register this target has` (6 float functions); RegAlloc gives a general register to a 16-byte value (3 functions). Owner llrm-arm64-lir; exit 2026-11-12.
12. TWOADDR-TIED (2026-11-09) `twoaddr.rs:609` `ties(what)` ties every `Binary`/`Move`-class operation (x86 two-address), so TwoAddress turns `SUBSWri d, s, #2` into `mov d, s; SUBSWri d, d, #2` on 40 of 43 functions. Owner llrm-arm64-lir; exit 2026-11-13: a target query for tied forms (arm64: MOVK, BFM).

## Numbered gaps

- ARM64-EIC-1: no `MachineEarlyIfConversion`/`MachineTraceMetrics` (pin: `aarch64-enable-early-ifcvt` on). `div_constants_long`: pin IR has 28 selects, the asm 121 `csel`/`cset`/`cinc`-family instructions, ours 35 and 35; the extra ones are made by the machine pass from `br` chains the pin IR still has. Programs with more in the pin than ours: 9 of 159 built (7 FAIL rows excluded); `div_constants_{long,short,ulong}` hold 284 of the 298 pin instructions, the other six 14. Exit: step 6 (~3,000 pin lines, not worth writing twice).
- ARM64-REALIGN (owner arm64-phi, exit 2026-11-30, parity-only, after the card queue): stack realignment with an `x19` base pointer (pin: `AArch64FrameLowering`, `AArch64RegisterInfo::hasBasePointer`). Step 0: 0 of 924 compiled cells refuse (1060 cells = 265 programs x O1/O2/O3/Os; 136 fail in the clang parser on both sides). Levels other than these four, Darwin and other directories are not counted.
- ARM64-DBGVALUE-NOTES (owner arm64-phi, exit 2026-11-24): `Op::DbgValue` into `LirBody::notes`. arm64's `DBG_VALUE` names a register, a note names an SSA value, so the producer must emit notes after phi elimination. `Lowered.debug_values` stays until then.
- ARM64-SCHED-HOIST (2026-11-10, owner arm64-phi until reassigned, exit 2026-11-26): the pre-RA scheduler leaves a load after independent stores that the pin hoists. Case: six stores to one `alloca`, a load of another, a `sdiv` chain on it, -O2. Since #8719 the load has no edge from the stores (graph: preds `[2, 4, 6, 9, 10, 11]` before, `[]` after) and our `bottom_up` still keeps the source order; pin `llc -O2` hoists `ldr` and both `sdiv` above the stores, and `-enable-misched=false` does not. Cause not examined: the pin's candidate choice (`GenericScheduler::tryCandidate`) against `machinesched::bottom_up`. 60 generated store/load shapes at -O2: output equal with and without the edge, so #8719 changes the graph, not yet an object.

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
