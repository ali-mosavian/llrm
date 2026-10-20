---
architecture-index: emission against AsmPrinter and MC
architecture-order: 510
---
# Emission against AsmPrinter and MC

TL;DR: global and function alignment are translated (E1, E2); constant pools and jump tables are aligned as LLVM does; loop alignment and relaxation remain. Owner backend-two (plan slot 64, split below). LLVM at `d1106deb71cc`.

Surveyed by reading `masm::Item` (no align item), `driver/data.rs`, `constpool.rs`, `objbuild.rs` and `cfi.rs` against `AsmPrinter.cpp`, `DataLayout.cpp`, `X86AsmBackend.cpp`.

| Row | LLVM | llrm today | State |
|---|---|---|---|
| E1 | `AsmPrinter::getGVAlignment`, `DataLayout::getPreferredAlign(GV)`: the type's preferred alignment, 16 for an initialized global over 128 bits, an explicit one honoured | translated: `driver/data.rs` aligns each global as `getPreferredAlign` does | translated ([#2407](https://github.com/ali-mosavian/llrm/pull/2407)) |
| E2 | `emitFunctionHeader` (function alignment, `.prefalign`), `MachineBlockPlacement::alignBlocks` (loop alignment), `MCAlignFragment` with `X86AsmBackend::writeNopData` (multi-byte nops) | functions aligned with multi-byte nops; loops and blocks byte aligned | functions translated, default on, m16 off by DEVIATE E2-1 ([#2443](https://github.com/ali-mosavian/llrm/pull/2443), [#2479](https://github.com/ali-mosavian/llrm/pull/2479)); loops wait for the block placer to set `LirBlock.align` |
| E3 | `emitConstantPool`: each entry at its alignment, entries sorted by it, mergeable sections by size | `constpool.rs`: one datum per distinct bytes, no alignment | equal: pool entries are 4 or 8 bytes, 8-byte ones first on 8; `.rodata` of a float is 4-aligned (readelf) |
| E4 | `emitJumpTableInfo`: table at the entry alignment | `masm::jump_tables`; alignment unchecked | equal: a jump table is 4-byte entries in `.rodata`, aligned 4 (readelf) |
| E5 | `MCAssembler::relaxOnce`: fixed point over fragments, align fragments included | `objbuild::_relaxed` (jwasm's fixed point; `_trampolined`, Watcom's, at -Os) | old; equal core; redo once E2 makes sizes offset dependent |
| E6 | CFI directives from `X86FrameLowering::emitCalleeSavedFrameMoves` | `cfi.rs` decodes the rows back from the bytes | DEVIATE by design: same rows, other route |
| E7 | `TargetLoweringObjectFile::SectionForGlobal`: rodata, data, bss, mergeable strings by kind | segments named by the frontend | old |
| E8 | object writers, `DwarfDebug` | `llrm-elf`, `llrm-coff`, `llrm-omf`, `debuginfo.rs` | format conformance, not code quality; not translated |

Encodings are iced's shortest form, as the MC emitter's, so no row.
