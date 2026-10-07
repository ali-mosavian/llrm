//! [`Info`] as CodeView C13: the `.debug$S` and `.debug$T` sections of a COFF object, the format
//! current MSVC and clang-cl emit. A fact C13 as written here cannot say is refused with what it
//! was, never dropped.

pub mod symbols;
mod types;

use std::collections::BTreeMap;

use llrm_object::debug::Info;
use llrm_object::{Arch, Object, Reloc, Unsupported};

/// `CV_SIGNATURE_C13`: what opens both sections.
const SIGNATURE: u32 = 4;

/// The register numbers a variable's location can name, from the target's description.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Registers {
    /// The register a frame location is relative to.
    pub frame: u16,
    /// CodeView's number of each register, by the target's name for it.
    pub numbers: BTreeMap<String, u16>,
}

/// A section's bytes and the relocations that fill its section and offset fields.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Section {
    pub image: Vec<u8>,
    pub relocs: Vec<Reloc>,
}

pub struct Encoded {
    pub symbols: Section,
    pub types: Section,
}

pub(crate) fn refused<T>(what: impl std::fmt::Display) -> Result<T, Unsupported> {
    Err(Unsupported(format!("CodeView: {what}")))
}

pub(crate) fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_le_bytes());
}

pub(crate) fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_le_bytes());
}

pub(crate) fn name(out: &mut Vec<u8>, text: &str) {
    out.extend(text.as_bytes());
    out.push(0);
}

/// A record: its length (everything after the field), its kind and its data, the whole padded to
/// four bytes. `pad` is the filler: zeros in symbols, `LF_PAD` in types.
pub(crate) fn record(out: &mut Vec<u8>, kind: u16, data: &[u8], pad: fn(usize) -> u8) {
    let missing = (4 - (4 + data.len()) % 4) % 4;
    put16(out, (2 + data.len() + missing) as u16);
    put16(out, kind);
    out.extend(data);
    out.extend((0..missing).map(|left| pad(missing - left)));
}

/// `object`'s debug sections.
pub fn encode(object: &Object, info: &Info, registers: &Registers) -> Result<Encoded, Unsupported> {
    let types = types::encode(object, info)?;
    let symbols = symbols::encode(object, info, registers, &types)?;
    let mut image = Vec::new();
    put32(&mut image, SIGNATURE);
    image.extend(types.records);
    Ok(Encoded { symbols, types: Section { image, relocs: Vec::new() } })
}

/// CodeView's machine number, for `S_COMPILE3`.
pub(crate) fn machine(arch: Arch) -> Result<u16, Unsupported> {
    match arch {
        Arch::I386 => Ok(0x03),
        Arch::X8664 => Ok(0xD0),
        other => refused(format!("{other:?} has no C13 machine")),
    }
}
