# Object model

One format-neutral `Object` (crate `llrm-object`), built by the backend, written by one crate per format
(`llrm-omf`, `llrm-elf32`, `llrm-elf64`, `llrm-macho`, `llrm-coff32`, `llrm-coff64`).

## Today

`backend/omfwrite.rs` (now `objbuild.rs`) lays out code and data into `Segment`s (image, spans, fixups, lines), resolves local
names to `(segment, offset)`, and in the same file turns that into OMF records. Layout and record
emission are one function chain. The fixup kinds in use are OMF's location codes (offset16, base,
ptr16:16, offset32), plus a `relative` flag.

## Model

```
Object   { name, arch, sections, symbols, omf_groups, debug }
Section  { name, role: Text|ROData|Data|Bss|Stack|Debug(kind), near, align, image, spans, relocs, lines }
Reloc    { at, kind, target, addend }
Kind     = Abs{width} | PcRel{width, from} | Branch{width} | SectionIndex | SectionOffset{width} | SegmentBase | FarPointer   (non_exhaustive)
Target   = Symbol(id) | OmfGroup(id)
Symbol   { name, binding: Public|Local, definition: Defined{section, offset} | Undefined, group }
```

- The addend is explicit; the image holds zeros. A writer places it: in the field for OMF, ELF REL and
  Mach-O, in the entry for RELA.
- A pc-relative value is `S + addend - (at + from)`. A call's `from` is the field's width. ELF's addend is
  `addend - from`; Mach-O's field is `addend - (from - 4)` and its type says the rest (`SIGNED_1/2/4`);
  OMF accepts only `from` equal to the width.
- `Branch{width}` is a call or jump's field, `PcRel{width, width}` for the formats that tell the two
  apart: Mach-O's `BRANCH`, ELF's `PLT32`.
- A reference to a symbol an object keeps local is the same `Target::Symbol`; each writer says it its
  way: OMF a fixup against the segment, ELF the section symbol and an offset, Mach-O a section reference
  with the address in the field.
- `SectionIndex` and `SectionOffset` are COFF's `SECTION` and `SECREL`: the target's section number, and its offset in that section.
- `SegmentBase`, `FarPointer`, `OmfGroup`, `Stack`, `near == false` exist for OMF. A writer that cannot
  say one returns `Unsupported(what)`; it never writes a near substitute. `Symbol.group` is OMF's
  frame hint; others ignore it.
- Debug data is ordinary sections with relocs, tagged with its format; a writer refuses a format it
  does not write.

Not in the model yet, to be added with the first writer that needs it: arm64 instruction fields
(`Kind::Insn`, a `Via::Got` beside them), weak and hidden symbols, symbol sizes, a difference of two
symbols, COMDAT (COFF needs it for inline functions and templates; no frontend produces one yet).

## Moves

| from | to |
| --- | --- |
| `omfwrite::Segment`, `Fixup` (location codes) | `llrm-object::{Section, Reloc, Kind}`; `objbuild` keeps the builder `Segment` |
| `omfwrite::_records, _ledata, _subrecord, _linnum`, record constants | `llrm-omf::write` |
| `omfwrite::_fresh_segment`, `_resolved_fixup` (unused) | deleted |
| layout, relaxation, `_code`, `_data`, `_items` | stay in `llrm-core`, return an `Object` |
| `codeview::segments` | stays, returns tagged debug sections |

## Choosing a format

`object.toml`: `formats = ["omf", "elf", "coff"]`, `default = "omf"`. `-fobject-format=omf|elf|macho|coff` picks one;
without it the default applies. A format the target does not list is refused, and so is an `Object` the
writer cannot express.

## Writers

| format | crate | status |
| --- | --- | --- |
| OMF | `llrm-omf` (`write`) | 16- and 32-bit |
| ELF32 | `llrm-elf32` | i386: `.text`/`.data`/`.rodata`/`.bss`, REL, `R_386_32`, `R_386_PC32`, `_16`, `PC16`, `_8`, `PC8` |

| ELF64 | `llrm-elf64` | x86-64: RELA, `R_X86_64_64`, `_PC32`, `_32`, `_16`, `_PC16`, `_8`, `_PC8`, `_PC64`; no target produces one yet |
| COFF i386 | `llrm-coff32` | `.text`/`.rdata`/`.data`/`.bss`; `DIR32`, `REL32`, `SECTION`, `SECREL`; `@feat.00` = 1 |
| COFF x86-64 | `llrm-coff64` | `ADDR64`, `ADDR32`, `REL32`..`REL32_5`, `SECTION`, `SECREL`; no target produces one yet |
| Mach-O | `llrm-macho` | x86-64: `__text`/`__const`/`__data`/`__bss`; `BRANCH`, `SIGNED`, `SIGNED_1/2/4`, `UNSIGNED`; no target produces one yet |

`llrm-coff` holds the COFF container both COFF writers share, as `llrm-elf` does for ELF. A further section
of a role is `.text$name`, which the linker merges into `.text`. A COFF object refuses what OMF alone has,
as ELF does. A pc-relative field's value is `addend - (from - baked)`, `baked` being what the relocation
type already subtracts (`REL32`: 4). More than 65535 relocations in a section use `LNK_NRELOC_OVFL`.

`llrm-elf` holds the container both share (sections, symbols, relocation tables); a machine gives it
its ELF number, its relocation types, and REL or RELA.

An ELF object is written for `-m32 -fobject-format=elf` into a file named `*.o`. It refuses what OMF alone
has: segments with a selector of their own, far pointers, groups, the stack segment, and `-g`'s CodeView.

Mach-O: the file is not `MH_SUBSECTIONS_VIA_SYMBOLS`, so a code section is one atom, a call between two
of its functions may be resolved where it is written, and `-dead_strip` keeps the whole section. A local
symbol is not in the symbol table. Known gaps: no `LC_BUILD_VERSION` (the platform is the linker's to be
told), a 32-bit absolute address is `UNSIGNED` and needs a non-PIE link, and arm64 needs local
symbols (`r_extern=1` only) and `ARM64_RELOC_ADDEND`, so `llrm-macho` takes a `Machine` as `llrm-elf`
does when the first arm64 target exists.
