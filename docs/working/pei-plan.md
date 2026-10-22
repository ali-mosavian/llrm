# Prologue/Epilogue Insertion and Frame Finalization (row 80): plan

Owner: w-6. Pin: PEI [PrologEpilogInserter.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp) and
[X86FrameLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp) at d1106deb71cc.
Ours stands in: `prologue.rs` (frame growth, `sub sp,N` and its release),
`masm.rs` (`parts`: enter/leave, saved registers), `frame.rs` (slot layout).

## The pin's order (`PEIImpl::run`, [L211](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L211))

1. `spillFPBP`; `calculateCallFrameInfo` ([L372](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L372)): max call frame size.
2. `calculateSaveRestoreBlocks` ([L413](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L413)): where the saves and restores go (shrink-wrap points).
3. `spillCalleeSavedRegs` ([L654](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L654)): `assignCalleeSavedSpillSlots` ([X86 L3092](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L3092)), `insertCSRSaves`/`Restores`.
4. `processFunctionBeforeFrameFinalized` ([X86 L4454](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L4454)).
5. `calculateFrameObjectOffsets` ([L856](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L856)): slot offsets, alignment, free-slot reuse.
6. `insertPrologEpilogCode` ([L1167](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L1167)): `emitPrologue` ([X86 L1677](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L1677)), `emitEpilogue` ([X86 L2580](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L2580)).
7. `replaceFrameIndices` ([L1376](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L1376)); the stack-size warning.

## Ours against it

| Pin step | Ours | State |
|---|---|---|
| 1 | `Frame` (max outgoing arguments) | to read |
| 2 | `shrinkwrap.rs` (`Piece`) | to read |
| 3 | `masm::saved_of`, `parts` | different shape |
| 5 | `frame.rs` layout | to read |
| 6 | `masm::parts`, `prologue.rs` | different shape |
| 7 | `masm` addresses frame cells by BP/SP | different shape |

## Translation order

1. PEI-1: `calculateFrameObjectOffsets` as a function of the frame's objects (i486 flat model), compared with `frame.rs` on the corpus.
2. PEI-2: `emitPrologue`/`emitEpilogue` i486 path: `push ebp; mov ebp,esp; sub esp,N` / `leave` or `mov esp,ebp; pop ebp` / `add esp,N`.
3. PEI-3: CSR spills and their save/restore blocks.

Each lands when it builds and passes the gate; the first is a numbered DEVIATE
until its comparison runs. Owner w-6, plan 10-22, PEI-1 11-06.

## FI-1: frame indices in `frame.rs` (owner w-6, review 2026-11-20)

Slot table row 80 is DEVIATE CG7. Today `Frame::slot` returns a displacement
the spiller writes into the instruction. The pin writes a frame index and
`replaceFrameIndices` fills the offset once `calculateFrameObjectOffsets` has
run. Exit: spill cells carry an index and one pass resolves them with
`frameobjects::calculate_frame_object_offsets`; then `emitPrologue` and
`emitEpilogue` (PEI-2) replace `prologue.rs` and `masm::parts`.
