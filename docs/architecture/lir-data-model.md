---
architecture-index: LIR data model, mutable instructions with register use lists
architecture-order: 550
---
# LIR data model: mutable instructions with register use lists

TL;DR: LIR instructions are immutable `Arc<Insn>`; a rename rebuilds the instruction (746 renames, 3.9 M instructions on d_faces in SplitKit alone). LLVM mutates a `MachineOperand` in place (`setReg`) and keeps use/def lists in `MachineRegisterInfo`. This note proposes block-granular copy-on-write with `&mut Insn` first, the arena and intrusive lists second, in seven slices, each landable and measured.

Status: design, owner uselist-port. Reference: LLVM at `d1106deb71cc` (`MachineInstr.h`, `MachineOperand.h`, `MachineRegisterInfo.h`, `MachineRegisterInfo.cpp::addRegOperandToUseList`, `MachineOperand::setReg`).

## Today

- `Insn` (`model/lir.rs:113`) holds its operands in five places: `defines`, `uses`, `requires`/`delivers` (with a physical register), `widths`, and the `Loc`s of `what: Semantics`. Beside them sit memoized facts (`effect`, `register_effects`, `ident`) that a clone drops.
- A block is `Insns(Arc<[Arc<Insn>]>)`; bodies share blocks and instructions by `Arc::ptr_eq` (`same_as`, 64 `ptr_eq` sites) so facts keyed by identity survive a copy.
- `Insn::try_renamed` builds a new instruction and keeps the old allocation when nothing moves. `LiveIntervals` keeps its own operand lists per register (`add_operands`/`remove_operands`) and `rewrite_instr(old, new)` swaps them, re-keying `insns` by `Insn::id`.
- Census (`crates/backend`): `Arc<Insn>` in 95 files / 857 hits; `Insn {` constructors 226 hits in 52 files; `with_insns` 118 hits in 49 files; `.what` 968 hits in 112 files; `.uses` 621; `.defines` 403; `.ident`/`id()` 275. Heaviest readers: `spiller.rs` (243), `peephole.rs` (122), `floatassign.rs` (91), `allocate.rs` (86), `constrain.rs` (59), `ssaspill.rs` (57), `phielim.rs` (48), `fpstack.rs` (48), `parcopy.rs` (47), `splitkit.rs` (43). Rename callers: `inlinespiller`, `spliteditor`, `spill`, `register_coalescer`, `coalesce`, `precse`, `phielim`, `spiller`, `liveintervals`.

## LLVM

An operand is a mutable object inside its instruction. `setReg` unlinks it from the old register's use/def list and links it to the new one in O(1); the list heads live in `MachineRegisterInfo`. Nothing is rebuilt, nothing re-keyed; an instruction keeps its identity (its pointer) for life. `replaceRegWith` walks the list.

## Options

| | change | cost per rename | risk |
|---|---|---|---|
| A | operands in one `Arc<Operands>` inside `Insn`; a rename clones only that | one small allocation | low; keeps `Arc<Insn>` and identity, still re-keys |
| B | a thawed block owns `Vec<Insn>`; passes edit `&mut Insn`; `Arc` only between passes (copy-on-write per block) | none inside a pass; one block copy per touched block per pass | medium; replaces `Arc<Insn>` handles by `InstrKey` lookups |
| C | B plus an arena of operand nodes with prev/next and per-register heads (the MIR `UseLists` design) | O(1) | high; every reader of `uses`/`defines` goes through the operand list |

Recommendation: B, then C where a pass needs the use list (SplitKit, coalescer, inline spiller). A is not worth landing: it keeps the identity problem.

## DEVIATEs from the pin

1. Block-level `Arc` persistence between passes. The pin invalidates analyses by pass preservation, not by block identity; here facts are cached by block identity. Exit: facts keyed on a pass-local version, then `Arc` goes. Owner: uselist-port.
2. `Insn::id` is the instruction's stable arena slot; the pin's `MachineInstr` identity is its address or iterator, the same role.
3. One ordered operand list replaces the five operand fields, as `MachineInstr` has its `MachineOperand` list; `Semantics` refers to operands by index.

(Confirmed by the coordinator as defaults; oversight may overrule.)

## Decisions to confirm

1. Persistence between passes stays (block-level `Arc`, `same_as`) because facts are cached by block identity; mutation happens only inside a thawed block. If the facts caches can key on a pass-local version number instead, `Arc` goes away entirely.
2. `Insn::id` stops being derived-on-first-ask and becomes the instruction's arena slot (stable under `setReg`).
3. The five operand fields collapse into one ordered operand list with a kind (def, use, implicit, tied, physical-constraint, memory-address part), as `MachineInstr` has; `Semantics` keeps only what is not an operand (opcode, flags, addressing shape) and refers to operands by index.

## Slices

0. Measure: counters for instructions per `try_renamed` / `rewrite_instr` and clone cost; d_faces total compile for the 3.9 M denominator (a number to disprove, not assume). Test: the counters are asserted on a fixture.
1. `InstrKey` as the one handle: `LiveIntervals.insns` and the SlotIndexes map keyed by it; `Insn::id` unchanged. No behaviour change.
2. `Insn` operands into one ordered list behind accessor methods (`uses()`, `defs()`, `set_reg`); old fields become methods; census sites migrate mechanically, spiller and peephole last.
3. Thawed blocks: `LirBody::block_mut` copies a block once (`Arc::make_mut`), `Insn` stored by value; `Insns` keeps `same_as` for frozen blocks.
4. `setReg` in place for SplitKit, `register_coalescer`, `inlinespiller`; delete `try_renamed` callers there. Measured: the 746 renames.
5. Register use lists (the C design) for `LiveIntervals` and `MachineRegisterInfo`-style queries (`use_instructions(reg)`, `replace_reg_with`); delete the per-register operand tables.
6. Remaining passes (`phielim`, `precse`, `coalesce`, `spiller`), then delete `try_renamed`.

## Cost and expectation

The saving is the clone and re-key work per rename plus the lost memoized `effect`, 3.9 M instructions of the SplitKit renames on d_faces by the coordinator's count; slice 0 confirms or corrects it before slice 4 is sold on it. The mechanical migration (slice 2) is the bulk of the work (about 1,000 reader sites) and is behaviour-neutral; each slice carries the identity check (`tools/identity.sh`) and the LIR-pass tests.

## Not in scope

Changing what an operand means (register classes, tied operands as LLVM models them) beyond what slice 2 needs to carry today's fields.
