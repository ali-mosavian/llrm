# Switching over to the translated greedy allocator: what is left of the old one (2026-10-22)

TL;DR: greedy is the only allocator that runs (0 of 9385 allocations take the old one, w-6). What it still calls out of the old modules is small and mostly instruction constructors; the bulk (about 14.5 K lines in five modules plus `rewritten`/`greedy_rewritten`'s rounds in allocate.rs) is dead on the corpus and deletable once seven functions and eight helpers have a home.

Source: grep of calls from outside each module (non-test), crates/backend/llrm-core at 2642d0fb15. Sizes: spiller.rs 7739 lines, splitkit.rs (backend) 2842, ssaspill.rs 2613, coalesce.rs 939, spillforward.rs 354, allocate.rs 6429.

## What the greedy path still calls

| Old item | Caller (greedy side) | What it does | The pin's counterpart | Action |
|---|---|---|---|---|
| `spiller::_store`, `_reload` | greedy/inlinespiller.rs (insert spill store / reload) | builds the store/load of a vreg to/from a frame cell | `TargetInstrInfo::storeRegToStackSlot` / `loadRegFromStackSlot`, called by `InlineSpiller::insertSpill` / `insertReload` | move into a `tii`-style module in greedy (x86 target hooks), keep behaviour |
| `spiller::_in_place` | inlinespiller.rs (fold a read of the spilled vreg to a memory operand) | folds the frame cell into the instruction | `InlineSpiller::foldMemoryOperand` -> `TII::foldMemoryOperand` (greedy/tii.rs already holds the encodability test) | merge with tii.rs |
| `spiller::_encodable` | greedy/tii.rs | can this instruction take a memory operand | `X86InstrInfo::foldMemoryOperandImpl` tables | already tii; move the function there |
| `spiller::remade_values` | greedy/shadow.rs | values rematerializable by one instruction | `TargetInstrInfo::isReMaterializable` + `isTriviallyReMaterializable`, used by `LiveRangeEdit::allUsesAvailableAt` | move; the pin's check is per def, not a precomputed set |
| `spiller::PLANS` | tests only | counter | none | delete with the module |
| `coalesce::_copy` | greedy.rs, shadow.rs, inlinespiller.rs (x5), calcspillweights.rs, phielimination.rs | reads (dst, src) of a plain move | `TargetInstrInfo::isCopyInstr` | the one helper everything shares: relocate to model/lir as `Insn::copy_of`; already the pair.rs `full_copy` is its twin |
| `splitkit::_next_value` (backend) | allocate.rs (floor), phielimination.rs, phiconst.rs, twoaddr/ssarepair via spiller | the next free value number | `MachineRegisterInfo::createVirtualRegister` | LiveIntervals::create_virtual_register exists; replace the floor |

## What only the old/rounds path uses (deletable after the above)

| Item | Used by | Replaced by |
|---|---|---|
| `spiller::spilled_from` (+ `folded_pair`, `folded_read_scans`, `_plain_move`) | allocate.rs rounds loop only | InlineSpiller (inline path) |
| `backend/splitkit.rs` (`Split`, `Occupied`, `Region`, `live_blocks`, `carved_moving`) | allocate.rs `rewritten` (old allocator), ssaspill | greedy/splitkit.rs (translated) |
| `ssaspill.rs` (`floating`, `untouchable`, `_tail`...) | flow.rs (`untouchable`), phielim.rs/phiconst.rs (`floating`), allocate.rs | two small predicates to relocate; the rest dead |
| `spillforward.rs` (`forwarded`) | peephole.rs | not part of the allocator: Peephole's forwarding; keep or relocate, it is not step 3 |
| `coalesce.rs` Graph/interference (`_interference*`, `Graph`) | twoaddr.rs, spiller.rs | twoaddr.rs still uses `Graph`/`_interference`/`_renamed` (w-1's TwoAddress translation removes them) |
| `allocate.rs::rewritten` and `greedy_rewritten`'s `ROUNDS` loop | the 7 rounds functions; `rewritten` 0 allocations | the ladder (below) |

## Rounds loop (separate row)

7 functions, all m16 QCport: ls.c `_ls_animate@3` (-Og..-Os), sc.c `_sc_bsplit@3` (-O2/-O3). One cause (w-6): two weight-inf intervals with a single eligible register. The pin does not fail there; its ladder (tryEvict, tryLastChanceRecoloring, tryRegionSplit / tryLocalSplit / tryInstructionSplit, spill) gives a reload per use. Code is the ladder, not a DEVIATE, unless it is shown the pin's ladder cannot express the m16 address-class constraint. Checked (`_ls_animate@3` -O2, LLRM_SHADOW_DEBUG): the failure is in the INLINE path, which then falls to the rounds. The failing `%53 [596r,604r)` and its interferer `%51 [588r,604r)` are both weight-inf, each may take EBX only, and both are read by one instruction, `[abs+516 + %53 + v16*1] := mov %51` (a base, an index and the stored word). %53 is the base (EBX is right); %51 is plain data and could take any general register, but it is confined to EBX. Cause: `InlineSpiller` gives a new value "the class of the value it stands for" (inlinespiller.rs, `tables.orders[reg]`), i.e. the parent's whole order. The pin does the same at creation (`Edit->createFrom(Reg)`, with the FIXME "infer regclass from instruction alone"), but then `LiveRangeEdit::calculateRegClassAndHint` runs `MRI.recomputeRegClass(NewVReg)` on every new value (LiveRangeEdit.cpp#L426), which widens it to the largest class its instructions accept. That call is the missing piece; it is not the ladder, and the ladder cannot place two values confined to the same single register. The m16 base/index two-colouring (regclass.rs) has no pin function: it is a target fact, a numbered DEVIATE for the pairing; `recomputeRegClass` for the spill- and split-created values is a translation.

## Plan of record (each its own PR, fail-first test, rows pricing; switch-over first, tuning after)

1. 10-26. Relocate the helpers with their callers; behaviour identical (objsweep identical). Done: `_copy`, `_next_value`, `ssaspill::floating/untouchable` (#3617); `_encodable` with the spill emission (#3626). Left: `remade_values`, which is a translation, not a move: the pin asks per def (`isTriviallyReMaterializable`, `allUsesAvailableAt`) where it reads a set precomputed by `_literals_by`/`_addresses_by`/`_stable_loads_by`; dated 10-30 with the spill emission.
2. 10-28. Call `recompute_class` (w-1, #3625) at the creation points. `recompute_class` (the pin's `calculateRegClassAndHint` -> `recomputeRegClass`, InlineSpiller.cpp#L1383, LiveRangeEdit.cpp#L426, SplitKit.cpp#L628) for spill- and split-created values, over `regclass::collected`, respecting the m16 base/index two-colouring; test: the 7 functions allocate in one greedy pass, no rounds, and a unit test of the recompute on the stored-word shape.
3. Spill emission into greedy/tii (`_store`, `_reload`, `_in_place`): done early, #3626, objects identical. `remade_values` as `isTriviallyReMaterializable` per definition: #3640, objects identical. Then a check that `slots.rs` (StackSlotColoring, executed and matched, codegen row 70) handles the spill slots of the new emission: a function with split and spill slots shares them as before, objects identical (10-30).
4. 10-31. Ladder for what is left of the rounds. After #3668 nothing is: QCport 260 compiles and vsgcc 71 x 4 levels x m32/m16 leave no function for the rounds (`LLRM_GREEDY_TRACE`; the same count was 6 on ls.c/sc.c before). Left to run: the tests/run corpus, then the ladder is not needed and the item becomes the deletion's guard test.
5. 10-30..11-01. Rows owned by w-1 that the deletion needs first: PHIElimination (row 61) and ProcessImplicitDefs (row 59), dates from w-1 by 10-27. Row 69, VirtRegRewriter (assign the placed registers, kill/undef flags, drop identity copies): the application of `Assignment` after `greedy_rewritten` is not spill emission; own line, read against `VirtRegRewriter.cpp` and priced 11-01.
6. 11-03. Delete `rewritten`, the rounds loop, `spilled_from`, backend/splitkit.rs, backend/ssaspill.rs remainder, spiller.rs, coalesce.rs remainder (the last two after w-1's TwoAddress translation), in one PR with `Deletes:`.

Not measured: line counts of what is dead inside spiller.rs (its tests are most of the 7.7 K); the call-site count of `Insn::copy_of`-style relocation beyond the grep above.

Owner: splitkit-port. TwoAddress (coalesce.rs Graph, `_interference`, `_renamed`) is w-1's translation. C4 (`removeCopyByCommutingDef`) 10-27 is separate.
