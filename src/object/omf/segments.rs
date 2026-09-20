//! Typed decoding for OMF segment definitions.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const SEGDEF: u8 = 0x98;
const SEGDEF32: u8 = 0x99;

/// The one-based segment table declared by a module's SEGDEF records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentTable {
    /// Segment entries in record order. Index zero is the `None` sentinel.
    pub segments: Vec<Option<Segment>>,
}

impl SegmentTable {
    pub fn parse(records: &[Record]) -> Result<Self, SegmentError> {
        parse(records)
    }
}

/// One decoded SEGDEF record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub alignment: Alignment,
    pub combine: Combine,
    pub big: bool,
    pub use32: bool,
    pub length: u64,
    pub name_index: u16,
    pub class_name_index: u16,
    pub overlay_name_index: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alignment {
    Absolute { frame: u16, offset: u8 },
    Byte,
    Word,
    Paragraph,
    Page,
    DoubleWord,
    Page4K,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Combine {
    Private,
    Public,
    Stack,
    Common,
}

/// Decode every 16- and 32-bit SEGDEF in record order.
pub fn parse(records: &[Record]) -> Result<SegmentTable, SegmentError> {
    let mut segments = vec![None];
    for record in records {
        let wide = match record.record_type() {
            SEGDEF => false,
            SEGDEF32 => true,
            _ => continue,
        };
        segments.push(Some(parse_record(record, wide)?));
    }
    Ok(SegmentTable { segments })
}

fn parse_record(record: &Record, wide: bool) -> Result<Segment, SegmentError> {
    let mut reader = Reader::new(
        record.body(),
        record.offset().and_then(|offset| offset.checked_add(3)),
    );
    let attributes = read(record, &mut reader, Reader::read_u8)?;
    let alignment = match attributes >> 5 {
        0 => Alignment::Absolute {
            frame: read(record, &mut reader, Reader::read_u16)?,
            offset: read(record, &mut reader, Reader::read_u8)?,
        },
        1 => Alignment::Byte,
        2 => Alignment::Word,
        3 => Alignment::Paragraph,
        4 => Alignment::Page,
        5 => Alignment::DoubleWord,
        6 => Alignment::Page4K,
        value => return Err(context(record, SegmentErrorKind::ReservedAlignment(value))),
    };
    let combine = match (attributes >> 2) & 7 {
        0 => Combine::Private,
        2 | 4 | 7 => Combine::Public,
        5 => Combine::Stack,
        6 => Combine::Common,
        value => return Err(context(record, SegmentErrorKind::ReservedCombine(value))),
    };
    let big = attributes & 0x02 != 0;
    let encoded_length = if wide {
        u64::from(read(record, &mut reader, Reader::read_u32)?)
    } else {
        u64::from(read(record, &mut reader, Reader::read_u16)?)
    };
    let length = if big && encoded_length == 0 {
        if wide { 0x1_0000_0000 } else { 0x1_0000 }
    } else {
        encoded_length
    };
    let segment = Segment {
        alignment,
        combine,
        big,
        use32: attributes & 1 != 0,
        length,
        name_index: read(record, &mut reader, Reader::read_index)?,
        class_name_index: read(record, &mut reader, Reader::read_index)?,
        overlay_name_index: read(record, &mut reader, Reader::read_index)?,
    };
    if !reader.is_empty() {
        return Err(context(
            record,
            SegmentErrorKind::TrailingBytes {
                body_offset: reader.position(),
                remaining: reader.remaining(),
            },
        ));
    }
    Ok(segment)
}

fn read<'a, T>(
    record: &Record,
    reader: &mut Reader<'a>,
    operation: impl FnOnce(&mut Reader<'a>) -> Result<T, ReadError>,
) -> Result<T, SegmentError> {
    operation(reader).map_err(|source| context(record, SegmentErrorKind::Read(source)))
}

fn context(record: &Record, kind: SegmentErrorKind) -> SegmentError {
    SegmentError {
        record_offset: record.offset(),
        record_type: record.record_type(),
        kind,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentError {
    pub record_offset: Option<usize>,
    pub record_type: u8,
    pub kind: SegmentErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentErrorKind {
    Read(ReadError),
    ReservedAlignment(u8),
    ReservedCombine(u8),
    TrailingBytes {
        body_offset: usize,
        remaining: usize,
    },
}

impl fmt::Display for SegmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "OMF SEGDEF type {:02x}", self.record_type)?;
        if let Some(offset) = self.record_offset {
            write!(formatter, " at byte {offset}")?;
        }
        write!(formatter, ": ")?;
        match &self.kind {
            SegmentErrorKind::Read(source) => source.fmt(formatter),
            SegmentErrorKind::ReservedAlignment(value) => {
                write!(formatter, "reserved alignment encoding {value}")
            }
            SegmentErrorKind::ReservedCombine(value) => {
                write!(formatter, "reserved combine encoding {value}")
            }
            SegmentErrorKind::TrailingBytes {
                body_offset,
                remaining,
            } => write!(
                formatter,
                "{remaining} trailing byte(s) at body byte {body_offset}"
            ),
        }
    }
}

impl std::error::Error for SegmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            SegmentErrorKind::Read(source) => Some(source),
            SegmentErrorKind::ReservedAlignment(_)
            | SegmentErrorKind::ReservedCombine(_)
            | SegmentErrorKind::TrailingBytes { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Alignment, Combine, SegmentErrorKind, parse};
    use crate::object::omf::record::Record;

    #[test]
    fn decodes_sixteen_and_thirty_two_bit_segments() {
        let records = [
            Record::new(0x98, vec![0x68, 0x34, 0x12, 1, 2, 0]).unwrap(),
            Record::new(0x99, vec![0x35, 0x78, 0x56, 0x34, 0x12, 0x81, 0, 2, 0]).unwrap(),
        ];

        let table = parse(&records).unwrap();

        let first = table.segments[1].as_ref().unwrap();
        assert_eq!(first.alignment, Alignment::Paragraph);
        assert_eq!(first.combine, Combine::Public);
        assert_eq!(first.length, 0x1234);
        assert_eq!((first.name_index, first.class_name_index), (1, 2));
        let second = table.segments[2].as_ref().unwrap();
        assert_eq!(second.alignment, Alignment::Byte);
        assert_eq!(second.combine, Combine::Stack);
        assert!(second.use32);
        assert_eq!(second.length, 0x1234_5678);
        assert_eq!(second.name_index, 0x100);
    }

    #[test]
    fn expands_the_big_zero_length_encoding() {
        let record = Record::new(0x98, vec![0x6a, 0, 0, 1, 0, 0]).unwrap();

        let table = parse(&[record]).unwrap();

        assert_eq!(table.segments[1].as_ref().unwrap().length, 0x1_0000);
    }

    #[test]
    fn rejects_reserved_attribute_encodings() {
        let record = Record::new(0x98, vec![0xe0, 0, 0, 0, 0, 0]).unwrap();

        let error = parse(&[record]).unwrap_err();

        assert_eq!(error.kind, SegmentErrorKind::ReservedAlignment(7));
    }
}
