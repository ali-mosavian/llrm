---
architecture-index: emission against AsmPrinter and MC
architecture-order: 510
---
# Emission against AsmPrinter and MC

TL;DR: llrm emits no code alignment at all and gives a global no alignment its type would ask for; those are the first two emission gaps. Owner backend-two (plan slot 64, split below). LLVM at `d1106deb71cc`.

Surveyed by reading `masm::Item` (no align item), `driver/data.rs`, `constpool.rs`, `objbuild.rs` and `cfi.rs` against `AsmPrinter.cpp`, `DataLayout.cpp`, `X86AsmBackend.cpp`.

| Row | LLVM | llrm today | State |
|---|---|---|---|
| E1 | `AsmPrinter::getGVAlignment`, `DataLayout::getPreferredAlign(GV)`: the type's preferred alignment, 16 for an initialized global over 128 bits, an explicit one honoured | `driver/data.rs` aligns only a global that states `align`; the rest are byte aligned | missing; first translation |
| E2 | `emitFunctionHeader` (function alignment, `.prefalign`), `MachineBlockPlacement::alignBlocks` (loop alignment), `MCAlignFragment` with `X86AsmBackend::writeNopData` (multi-byte nops) | no code alignment: functions and loops are byte aligned, no nop fill | missing |
| E3 | `emitConstantPool`: each entry at its alignment, entries sorted by it, mergeable sections by size | `constpool.rs`: one datum per distinct bytes, no alignment | partial |
| E4 | `emitJumpTableInfo`: table at the entry alignment | `masm::jump_tables`; alignment unchecked | to check |
| E5 | `MCAssembler::relaxOnce`: fixed point over fragments, align fragments included | `objbuild::_relaxed` (jwasm's fixed point; `_trampolined`, Watcom's, at -Os) | old; equal core; redo once E2 makes sizes offset dependent |
| E6 | CFI directives from `X86FrameLowering::emitCalleeSavedFrameMoves` | `cfi.rs` decodes the rows back from the bytes | DEVIATE by design: same rows, other route |
| E7 | `TargetLoweringObjectFile::SectionForGlobal`: rodata, data, bss, mergeable strings by kind | segments named by the frontend | old |
| E8 | object writers, `DwarfDebug` | `llrm-elf`, `llrm-coff`, `llrm-omf`, `debuginfo.rs` | format conformance, not code quality; not translated |

Encodings are iced's shortest form, as the MC emitter's, so no row.
