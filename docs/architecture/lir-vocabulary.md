---
architecture-index: LIR vocabulary, one and target-generic
architecture-order: 540
---
# LIR vocabulary: one, target-generic

Agreed by backend-two, arm64-spike and lir-cleanup-port. It is the shape llrm-codegen's machine
layer reads, so that no pass names an x86 mnemonic, register or flag bit (the target-facts
audit, #2862).

## Decisions

1. **`RegId`** is a one-byte id numbered by the target. `iced_x86::Register` appears only inside
   `RegId::new` and `RegId::iced()` (arm64-spike's change, on `feat/arm64-spike`).
2. **`Flag`** is a set per target. The type holds no iced bit values.
3. **Opcode** is the target's id, one per row of its description, with name, operand shape and
   flags. `Operation` stays as a coarse class (move, jump, branch, call, return, …) that passes
   may test; a pass never matches a mnemonic string. Two names for one row would drift, so the
   id is the only key.
4. **Operands.** An `Insn` carries `operands: SmallVec<[Operand; 4]>`. An `Operand` is a place
   (`Loc`) with an access (`Use`, `Def`, `UseDef`, `CondDef`) and the lanes it touches, or an
   immediate, shift or condition field. Implicit operands (flags, sp, string-op pointers, the
   `rep` count as `CondDef`) are listed with their register. Filled at isel from the
   description's rows, as `MachineInstr` operands from `MCInstrDesc`.
5. **Calls and returns** keep `requires`, `delivers` and `clobbers` as typed implicit operands
   built from the call contract; the clobber set is `RegisterInfo::call_preserved`.
6. **`Effect`** (lane sets) is a lazily built view of the operands, cached on the `Insn`.
   No row lookup, no string match.
7. **Fixed-register constraints** stay `requires`/`delivers` until the allocator reads operand
   constraints.
8. **Encoding** keeps only bytes: `arrival` and `cfi` decode emitted code through the target's
   `Encoding` (decode, flow, stack effect, flags). iced stays inside it.

Hooks beside the vocabulary, named for LLVM's: `TargetRegisterInfo` (classes, reserved,
`call_preserved`), `TargetInstrInfo` (`copy_phys`, `store_slot`, `load_slot`, `is_copy`,
`remat_constant`, tied operand), `TargetFrameLowering` (callee saves, prologue, epilogue,
`has_fp`, save/restore cost).

## What it buys

Parity, not speed. d_faces -O2 m32 i486, callgrind at c9a3d3291: the peephole group is 92 M of a
1885 M compile. Decoding an instruction's effects is 12.8 M inclusive; the part operands remove
(the row lookup and string match) is about 7 M, 0.4% of the compile. The rest is pass structure:
liveness solves (~25 M across machinecp, spillforward, postrasink) and per-block rebuilds
(~10 M). Operands do not touch those; one shared liveness and no per-pass clone do.

## Order

Migration follows the post-RA verifier plan (#2933): readers of physical registers first, then
virtual defines and uses. Each step lands byte-identical at m16/m32 with no compile-time
regression (method M, `instructions:u`).
