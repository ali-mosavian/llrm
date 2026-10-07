//! [`Info`] as CodeView C13: the `.debug$S` and `.debug$T` sections of a COFF object, the format
//! current MSVC and clang-cl emit. A fact C13 as written here cannot say is refused with what it
//! was, never dropped.

pub mod symbols;
mod types;

use llrm_object::debug::Info;
use llrm_object::{Arch, Object, Reloc, Unsupported};

/// `CV_SIGNATURE_C13`: what opens both sections.
const SIGNATURE: u32 = 4;

/// The register numbers a variable's location can name: the model's own, by the target's name.
pub(crate) struct Registers<'a> {
    info: &'a Info,
}

impl Registers<'_> {
    pub(crate) fn number(&self, register: &str) -> Result<u16, Unsupported> {
        match self.info.registers.iter().find(|one| one.name == register) {
            Some(one) => one.codeview.map_or_else(|| refused(format!("register {register} has no CodeView number")), Ok),
            None => refused(format!("register {register} is not in the target's register file")),
        }
    }

    /// The register a frame location is relative to.
    pub(crate) fn frame(&self) -> Result<u16, Unsupported> {
        self.number(&self.info.frame_register)
    }
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
pub(crate) fn record(out: &mut Vec<u8>, kind: u16, data: &[u8], pad: fn(usize) -> u8) -> Result<(), Unsupported> {
    let missing = (4 - (4 + data.len()) % 4) % 4;
    let length = u16::try_from(2 + data.len() + missing).or_else(|_| refused(format!("a record of kind {kind:#x} is {} bytes, which its 16-bit length cannot say", data.len())))?;
    put16(out, length);
    put16(out, kind);
    out.extend(data);
    out.extend((0..missing).map(|left| pad(missing - left)));
    Ok(())
}

/// `object`'s debug sections.
pub fn encode(object: &Object, info: &Info) -> Result<Encoded, Unsupported> {
    let types = types::encode(object, info)?;
    let symbols = symbols::encode(object, info, &Registers { info }, &types)?;
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
