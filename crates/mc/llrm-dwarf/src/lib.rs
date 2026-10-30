//! [`llrm_debug::Info`] as DWARF 4 or 5: `.debug_abbrev`,
//! `.debug_info`, `.debug_str`, `.debug_line` (and `.debug_line_str` in 5),
//! `.debug_aranges`. A format of object file calls [`expanded`] and writes the
//! result as it writes any object: the sections are ordinary ones, their cross
//! references relocations against another section (`Target::Section`) and an
//! address one against a symbol of the code's section.
//!
//! A fact DWARF as written here cannot say is refused by name, never dropped.

mod buffer;
mod die;
pub mod eh_frame;

/// DWARF says where a value is over a range of code and places a frame cell
/// from the canonical frame address.
pub const CAPS: llrm_debug::DebugCaps = llrm_debug::DebugCaps { location_ranges: true, cfa_locations: true };
mod frame;
mod line;
mod ranges;
mod types;

pub use buffer::{sleb, uleb};
use llrm_debug::{Format, Info};
use llrm_object::{DebugSection, Definition, Fix, FixupKind, Object, Reloc, Role, Section, Target, Unsupported};

/// The version DWARF is written in when the format does not say.
pub const DEFAULT_VERSION: u16 = 5;

fn refused<T>(what: impl std::fmt::Display) -> Result<T, Unsupported> {
    Err(Unsupported(format!("DWARF: {what}")))
}

/// Where in `object` a section's address is: a symbol defined in it, and the
/// symbol's offset.
pub(crate) fn anchor(
    object: &Object,
    section: usize,
) -> Result<(usize, usize), Unsupported> {
    object
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, one)| match one.definition {
            Definition::Defined { section: at, offset } if at == section => Some((index, offset)),
            _ => None,
        })
        .ok_or_else(|| {
            Unsupported(format!("DWARF: section {} has no symbol to address it by", object.sections[section].name))
        })
}

/// `object` with its debug information written as DWARF sections, and `debug`
/// cleared.
pub fn expanded(
    object: &Object,
    info: &Info,
) -> Result<Object, Unsupported> {
    expanded_for(object, info, true)
}

/// [`expanded`] for a format that does (`relocs_across_sections`: ELF, COFF,
/// OMF) or does not (Mach-O) relocate one debug section's offset into another
/// (`MCAsmInfo::doesDwarfUseRelocationsAcrossSections`): without, the offset
/// itself is in the field, as dsymutil reads the object's own sections.
pub fn expanded_for(
    object: &Object,
    info: &Info,
    relocs_across_sections: bool,
) -> Result<Object, Unsupported> {
    let version = match info.options.format {
        Format::Default => DEFAULT_VERSION,
        Format::Dwarf { version } if matches!(version, 2..=5) => version,
        Format::Dwarf { version } => return refused(format!("version {version} is not written: 2 to 5 are")),
        Format::CodeView => return refused("this object format cannot carry CodeView"),
        Format::TurboDebugger => return refused("this object format cannot carry Turbo Debugger's information"),
    };
    let address = u32::from(info.target_of(object.bits).address_bytes);
    let base = object.sections.len();
    // The sections' places: abbrev, str, line_str (5), line, info, aranges.
    let mut names = vec![".debug_abbrev", ".debug_str"];
    if version >= 5 {
        names.push(".debug_line_str");
    }
    names.extend([".debug_line", ".debug_info", ".debug_aranges"]);
    // The section of location lists, where any variable has one.
    let lists = info
        .globals
        .iter()
        .chain(info.functions.iter().flat_map(|one| one.variables.iter().chain(one.blocks.iter().flat_map(blocks))))
        .any(|one| {
            matches!(
                &one.location,
                llrm_debug::Location::List(entries) if !entries.is_empty()
            )
        });
    if lists {
        names.push(if version >= 5 { ".debug_loclists" } else { ".debug_loc" });
    }
    // The section of ranges, where a DIE has several.
    let several = info.code.len() > 1
        || info.functions.iter().any(|one| one.ranges.len() > 1 || (address != 2 && one.blocks.iter().any(many)));
    if several {
        names.push(if version >= 5 { ".debug_rnglists" } else { ".debug_ranges" });
    }
    let frame = frame::section(object, info, address as usize)?;
    if frame.is_some() {
        names.push(".debug_frame");
    }
    let at = |name: &str| base + names.iter().position(|one| *one == name).expect("a section of this version");
    let places = die::Places {
        abbrev: at(".debug_abbrev"),
        strings: at(".debug_str"),
        line: at(".debug_line"),
        info: at(".debug_info"),
        line_strings: (version >= 5).then(|| at(".debug_line_str")),
        locations: lists.then(|| at(if version >= 5 { ".debug_loclists" } else { ".debug_loc" })),
        ranges: several.then(|| at(if version >= 5 { ".debug_rnglists" } else { ".debug_ranges" })),
    };
    let mut out = die::Out::new(version, address as u8, places);
    out.segment = info.target_of(object.bits).segment_base;
    let line = line::program(object, info, &mut out)?;
    let (info_part, abbrev, locations, ranges) = die::unit(object, info, &mut out)?;
    let aranges = die::aranges(object, info, &out)?;
    let mut sections = Vec::new();
    for name in &names {
        let part = match *name {
            ".debug_abbrev" => abbrev.clone(),
            ".debug_str" => out.strings.part(),
            ".debug_line_str" => out.line_strings.part(),
            ".debug_line" => line.clone(),
            ".debug_info" => info_part.clone(),
            ".debug_loclists" | ".debug_loc" => locations.clone(),
            ".debug_rnglists" | ".debug_ranges" => ranges.clone().expect("named only where a DIE has several ranges"),
            ".debug_frame" => frame.clone().expect("named only where there is one"),
            _ => aranges.clone(),
        };
        sections.push(section(name, part));
    }
    let mut made = object.clone();
    made.sections.extend(sections);
    if !relocs_across_sections {
        place_section_offsets(&mut made.sections[base..], base);
    }
    Ok(made)
}

/// Writes the offset a relocation of a debug section names in another debug
/// section (`first` is the index of the first) into the field, and drops it.
fn place_section_offsets(
    sections: &mut [llrm_object::Section],
    first: usize,
) {
    for section in sections {
        let (placed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut section.relocs)
            .into_iter()
            .partition(
                |one| matches!(one.target, Target::Section(home) if home >= first)
                    && !one.pc_rel
                    && one.kind == llrm_object::FixupKind::DATA_4,
            );
        for one in &placed {
            section.image[one.at..one.at + 4].copy_from_slice(&(one.addend as u32).to_le_bytes());
        }
        section.relocs = kept;
    }
}

/// A block, or one in it, of several code ranges.
fn many(block: &llrm_debug::Block) -> bool {
    block.ranges.len() > 1 || block.blocks.iter().any(many)
}

/// A block's variables and those of the blocks in it.
fn blocks(block: &llrm_debug::Block) -> Box<dyn Iterator<Item = &llrm_debug::Variable> + '_> {
    Box::new(block.variables.iter().chain(block.blocks.iter().flat_map(blocks)))
}

fn section(
    name: &str,
    part: buffer::Done,
) -> Section {
    let spans = if part.bytes.is_empty() { Vec::new() } else { vec![[0, part.bytes.len()]] };
    Section {
        name: name.to_owned(),
        role: Role::Debug(DebugSection::Dwarf),
        near: true,
        align: 1,
        image: part.bytes,
        spans,
        relocs: part.relocs,
    }
}

pub(crate) fn symbol_reloc(
    at: usize,
    width: usize,
    symbol: usize,
    addend: i64,
) -> Reloc {
    Reloc::new(at, Fix::abs(width), Target::Symbol(symbol), addend)
}

/// A segment base: the selector of the segment `section` is in, which the
/// linker places (a 16-bit program's addresses are a segment and an offset).
pub(crate) fn segment_reloc(
    kind: FixupKind,
    at: usize,
    section: usize,
) -> Reloc {
    Reloc::new(at, Fix::plain(kind), Target::Section(section), 0)
}

/// The kind of fixup that names a segment's selector: the target's to say.
pub(crate) fn segment_kind(
    object: &Object,
    info: &Info,
) -> Result<FixupKind, Unsupported> {
    match info.target_of(object.bits).segment_base {
        Some(kind) => Ok(kind),
        None => refused("a segmented address needs the target's segment fixup"),
    }
}

pub(crate) fn section_reloc(
    at: usize,
    section: usize,
    addend: i64,
) -> Reloc {
    Reloc::new(at, Fix::abs(4), Target::Section(section), addend)
}

#[cfg(test)]
mod tests;
