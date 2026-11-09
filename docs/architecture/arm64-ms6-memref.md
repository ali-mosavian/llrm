# MS6 gap 3: the memory operand on the `Insn`

Status 2026-11-09, owner arm64-phi. Closes spike rows 5 and 7.

## Today

- Shared passes see memory as the lane `Lanes::MEMORY` (`Insn.declared`) and as `Loc::Mem` cells.
  x86 cells carry an `addr`; the scheduler, sink and LICM compare them by place.
- arm64 states the lane only: a load reads it, a store writes it, so no access passes a store.
  Its `MemOperand` (object, offset, volatile) sits in `Lowered.memory_operands`, keyed by `at`.
  A stage that clones or inserts an instruction loses it.

## Pin

`MachineInstr` keeps `MachineMemOperand`s; `ScheduleDAGInstrs::addChainDependencies` and
`MachineSink`/`MachineLICM` ask `MachineInstr::mayAlias` (`AliasAnalysis`, with the operand's
object, offset and size). No lane.

## Change

1. `Insn.memory: Option<Arc<MemRef>>` in the shared model: `object: Option<Object<u32>>`
   (`llrm_analysis::placealias::Object`, symbol by index into `LirBody.symbols`), `offset`,
   `bytes`. Target-neutral; no operand is built by a shared pass.
2. `Insn::may_alias(&self, other) -> bool`: `placealias::alias_of` when both have a `MemRef`,
   else the lane answer (true). Machinesched's store/load chain, `machinesink::may_store`
   and MachineLICM's store check ask it. x86 never sets `memory`, so its answers are unchanged.
3. `LirBody.symbols`: the names an address names move from `Lowered.symbols` into the body.
4. Debug notes: `Op::DbgValue` becomes `LirBody::notes` (`DebugNote`), as x86 `-g` does.
5. arm64 `to_lir` sets `memory` from `MemOperand`; `from_lir` reads it back from the `Insn`.
   `Lowered.memory_operands` and `debug_values` go.

## Acceptance

- Fail-first: a load after a store to a different frame slot / global is scheduled past it
  (red on the parent: lane orders them); same object, overlapping bytes stays ordered.
- A cloned load keeps its operand through a shared stage.
- x86 identity (m16/m32/m64 named programs, md5s, one executed); structural statement for
  m68k, wasm, z80, 6502 (they set no `memory`).
- arm64 A/B O1/O2/Os with moved cells listed, bytes and executed instructions.
- DEVIATE rows: none expected; row 7 closes.
