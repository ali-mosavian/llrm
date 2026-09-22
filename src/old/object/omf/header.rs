//! Typed OMF module-header decoding.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const THEADR: u8 = 0x80;
const LHEADR: u8 = 0x82;

/// The identity declared by one THEADR or LHEADR record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleHeader {
    pub record_index: usize,
    pub record_offset: Option<usize>,
    pub name: Vec<u8>,
    pub kind: HeaderKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderKind {
    Translator,
    Library,
}

/// Decodes the optional, unique module header in `records`.
pub fn parse(records: &[Record]) -> Result<Option<ModuleHeader>, HeaderError> {
    let mut header = None;
    for (record_index, record) in records.iter().enumerate() {
        let kind = match record.record_type() {
            THEADR => HeaderKind::Translator,
            LHEADR => HeaderKind::Library,
            _ => continue,
        };
        if header.is_some() {
            return Err(HeaderError::Duplicate {
                record_offset: record.offset(),
                record_type: record.record_type(),
            });
        }

        let mut reader = Reader::new(
            record.body(),
            record.offset().and_then(|offset| offset.checked_add(3)),
        );
        let name = reader
            .read_name()
            .map_err(|source| HeaderError::Read {
                record_offset: record.offset(),
                record_type: record.record_type(),
                source,
            })?
            .to_vec();
        if !reader.is_empty() {
            return Err(HeaderError::TrailingData {
                record_offset: record.offset(),
                record_type: record.record_type(),
                body_offset: reader.position(),
                remaining: reader.remaining(),
            });
        }

        header = Some(ModuleHeader {
            record_index,
            record_offset: record.offset(),
            name,
            kind,
        });
    }
    Ok(header)
}

/// A malformed or ambiguous OMF module header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderError {
    Read {
        record_offset: Option<usize>,
        record_type: u8,
        source: ReadError,
    },
    TrailingData {
        record_offset: Option<usize>,
        record_type: u8,
        body_offset: usize,
        remaining: usize,
    },
    Duplicate {
        record_offset: Option<usize>,
        record_type: u8,
    },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (record_offset, record_type) = match self {
            Self::Read {
                record_offset,
                record_type,
                ..
            }
            | Self::TrailingData {
                record_offset,
                record_type,
                ..
            }
            | Self::Duplicate {
                record_offset,
                record_type,
            } => (record_offset, record_type),
        };
        write!(formatter, "OMF module header type {record_type:02x}")?;
        if let Some(offset) = record_offset {
            write!(formatter, " at byte {offset}")?;
        }
        match self {
            Self::Read { source, .. } => write!(formatter, ": {source}"),
            Self::TrailingData {
                body_offset,
                remaining,
                ..
            } => write!(
                formatter,
                ": {remaining} trailing byte(s) at body byte {body_offset}"
            ),
            Self::Duplicate { .. } => write!(formatter, ": duplicate module header"),
        }
    }
}

impl std::error::Error for HeaderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::TrailingData { .. } | Self::Duplicate { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HeaderError, HeaderKind, ModuleHeader, parse};
    use crate::old::object::omf::record::Record;

    #[test]
    fn decodes_raw_translator_and_library_names() {
        let translator = Record::new(0x80, vec![2, 0xff, b'x']).unwrap();
        assert_eq!(
            parse(&[translator]),
            Ok(Some(ModuleHeader {
                record_index: 0,
                record_offset: None,
                name: vec![0xff, b'x'],
                kind: HeaderKind::Translator,
            }))
        );

        let library = Record::new(0x82, vec![3, b'l', b'i', b'b']).unwrap();
        assert_eq!(
            parse(&[library]).unwrap().unwrap().kind,
            HeaderKind::Library
        );
    }

    #[test]
    fn rejects_trailing_and_duplicate_header_data() {
        let trailing = Record::new(0x80, vec![1, b'm', 0]).unwrap();
        assert!(matches!(
            parse(&[trailing]),
            Err(HeaderError::TrailingData { remaining: 1, .. })
        ));

        let headers = [
            Record::new(0x80, vec![1, b'a']).unwrap(),
            Record::new(0x82, vec![1, b'b']).unwrap(),
        ];
        assert!(matches!(
            parse(&headers),
            Err(HeaderError::Duplicate {
                record_type: 0x82,
                ..
            })
        ));
    }
}
