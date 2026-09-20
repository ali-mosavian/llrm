//! Typed views over OMF source line records.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const LINNUM: u8 = 0x94;
const LINNUM32: u8 = 0x95;

/// One source line mapped to an offset in a segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineEntry {
    pub line: u16,
    pub offset: u32,
}

/// The mappings carried by one LINNUM or LINNUM32 record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineTable {
    pub record_index: usize,
    pub record_offset: Option<usize>,
    pub group_index: u16,
    pub segment_index: u16,
    pub entries: Vec<LineEntry>,
}

/// Decodes source line tables in record-stream order.
pub fn parse(records: &[Record]) -> Result<Vec<LineTable>, LineError> {
    let mut tables = Vec::new();
    for (record_index, record) in records.iter().enumerate() {
        let wide = match record.record_type() {
            LINNUM => false,
            LINNUM32 => true,
            _ => continue,
        };
        tables.push(parse_record(record_index, record, wide)?);
    }
    Ok(tables)
}

fn parse_record(record_index: usize, record: &Record, wide: bool) -> Result<LineTable, LineError> {
    let mut reader = Reader::new(
        record.body(),
        record.offset().and_then(|offset| offset.checked_add(3)),
    );
    let group_index = read(record, &mut reader, Reader::read_index)?;
    let segment_index = read(record, &mut reader, Reader::read_index)?;
    let mut entries = Vec::new();
    while !reader.is_empty() {
        let line = read(record, &mut reader, Reader::read_u16)?;
        let offset = if wide {
            read(record, &mut reader, Reader::read_u32)?
        } else {
            u32::from(read(record, &mut reader, Reader::read_u16)?)
        };
        entries.push(LineEntry { line, offset });
    }
    Ok(LineTable {
        record_index,
        record_offset: record.offset(),
        group_index,
        segment_index,
        entries,
    })
}

fn read<'a, T>(
    record: &Record,
    reader: &mut Reader<'a>,
    operation: impl FnOnce(&mut Reader<'a>) -> Result<T, ReadError>,
) -> Result<T, LineError> {
    operation(reader).map_err(|source| LineError {
        record_offset: record.offset(),
        record_type: record.record_type(),
        source,
    })
}

/// A malformed source line record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineError {
    pub record_offset: Option<usize>,
    pub record_type: u8,
    pub source: ReadError,
}

impl fmt::Display for LineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "OMF line record type {:02x}", self.record_type)?;
        if let Some(offset) = self.record_offset {
            write!(formatter, " at byte {offset}")?;
        }
        write!(formatter, ": {}", self.source)
    }
}

impl std::error::Error for LineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, LineEntry};
    use crate::object::omf::read::ReadError;
    use crate::object::omf::record::Record;

    #[test]
    fn decodes_sixteen_and_thirty_two_bit_line_offsets() {
        let records = [
            Record::new(0x94, vec![1, 2, 10, 0, 0x34, 0x12]).unwrap(),
            Record::new(0x95, vec![3, 4, 20, 0, 0x78, 0x56, 0x34, 0x12]).unwrap(),
        ];

        let tables = parse(&records).unwrap();

        assert_eq!((tables[0].group_index, tables[0].segment_index), (1, 2));
        assert_eq!(
            tables[0].entries,
            [LineEntry {
                line: 10,
                offset: 0x1234,
            }]
        );
        assert_eq!((tables[1].group_index, tables[1].segment_index), (3, 4));
        assert_eq!(
            tables[1].entries,
            [LineEntry {
                line: 20,
                offset: 0x1234_5678,
            }]
        );
    }

    #[test]
    fn rejects_a_partial_line_entry() {
        let record = Record::new(0x94, vec![1, 2, 10, 0, 0x34]).unwrap();

        let error = parse(&[record]).unwrap_err();

        assert_eq!(
            error.source,
            ReadError::TruncatedRead {
                body_offset: 4,
                absolute_offset: None,
                requested: 2,
                remaining: 1,
            }
        );
    }
}
