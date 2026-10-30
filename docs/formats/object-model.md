---
architecture-index: object model
architecture-order: 500
---
# Object model

For contributors changing a writer or the backend's object layout. The backend lays code and data out
once as a format-neutral `Object`; one writer crate per format serializes it.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  mir["<b>MIR</b>"]:::mid
  isel["<b>isel</b>"]:::mid
  masm["<b>masm::Module</b>"]:::mid
  ob["<b>objbuild</b><br/><small>layout, relaxation</small>"]:::mid
  obj["<b>Object</b><br/><small>sections, symbols,<br/>omf_groups,<br/>debug: Some(Info)</small>"]:::opt
  subgraph g_wr["WRITERS BY -FOBJECT-FORMAT=<br/>(OBJECT.TOML)#59; EACH ENCODES<br/>THE DEBUG INFO"]
    omf["<b>omf</b><br/><small>.obj/.o</small>"]:::out
    e32["<b>elf32</b><br/><small>.o</small>"]:::out
    e64["<b>elf64</b><br/><small>.o</small>"]:::out
    mo["<b>macho</b><br/><small>.o</small>"]:::out
    c32["<b>coff32</b><br/><small>.obj</small>"]:::out
    c64["<b>coff64</b><br/><small>.obj</small>"]:::out
  end
  mir --> isel
  isel --> masm
  masm --> ob
  ob --> obj
  obj --> g_wr
  style g_wr fill:#16a34a12,stroke:#16a34a,stroke-width:1.6px,stroke-dasharray:7 5,color:#16a34a
```

*From MIR to an object file*

`objbuild::written_in` ([source](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/backend/llrm-core/src/backend/objbuild.rs#L434))
builds the `Object` and calls the writer for the chosen format and the object's `Arch`. A pair with no
writer returns `Unsupported`.

## The model

`crates/mc/llrm-object/src/lib.rs`:

```
Object   { name, arch: I8086|I386|X8664, sections, symbols, omf_groups, debug: Option<debug::Info> }
Section  { name, role: Text|ROData|Data|Bss|Stack|Debug, near, align, image, spans, relocs }
Reloc    { at, kind, target, addend }
Kind     = Abs{width} | PcRel{width, from} | Branch{width}
         | SectionIndex | SectionOffset{width} | SegmentBase | FarPointer      (non_exhaustive)
Target   = Symbol(i) | OmfGroup(i) | Section(i)
Symbol   { name, binding: Public|Local, definition: Defined{section, offset}|Undefined, group }
```

- `image` holds zeros where a relocation patches; `spans` are the `[start, end)` ranges that hold data, the
  rest is zeros a format may leave unstored. A `Bss` image is zeros.
- The addend is explicit. A writer places it in the field (OMF, ELF REL, COFF, Mach-O) or in the entry
  (ELF RELA).
- A pc-relative value is `S + addend - (at + from)`. An x86 call has `from` equal to the field's width.
  Each writer folds what its relocation type already subtracts: ELF's addend is `addend - from`, COFF's
  field is `addend - (from - baked)` (`REL32`: 4), Mach-O's `addend - beyond` with `SIGNED_1/2/4`.
- `Branch{width}` is `PcRel{width, from: width}` for the formats that tell a branch from a data reference:
  Mach-O `BRANCH`, ELF `PLT32`.
- `SectionIndex` and `SectionOffset` are COFF's `SECTION` and `SECREL`.
- `Target::Section` names the start of a section: what a DWARF section refers to another one by.
- A reference to a symbol an object keeps local is the same `Target::Symbol`; each writer says it its way:
  OMF a fixup against the segment, ELF the section symbol plus an offset, COFF a static symbol, Mach-O a
  section reference with the address in the field.
- `SegmentBase`, `FarPointer`, `OmfGroup`, `Role::Stack` and `near == false` exist for OMF. A writer that
  cannot say a fact returns `Unsupported(what)`; it never writes a near substitute. `Symbol.group` is OMF's
  frame hint; the other writers ignore it.
- `Arch::bits()` is 16, 32 or 64; OMF writes 16 and 32.

Not in the model: arm64 instruction fields, weak and hidden symbols, symbol sizes, a difference of two
symbols, COMDAT.

## Choosing a format

A target's `object.toml` lists `formats` and a `default`. `-fobject-format=omf|elf|macho|coff` picks one
([`ObjectFormat::choose`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/target/llrm-target/src/object.rs#L102));
without it the default applies. A format the target does not list is refused with the list it has.

| target | formats | default | writers reached |
|---|---|---|---|
| `x86-m16` | omf | omf | `llrm-omf` |
| `x86-m32` | omf, elf, coff | omf | `llrm-omf`, `llrm-elf32`, `llrm-coff32` |

`llrm-elf64`, `llrm-coff64` and `llrm-macho` are written and unit-tested, and `objbuild` dispatches
`X8664` objects to them, but no target selects them. Per-format layout: [OMF](omf.md),
[COFF, ELF and Mach-O](coff-elf-macho.md). Debug information: [debug-info](debug-info.md).

## Writers

| format | crate | container | machine crate |
|---|---|---|---|
| OMF | `llrm-omf` (`write`) | records | none; 16- and 32-bit by `Arch` |
| ELF | `llrm-elf` | sections, symtab, REL/RELA | `llrm-elf32` (i386), `llrm-elf64` (x86-64) |
| COFF | `llrm-coff` | sections, symbols, relocations | `llrm-coff32`, `llrm-coff64` |
| Mach-O | `llrm-macho` | one segment, nlist | x86-64 only, in the crate |

ELF and COFF are generic over a `Machine` trait that gives the machine number, the relocation type for each
`Kind` and, for ELF, REL or RELA.

## Debug information

`Object.debug` is `llrm_object::debug::Info`, built once by the backend and encoded by each writer. See
[debug-info](debug-info.md). A writer refuses what it cannot say, by name; `Role::Debug` marks the sections
a writer makes of it.
