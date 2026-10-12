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
Object   { name, arch, sections, symbols, omf_groups, debug: Option<debug::Info> }
Section  { name, role: Text|ROData|Data|Bss|Stack|Debug, near, align, image, spans, relocs }
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
- Debug information is `llrm_object::debug::Info`, once (below). A writer encodes it its own way and
  refuses a fact it cannot say; `Role::Debug` is only the sections a writer makes of it.

Not in the model yet, to be added with the first writer that needs it: arm64 instruction fields
(`Kind::Insn`, a `Via::Got` beside them), weak and hidden symbols, symbol sizes, a difference of two
symbols, COMDAT (COFF needs it for inline functions and templates; no frontend produces one yet).

## Debug information

`backend/debuginfo.rs` builds `debug::Info` from MIR's metadata (`llrm_mir::debuginfo`, which the
frontends write) and the layout: types (an arena), functions (symbol, ranges, body, parameters and
locals each with a `Location`: frame cell, register, list of ranges, static), module code and data,
files and lines, and the target's register file with its DWARF and CodeView numbers (`registers.regs`).
Writers are target-blind: `llrm-omf::codeview` writes $$SYMBOLS, $$TYPES and LINNUM from it, and
`llrm-dwarf` writes DWARF 5 (4 with `-gdwarf-4`: the same writer, its units and line table headers
differ) as `.debug_info`, `.debug_abbrev`, `.debug_str`, `.debug_line`, `.debug_line_str` and
`.debug_aranges`; `llrm-elf` and `llrm-macho` (`__DWARF`, whose sections one another refer to by
offset, unrelocated) call it where an object has `Info`. DWARF 5 is the default for its range lists,
`line_strp` and MD5 file checksums, which gdb 10+ and LLVM read. Its location, type and register
facts are the model's: a register's number is the target's `registers.regs` `dwarf` column.
Enums, typedefs, qualifiers, block scopes, register and listed locations, columns and checksums are in
the model; a frontend fills them as it learns to, and a writer that cannot say one refuses it by name.

A `Location::Frame` is relative to the frame register as the code would set it. A format that finds a cell from the canonical frame address (DWARF: `FrameBase::Cfa`, `CFA_LOCATIONS`) needs no frame register kept for it; one that does not (CodeView, Turbo Debugger) is told of a cell only where the code keeps the register anyway.

**`-g` never changes the code; what a format cannot say is left out** (gcc's way). A variable lives in MIR as debug records
(`#dbg_declare`, `#dbg_value`, `#dbg_piece`, `#dbg_gone`), a side table kept true by the edit points, not instructions; no
global or store is kept for a debugger. On a `CFA_LOCATIONS` format the backend finds where each value is by following the
final code (`backend/valuetrack.rs`: LLVM's instruction-referencing LiveDebugValues) and writes ranges. A record a pass could not
keep true says nothing, never a wrong thing: a variable is `<optimized out>` where it is not known. A format with one place per scope (CodeView 4, Turbo Debugger) says a variable only where a cell holds it
for the scope: a register parameter, or a local the allocator keeps in a register, is left out, and no store is added to make one
sayable. `tools/g-identical.sh` and `g_leaves_the_code_of_the_bench_programs_alone` (C on ELF, COFF and OMF, 16- and 32-bit;
BASIC and Nib) hold the rule.

## Moves

| from | to |
| --- | --- |
| `omfwrite::Segment`, `Fixup` (location codes) | `llrm-object::{Section, Reloc, Kind}`; `objbuild` keeps the builder `Segment` |
| `omfwrite::_records, _ledata, _subrecord, _linnum`, record constants | `llrm-omf::write` |
| `omfwrite::_fresh_segment`, `_resolved_fixup` (unused) | deleted |
| layout, relaxation, `_code`, `_data`, `_items` | stay in `llrm-core`, return an `Object` |
| `codeview::segments` | `backend/debuginfo.rs` builds `debug::Info`; `llrm-omf::codeview` writes it |

## Choosing a format

`object.toml`: `formats = ["omf", "elf", "coff"]`, `default = "omf"`. `-fobject-format=omf|elf|macho|coff` picks one;
without it the default applies. A format the target does not list is refused, and so is an `Object` the
writer cannot express.

## Writers

| format | crate | status |
| --- | --- | --- |
| OMF | `llrm-omf` (`write`) | 16- and 32-bit |
| ELF32 | `llrm-elf32` | i386: `.text`/`.data`/`.rodata`/`.bss`, REL, `R_386_32`, `R_386_PC32`, `_16`, `PC16`, `_8`, `PC8`; `-g` is DWARF |

| ELF64 | `llrm-elf64` | x86-64: RELA, `R_X86_64_64`, `_PC32`, `_32`, `_16`, `_PC16`, `_8`, `_PC8`, `_PC64`; no target produces one yet |
| COFF i386 | `llrm-coff32` | `.text`/`.rdata`/`.data`/`.bss`; `DIR32`, `REL32`, `SECTION`, `SECREL`; `@feat.00` = 1 |
| COFF x86-64 | `llrm-coff64` | `ADDR64`, `ADDR32`, `REL32`..`REL32_5`, `SECTION`, `SECREL`; no target produces one yet |
| Mach-O | `llrm-macho` | x86-64: `__text`/`__const`/`__data`/`__bss`; `BRANCH`, `SIGNED`, `SIGNED_1/2/4`, `UNSIGNED`; no target produces one yet |

`llrm-coff` holds the COFF container both COFF writers share, as `llrm-elf` does for ELF. A further section
of a role is `.text$name`, which the linker merges into `.text`. A COFF object refuses what OMF alone has,
as ELF does. A pc-relative field's value is `addend - (from - baked)`, `baked` being what the relocation
type already subtracts (`REL32`: 4). More than 65535 relocations in a section use `LNK_NRELOC_OVFL`.

COFF debug information is CodeView C13, written from `Info` into `.debug$S` and `.debug$T` (`llrm-coff/src/codeview`):
a function is `S_GPROC32`/`S_LPROC32`, a variable `S_LOCAL` with one `S_DEFRANGE_REGISTER_REL` (frame) or
`S_DEFRANGE_REGISTER` per range (pieces of 0xF000 bytes, a range's length being 16 bits), a block `S_BLOCK32`, a
global `S_GDATA32`/`S_LDATA32`, a named struct, enum or typedef `S_UDT`. A code or data address is a `SECREL` and a
`SECTION` relocation against the object's symbol. A struct reached from its own field is written as a forward
reference first. Register numbers come from `Info.registers`. Refused by name: far and huge pointers, BASIC's types,
a function or block in several ranges, a register with no CodeView number.

`llrm-elf` holds the container both share (sections, symbols, relocation tables); a machine gives it
its ELF number, its relocation types, and REL or RELA.

An ELF object is written for `-m32 -fobject-format=elf` into a file named `*.o`. It refuses what OMF alone
has: segments with a selector of their own, far pointers, groups, the stack segment, and `-g`'s CodeView or
Turbo Debugger flavors (`-g` and `-gdwarf[-4|-5]` are DWARF).

Mach-O: the file is not `MH_SUBSECTIONS_VIA_SYMBOLS`, so a code section is one atom, a call between two
of its functions may be resolved where it is written, and `-dead_strip` keeps the whole section. A local
symbol is not in the symbol table. Known gaps: no `LC_BUILD_VERSION` (the platform is the linker's to be
told), a 32-bit absolute address is `UNSIGNED` and needs a non-PIE link, and arm64 needs local
symbols (`r_extern=1` only) and `ARM64_RELOC_ADDEND`, so `llrm-macho` takes a `Machine` as `llrm-elf`
does when the first arm64 target exists.

## Call frame information

`Function.frame` is a table of rows: from an offset into the function, the frame address is `cfa_offset` past
`cfa_register`, and each register in `saved` is in memory at that address plus its offset. The backend reads it
from the code it emitted (`backend/cfi.rs`: the code is decoded and followed along every path, so a frame register,
none, and registers saved where first needed are all the code's own), with the bytes a callee pops after a call from
the compiler's record of the call (`Mark::Pops`). A function whose paths reach one place at two stack depths has no
rows. DWARF writes them as `.debug_frame`; CodeView and Turbo Debugger have no such table. The return address's
column is the target's register of class `pc`.

## Register parameters and the format's ranges

A parameter that arrives in a register is there until the function stores it. Whether the debug format can say that is the
writer's fact (`llrm_dwarf::LOCATION_RANGES`, `llrm_coff::LOCATION_RANGES`, `llrm_omf::LOCATION_RANGES`; `Options::location_ranges`
picks the writer from the object format and the `-g` flavor). Where it can (DWARF location lists, C13 ranges), the parameter keeps its
register range and a cell once stored. Where it cannot (CodeView 4, its BASIC-era dialect, Turbo Debugger), instruction selection
stores each such parameter to a cell of its own at the entry, before the body a debugger stops at begins, and describes the cell.
