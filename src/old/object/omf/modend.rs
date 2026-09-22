//! Typed decoding of Intel OMF MODEND and MODEND32 records.
//!
//! A module end can name an optional physical or logical entry address.  A
//! logical address uses the same frame and target encodings as a direct
//! FIXUPP reference. Thread state belongs to the complete record stream and
//! is deliberately not an implicit input to this record-local decoder, so it
//! accepts explicit methods and refuses threaded forms rather than treating a
//! thread number as a datum index.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const MODEND: u8 = 0x8a;
const MODEND32: u8 = 0x8b;

/// One decoded MODEND or MODEND32 record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleEnd {
    /// Position of this record in the supplied record stream.
    pub record_index: usize,
    /// Absolute offset of this record's header when parsed from bytes.
    pub record_offset: Option<usize>,
    /// Whether this module supplies the link's main entry point.
    pub is_main_module: bool,
    /// The optional module start address.
    pub start: Option<StartAddress>,
}

/// The form of a MODEND start address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartAddress {
    /// A literal segment frame and offset.
    Physical(PhysicalStartAddress),
    /// A linker-resolved frame, target, and optional displacement.
    Logical(LogicalStartAddress),
}

/// A physical MODEND entry point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalStartAddress {
    pub frame: u16,
    /// Absolute physical starts retain the original 16-bit offset form.
    pub offset: u32,
}

/// A logical MODEND entry point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalStartAddress {
    pub frame: StartFrame,
    pub target: StartTarget,
}

/// The frame selected by a logical MODEND start address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartFrame {
    pub method: FrameMethod,
    pub datum: Option<FrameDatum>,
}

/// The namespace or implicit source selected for a logical start frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameMethod {
    Segment,
    Group,
    External,
    Absolute,
    Location,
    Target,
    None,
}

/// The explicit datum accompanying a logical start frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameDatum {
    /// A one-based SEGDEF, GRPDEF, or EXTDEF index.
    Index(u16),
    /// A literal segment frame number.
    Absolute(u16),
}

/// The target selected by a logical MODEND start address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartTarget {
    pub method: TargetMethod,
    /// A one-based SEGDEF, GRPDEF, or EXTDEF index, or an absolute frame for
    /// [`TargetMethod::Absolute`].
    pub datum: u16,
    /// The 16-bit MODEND or 32-bit MODEND32 displacement.
    pub displacement: u32,
}

/// The namespace selected for a logical start target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetMethod {
    Segment,
    Group,
    External,
    Absolute,
}

/// Decode every MODEND/MODEND32 record in input order.
pub fn parse(records: &[Record]) -> Result<Vec<ModuleEnd>, ModendError> {
    let mut ends = Vec::new();
    for (record_index, record) in records.iter().enumerate() {
        let wide = match record.record_type() {
            MODEND => false,
            MODEND32 => true,
            _ => continue,
        };
        ends.push(parse_record(record_index, record, wide)?);
    }
    Ok(ends)
}

fn parse_record(
    record_index: usize,
    record: &Record,
    wide: bool,
) -> Result<ModuleEnd, ModendError> {
    let mut reader = reader_for(record);
    let module_type = read(record_index, record, &mut reader, Reader::read_u8)?;
    if module_type & 0x3e != 0 {
        return Err(error(
            record_index,
            record,
            0,
            ModendErrorKind::ReservedModuleTypeBits(module_type & 0x3e),
        ));
    }

    let has_start = module_type & 0x40 != 0;
    let is_logical = module_type & 1 != 0;
    let start = if has_start {
        if is_logical {
            Some(StartAddress::Logical(parse_logical_start(
                record_index,
                record,
                &mut reader,
                wide,
            )?))
        } else {
            Some(StartAddress::Physical(PhysicalStartAddress {
                frame: read(record_index, record, &mut reader, Reader::read_u16)?,
                offset: u32::from(read(record_index, record, &mut reader, Reader::read_u16)?),
            }))
        }
    } else {
        None
    };

    if !reader.is_empty() {
        return Err(error(
            record_index,
            record,
            reader.position(),
            ModendErrorKind::TrailingBytes {
                remaining: reader.remaining(),
            },
        ));
    }

    Ok(ModuleEnd {
        record_index,
        record_offset: record.offset(),
        is_main_module: module_type & 0x80 != 0,
        start,
    })
}

fn parse_logical_start(
    record_index: usize,
    record: &Record,
    reader: &mut Reader<'_>,
    wide: bool,
) -> Result<LogicalStartAddress, ModendError> {
    let end_data_offset = reader.position();
    let end_data = read(record_index, record, reader, Reader::read_u8)?;
    let frame_method = (end_data >> 4) & 7;
    let target_method = end_data & 7;

    if end_data & 0x80 != 0 {
        return Err(error(
            record_index,
            record,
            end_data_offset,
            ModendErrorKind::UnsupportedFrameThread(frame_method),
        ));
    }
    if end_data & 0x08 != 0 {
        return Err(error(
            record_index,
            record,
            end_data_offset,
            ModendErrorKind::UnsupportedTargetThread(target_method & 3),
        ));
    }
    if end_data & 0x04 != 0 {
        return Err(error(
            record_index,
            record,
            end_data_offset,
            ModendErrorKind::InvalidTargetDisplacementBit,
        ));
    }

    let frame = StartFrame {
        method: decode_frame_method(record_index, record, end_data_offset, frame_method)?,
        datum: read_frame_datum(record_index, record, reader, frame_method)?,
    };
    let target = StartTarget {
        method: decode_target_method(record_index, record, end_data_offset, target_method)?,
        datum: read_target_datum(record_index, record, reader, target_method)?,
        displacement: read_offset(record_index, record, reader, wide)?,
    };

    Ok(LogicalStartAddress { frame, target })
}

fn decode_frame_method(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    method: u8,
) -> Result<FrameMethod, ModendError> {
    match method {
        0 => Ok(FrameMethod::Segment),
        1 => Ok(FrameMethod::Group),
        2 => Ok(FrameMethod::External),
        3 => Ok(FrameMethod::Absolute),
        4 => Ok(FrameMethod::Location),
        5 => Ok(FrameMethod::Target),
        6 => Ok(FrameMethod::None),
        value => Err(error(
            record_index,
            record,
            body_offset,
            ModendErrorKind::UnsupportedFrameMethod(value),
        )),
    }
}

fn read_frame_datum(
    record_index: usize,
    record: &Record,
    reader: &mut Reader<'_>,
    method: u8,
) -> Result<Option<FrameDatum>, ModendError> {
    match method {
        0..=2 => Ok(Some(FrameDatum::Index(read(
            record_index,
            record,
            reader,
            Reader::read_index,
        )?))),
        3 => Ok(Some(FrameDatum::Absolute(read(
            record_index,
            record,
            reader,
            Reader::read_u16,
        )?))),
        4..=6 => Ok(None),
        value => Err(error(
            record_index,
            record,
            reader.position(),
            ModendErrorKind::UnsupportedFrameMethod(value),
        )),
    }
}

fn decode_target_method(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    method: u8,
) -> Result<TargetMethod, ModendError> {
    match method {
        0 => Ok(TargetMethod::Segment),
        1 => Ok(TargetMethod::Group),
        2 => Ok(TargetMethod::External),
        3 => Ok(TargetMethod::Absolute),
        value => Err(error(
            record_index,
            record,
            body_offset,
            ModendErrorKind::UnsupportedTargetMethod(value),
        )),
    }
}

fn read_target_datum(
    record_index: usize,
    record: &Record,
    reader: &mut Reader<'_>,
    method: u8,
) -> Result<u16, ModendError> {
    if method & 3 == 3 {
        read(record_index, record, reader, Reader::read_u16)
    } else {
        read(record_index, record, reader, Reader::read_index)
    }
}

fn read_offset(
    record_index: usize,
    record: &Record,
    reader: &mut Reader<'_>,
    wide: bool,
) -> Result<u32, ModendError> {
    if wide {
        read(record_index, record, reader, Reader::read_u32)
    } else {
        Ok(u32::from(read(
            record_index,
            record,
            reader,
            Reader::read_u16,
        )?))
    }
}

fn reader_for(record: &Record) -> Reader<'_> {
    Reader::new(
        record.body(),
        record.offset().and_then(|offset| offset.checked_add(3)),
    )
}

fn read<'a, T>(
    record_index: usize,
    record: &Record,
    reader: &mut Reader<'a>,
    operation: impl FnOnce(&mut Reader<'a>) -> Result<T, ReadError>,
) -> Result<T, ModendError> {
    operation(reader).map_err(|source| ModendError::Read {
        record_index,
        record_offset: record.offset(),
        record_type: record.record_type(),
        source,
    })
}

fn error(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    kind: ModendErrorKind,
) -> ModendError {
    ModendError::Malformed {
        record_index,
        record_offset: record.offset(),
        record_type: record.record_type(),
        body_offset,
        source_offset: body_source_offset(record, body_offset),
        kind,
    }
}

fn body_source_offset(record: &Record, body_offset: usize) -> Option<usize> {
    record
        .offset()
        .and_then(|record_offset| record_offset.checked_add(3))
        .and_then(|body_offset_start| body_offset_start.checked_add(body_offset))
}

/// A malformed or unsupported MODEND record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModendError {
    /// A bounded primitive read ran past the record body.
    Read {
        record_index: usize,
        record_offset: Option<usize>,
        record_type: u8,
        source: ReadError,
    },
    /// A complete record body uses an unsupported or inconsistent encoding.
    Malformed {
        record_index: usize,
        record_offset: Option<usize>,
        record_type: u8,
        body_offset: usize,
        source_offset: Option<usize>,
        kind: ModendErrorKind,
    },
}

/// The specific semantic problem in a MODEND record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModendErrorKind {
    ReservedModuleTypeBits(u8),
    UnsupportedFrameThread(u8),
    UnsupportedTargetThread(u8),
    InvalidTargetDisplacementBit,
    UnsupportedFrameMethod(u8),
    UnsupportedTargetMethod(u8),
    TrailingBytes { remaining: usize },
}

impl fmt::Display for ModendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read {
                record_index,
                record_offset,
                record_type,
                source,
            } => {
                write!(
                    formatter,
                    "OMF MODEND record {record_index} type {record_type:02x}"
                )?;
                if let Some(offset) = record_offset {
                    write!(formatter, " at byte {offset}")?;
                }
                write!(formatter, ": {source}")
            }
            Self::Malformed {
                record_index,
                record_offset,
                record_type,
                body_offset,
                source_offset,
                kind,
            } => {
                write!(
                    formatter,
                    "malformed OMF MODEND record {record_index} type {record_type:02x} at body byte {body_offset}"
                )?;
                if let Some(offset) = record_offset {
                    write!(formatter, " (record byte {offset})")?;
                }
                if let Some(offset) = source_offset {
                    write!(formatter, " (source byte {offset})")?;
                }
                write!(formatter, ": {kind}")
            }
        }
    }
}

impl std::error::Error for ModendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Malformed { .. } => None,
        }
    }
}

impl fmt::Display for ModendErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedModuleTypeBits(bits) => {
                write!(formatter, "reserved module-type bits {bits:#04x} are set")
            }
            Self::UnsupportedFrameThread(number) => {
                write!(
                    formatter,
                    "frame thread {number} cannot be resolved in MODEND"
                )
            }
            Self::UnsupportedTargetThread(number) => {
                write!(
                    formatter,
                    "target thread {number} cannot be resolved in MODEND"
                )
            }
            Self::InvalidTargetDisplacementBit => {
                write!(formatter, "MODEND target-displacement bit must be clear")
            }
            Self::UnsupportedFrameMethod(method) => {
                write!(formatter, "unsupported frame method {method}")
            }
            Self::UnsupportedTargetMethod(method) => {
                write!(formatter, "unsupported target method {method}")
            }
            Self::TrailingBytes { remaining } => {
                write!(formatter, "{remaining} trailing byte(s)")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrameDatum, FrameMethod, ModendError, ModendErrorKind, StartAddress, TargetMethod, parse,
    };
    use crate::old::object::omf::record::Record;

    #[test]
    fn decodes_a_module_end_without_a_start_address() {
        let record = Record::new(0x8a, vec![0x80]).unwrap();

        let ends = parse(&[record]).unwrap();

        assert_eq!(ends.len(), 1);
        assert!(ends[0].is_main_module);
        assert_eq!(ends[0].start, None);
    }

    #[test]
    fn keeps_a_modend32_physical_start_sixteen_bit() {
        let record = Record::new(0x8b, vec![0x40, 0x34, 0x12, 0x78, 0x56]).unwrap();

        let ends = parse(&[record]).unwrap();

        assert_eq!(
            ends[0].start,
            Some(StartAddress::Physical(super::PhysicalStartAddress {
                frame: 0x1234,
                offset: 0x5678,
            }))
        );
    }

    #[test]
    fn decodes_a_sixteen_bit_logical_start_address() {
        // Main, start present, logical. Frame: group index 0x123. Target:
        // external index 4 plus a 16-bit displacement.
        let record = Record::new(0x8a, vec![0xc1, 0x12, 0x81, 0x23, 4, 0x78, 0x56]).unwrap();

        let ends = parse(&[record]).unwrap();
        let Some(StartAddress::Logical(start)) = ends[0].start.as_ref() else {
            panic!("expected a logical start address");
        };
        assert_eq!(start.frame.method, FrameMethod::Group);
        assert_eq!(start.frame.datum, Some(FrameDatum::Index(0x123)));
        assert_eq!(start.target.method, TargetMethod::External);
        assert_eq!(start.target.datum, 4);
        assert_eq!(start.target.displacement, 0x5678);
    }

    #[test]
    fn decodes_a_thirty_two_bit_logical_start_address() {
        // Start present, logical. Both the frame and target use absolute
        // frame numbers; MODEND32 carries a four-byte displacement.
        let record = Record::new(
            0x8b,
            vec![0x41, 0x33, 0x34, 0x12, 0x78, 0x56, 0x78, 0x56, 0x34, 0x12],
        )
        .unwrap();

        let ends = parse(&[record]).unwrap();
        let Some(StartAddress::Logical(start)) = ends[0].start.as_ref() else {
            panic!("expected a logical start address");
        };
        assert_eq!(start.frame.method, FrameMethod::Absolute);
        assert_eq!(start.frame.datum, Some(FrameDatum::Absolute(0x1234)));
        assert_eq!(start.target.method, TargetMethod::Absolute);
        assert_eq!(start.target.datum, 0x5678);
        assert_eq!(start.target.displacement, 0x1234_5678);
    }

    #[test]
    fn rejects_a_threaded_logical_start_address() {
        let record = Record::new(0x8a, vec![0x41, 0x80]).unwrap();

        let error = parse(&[record]).unwrap_err();

        assert_eq!(
            error,
            ModendError::Malformed {
                record_index: 0,
                record_offset: None,
                record_type: 0x8a,
                body_offset: 1,
                source_offset: None,
                kind: ModendErrorKind::UnsupportedFrameThread(0),
            }
        );
    }

    #[test]
    fn rejects_a_suppressed_target_displacement() {
        let record = Record::new(0x8a, vec![0x41, 0x04]).unwrap();

        assert!(matches!(
            parse(&[record]),
            Err(ModendError::Malformed {
                kind: ModendErrorKind::InvalidTargetDisplacementBit,
                ..
            })
        ));
    }

    #[test]
    fn rejects_trailing_payload() {
        let record = Record::new(0x8a, vec![0, 0xff]).unwrap();

        let error = parse(&[record]).unwrap_err();

        assert_eq!(
            error,
            ModendError::Malformed {
                record_index: 0,
                record_offset: None,
                record_type: 0x8a,
                body_offset: 1,
                source_offset: None,
                kind: ModendErrorKind::TrailingBytes { remaining: 1 },
            }
        );
    }
}
