//! Coordinated semantic decoding of one OMF object module.

use std::fmt;

use crate::support::PhysicalRegister;

use super::data::{self, DataBlock, DataError};
use super::declarations::{self, DeclarationError, Declarations};
use super::fixups::{self, Fixup, FixupError};
use super::header::{self, HeaderError, ModuleHeader};
use super::lines::{self, LineError, LineTable};
use super::modend::{self, ModendError, ModuleEnd};
use super::record::Record;
use super::segments::{self, SegmentError, SegmentTable};
use super::symbols::{self, SymbolError, SymbolTables};

/// The address namespace used by Python's object/MIR/LIR pipeline.
///
/// Direct port of `qbopt.objectfile.module:Space`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Space {
    Segment,
    External,
    Frame,
    Literal,
    Group,
    Far,
    Stack,
}

impl Space {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Segment => "seg",
            Self::External => "external",
            Self::Frame => "bp",
            Self::Literal => "abs",
            Self::Group => "grp",
            Self::Far => "far",
            Self::Stack => "sp",
        }
    }
}

impl fmt::Display for Space {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The direct-port sentinel for Python's `Register.NONE` in `Addr`.
pub const NO_REGISTER: PhysicalRegister = PhysicalRegister::new(0);

/// A symbolic or concrete machine address.
///
/// Direct port of `qbopt.objectfile.module:Addr`. All five fields are part of
/// its identity, including `base` and `segment`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Addr {
    pub space: Space,
    pub disp: i64,
    pub index: u32,
    pub base: PhysicalRegister,
    pub segment: PhysicalRegister,
}

impl Addr {
    pub const fn new(space: Space, disp: i64) -> Self {
        Self {
            space,
            disp,
            index: 0,
            base: NO_REGISTER,
            segment: NO_REGISTER,
        }
    }

    /// Python `Addr.direct`: whether this address has no run-time base.
    pub fn direct(self) -> bool {
        self.base == NO_REGISTER
    }

    /// Python `Addr.plus`: retain address identity while moving its displacement.
    pub const fn plus(self, bytes_along: i64) -> Self {
        Self {
            disp: self.disp + bytes_along,
            ..self
        }
    }
}

/// Python `qbopt.objectfile.module:frame_relative`.
#[must_use]
pub const fn frame_relative(literal: i64) -> Addr {
    Addr::new(Space::Frame, literal)
}

/// Python `qbopt.objectfile.module:far_pointer`.
#[must_use]
pub const fn far_pointer(literal: i64, base: PhysicalRegister, segment: PhysicalRegister) -> Addr {
    Addr {
        space: Space::Far,
        disp: literal,
        index: 0,
        base,
        segment,
    }
}

/// Python `qbopt.objectfile.module:literal_only`.
///
/// `literal` is a raw decoded displacement. OMF's 16-bit source forms keep it
/// within the signed address representation used by [`Addr`].
#[must_use]
pub const fn literal_only(_field_offset: usize, literal: u64) -> Addr {
    Addr::new(Space::Literal, literal as i64)
}

/// The independently decoded tables and relocations of one object module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedModule<'a> {
    pub header: Option<ModuleHeader>,
    pub symbols: SymbolTables,
    pub segments: SegmentTable,
    pub declarations: Declarations,
    pub data: Vec<DataBlock<'a>>,
    pub lines: Vec<LineTable>,
    pub fixups: Vec<Fixup>,
    pub module_ends: Vec<ModuleEnd>,
}

impl<'a> DecodedModule<'a> {
    pub fn parse(records: &'a [Record]) -> Result<Self, ModuleError> {
        Ok(Self {
            header: header::parse(records).map_err(ModuleError::Header)?,
            symbols: symbols::parse(records).map_err(ModuleError::Symbols)?,
            segments: segments::parse(records).map_err(ModuleError::Segments)?,
            declarations: declarations::parse(records).map_err(ModuleError::Declarations)?,
            data: data::parse(records).map_err(ModuleError::Data)?,
            lines: lines::parse(records).map_err(ModuleError::Lines)?,
            fixups: fixups::parse(records).map_err(ModuleError::Fixups)?,
            module_ends: modend::parse(records).map_err(ModuleError::ModuleEnd)?,
        })
    }
}

/// A malformed record family in an OMF object module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleError {
    Header(HeaderError),
    Symbols(SymbolError),
    Segments(SegmentError),
    Declarations(DeclarationError),
    Data(DataError),
    Lines(LineError),
    Fixups(FixupError),
    ModuleEnd(ModendError),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(error) => error.fmt(formatter),
            Self::Symbols(error) => error.fmt(formatter),
            Self::Segments(error) => error.fmt(formatter),
            Self::Declarations(error) => error.fmt(formatter),
            Self::Data(error) => error.fmt(formatter),
            Self::Lines(error) => error.fmt(formatter),
            Self::Fixups(error) => error.fmt(formatter),
            Self::ModuleEnd(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ModuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Header(error) => Some(error),
            Self::Symbols(error) => Some(error),
            Self::Segments(error) => Some(error),
            Self::Declarations(error) => Some(error),
            Self::Data(error) => Some(error),
            Self::Lines(error) => Some(error),
            Self::Fixups(error) => Some(error),
            Self::ModuleEnd(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Addr, DecodedModule, Space, far_pointer, frame_relative, literal_only};
    use crate::object::omf::fixups::{FrameMethod, TargetMethod};
    use crate::object::omf::record::Record;
    use crate::target::x86::X86Register;

    #[test]
    fn omf_lift_classify_module_address_helpers_match_python() {
        assert_eq!(frame_relative(-24), Addr::new(Space::Frame, -24));
        assert_eq!(literal_only(7, 0x1234), Addr::new(Space::Literal, 0x1234));
        assert_eq!(
            far_pointer(8, X86Register::Bx.physical(), X86Register::Es.physical(),),
            Addr {
                space: Space::Far,
                disp: 8,
                index: 0,
                base: X86Register::Bx.physical(),
                segment: X86Register::Es.physical(),
            }
        );
    }

    #[test]
    fn decodes_connected_tables_data_and_relocations() {
        let records = [
            Record::new(0x96, vec![4, b'c', b'o', b'd', b'e']).unwrap(),
            Record::new(0x98, vec![0x68, 2, 0, 1, 0, 0]).unwrap(),
            Record::new(0xa0, vec![1, 0, 0, 0, 0]).unwrap(),
            Record::new(0x9c, vec![0x84, 0, 0x44, 1]).unwrap(),
        ];

        let module = DecodedModule::parse(&records).unwrap();

        assert_eq!(module.symbols.names[1], b"code");
        assert_eq!(module.segments.segments[1].as_ref().unwrap().length, 2);
        assert_eq!(module.data[0].bytes, [0, 0]);
        assert_eq!(module.fixups[0].frame.method, FrameMethod::Location);
        assert_eq!(module.fixups[0].target.method, TargetMethod::Segment);
        assert_eq!(module.fixups[0].target.datum, 1);
    }
}
