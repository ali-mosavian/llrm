//! Typed views over OMF enumerated-data records.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const LEDATA: u8 = 0xa0;
const LEDATA32: u8 = 0xa1;

/// One LEDATA payload in record-stream order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataBlock<'a> {
    pub record_index: usize,
    pub record_offset: Option<usize>,
    pub segment_index: u16,
    pub offset: u32,
    pub bytes: &'a [u8],
}

/// Decode every LEDATA/LEDATA32 record without copying its payload.
pub fn parse(records: &[Record]) -> Result<Vec<DataBlock<'_>>, DataError> {
    let mut blocks = Vec::new();
    for (record_index, record) in records.iter().enumerate() {
        let wide = match record.record_type() {
            LEDATA => false,
            LEDATA32 => true,
            _ => continue,
        };
        blocks.push(parse_record(record_index, record, wide)?);
    }
    Ok(blocks)
}

fn parse_record(
    record_index: usize,
    record: &Record,
    wide: bool,
) -> Result<DataBlock<'_>, DataError> {
    let mut reader = Reader::new(
        record.body(),
        record.offset().and_then(|offset| offset.checked_add(3)),
    );
    let segment_index = reader
        .read_index()
        .map_err(|source| context(record, source))?;
    let offset = if wide {
        reader
            .read_u32()
            .map_err(|source| context(record, source))?
    } else {
        u32::from(
            reader
                .read_u16()
                .map_err(|source| context(record, source))?,
        )
    };
    let bytes = reader
        .read_bytes(reader.remaining())
        .map_err(|source| context(record, source))?;
    Ok(DataBlock {
        record_index,
        record_offset: record.offset(),
        segment_index,
        offset,
        bytes,
    })
}

fn context(record: &Record, source: ReadError) -> DataError {
    DataError {
        record_offset: record.offset(),
        record_type: record.record_type(),
        source,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataError {
    pub record_offset: Option<usize>,
    pub record_type: u8,
    pub source: ReadError,
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "OMF LEDATA type {:02x}", self.record_type)?;
        if let Some(offset) = self.record_offset {
            write!(formatter, " at byte {offset}")?;
        }
        write!(formatter, ": {}", self.source)
    }
}

impl std::error::Error for DataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::{DataError, parse};
    use crate::object::omf::read::ReadError;
    use crate::object::omf::record::Record;

    #[test]
    fn decodes_sixteen_and_thirty_two_bit_data_without_copying() {
        let records = [
            Record::new(0xa0, vec![2, 0x34, 0x12, 0xaa, 0xbb]).unwrap(),
            Record::new(0xa1, vec![0x81, 0x23, 0x78, 0x56, 0x34, 0x12, 0xcc]).unwrap(),
        ];

        let blocks = parse(&records).unwrap();

        assert_eq!((blocks[0].segment_index, blocks[0].offset), (2, 0x1234));
        assert_eq!(blocks[0].bytes, [0xaa, 0xbb]);
        assert_eq!(
            (blocks[1].segment_index, blocks[1].offset),
            (0x123, 0x1234_5678)
        );
        assert_eq!(blocks[1].bytes, [0xcc]);
        assert!(std::ptr::eq(
            blocks[0].bytes.as_ptr(),
            records[0].body()[3..].as_ptr()
        ));
    }

    #[test]
    fn rejects_a_truncated_data_offset() {
        let record = Record::new(0xa0, vec![1, 0x34]).unwrap();

        let error = parse(&[record]).unwrap_err();

        assert_eq!(
            error,
            DataError {
                record_offset: None,
                record_type: 0xa0,
                source: ReadError::TruncatedRead {
                    body_offset: 1,
                    absolute_offset: None,
                    requested: 2,
                    remaining: 1,
                },
            }
        );
    }
}
