//! Typed decoding of Intel OMF FIXUPP subrecords.
//!
//! FIXUPP records are interpreted in stream order: their patch offsets are
//! relative to the preceding enumerated-data record, and THREAD definitions
//! remain in effect for later FIXUPP records.  This module deliberately stops
//! at decoding; it does not try to resolve OMF indices to symbols or rewrite
//! records.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const LEDATA: u8 = 0xa0;
const LEDATA32: u8 = 0xa1;
const LIDATA: u8 = 0xa2;
const LIDATA32: u8 = 0xa3;
const FIXUPP: u8 = 0x9c;
const FIXUPP32: u8 = 0x9d;

/// How a relocation field is interpreted by the linker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixupMode {
    /// The relocated value is relative to the relocation field.
    SelfRelative,
    /// The relocated value is relative to the target segment.
    SegmentRelative,
}

/// The type of field patched by a FIXUPP subrecord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    LowByte,
    Offset16,
    Base16,
    Pointer16_16,
    HighByte,
    LoaderResolvedOffset16,
    Offset32,
    Pointer16_32,
    LoaderResolvedOffset32,
}

impl Location {
    fn decode(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::LowByte),
            1 => Some(Self::Offset16),
            2 => Some(Self::Base16),
            3 => Some(Self::Pointer16_16),
            4 => Some(Self::HighByte),
            5 => Some(Self::LoaderResolvedOffset16),
            9 => Some(Self::Offset32),
            11 => Some(Self::Pointer16_32),
            13 => Some(Self::LoaderResolvedOffset32),
            _ => None,
        }
    }

    const fn byte_width(self) -> usize {
        match self {
            Self::LowByte | Self::HighByte => 1,
            Self::Offset16 | Self::Base16 | Self::LoaderResolvedOffset16 => 2,
            Self::Pointer16_16 | Self::Offset32 | Self::LoaderResolvedOffset32 => 4,
            Self::Pointer16_32 => 6,
        }
    }
}

/// The namespace or implicit source selected for a FIXUPP frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameMethod {
    Segment,
    Group,
    External,
    Location,
    Target,
}

/// The optional datum accompanying a frame method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameDatum {
    /// A one-based SEGDEF, GRPDEF, or EXTDEF index.
    Index(u16),
}

/// A fully resolved frame, whether written directly or selected via THREAD.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame {
    pub method: FrameMethod,
    pub datum: Option<FrameDatum>,
}

/// The namespace or implicit source selected for a FIXUPP target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetMethod {
    Segment,
    Group,
    External,
}

/// A fully resolved target, whether written directly or selected via THREAD.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Target {
    pub method: TargetMethod,
    /// The one-based SEGDEF, GRPDEF, or EXTDEF index.
    pub datum: u16,
    /// Whether the target displacement was present. Together with `method`,
    /// this preserves the OMF target method: methods 0--2 have a displacement;
    /// methods 4--6 omit it.
    pub displacement_present: bool,
}

/// One decoded relocation, in input order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixup {
    /// Index of the FIXUPP record in the supplied record stream.
    pub record_index: usize,
    /// Absolute offset of that record header, if the stream was parsed from
    /// bytes rather than constructed in memory.
    pub record_offset: Option<usize>,
    /// Offset of this subrecord's LOCAT byte in its record body.
    pub body_offset: usize,
    /// Absolute offset of this subrecord's LOCAT byte, when available.
    pub source_offset: Option<usize>,
    /// The LEDATA/LEDATA32 segment selected by the preceding data record.
    pub segment_index: u16,
    /// Offset in `segment_index` of the field being patched.
    pub patch_offset: u32,
    pub location: Location,
    pub mode: FixupMode,
    pub frame: Frame,
    pub target: Target,
    /// The optional target displacement.  FIXUPP uses 16 bits; FIXUPP32 uses
    /// 32 bits.  A suppressed displacement is represented by zero.
    pub displacement: u32,
    /// Offset of the displacement in the FIXUPP body, if present.
    pub displacement_body_offset: Option<usize>,
}

/// Decode all FIXUPP/FIXUPP32 records in stream order.
///
/// Frame and target THREAD state persists for the complete supplied stream,
/// as required by Intel OMF. A FIXUPP uses the most recent enumerated-data
/// context in the stream; LIDATA deliberately clears that context because
/// this decoder does not expand iterated data.
pub fn parse(records: &[Record]) -> Result<Vec<Fixup>, FixupError> {
    let mut decoder = Decoder::default();
    let mut fixups = Vec::new();

    for (record_index, record) in records.iter().enumerate() {
        match record.record_type() {
            LEDATA => decoder.set_data_context(record_index, record, false)?,
            LEDATA32 => decoder.set_data_context(record_index, record, true)?,
            LIDATA | LIDATA32 => decoder.clear_data_context(record_index, record),
            FIXUPP => decoder.parse_fixupp(record_index, record, false, &mut fixups)?,
            FIXUPP32 => decoder.parse_fixupp(record_index, record, true, &mut fixups)?,
            _ => {}
        }
    }

    Ok(fixups)
}

#[derive(Clone, Copy, Debug)]
struct DataContext {
    segment_index: u16,
    base: u32,
    byte_length: usize,
}

#[derive(Clone, Copy, Debug)]
struct UnsupportedDataContext {
    record_index: usize,
    record_offset: Option<usize>,
    record_type: u8,
}

#[derive(Clone, Copy, Debug, Default)]
struct Decoder {
    data: Option<DataContext>,
    unsupported_data: Option<UnsupportedDataContext>,
    frame_threads: [Option<Frame>; 4],
    target_threads: [Option<TargetThread>; 4],
}

/// A target THREAD carries its datum; each FIXUP use supplies the P bit.
#[derive(Clone, Copy, Debug)]
struct TargetThread {
    method: TargetMethod,
    datum: u16,
}

impl Decoder {
    fn set_data_context(
        &mut self,
        record_index: usize,
        record: &Record,
        wide: bool,
    ) -> Result<(), FixupError> {
        let mut reader = reader_for(record);
        let segment_index = read(record_index, record, &mut reader, Reader::read_index)?;
        let base = if wide {
            read(record_index, record, &mut reader, Reader::read_u32)?
        } else {
            u32::from(read(record_index, record, &mut reader, Reader::read_u16)?)
        };
        self.data = Some(DataContext {
            segment_index,
            base,
            byte_length: reader.remaining(),
        });
        self.unsupported_data = None;
        Ok(())
    }

    fn clear_data_context(&mut self, record_index: usize, record: &Record) {
        self.data = None;
        self.unsupported_data = Some(UnsupportedDataContext {
            record_index,
            record_offset: record.offset(),
            record_type: record.record_type(),
        });
    }

    fn parse_fixupp(
        &mut self,
        record_index: usize,
        record: &Record,
        wide: bool,
        fixups: &mut Vec<Fixup>,
    ) -> Result<(), FixupError> {
        let mut reader = reader_for(record);
        while !reader.is_empty() {
            let body_offset = reader.position();
            let lead = read(record_index, record, &mut reader, Reader::read_u8)?;
            if lead & 0x80 == 0 {
                self.parse_thread(record_index, record, body_offset, lead, &mut reader)?;
                continue;
            }

            let Some(data) = self.data else {
                return Err(match self.unsupported_data {
                    Some(context) => FixupError::UnsupportedDataContext {
                        record_index,
                        record_offset: record.offset(),
                        record_type: record.record_type(),
                        data_record_index: context.record_index,
                        data_record_offset: context.record_offset,
                        data_record_type: context.record_type,
                    },
                    None => FixupError::MissingDataContext {
                        record_index,
                        record_offset: record.offset(),
                        record_type: record.record_type(),
                    },
                });
            };

            let location_code = (lead >> 2) & 0x0f;
            let Some(location) = Location::decode(location_code) else {
                return Err(error(
                    record_index,
                    record,
                    body_offset,
                    FixupErrorKind::UnsupportedLocation(location_code),
                ));
            };
            let mode = if lead & 0x40 == 0 {
                FixupMode::SelfRelative
            } else {
                FixupMode::SegmentRelative
            };
            let relative_offset = (u16::from(lead & 3) << 8)
                | u16::from(read(record_index, record, &mut reader, Reader::read_u8)?);
            let location_width = location.byte_width();
            let Some(end) = usize::from(relative_offset).checked_add(location_width) else {
                return Err(error(
                    record_index,
                    record,
                    body_offset,
                    FixupErrorKind::PatchOutsideData {
                        relative_offset,
                        location_width,
                        data_length: data.byte_length,
                    },
                ));
            };
            if end > data.byte_length {
                return Err(error(
                    record_index,
                    record,
                    body_offset,
                    FixupErrorKind::PatchOutsideData {
                        relative_offset,
                        location_width,
                        data_length: data.byte_length,
                    },
                ));
            }
            let fixdat = read(record_index, record, &mut reader, Reader::read_u8)?;
            let frame = self.read_frame(record_index, record, body_offset, fixdat, &mut reader)?;
            let target =
                self.read_target(record_index, record, body_offset, fixdat, &mut reader)?;
            let displacement_body_offset = if fixdat & 0x04 == 0 {
                Some(reader.position())
            } else {
                None
            };
            let displacement = match displacement_body_offset {
                Some(_) if wide => read(record_index, record, &mut reader, Reader::read_u32)?,
                Some(_) => u32::from(read(record_index, record, &mut reader, Reader::read_u16)?),
                None => 0,
            };
            let Some(patch_offset) = data.base.checked_add(u32::from(relative_offset)) else {
                return Err(error(
                    record_index,
                    record,
                    body_offset,
                    FixupErrorKind::PatchOffsetOverflow {
                        base: data.base,
                        relative_offset,
                    },
                ));
            };
            fixups.push(Fixup {
                record_index,
                record_offset: record.offset(),
                body_offset,
                source_offset: body_source_offset(record, body_offset),
                segment_index: data.segment_index,
                patch_offset,
                location,
                mode,
                frame,
                target,
                displacement,
                displacement_body_offset,
            });
        }
        Ok(())
    }

    fn parse_thread(
        &mut self,
        record_index: usize,
        record: &Record,
        body_offset: usize,
        lead: u8,
        reader: &mut Reader<'_>,
    ) -> Result<(), FixupError> {
        let number = usize::from(lead & 3);
        let method = (lead >> 2) & 7;
        if lead & 0x40 != 0 {
            self.frame_threads[number] = Some(read_frame_value(
                record_index,
                record,
                body_offset,
                method,
                reader,
            )?);
        } else {
            self.target_threads[number] = Some(read_target_thread(
                record_index,
                record,
                body_offset,
                method,
                reader,
            )?);
        }
        Ok(())
    }

    fn read_frame(
        &self,
        record_index: usize,
        record: &Record,
        body_offset: usize,
        fixdat: u8,
        reader: &mut Reader<'_>,
    ) -> Result<Frame, FixupError> {
        if fixdat & 0x80 != 0 {
            let number = usize::from((fixdat >> 4) & 3);
            return self.frame_threads[number].ok_or_else(|| {
                error(
                    record_index,
                    record,
                    body_offset,
                    FixupErrorKind::UnresolvedFrameThread(number as u8),
                )
            });
        }
        read_frame_value(record_index, record, body_offset, (fixdat >> 4) & 7, reader)
    }

    fn read_target(
        &self,
        record_index: usize,
        record: &Record,
        body_offset: usize,
        fixdat: u8,
        reader: &mut Reader<'_>,
    ) -> Result<Target, FixupError> {
        if fixdat & 0x08 != 0 {
            let number = usize::from(fixdat & 3);
            let thread = self.target_threads[number].ok_or_else(|| {
                error(
                    record_index,
                    record,
                    body_offset,
                    FixupErrorKind::UnresolvedTargetThread(number as u8),
                )
            })?;
            return Ok(Target {
                method: thread.method,
                datum: thread.datum,
                displacement_present: fixdat & 0x04 == 0,
            });
        }
        read_target_value(record_index, record, body_offset, fixdat & 7, reader)
    }
}

fn read_frame_value(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    method: u8,
    reader: &mut Reader<'_>,
) -> Result<Frame, FixupError> {
    let (method, datum) = match method {
        0 => (
            FrameMethod::Segment,
            Some(FrameDatum::Index(read(
                record_index,
                record,
                reader,
                Reader::read_index,
            )?)),
        ),
        1 => (
            FrameMethod::Group,
            Some(FrameDatum::Index(read(
                record_index,
                record,
                reader,
                Reader::read_index,
            )?)),
        ),
        2 => (
            FrameMethod::External,
            Some(FrameDatum::Index(read(
                record_index,
                record,
                reader,
                Reader::read_index,
            )?)),
        ),
        3 => {
            return Err(error(
                record_index,
                record,
                body_offset,
                FixupErrorKind::UnsupportedFrameMethod(method),
            ));
        }
        4 => (FrameMethod::Location, None),
        5 => (FrameMethod::Target, None),
        value => {
            return Err(error(
                record_index,
                record,
                body_offset,
                FixupErrorKind::UnsupportedFrameMethod(value),
            ));
        }
    };
    Ok(Frame { method, datum })
}

fn read_target_thread(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    method: u8,
    reader: &mut Reader<'_>,
) -> Result<TargetThread, FixupError> {
    let (method, datum) = read_target_parts(record_index, record, body_offset, method, reader)?;
    Ok(TargetThread { method, datum })
}

fn read_target_value(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    method: u8,
    reader: &mut Reader<'_>,
) -> Result<Target, FixupError> {
    let displacement_present = method & 4 == 0;
    let (method, datum) = read_target_parts(record_index, record, body_offset, method & 3, reader)?;
    Ok(Target {
        method,
        datum,
        displacement_present,
    })
}

fn read_target_parts(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    method: u8,
    reader: &mut Reader<'_>,
) -> Result<(TargetMethod, u16), FixupError> {
    let (method, datum) = match method {
        0 => (
            TargetMethod::Segment,
            read(record_index, record, reader, Reader::read_index)?,
        ),
        1 => (
            TargetMethod::Group,
            read(record_index, record, reader, Reader::read_index)?,
        ),
        2 => (
            TargetMethod::External,
            read(record_index, record, reader, Reader::read_index)?,
        ),
        value => {
            return Err(error(
                record_index,
                record,
                body_offset,
                FixupErrorKind::UnsupportedTargetMethod(value),
            ));
        }
    };
    Ok((method, datum))
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
) -> Result<T, FixupError> {
    operation(reader).map_err(|source| FixupError::Read {
        record_index,
        record_offset: record.offset(),
        record_type: record.record_type(),
        source,
    })
}

fn body_source_offset(record: &Record, body_offset: usize) -> Option<usize> {
    record
        .offset()
        .and_then(|record_offset| record_offset.checked_add(3))
        .and_then(|body_start| body_start.checked_add(body_offset))
}

fn error(
    record_index: usize,
    record: &Record,
    body_offset: usize,
    kind: FixupErrorKind,
) -> FixupError {
    FixupError::Malformed {
        record_index,
        record_offset: record.offset(),
        record_type: record.record_type(),
        body_offset,
        source_offset: body_source_offset(record, body_offset),
        kind,
    }
}

/// A refusal or malformed FIXUPP record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixupError {
    /// A bounded primitive read ran past a record body.
    Read {
        record_index: usize,
        record_offset: Option<usize>,
        record_type: u8,
        source: ReadError,
    },
    /// A FIXUPP appeared before any decodable LEDATA/LEDATA32 record.
    MissingDataContext {
        record_index: usize,
        record_offset: Option<usize>,
        record_type: u8,
    },
    /// The preceding data record is LIDATA/LIDATA32, whose expanded patch
    /// coordinates are intentionally outside this decoder's scope.
    UnsupportedDataContext {
        record_index: usize,
        record_offset: Option<usize>,
        record_type: u8,
        data_record_index: usize,
        data_record_offset: Option<usize>,
        data_record_type: u8,
    },
    /// A syntactically complete subrecord contains a reserved encoding or an
    /// impossible coordinate.
    Malformed {
        record_index: usize,
        record_offset: Option<usize>,
        record_type: u8,
        body_offset: usize,
        source_offset: Option<usize>,
        kind: FixupErrorKind,
    },
}

/// The specific semantic problem in a malformed FIXUPP subrecord.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixupErrorKind {
    UnsupportedLocation(u8),
    UnsupportedFrameMethod(u8),
    UnsupportedTargetMethod(u8),
    UnresolvedFrameThread(u8),
    UnresolvedTargetThread(u8),
    PatchOutsideData {
        relative_offset: u16,
        location_width: usize,
        data_length: usize,
    },
    PatchOffsetOverflow {
        base: u32,
        relative_offset: u16,
    },
}

impl fmt::Display for FixupError {
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
                    "OMF FIXUPP record {record_index} type {record_type:02x}"
                )?;
                if let Some(offset) = record_offset {
                    write!(formatter, " at byte {offset}")?;
                }
                write!(formatter, ": {source}")
            }
            Self::MissingDataContext {
                record_index,
                record_offset,
                record_type,
            } => {
                write!(
                    formatter,
                    "OMF FIXUPP record {record_index} type {record_type:02x}"
                )?;
                if let Some(offset) = record_offset {
                    write!(formatter, " at byte {offset}")?;
                }
                write!(formatter, " has no preceding LEDATA context")
            }
            Self::UnsupportedDataContext {
                record_index,
                record_offset,
                record_type,
                data_record_index,
                data_record_offset,
                data_record_type,
            } => {
                write!(
                    formatter,
                    "OMF FIXUPP record {record_index} type {record_type:02x}"
                )?;
                if let Some(offset) = record_offset {
                    write!(formatter, " at byte {offset}")?;
                }
                write!(
                    formatter,
                    " follows unsupported data record {data_record_index} type {data_record_type:02x}"
                )?;
                if let Some(offset) = data_record_offset {
                    write!(formatter, " at byte {offset}")?;
                }
                Ok(())
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
                    "malformed OMF FIXUPP record {record_index} type {record_type:02x} at body byte {body_offset}"
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

impl std::error::Error for FixupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::MissingDataContext { .. }
            | Self::UnsupportedDataContext { .. }
            | Self::Malformed { .. } => None,
        }
    }
}

impl fmt::Display for FixupErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLocation(value) => {
                write!(formatter, "unsupported location code {value}")
            }
            Self::UnsupportedFrameMethod(value) => {
                write!(formatter, "unsupported frame method {value}")
            }
            Self::UnsupportedTargetMethod(value) => {
                write!(formatter, "unsupported target method {value}")
            }
            Self::UnresolvedFrameThread(number) => {
                write!(formatter, "undefined frame thread {number}")
            }
            Self::UnresolvedTargetThread(number) => {
                write!(formatter, "undefined target thread {number}")
            }
            Self::PatchOutsideData {
                relative_offset,
                location_width,
                data_length,
            } => write!(
                formatter,
                "{location_width}-byte patch at relative offset {relative_offset:#x} extends beyond {data_length}-byte LEDATA payload"
            ),
            Self::PatchOffsetOverflow {
                base,
                relative_offset,
            } => write!(
                formatter,
                "patch offset overflows: base {base:#x}, relative offset {relative_offset:#x}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FixupError, FixupErrorKind, FixupMode, Frame, FrameDatum, FrameMethod, Location, Target,
        TargetMethod, parse,
    };
    use crate::object::omf::read::ReadError;
    use crate::object::omf::record::Record;

    fn record(record_type: u8, body: &[u8]) -> Record {
        Record::new(record_type, body.to_vec()).unwrap()
    }

    #[test]
    fn decodes_an_explicit_fixup_against_the_current_ledata_base() {
        let records = [
            record(0xa0, &[2, 0x34, 0x12, 0, 0, 0, 0, 0, 0, 0]),
            // LOCAT offset16, self-relative, relative offset 5; explicit
            // segment frame 1; external target 3; displacement 0x20.
            record(0x9c, &[0x84, 5, 0x02, 1, 3, 0x20, 0]),
        ];

        let fixups = parse(&records).unwrap();

        assert_eq!(fixups.len(), 1);
        assert_eq!(fixups[0].segment_index, 2);
        assert_eq!(fixups[0].patch_offset, 0x1239);
        assert_eq!(fixups[0].location, Location::Offset16);
        assert_eq!(fixups[0].mode, FixupMode::SelfRelative);
        assert_eq!(
            fixups[0].frame,
            Frame {
                method: FrameMethod::Segment,
                datum: Some(FrameDatum::Index(1)),
            }
        );
        assert_eq!(
            fixups[0].target,
            Target {
                method: TargetMethod::External,
                datum: 3,
                displacement_present: true,
            }
        );
        assert_eq!(fixups[0].displacement, 0x20);
        assert_eq!(fixups[0].displacement_body_offset, Some(5));
    }

    #[test]
    fn resolves_frame_and_target_threads() {
        let records = [
            record(0xa0, &[1, 0, 0, 0, 0, 0]),
            // Frame thread 2: segment 4.  Target thread 1: external 7.
            record(0x9c, &[0x42, 4, 0x09, 7, 0x84, 2, 0xa9, 0x34, 0x12]),
        ];

        let fixups = parse(&records).unwrap();

        assert_eq!(fixups.len(), 1);
        assert_eq!(fixups[0].patch_offset, 2);
        assert_eq!(
            fixups[0].frame,
            Frame {
                method: FrameMethod::Segment,
                datum: Some(FrameDatum::Index(4)),
            }
        );
        assert_eq!(
            fixups[0].target,
            Target {
                method: TargetMethod::External,
                datum: 7,
                displacement_present: true,
            }
        );
        assert_eq!(fixups[0].displacement, 0x1234);
    }

    #[test]
    fn retains_threads_across_fixupp_records() {
        let records = [
            // THREAD records are valid before any enumerated-data record.
            record(0x9c, &[0x40, 3, 0x0a, 9]),
            record(0xa0, &[1, 0, 0, 0, 0, 0, 0, 0]),
            // Frame thread 0 and target thread 2, with a suppressed displacement.
            record(0x9c, &[0x84, 3, 0x8a | 0x04]),
        ];

        let fixups = parse(&records).unwrap();

        assert_eq!(fixups.len(), 1);
        assert_eq!(
            fixups[0].frame,
            Frame {
                method: FrameMethod::Segment,
                datum: Some(FrameDatum::Index(3)),
            }
        );
        assert_eq!(
            fixups[0].target,
            Target {
                method: TargetMethod::External,
                datum: 9,
                displacement_present: false,
            }
        );
        assert_eq!(fixups[0].displacement, 0);
    }

    #[test]
    fn tracks_the_most_recent_ledata_base() {
        let records = [
            record(0xa0, &[1, 0x10, 0, 0, 0, 0, 0, 0, 0]),
            record(0xa0, &[2, 0x20, 0, 0, 0, 0, 0, 0, 0, 0]),
            // Segment-relative offset16; frame by location; target segment
            // with P set, which selects method 4 and omits displacement.
            record(0x9c, &[0xc4, 5, 0x44, 1]),
        ];

        let fixups = parse(&records).unwrap();

        assert_eq!(fixups[0].segment_index, 2);
        assert_eq!(fixups[0].patch_offset, 0x25);
        assert_eq!(fixups[0].mode, FixupMode::SegmentRelative);
        assert_eq!(fixups[0].frame.method, FrameMethod::Location);
        assert_eq!(fixups[0].frame.datum, None);
        assert_eq!(fixups[0].target.method, TargetMethod::Segment);
        assert_eq!(fixups[0].target.datum, 1);
        assert!(!fixups[0].target.displacement_present);
    }

    #[test]
    fn rejects_truncated_and_unresolved_thread_subrecords() {
        let truncated = [record(0xa0, &[1, 0, 0]), record(0x9c, &[0x84])];
        assert!(matches!(
            parse(&truncated),
            Err(FixupError::Read {
                source: ReadError::TruncatedRead {
                    body_offset: 1,
                    requested: 1,
                    remaining: 0,
                    ..
                },
                ..
            })
        ));

        let unresolved = [
            record(0xa0, &[1, 0, 0, 0, 0]),
            record(0x9c, &[0x84, 0, 0x88 | 0x04]),
        ];
        assert!(matches!(
            parse(&unresolved),
            Err(FixupError::Malformed {
                kind: FixupErrorKind::UnresolvedFrameThread(0),
                ..
            })
        ));
    }

    #[test]
    fn rejects_a_reserved_target_thread_method() {
        let records = [record(0x9c, &[0x10, 1])];

        assert!(matches!(
            parse(&records),
            Err(FixupError::Malformed {
                kind: FixupErrorKind::UnsupportedTargetMethod(4),
                ..
            })
        ));
    }
}
