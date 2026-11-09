# MS6 gap 3: the memory operand on the `Insn`

Status 2026-11-09, owner arm64-phi. Closes spike rows 5 and 7 (DEVIATE M7).

## Today

- `machinesched` keeps one chain: a store orders every later access, a load every later store. No
  alias analysis. `machinesink::may_store` (DEVIATE M7) counts any store; `machinelicm` reuses it.
- arm64 states the memory lane only. Its `MemOperand` (object, offset, volatile) sits in
  `Lowered.memory_operands`, keyed by `at`; a stage that clones or inserts an instruction loses it.
- arm64 runs the shared phases only under `LLRM_ARM64_STAGES` (`lir_body::stages::run`, observation);
  its output does not change with this work.

## Pin (d1106deb71cc)

`ScheduleDAGInstrs.cpp#L558` (chain edge) and `MachineSink.cpp#L1676`/`#L1736` (`hasStoreBetween`)
ask `MachineInstr::mayAlias` (`MachineInstr.cpp#L1558`): a call may alias; two non-stores do not;
both must be memory operations; no memory operand means may alias; each (store, other) pair goes to
`MemOperandsHaveAlias` (`#L1473`): same value, then overlap by offset and width; else alias analysis.
`MachineLICM.cpp` does not ask it (any store in the loop); that stays.

## Change

1. `Insn.memory: Option<Arc<MemRef>>` (`object: Option<Object<u32>>`, `offset`, `bytes`), default
   `None`. Symbols by index into `LirBody.symbols`. No shared pass builds one.
2. `Insn::may_alias(&self, other)` translates `mayAlias` above, in its order, over one memory
   operand per instruction (the pin's list of up to `getMemOperandAACheckLimit` = 16 pairs is one
   pair here; DEVIATE MS6-ONE-MMO until a target needs two). Without a `MemRef` on both, the answer
   is the old one (true), so x86 is unchanged.
3. `machinesched`: with `MemRef`s, earlier stores and loads are kept as lists and an edge is made
   only where `may_alias`; without, the single chain as today. `machinesink::hasStoreBetween` asks
   `may_alias` for the load it sinks.
4. `LirBody.symbols` takes `Lowered.symbols`.
5. arm64 `to_lir` sets `memory` on a load or store; `from_lir` reads it back. `Lowered.memory_operands`
   goes. Pairs, atomics and calls state none (they may alias anything).

Not in this change: `Op::DbgValue` into `LirBody::notes`. A note names an SSA value; arm64's names a
register, so it needs the producer to emit notes after phi elimination. `Lowered.debug_values` stays.

## Acceptance

- Fail-first: arm64 `to_lir` of a store to one slot and a load of another gives a scheduler graph with
  no edge between them (red with the alias path off); the same slot stays ordered. The sink leaves a
  load past a store to another object, not past one to its own.
- A copy made by `Insn::with_what` keeps the operand.
- x86 identity (named m16/m32/m64 programs, md5s); structural statement for m68k, wasm, z80, 6502.
- arm64 and x86 A/B: identical objects expected (no default path asks `may_alias` of an access
  with an operand; x86 states none); measured.
