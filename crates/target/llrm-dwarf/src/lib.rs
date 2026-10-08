//! [`llrm_object::debug::Info`] as DWARF 4 or 5: `.debug_abbrev`, `.debug_info`, `.debug_str`,
//! `.debug_line` (and `.debug_line_str` in 5), `.debug_aranges`. A format of object file calls
//! [`expanded`] and writes the result as it writes any object: the sections are ordinary ones,
//! their cross references relocations against another section (`Target::Section`) and an address
//! one against a symbol of the code's section.
//!
//! A fact DWARF as written here cannot say is refused by name, never dropped.

mod buffer;
mod die;
mod frame;
mod line;
mod types;

use llrm_object::debug::{Format, Info};
use llrm_object::{Definition, Object, Reloc, Role, Section, Target, Unsupported};

pub use buffer::{sleb, uleb};

/// The version DWARF is written in when the format does not say.
pub const DEFAULT_VERSION: u16 = 5;

fn refused<T>(what: impl std::fmt::Display) -> Result<T, Unsupported> {
    Err(Unsupported(format!("DWARF: {what}")))
}

/// Where in `object` a section's address is: a symbol defined in it, and the symbol's offset.
pub(crate) fn anchor(object: &Object, section: usize) -> Result<(usize, usize), Unsupported> {
    object
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, one)| match one.definition {
            Definition::Defined { section: at, offset } if at == section => Some((index, offset)),
            _ => None,
        })
        .ok_or_else(|| Unsupported(format!("DWARF: section {} has no symbol to address it by", object.sections[section].name)))
}

/// `object` with its debug information written as DWARF sections, and `debug` cleared.
pub fn expanded(object: &Object, info: &Info) -> Result<Object, Unsupported> {
    let version = match info.format {
        Format::Default => DEFAULT_VERSION,
        Format::Dwarf { version } if matches!(version, 4 | 5) => version,
        Format::Dwarf { version } => return refused(format!("version {version} is not written: 4 and 5 are")),
        Format::CodeView => return refused("this object format cannot carry CodeView"),
        Format::TurboDebugger => return refused("this object format cannot carry Turbo Debugger's information"),
    };
    let address = object.arch.bits() / 8;
    let base = object.sections.len();
    // The sections' places: abbrev, str, line_str (5), line, info, aranges.
    let mut names = vec![".debug_abbrev", ".debug_str"];
    if version >= 5 {
        names.push(".debug_line_str");
    }
    names.extend([".debug_line", ".debug_info", ".debug_aranges"]);
    // The section of location lists, where any variable has one.
    let lists = info.globals.iter().chain(info.functions.iter().flat_map(|one| one.variables.iter().chain(one.blocks.iter().flat_map(blocks)))).any(|one| matches!(&one.location, llrm_object::debug::Location::List(entries) if !entries.is_empty()));
    if lists {
        names.push(if version >= 5 { ".debug_loclists" } else { ".debug_loc" });
    }
    let frame = frame::section(object, info, address as usize)?;
    if frame.is_some() {
        names.push(".debug_frame");
    }
    let at = |name: &str| base + names.iter().position(|one| *one == name).expect("a section of this version");
    let places = die::Places { abbrev: at(".debug_abbrev"), strings: at(".debug_str"), line: at(".debug_line"), info: at(".debug_info"), line_strings: (version >= 5).then(|| at(".debug_line_str")), locations: lists.then(|| at(if version >= 5 { ".debug_loclists" } else { ".debug_loc" })) };
    let mut out = die::Out::new(version, address as u8, places);
    let line = line::program(object, info, &mut out)?;
    let (info_part, abbrev, locations) = die::unit(object, info, &mut out)?;
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
            ".debug_frame" => frame.clone().expect("named only where there is one"),
            _ => aranges.clone(),
        };
        sections.push(section(name, part));
    }
    let mut made = Object { debug: None, ..object.clone() };
    made.sections.extend(sections);
    Ok(made)
}

/// A block's variables and those of the blocks in it.
fn blocks(block: &llrm_object::debug::Block) -> Box<dyn Iterator<Item = &llrm_object::debug::Variable> + '_> {
    Box::new(block.variables.iter().chain(block.blocks.iter().flat_map(blocks)))
}

fn section(name: &str, part: buffer::Done) -> Section {
    let spans = if part.bytes.is_empty() { Vec::new() } else { vec![[0, part.bytes.len()]] };
    Section { name: name.to_owned(), role: Role::Debug, near: true, align: 1, image: part.bytes, spans, relocs: part.relocs }
}

pub(crate) fn symbol_reloc(at: usize, width: usize, symbol: usize, addend: i64) -> Reloc {
    Reloc { at, kind: llrm_object::Kind::Abs { width }, target: Target::Symbol(symbol), addend }
}

pub(crate) fn section_reloc(at: usize, section: usize, addend: i64) -> Reloc {
    Reloc { at, kind: llrm_object::Kind::Abs { width: 4 }, target: Target::Section(section), addend }
}

#[cfg(test)]
mod tests;
