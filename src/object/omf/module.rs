//! Coordinated semantic decoding of one OMF object module.

use std::fmt;

use super::data::{self, DataBlock, DataError};
use super::declarations::{self, DeclarationError, Declarations};
use super::fixups::{self, Fixup, FixupError};
use super::lines::{self, LineError, LineTable};
use super::record::Record;
use super::segments::{self, SegmentError, SegmentTable};
use super::symbols::{self, SymbolError, SymbolTables};

/// The independently decoded tables and relocations of one object module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedModule<'a> {
    pub symbols: SymbolTables,
    pub segments: SegmentTable,
    pub declarations: Declarations,
    pub data: Vec<DataBlock<'a>>,
    pub lines: Vec<LineTable>,
    pub fixups: Vec<Fixup>,
}

impl<'a> DecodedModule<'a> {
    pub fn parse(records: &'a [Record]) -> Result<Self, ModuleError> {
        Ok(Self {
            symbols: symbols::parse(records).map_err(ModuleError::Symbols)?,
            segments: segments::parse(records).map_err(ModuleError::Segments)?,
            declarations: declarations::parse(records).map_err(ModuleError::Declarations)?,
            data: data::parse(records).map_err(ModuleError::Data)?,
            lines: lines::parse(records).map_err(ModuleError::Lines)?,
            fixups: fixups::parse(records).map_err(ModuleError::Fixups)?,
        })
    }
}

/// A malformed record family in an OMF object module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleError {
    Symbols(SymbolError),
    Segments(SegmentError),
    Declarations(DeclarationError),
    Data(DataError),
    Lines(LineError),
    Fixups(FixupError),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symbols(error) => error.fmt(formatter),
            Self::Segments(error) => error.fmt(formatter),
            Self::Declarations(error) => error.fmt(formatter),
            Self::Data(error) => error.fmt(formatter),
            Self::Lines(error) => error.fmt(formatter),
            Self::Fixups(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ModuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Symbols(error) => Some(error),
            Self::Segments(error) => Some(error),
            Self::Declarations(error) => Some(error),
            Self::Data(error) => Some(error),
            Self::Lines(error) => Some(error),
            Self::Fixups(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DecodedModule;
    use crate::object::omf::fixups::{FrameMethod, TargetMethod};
    use crate::object::omf::record::Record;

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
