//! Typed OMF name and external-symbol tables.
//!
//! OMF refers to LNAMES and EXTDEF entries by one-based index.  This module
//! preserves that layout directly: slot zero in each table is a sentinel and
//! all names remain raw bytes, since OMF does not require UTF-8.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const LNAMES: u8 = 0x96;
const EXTDEF: u8 = 0x8c;
const EXTDEF32: u8 = 0x8d;

/// One external declaration from an EXTDEF record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct External {
    /// The external's raw OMF name bytes.
    pub name: Vec<u8>,
    /// The declaration's one- or two-byte OMF type index.
    pub type_index: u16,
}

/// The one-based LNAMES and EXTDEF tables from an OMF record stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolTables {
    /// LNAMES entries in record-stream order; index zero is an empty sentinel.
    pub names: Vec<Vec<u8>>,
    /// EXTDEF entries in record-stream order; index zero is the `None` sentinel.
    pub externals: Vec<Option<External>>,
}

impl SymbolTables {
    /// Decodes LNAMES and EXTDEF records from `records`.
    pub fn parse(records: &[Record]) -> Result<Self, SymbolError> {
        parse(records)
    }
}

/// Decodes one-based LNAMES and EXTDEF tables from an OMF record stream.
///
/// Only LNAMES (0x96) is a name-table record. EXTDEF's 32-bit twin (0x8d)
/// carries the same name and type-index entry form as EXTDEF (0x8c), so both
/// contribute to the external table. No broad low-bit matching is used: the
/// other OMF records with adjacent type numbers have different layouts.
pub fn parse(records: &[Record]) -> Result<SymbolTables, SymbolError> {
    let mut tables = SymbolTables {
        names: vec![Vec::new()],
        externals: vec![None],
    };

    for record in records {
        match record.record_type() {
            LNAMES => read_names(record, &mut tables.names)?,
            EXTDEF | EXTDEF32 => read_externals(record, &mut tables.externals)?,
            _ => {}
        }
    }

    Ok(tables)
}

fn read_names(record: &Record, names: &mut Vec<Vec<u8>>) -> Result<(), SymbolError> {
    let mut reader = reader_for(record);
    while !reader.is_empty() {
        names.push(read_name(record, &mut reader)?);
    }
    Ok(())
}

fn read_externals(
    record: &Record,
    externals: &mut Vec<Option<External>>,
) -> Result<(), SymbolError> {
    let mut reader = reader_for(record);
    while !reader.is_empty() {
        let name = read_name(record, &mut reader)?;
        let type_index = reader.read_index().map_err(|source| SymbolError::Read {
            record_offset: record.offset(),
            record_type: record.record_type(),
            source,
        })?;
        externals.push(Some(External { name, type_index }));
    }
    Ok(())
}

fn reader_for(record: &Record) -> Reader<'_> {
    let body_offset = record.offset().and_then(|offset| offset.checked_add(3));
    Reader::new(record.body(), body_offset)
}

fn read_name(record: &Record, reader: &mut Reader<'_>) -> Result<Vec<u8>, SymbolError> {
    reader
        .read_name()
        .map(|name| name.to_vec())
        .map_err(|source| SymbolError::Read {
            record_offset: record.offset(),
            record_type: record.record_type(),
            source,
        })
}

/// A malformed primitive field in a symbol-table record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolError {
    /// A LNAMES or EXTDEF record ended in the middle of one of its entries.
    Read {
        /// Byte offset of the record header in the source stream, if known.
        record_offset: Option<usize>,
        /// The malformed record's raw OMF type.
        record_type: u8,
        /// The bounded primitive read which failed.
        source: ReadError,
    },
}

impl fmt::Display for SymbolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read {
                record_offset,
                record_type,
                source,
            } => {
                write!(formatter, "OMF symbol record type {record_type:02x}")?;
                if let Some(record_offset) = record_offset {
                    write!(formatter, " at byte {record_offset}")?;
                }
                write!(formatter, ": {source}")
            }
        }
    }
}

impl std::error::Error for SymbolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{External, SymbolError, parse};
    use crate::old::object::omf::read::ReadError;
    use crate::old::object::omf::record::Record;

    #[test]
    fn builds_one_based_tables_across_multiple_records() {
        let records = [
            Record::new(0x96, vec![4, b'c', b'o', b'd', b'e']).unwrap(),
            Record::new(0x97, vec![7, b'i', b'g', b'n', b'o', b'r', b'e', b'd']).unwrap(),
            Record::new(0x96, vec![4, b'd', b'a', b't', b'a']).unwrap(),
            Record::new(0x8c, vec![3, b'f', b'o', b'o', 0x7f]).unwrap(),
            Record::new(0x8d, vec![2, 0xff, b'x', 0x81, 0x23]).unwrap(),
        ];

        let tables = parse(&records).unwrap();

        assert_eq!(
            tables.names,
            vec![Vec::new(), b"code".to_vec(), b"data".to_vec()]
        );
        assert_eq!(
            tables.externals,
            vec![
                None,
                Some(External {
                    name: b"foo".to_vec(),
                    type_index: 0x7f,
                }),
                Some(External {
                    name: vec![0xff, b'x'],
                    type_index: 0x0123,
                }),
            ]
        );
    }

    #[test]
    fn rejects_a_truncated_lnames_name() {
        let record = Record::new(0x96, vec![3, b'a']).unwrap();

        assert_eq!(
            parse(&[record]),
            Err(SymbolError::Read {
                record_offset: None,
                record_type: 0x96,
                source: ReadError::TruncatedNamePayload {
                    body_offset: 1,
                    absolute_offset: None,
                    requested: 3,
                    remaining: 1,
                },
            })
        );
    }

    #[test]
    fn rejects_an_extdef_with_a_truncated_type_index() {
        let record = Record::new(0x8c, vec![3, b'f', b'o', b'o', 0x80]).unwrap();

        assert_eq!(
            parse(&[record]),
            Err(SymbolError::Read {
                record_offset: None,
                record_type: 0x8c,
                source: ReadError::TruncatedIndex {
                    body_offset: 4,
                    absolute_offset: None,
                    requested: 2,
                    remaining: 1,
                },
            })
        );
    }
}
