//! An [`Object`] as a 64-bit Mach-O relocatable file for x86-64 or arm64: one
//! segment holding the sections, a symbol table, and the 8-byte relocation
//! entries Mach-O keeps per section, with the addend in the field. A reference
//! to a symbol the object defines and exports, or declares, names it; a
//! reference to a local one names its section, and the field holds the address.
//!
//! arm64 lists its local symbols and names them in every relocation, as LLVM's
//! writer does; an addend goes in an `RELOC_ADDEND` entry before its
//! relocation.
//!
//! An arm64 object carries `LC_BUILD_VERSION` (macOS 11, no tools, no SDK), as
//! LLVM's does for `arm64-apple-macos`; `ld` takes the platform from it.
//!
//! An x86-64 file is not marked `MH_SUBSECTIONS_VIA_SYMBOLS`: a code section
//! stays one atom, so a call between two functions of it may be resolved where
//! it is written. An arm64 file is, as clang's is: a weak definition
//! (`N_WEAK_DEF`) replaces only itself, and each reference between two symbols
//! is a relocation.

use llrm_object::{Binding, Definition, Object, Reloc, Role, Section, Target, Unsupported, assembler::AsmBackend};

const MH_MAGIC_64: u32 = 0xFEED_FACF;
const MH_OBJECT: u32 = 1;
const MH_SUBSECTIONS_VIA_SYMBOLS: u32 = 0x2000;
pub const LC_SYMTAB: u32 = 0x2;
const LC_DYSYMTAB: u32 = 0xB;
const LC_SEGMENT_64: u32 = 0x19;
pub const LC_BUILD_VERSION: u32 = 0x32;
pub const PLATFORM_MACOS: u32 = 1;
const S_ZEROFILL: u32 = 1;
const S_COALESCED: u32 = 0xB;
const S_ATTR_NO_TOC: u32 = 0x4000_0000;
const S_ATTR_STRIP_STATIC_SYMS: u32 = 0x2000_0000;
const S_ATTR_LIVE_SUPPORT: u32 = 0x0800_0000;
const S_MOD_INIT_FUNC_POINTERS: u32 = 9;
const S_ATTR_PURE_INSTRUCTIONS: u32 = 0x8000_0000;
const S_ATTR_SOME_INSTRUCTIONS: u32 = 0x0000_0400;
const S_ATTR_DEBUG: u32 = 0x0200_0000;
pub const N_EXT: u8 = 1;
pub const N_WEAK_DEF: u16 = 0x80;
const N_SECT: u8 = 0xE;
/// `ARM64_RELOC_ADDEND`, the one entry Mach-O has for an addend a field cannot
/// hold.
const RELOC_ADDEND: u32 = 10;

fn unsupported(text: impl Into<String>) -> Unsupported {
    Unsupported(text.into())
}

fn put32(
    out: &mut Vec<u8>,
    value: u32,
) {
    out.extend(value.to_le_bytes());
}

fn put64(
    out: &mut Vec<u8>,
    value: u64,
) {
    out.extend(value.to_le_bytes());
}

/// A fixed-width name field, NUL padded.
fn name16(
    out: &mut Vec<u8>,
    name: &str,
) {
    let mut field = [0u8; 16];
    field[..name.len()].copy_from_slice(name.as_bytes());
    out.extend(field);
}

/// The Mach-O name of a DWARF section.
fn debug_name(name: &str) -> Result<&'static str, Unsupported> {
    Ok(match name {
        ".debug_abbrev" => "__debug_abbrev",
        ".debug_aranges" => "__debug_aranges",
        ".debug_info" => "__debug_info",
        ".debug_frame" => "__debug_frame",
        ".debug_line" => "__debug_line",
        ".debug_line_str" => "__debug_line_str",
        ".debug_loc" => "__debug_loc",
        ".debug_loclists" => "__debug_loclists",
        ".debug_ranges" => "__debug_ranges",
        ".debug_rnglists" => "__debug_rnglists",
        ".debug_str" => "__debug_str",
        other => return Err(unsupported(format!("{other}: no Mach-O name for this debug section"))),
    })
}

/// The segment, section and type-and-attributes words of `section`.
fn spelling(section: &Section) -> Result<(&'static str, &'static str, u32), Unsupported> {
    if !section.near {
        return Err(unsupported(format!(
            "{}: a segment addressed by its own selector has no Mach-O section",
            section.name
        )));
    }
    Ok(match section.role {
        Role::Text => ("__TEXT", "__text", S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS),
        // A constant with relocations is written to by the dynamic linker: ld64
        // refuses it in __TEXT.
        Role::ROData if !section.relocs.is_empty() => ("__DATA", "__const", 0),
        Role::ROData => ("__TEXT", "__const", 0),
        Role::Data => ("__DATA", "__data", 0),
        Role::Bss => ("__DATA", "__bss", S_ZEROFILL),
        Role::Stack => {
            return Err(unsupported(format!("{}: an OMF stack segment has no Mach-O section", section.name)));
        }
        // `.debug_info` is `__debug_info` of the segment `__DWARF`.
        Role::InitFunctions => ("__DATA", "__mod_init_func", S_MOD_INIT_FUNC_POINTERS),
        Role::Debug(_) => ("__DWARF", debug_name(&section.name)?, S_ATTR_DEBUG),
        // `__eh_frame` is coalesced, the linker drops what nothing keeps live;
        // `__compact_unwind` is the linker's input and not part of the image.
        Role::Unwind if section.name == "__eh_frame" => {
            ("__TEXT", "__eh_frame", S_COALESCED | S_ATTR_NO_TOC | S_ATTR_STRIP_STATIC_SYMS | S_ATTR_LIVE_SUPPORT)
        }
        Role::Unwind if section.name == "__compact_unwind" => ("__LD", "__compact_unwind", S_ATTR_DEBUG),
        Role::Unwind => {
            return Err(unsupported(format!("{}: Mach-O unwind is __compact_unwind and __eh_frame", section.name)));
        }
    })
}

/// What a relocation entry says of a field.
pub struct Described {
    pub kind: u32,
    /// log2 of the field's bytes.
    pub length: u32,
    pub pcrel: bool,
    /// The bytes between the field's end and the place it is relative to.
    pub beyond: i64,
    /// Whether the field holds the addend. The fields of arm64's instructions
    /// do not: the instruction keeps its zeros, and an addend travels in an
    /// `ARM64_RELOC_ADDEND` entry before the relocation.
    pub addend_in_field: bool,
}

/// What a target says of the Mach-O object it writes (LLVM's
/// `MCMachObjectTargetWriter`).
pub trait MachTargetWriter {
    /// The target's assembler backend, which fills the fields the file keeps
    /// (`MCAsmBackend::applyFixup`).
    fn backend(&self) -> &dyn AsmBackend;

    /// `cputype` and `cpusubtype`.
    fn cpu(&self) -> (u32, u32);

    /// Whether the symbol table lists the symbols the object keeps local. A
    /// relocation then names the symbol and not its section.
    fn lists_locals(&self) -> bool;

    /// The platform and minimum OS (`xxxx.yy.zz` packed) of `LC_BUILD_VERSION`,
    /// if the machine's objects carry one.
    fn build_version(&self) -> Option<(u32, u32)> {
        None
    }

    /// Whether the linker may treat each symbol of a section as an atom of its
    /// own (`MH_SUBSECTIONS_VIA_SYMBOLS`): then a weak definition replaces
    /// only itself, and every reference between two symbols of a section
    /// carries a relocation.
    fn subsections_via_symbols(&self) -> bool {
        false
    }

    fn relocation(
        &self,
        reloc: &Reloc,
    ) -> Result<Described, Unsupported>;
}

fn align_up(
    value: usize,
    to: usize,
) -> usize {
    value.div_ceil(to) * to
}

/// `object` as a Mach-O relocatable file.
pub fn write<W: MachTargetWriter>(
    object: &Object,
    flavor: &W,
) -> Result<Vec<u8>, Unsupported> {
    if !object.omf_groups.is_empty() {
        return Err(unsupported("a group of segments is OMF's"));
    }
    let spelled: Vec<(&str, &str, u32)> = object.sections.iter().map(spelling).collect::<Result<_, _>>()?;
    for (index, one) in spelled.iter().enumerate() {
        if spelled[..index].iter().any(|other| other.0 == one.0 && other.1 == one.1) {
            return Err(unsupported(format!(
                "{}: two sections would both be ({},{})",
                object.sections[index].name, one.0, one.1
            )));
        }
    }

    // Sections with contents come first in the file, then the zero-fill ones,
    // which have none.
    let mut order: Vec<usize> = (0..object.sections.len()).filter(|&one| spelled[one].2 != S_ZEROFILL).collect();
    order.extend((0..object.sections.len()).filter(|&one| spelled[one].2 == S_ZEROFILL));
    let ordinal =
        |section: usize| order.iter().position(|&one| one == section).expect("a section is in the order") as u32 + 1;
    let mut address = vec![0usize; object.sections.len()];
    let mut next = 0;
    for &one in &order {
        next = align_up(next, object.sections[one].align.max(1).next_power_of_two());
        address[one] = next;
        next += object.sections[one].image.len();
    }
    let vm_size = next;
    let file_backed = order.iter().filter(|&&one| spelled[one].2 != S_ZEROFILL).count();
    let file_size =
        order.iter().take(file_backed).last().map_or(0, |&one| address[one] + object.sections[one].image.len());

    // Symbols the object exports, then the ones it declares; each by name
    // within its kind.
    let mut defined: Vec<usize> = (0..object.symbols.len())
        .filter(|&one| {
            matches!(object.symbols[one].binding, Binding::Public | Binding::Weak)
                && matches!(object.symbols[one].definition, Definition::Defined { .. })
        })
        .collect();
    // The local ones come first in the table, where the machine lists them.
    let locals: Vec<usize> = (0..object.symbols.len())
        .filter(|&one| {
            flavor.lists_locals()
                && object.symbols[one].binding == Binding::Local
                && matches!(object.symbols[one].definition, Definition::Defined { .. })
        })
        .collect();
    // ...in the order the object defines them, as LLVM lists them.
    let mut undefined: Vec<usize> =
        (0..object.symbols.len()).filter(|&one| object.symbols[one].definition == Definition::Undefined).collect();
    defined.sort_by(|&a, &b| object.symbols[a].name.cmp(&object.symbols[b].name));
    undefined.sort_by(|&a, &b| object.symbols[a].name.cmp(&object.symbols[b].name));
    let mut index_of = vec![None; object.symbols.len()];
    for (at, &one) in locals.iter().chain(&defined).chain(&undefined).enumerate() {
        index_of[one] = Some(at as u32);
    }
    let mut strings = vec![0u8];
    let mut nlist = Vec::new();
    for &one in locals.iter().chain(&defined).chain(&undefined) {
        let symbol = &object.symbols[one];
        let name = strings.len() as u32;
        strings.extend(symbol.name.as_bytes());
        strings.push(0);
        put32(&mut nlist, name);
        match symbol.definition {
            Definition::Defined { section, offset } => {
                let external = if symbol.binding == Binding::Local { 0 } else { N_EXT };
                nlist.extend([N_SECT | external, ordinal(section) as u8]);
                nlist.extend((if symbol.binding == Binding::Weak { N_WEAK_DEF } else { 0 }).to_le_bytes());
                put64(&mut nlist, (address[section] + offset) as u64);
            }
            Definition::Undefined => {
                nlist.extend([N_EXT, 0]);
                nlist.extend(0u16.to_le_bytes());
                put64(&mut nlist, 0);
            }
        }
    }

    // Each section's bytes with the fields filled, and its relocation entries.
    let mut images: Vec<Vec<u8>> = Vec::new();
    let mut entries: Vec<Vec<u8>> = Vec::new();
    for (index, section) in object.sections.iter().enumerate() {
        let mut image = section.image.clone();
        let mut made: Vec<(usize, [u32; 2])> = Vec::new();
        for one in &section.relocs {
            // The unwind sections name code by its place in the object: a
            // pc-relative reference from them to a section is resolved here, as
            // LLVM resolves a difference of two sections' symbols (no entry).
            if section.role == Role::Unwind && one.pc_rel && matches!(one.target, Target::Section(_)) {
                let Target::Section(home) = one.target else { unreachable!("matched") };
                let value = address[home] as i64 + one.addend - (address[index] + one.at) as i64;
                flavor.backend().apply_fixup(&mut image, one.at, one.kind, true, value, &section.name)?;
                continue;
            }
            let Described { kind, length, pcrel, beyond, addend_in_field } = flavor.relocation(one)?;
            // A section's start is a local symbol at offset 0 of it.
            let (target, home) = match one.target {
                Target::Symbol(target) => (Some(target), None),
                Target::Section(section) => (None, Some(section)),
                Target::OmfGroup(_) => return Err(unsupported("a reference to a group is OMF's")),
            };
            // One debug section's offset into another is not relocated:
            // dsymutil reads the object's own sections, where it is
            // the offset itself.
            if matches!(section.role, Role::Debug(_))
                && home.is_some_and(|home| matches!(object.sections[home].role, Role::Debug(_)))
            {
                let offset = u32::try_from(one.addend)
                    .map_err(|_| unsupported(format!("{}: an offset of {} does not fit", section.name, one.addend)))?;
                flavor.backend().apply_fixup(&mut image, one.at, one.kind, false, i64::from(offset), &section.name)?;
                continue;
            }
            let (symbolnum, external, field) = match target.and_then(|target| index_of[target]) {
                Some(symbol) => (symbol, true, one.addend - beyond),
                None if !addend_in_field => {
                    return Err(unsupported(format!(
                        "{}: an instruction field refers to a section, not to a symbol",
                        section.name
                    )));
                }
                None => {
                    // A local symbol: its section, and the address in the
                    // object's own space, which the
                    // linker moves with the section.
                    let (home, offset) = match (target, home) {
                        (Some(target), _) => match object.symbols[target].definition {
                            Definition::Defined { section: home, offset } => (home, offset),
                            Definition::Undefined => unreachable!("an undefined symbol is listed"),
                        },
                        (None, Some(home)) => (home, 0),
                        (None, None) => unreachable!("a target is a symbol or a section"),
                    };
                    let place = if pcrel { (address[index] + one.at + 4) as i64 } else { 0 };
                    (ordinal(home), false, (address[home] + offset) as i64 + one.addend - beyond - place)
                }
            };
            if !addend_in_field {
                // The addend rides in an entry before the one it belongs to.
                if one.addend != 0 {
                    if !(-(1 << 23)..1 << 23).contains(&one.addend) {
                        return Err(unsupported(format!(
                            "{}: an addend of {} does not fit 24 bits",
                            section.name, one.addend
                        )));
                    }
                    let addend = (one.addend as u32) & 0x00FF_FFFF;
                    made.push((one.at, [one.at as u32, addend | length << 25 | RELOC_ADDEND << 28]));
                }
                made.push((
                    one.at,
                    [
                        one.at as u32,
                        symbolnum | u32::from(pcrel) << 24 | length << 25 | u32::from(external) << 27 | kind << 28,
                    ],
                ));
                continue;
            }
            flavor.backend().apply_fixup(&mut image, one.at, one.kind, pcrel, field, &section.name)?;
            made.push((
                one.at,
                [
                    one.at as u32,
                    symbolnum | u32::from(pcrel) << 24 | length << 25 | u32::from(external) << 27 | kind << 28,
                ],
            ));
        }
        // LLVM writes them last first.
        made.sort_by_key(|one| std::cmp::Reverse(one.0));
        let mut bytes = Vec::new();
        for (_, [address, info]) in made {
            put32(&mut bytes, address);
            put32(&mut bytes, info);
        }
        images.push(image);
        entries.push(bytes);
    }

    // Layout: header, load commands, section contents, relocations, symbols,
    // strings.
    let version = flavor.build_version();
    let commands = 72 + 80 * order.len() + 24 + 80 + if version.is_some() { 24 } else { 0 };
    // The section data follows the load commands, as `MachObjectWriter` places
    // it; with no build version the commands end 16-aligned.
    let first = 32 + commands;
    let mut offsets = vec![0usize; object.sections.len()];
    for &one in order.iter().take(file_backed) {
        offsets[one] = first + address[one];
    }
    // The section data is padded to 8 bytes, as `MachObjectWriter::writeObject`
    // does for a 64-bit file
    // (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/MC/MachObjectWriter.cpp#L914>).
    let mut at = align_up(first + file_size, 8);
    let mut reloc_offsets = vec![0usize; object.sections.len()];
    for &one in &order {
        if !entries[one].is_empty() {
            at = align_up(at, 4);
            reloc_offsets[one] = at;
            at += entries[one].len();
        }
    }
    let symbols_at = align_up(at, 8);
    let strings_at = symbols_at + nlist.len();

    let mut out = Vec::new();
    let (cpu_type, cpu_subtype) = flavor.cpu();
    for field in [
        MH_MAGIC_64,
        cpu_type,
        cpu_subtype,
        MH_OBJECT,
        if version.is_some() { 4 } else { 3 },
        commands as u32,
        if flavor.subsections_via_symbols() { MH_SUBSECTIONS_VIA_SYMBOLS } else { 0 },
        0,
    ] {
        put32(&mut out, field);
    }
    put32(&mut out, LC_SEGMENT_64);
    put32(&mut out, (72 + 80 * order.len()) as u32);
    name16(&mut out, "");
    for field in [0, vm_size as u64, first as u64, file_size as u64] {
        put64(&mut out, field);
    }
    for field in [7, 7, order.len() as u32, 0] {
        put32(&mut out, field);
    }
    for &one in &order {
        let (segment, name, flags) = spelled[one];
        name16(&mut out, name);
        name16(&mut out, segment);
        put64(&mut out, address[one] as u64);
        put64(&mut out, images[one].len() as u64);
        put32(&mut out, offsets[one] as u32);
        put32(&mut out, object.sections[one].align.max(1).next_power_of_two().trailing_zeros());
        put32(&mut out, reloc_offsets[one] as u32);
        put32(&mut out, (entries[one].len() / 8) as u32);
        for field in [flags, 0, 0, 0] {
            put32(&mut out, field);
        }
    }
    if let Some((platform, minos)) = version {
        // No tools, and no SDK: the version of the SDK is the build's, not the
        // object's.
        for field in [LC_BUILD_VERSION, 24, platform, minos, 0, 0] {
            put32(&mut out, field);
        }
    }
    put32(&mut out, LC_SYMTAB);
    put32(&mut out, 24);
    for field in [
        symbols_at as u32,
        (locals.len() + defined.len() + undefined.len()) as u32,
        strings_at as u32,
        strings.len() as u32,
    ] {
        put32(&mut out, field);
    }
    put32(&mut out, LC_DYSYMTAB);
    put32(&mut out, 80);
    let (nlocal, nextdef) = (locals.len() as u32, defined.len() as u32);
    for field in [0, nlocal, nlocal, nextdef, nlocal + nextdef, undefined.len() as u32] {
        put32(&mut out, field);
    }
    out.resize(out.len() + 72 - 24, 0);
    out.resize(first, 0);
    for &one in order.iter().take(file_backed) {
        out.resize(offsets[one], 0);
        out.extend(&images[one]);
    }
    for &one in &order {
        if !entries[one].is_empty() {
            out.resize(reloc_offsets[one], 0);
            out.extend(&entries[one]);
        }
    }
    out.resize(symbols_at, 0);
    out.extend(&nlist);
    out.extend(&strings);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::debug_name;

    /// The DWARF writer's `.debug_frame` and range sections had no Mach-O name,
    /// so `-g` on a function with frame rows or several ranges refused the
    /// whole object.
    #[test]
    fn every_dwarf_section_the_writer_makes_has_a_mach_o_name() {
        for (dwarf, macho) in [
            (".debug_frame", "__debug_frame"),
            (".debug_ranges", "__debug_ranges"),
            (".debug_rnglists", "__debug_rnglists"),
        ] {
            assert_eq!(debug_name(dwarf).unwrap(), macho);
        }
    }
}
