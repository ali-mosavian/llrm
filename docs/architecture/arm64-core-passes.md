---
architecture-index: arm64 against the core machine passes, an inventory
architecture-order: 7
---
# arm64 against the core machine passes

TL;DR: the allocator core (`greedy/`, `liveness/`, `coalescer/`) names no x86 fact in 28 of 32 files; the x86 coupling is in `backend/` (52 of 100 files) and sits at the places LLVM puts target hooks. Running the passes on arm64 waits for those hooks; this page is the inventory the hooks are measured against.

Status: static inventory at the spike's head, `crates/codegen/llrm-codegen/src`, non-test code. A "fact" is `llrm_x86::`, an iced type, an x86 register name (`RegId::AX`...), or an x86 mnemonic string. The arm64 selector's output does not run through them yet (see [the spike](arm64-spike.md) for why it has its own backend).

| directory | files | lines | files naming an x86 fact |
|---|---|---|---|
| `greedy/` | 15 | 7,738 | 2 (`inlinespiller`, one register and two mnemonics) |
| `liveness/` | 11 | 4,827 | 0 |
| `coalescer/` | 6 | 1,247 | 1 |
| `backend/` | 100 | 35,532 | 52 |
| of which `backend/isel`, `jumps`, `peep` | 11 | 4,779 | 5 |

Across `backend/` and `greedy/`: 296 mnemonic strings, 29 `llrm_x86` uses, 26 iced uses, 26 register names. The mnemonics are concentrated: `isel/wide.rs` 84, `division.rs` 31, `masm.rs` 23, `schedule.rs` 16, `arithmetic.rs` 15, `rmw.rs` 14.

## What each x86 fact stands for in LLVM

| here | LLVM's hook | arm64 needs |
|---|---|---|
| `"mov"` as the copy, `"push"`/`"pop"` as spill code, `parcopy`, `phielim`, `ssaspill`, `spiller` | `TargetInstrInfo::copyPhysReg`, `storeRegToStackSlot`, `loadRegFromStackSlot`, `isCopyInstr` | `InstrInfo` (backend-two, M5/M6): LIR in, LIR out, width, slot |
| `prologue.rs`, `frame.rs`, `masm.rs`'s frame parts, `framefree` | `TargetFrameLowering` | callee-save pairs, `fp`/`lr`, a frame size policy (`crate::frame` here) |
| `iced` in `arrival`, `valuetrack`, `cfi`, `peephole`/`guards`, `overlap`, `storecombine` | `MCCodeEmitter`/`MCDisassembler` queries | the `Encoding` trait (backend-two); `crate::mc` is the arm64 side |
| `RegId::AX` and kin (26) | `TargetRegisterInfo` roles | a `reserved` role in `registers.regs` |
| `twoaddr`, `rmw`, `stepflags`, flags in `peephole` | `TwoAddressInstructionPass`, `MachineInstr` flag operands | arm64 is three-address, NZCV flags: the passes ask the target |
| `x86` isel (`isel/*`, `division`, `arithmetic`, `schedule`) | `AArch64ISelLowering`, `AArch64InstrInfo` | the selector (`crate::isel` here) |

## What this spike did instead, and what it cost

The spike's own selector, allocator (ranges with holes, coalescing; DEVIATE 1), CSE/hoisting (DEVIATE 3) and pairing run on arm64 instructions in `llrm-arm64`. Against `llc -O2` on the same IR it is 1.07x the code and 1.14x the instructions executed (geomeans, `tools/targets/arm64_compare.py backend`). The gap is the passes this page lists: machine CSE/LICM over a real loop nest, post-RA copy propagation, `ldp`/`stp` formation with LLVM's window, post-index addressing (LSR plus the load/store optimizer).

## The experiment, once the hooks exist

1. Emit the selector's output as `LirBody` (a converter beside `crate::mfunc`).
2. Run `flow::machine_with`'s phases one at a time on it, naming the first fact each stops at.
3. Replace the stops with target calls, in the order above.

Step 1 needs `Operation`/`Semantics` names for arm64's instructions (`LirBody` carries x86-flavoured operations: `Push`, `Leave`, `Fill`, x87 loads) and the effect rows (`Info.effects`) from an `arm64.instr`.
