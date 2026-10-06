# Object model

One format-neutral `Object` (crate `llrm-object`), built by the backend, written by one crate per format
(`llrm-omf`, `llrm-elf32`, `llrm-elf64`, `llrm-macho`).

## Today

`backend/omfwrite.rs` (now `objbuild.rs`) lays out code and data into `Segment`s (image, spans, fixups, lines), resolves local
names to `(segment, offset)`, and in the same file turns that into OMF records. Layout and record
emission are one function chain. The fixup kinds in use are OMF's location codes (offset16, base,
ptr16:16, offset32), plus a `relative` flag.

## Model

```
Object   { name, arch, mode, sections, symbols, omf_groups, atomized }
Section  { name, role: Text|ROData|Data|Bss|Stack|Debug(fmt), align, size, image, spans, relocs, lines, near }
Reloc    { at, kind, target, addend, pc_offset }
Kind     = Data{width, pcrel} | SegmentBase | FarPointer      (non_exhaustive: arm64 instruction fields join later)
Target   = Symbol(id) | Section(id) | OmfGroup(id)
Symbol   { name, binding: Public|Local|Weak, hidden, kind: Func|Object|None, size, definition: Section{index, offset} | Undefined, group }
```

- The addend is explicit; the image holds zeros. A writer places it: in the field for OMF and ELF REL,
  in the entry for RELA, beside the entry for arm64 Mach-O.
- A pc-relative value is `S + addend - (at + pc_offset)`. x86 call: `pc_offset` 4. ELF's addend is
  `addend - pc_offset`. OMF accepts only `pc_offset` equal to the width.
- A local reference is `Target::Section` plus an addend; a writer that needs a symbol for it (arm64
  Mach-O) makes a local one.
- `atomized`: every global symbol starts an atom and no reference between atoms is pre-resolved, as
  Mach-O's subsections-via-symbols needs. The backend sets it when the format is Mach-O.
- `SegmentBase`, `FarPointer`, `OmfGroup`, `Stack`, `near == false` exist for OMF. A writer that cannot
  say one returns `Unsupported(what)`; it never writes a near substitute. `Symbol.group` is OMF's
  frame hint; others ignore it.
- Debug data is ordinary sections with relocs, tagged with its format; a writer refuses a format it
  does not write.

## Moves

| from | to |
| --- | --- |
| `omfwrite::Segment`, `Fixup` (location codes) | `llrm-object::{Section, Reloc, Kind}`; `objbuild` keeps the builder `Segment` |
| `omfwrite::_records, _ledata, _subrecord, _linnum`, record constants | `llrm-omf::write` |
| `omfwrite::_fresh_segment`, `_resolved_fixup` (unused) | deleted |
| layout, relaxation, `_code`, `_data`, `_items` | stay in `llrm-core`, return an `Object` |
| `codeview::segments` | stays, returns tagged debug sections |

## Choosing a format

`object.toml`: `formats = ["omf", "elf"]`, `default = "omf"`. `-fobject-format=omf|elf|macho` picks one;
without it the default applies. A format the target does not list is refused, and so is an `Object` the
writer cannot express.

## Writers

| format | crate | status |
| --- | --- | --- |
| OMF | `llrm-omf` (`write`) | 16- and 32-bit |
| ELF32 | `llrm-elf32` | i386: `.text`/`.data`/`.rodata`/`.bss`, REL, `R_386_32`, `R_386_PC32`, `_16`, `PC16`, `_8`, `PC8` |

| ELF64 | `llrm-elf64` | x86-64: RELA, `R_X86_64_64`, `_PC32`, `_32`, `_16`, `_PC16`, `_8`, `_PC8`, `_PC64`; no target produces one yet |

`llrm-elf` holds the container both share (sections, symbols, relocation tables); a machine gives it
its ELF number, its relocation types, and REL or RELA.

An ELF object is written for `-m32 -fobject-format=elf` into a file named `*.o`. It refuses what OMF alone
has: segments with a selector of their own, far pointers, groups, the stack segment, and `-g`'s CodeView.
