//! [`Info`] as CodeView C13: the `.debug$S` and `.debug$T` sections of a COFF
//! object (`expanded` adds them), the format current MSVC and clang-cl emit. A
//! fact C13 as written here cannot say is refused with what it was, never
//! dropped.

pub mod symbols;
mod types;

use llrm_debug::Info;
use llrm_object::{Object, Reloc, Unsupported};

/// `CV_SIGNATURE_C13`: what opens both sections.
const SIGNATURE: u32 = 4;

/// The register numbers a variable's location can name: the model's own, by the
/// target's name.
pub(crate) struct Registers<'a> {
    info: &'a Info,
}

impl Registers<'_> {
    pub(crate) fn number(
        &self,
        register: &str,
    ) -> Result<u16, Unsupported> {
        match self.info.registers().named(register) {
            Some(one) => {
                one.codeview.map_or_else(|| refused(format!("register {register} has no CodeView number")), Ok)
            }
            None => refused(format!("register {register} is not in the target's register file")),
        }
    }

    /// The register a frame location is relative to.
    pub(crate) fn frame(&self) -> Result<u16, Unsupported> {
        self.number(&self.info.registers().frame)
    }
}

/// A section's bytes and the relocations that fill its section and offset
/// fields.
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

pub(crate) fn put16(
    out: &mut Vec<u8>,
    value: u16,
) {
    out.extend(value.to_le_bytes());
}

pub(crate) fn put32(
    out: &mut Vec<u8>,
    value: u32,
) {
    out.extend(value.to_le_bytes());
}

pub(crate) fn name(
    out: &mut Vec<u8>,
    text: &str,
) {
    out.extend(text.as_bytes());
    out.push(0);
}

/// A record: its length (everything after the field), its kind and its data,
/// the whole padded to four bytes. `pad` is the filler: zeros in symbols,
/// `LF_PAD` in types.
pub(crate) fn record(
    out: &mut Vec<u8>,
    kind: u16,
    data: &[u8],
    pad: fn(usize) -> u8,
) -> Result<(), Unsupported> {
    let missing = (4 - (4 + data.len()) % 4) % 4;
    let length = u16::try_from(2 + data.len() + missing).or_else(|_| {
        refused(format!("a record of kind {kind:#x} is {} bytes, which its 16-bit length cannot say", data.len()))
    })?;
    put16(out, length);
    put16(out, kind);
    out.extend(data);
    out.extend((0..missing).map(|left| pad(missing - left)));
    Ok(())
}

/// `object` with its debug information as C13 sections, the information itself
/// dropped.
pub fn expanded(
    object: &Object,
    info: &llrm_debug::Info,
    machine: Option<u16>,
) -> Result<Object, Unsupported> {
    let encoded = encode(object, info, machine)?;
    let section = |name: &str, one: Section| llrm_object::Section {
        name: name.to_owned(),
        role: llrm_object::Role::Debug(if name == ".debug$S" {
            llrm_object::DebugSection::CodeViewSymbols
        } else {
            llrm_object::DebugSection::CodeViewTypes
        }),
        near: true,
        align: 1,
        spans: vec![[0, one.image.len()]],
        image: one.image,
        relocs: one.relocs,
    };
    let mut sections = object.sections.clone();
    sections.push(section(".debug$S", encoded.symbols));
    sections.push(section(".debug$T", encoded.types));
    Ok(Object { sections, ..object.clone() })
}

/// `object`'s debug sections.
fn encode(
    object: &Object,
    info: &Info,
    machine: Option<u16>,
) -> Result<Encoded, Unsupported> {
    let types = types::encode(object, info)?;
    let symbols = symbols::encode(object, info, machine, &Registers { info }, &types)?;
    let mut image = Vec::new();
    put32(&mut image, SIGNATURE);
    image.extend(types.records);
    Ok(Encoded { symbols, types: Section { image, relocs: Vec::new() } })
}

/// CodeView's machine number, for `S_COMPILE3`: the debug information's own,
/// else the target's.
pub(crate) fn machine(
    object: &Object,
    info: &Info,
    target: Option<u16>,
) -> Result<u16, Unsupported> {
    match info.target_of(object.bits).codeview_machine.or(target) {
        Some(number) => Ok(number),
        None => refused(format!("a {}-bit object has no C13 machine", object.bits)),
    }
}
