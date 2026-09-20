//! Construction of fresh, target-neutral 16-bit Intel OMF modules.
//!
//! Callers provide OMF sections, symbols, and relocation semantics. This
//! module owns record ordering, indices, chunking, and checksums; it knows
//! nothing about source languages, instruction sets, or MC fixup identities.

use std::error::Error;
use std::fmt;

use super::fixups::{FixupMode, Location};
use super::record::{Record, RecordError};
use super::segments::{Alignment, Combine};

const THEADR: u8 = 0x80;
const MODEND: u8 = 0x8a;
const EXTDEF: u8 = 0x8c;
const PUBDEF: u8 = 0x90;
const LNAMES: u8 = 0x96;
const SEGDEF: u8 = 0x98;
const FIXUPP: u8 = 0x9c;
const LEDATA: u8 = 0xa0;

/// Maximum initialized payload per LEDATA record.
///
/// FIXUPP patch offsets have ten bits. The established writer uses 1,000
/// bytes, leaving room for a relocation field without approaching that limit.
pub const DATA_CHUNK_SIZE: usize = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectModule {
    pub name: Vec<u8>,
    pub segments: Vec<ObjectSegment>,
    pub externals: Vec<ExternalSymbol>,
    pub publics: Vec<PublicSymbol>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectSegment {
    pub name: Vec<u8>,
    pub class_name: Vec<u8>,
    pub alignment: Alignment,
    pub combine: Combine,
    pub length: u32,
    pub initialized: Vec<InitializedSpan>,
    pub relocations: Vec<ObjectRelocation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializedSpan {
    pub offset: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalSymbol {
    pub name: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicSymbol {
    pub name: Vec<u8>,
    /// One-based SEGDEF index.
    pub segment_index: u16,
    pub offset: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectRelocation {
    pub offset: u32,
    pub location: Location,
    pub mode: FixupMode,
    pub target: RelocationTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelocationTarget {
    /// One-based SEGDEF index.
    Segment(u16),
    /// One-based EXTDEF index.
    External(u16),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriteError {
    Record(RecordError),
    NameTooLong {
        kind: &'static str,
        length: usize,
    },
    TooManySegments {
        count: usize,
    },
    TooManyExternals {
        count: usize,
    },
    UnsupportedAlignment {
        segment: usize,
        alignment: Alignment,
    },
    SegmentTooLarge {
        segment: usize,
        length: u32,
    },
    SpanOverflow {
        segment: usize,
        offset: u32,
        length: usize,
    },
    SpanOutsideSegment {
        segment: usize,
        start: u32,
        end: u32,
        length: u32,
    },
    OverlappingSpans {
        segment: usize,
        previous_end: u32,
        next_start: u32,
    },
    UnsupportedLocation {
        segment: usize,
        location: Location,
    },
    RelocationOutsideData {
        segment: usize,
        offset: u32,
        width: u8,
    },
    OverlappingRelocations {
        segment: usize,
        previous_end: u32,
        next_start: u32,
    },
    InvalidSegmentIndex {
        index: u16,
        count: usize,
    },
    InvalidExternalIndex {
        index: u16,
        count: usize,
    },
    PublicOutsideSegment {
        segment: u16,
        offset: u32,
        length: u32,
    },
    RelocationCannotFit {
        segment: usize,
        offset: u32,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record(error) => error.fmt(formatter),
            Self::NameTooLong { kind, length } => {
                write!(
                    formatter,
                    "OMF {kind} name is {length} bytes; maximum is 255"
                )
            }
            Self::TooManySegments { count } => {
                write!(
                    formatter,
                    "OMF module has {count} segments; maximum is 32767"
                )
            }
            Self::TooManyExternals { count } => {
                write!(
                    formatter,
                    "OMF module has {count} externals; maximum is 32767"
                )
            }
            Self::UnsupportedAlignment { segment, alignment } => {
                write!(
                    formatter,
                    "OMF segment {segment} has unsupported alignment {alignment:?}"
                )
            }
            Self::SegmentTooLarge { segment, length } => {
                write!(
                    formatter,
                    "OMF16 segment {segment} is {length} bytes; maximum is 65536"
                )
            }
            Self::SpanOverflow {
                segment,
                offset,
                length,
            } => write!(
                formatter,
                "OMF segment {segment} initialized span at {offset:#x} overflows by {length} bytes"
            ),
            Self::SpanOutsideSegment {
                segment,
                start,
                end,
                length,
            } => write!(
                formatter,
                "OMF segment {segment} initialized span {start:#x}..{end:#x} exceeds length {length:#x}"
            ),
            Self::OverlappingSpans {
                segment,
                previous_end,
                next_start,
            } => write!(
                formatter,
                "OMF segment {segment} initialized spans overlap at {next_start:#x} before {previous_end:#x}"
            ),
            Self::UnsupportedLocation { segment, location } => write!(
                formatter,
                "OMF segment {segment} uses unsupported 16-bit FIXUPP location {location:?}"
            ),
            Self::RelocationOutsideData {
                segment,
                offset,
                width,
            } => write!(
                formatter,
                "OMF segment {segment} relocation field {offset:#x}..{:#x} is outside initialized data",
                u64::from(*offset) + u64::from(*width)
            ),
            Self::OverlappingRelocations {
                segment,
                previous_end,
                next_start,
            } => write!(
                formatter,
                "OMF segment {segment} relocation fields overlap at {next_start:#x} before {previous_end:#x}"
            ),
            Self::InvalidSegmentIndex { index, count } => write!(
                formatter,
                "OMF segment index {index} is outside the one-based table of {count} segments"
            ),
            Self::InvalidExternalIndex { index, count } => write!(
                formatter,
                "OMF external index {index} is outside the one-based table of {count} externals"
            ),
            Self::PublicOutsideSegment {
                segment,
                offset,
                length,
            } => write!(
                formatter,
                "OMF public at {offset:#x} is outside segment {segment} length {length:#x}"
            ),
            Self::RelocationCannotFit { segment, offset } => write!(
                formatter,
                "OMF segment {segment} relocation at {offset:#x} cannot fit in one LEDATA record"
            ),
        }
    }
}

impl Error for WriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Record(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RecordError> for WriteError {
    fn from(error: RecordError) -> Self {
        Self::Record(error)
    }
}

/// Constructs a deterministic, fresh 16-bit OMF record stream.
pub fn records(module: &ObjectModule) -> Result<Vec<Record>, WriteError> {
    validate_count(module.segments.len(), true)?;
    validate_count(module.externals.len(), false)?;
    let mut out = vec![Record::new(THEADR, encoded_name("module", &module.name)?)?];

    let mut names = vec![Vec::new()];
    let mut segment_name_indices = Vec::with_capacity(module.segments.len());
    for segment in &module.segments {
        let class = push_name(&mut names, "class", &segment.class_name)?;
        let name = push_name(&mut names, "segment", &segment.name)?;
        segment_name_indices.push((name, class));
    }
    let mut lnames = Vec::new();
    for name in &names {
        lnames.extend_from_slice(&encoded_name("logical", name)?);
    }
    out.push(Record::new(LNAMES, lnames)?);

    for (position, (segment, (name, class))) in module
        .segments
        .iter()
        .zip(&segment_name_indices)
        .enumerate()
    {
        let index = position + 1;
        validate_segment(index, segment)?;
        let mut body = vec![segment_attributes(index, segment)?];
        let encoded_length = if segment.length == 0x1_0000 {
            0
        } else {
            segment.length as u16
        };
        body.extend_from_slice(&encoded_length.to_le_bytes());
        body.extend_from_slice(&encode_index(*name));
        body.extend_from_slice(&encode_index(*class));
        body.extend_from_slice(&encode_index(1));
        out.push(Record::new(SEGDEF, body)?);
    }

    if !module.externals.is_empty() {
        let mut body = Vec::new();
        for external in &module.externals {
            body.extend_from_slice(&encoded_name("external", &external.name)?);
            body.extend_from_slice(&encode_index(0));
        }
        out.push(Record::new(EXTDEF, body)?);
    }

    for segment_index in 1..=module.segments.len() {
        let mut body = Vec::new();
        for public in module
            .publics
            .iter()
            .filter(|public| usize::from(public.segment_index) == segment_index)
        {
            validate_public(module, public)?;
            body.extend_from_slice(&encoded_name("public", &public.name)?);
            body.extend_from_slice(&(public.offset as u16).to_le_bytes());
            body.extend_from_slice(&encode_index(0));
        }
        if !body.is_empty() {
            let mut header = encode_index(0);
            header.extend_from_slice(&encode_index(segment_index as u16));
            header.extend_from_slice(&body);
            out.push(Record::new(PUBDEF, header)?);
        }
    }
    for public in &module.publics {
        validate_public(module, public)?;
    }

    for (position, segment) in module.segments.iter().enumerate() {
        out.extend(data_records(
            position + 1,
            segment,
            module.segments.len(),
            module.externals.len(),
        )?);
    }
    out.push(Record::new(MODEND, vec![0])?);
    Ok(out)
}

pub fn to_bytes(module: &ObjectModule) -> Result<Vec<u8>, WriteError> {
    let records = records(module)?;
    Ok(records.iter().flat_map(Record::to_bytes).collect())
}

fn validate_count(count: usize, segments: bool) -> Result<(), WriteError> {
    if count <= 0x7fff {
        Ok(())
    } else if segments {
        Err(WriteError::TooManySegments { count })
    } else {
        Err(WriteError::TooManyExternals { count })
    }
}

fn push_name(names: &mut Vec<Vec<u8>>, kind: &'static str, name: &[u8]) -> Result<u16, WriteError> {
    encoded_name(kind, name)?;
    names.push(name.to_vec());
    let index = u16::try_from(names.len())
        .map_err(|_| WriteError::TooManySegments { count: names.len() })?;
    if index > 0x7fff {
        return Err(WriteError::TooManySegments { count: names.len() });
    }
    Ok(index)
}

fn encoded_name(kind: &'static str, name: &[u8]) -> Result<Vec<u8>, WriteError> {
    let length = u8::try_from(name.len()).map_err(|_| WriteError::NameTooLong {
        kind,
        length: name.len(),
    })?;
    let mut encoded = Vec::with_capacity(name.len() + 1);
    encoded.push(length);
    encoded.extend_from_slice(name);
    Ok(encoded)
}

fn encode_index(index: u16) -> Vec<u8> {
    if index < 0x80 {
        vec![index as u8]
    } else {
        vec![0x80 | (index >> 8) as u8, index as u8]
    }
}

fn segment_attributes(index: usize, segment: &ObjectSegment) -> Result<u8, WriteError> {
    let alignment = match segment.alignment {
        Alignment::Byte => 1,
        Alignment::Word => 2,
        Alignment::Paragraph => 3,
        Alignment::Page => 4,
        Alignment::DoubleWord => 5,
        Alignment::Page4K => 6,
        Alignment::Absolute { .. } => {
            return Err(WriteError::UnsupportedAlignment {
                segment: index,
                alignment: segment.alignment,
            });
        }
    };
    let combine = match segment.combine {
        Combine::Private => 0,
        Combine::Public => 2,
        Combine::Stack => 5,
        Combine::Common => 6,
    };
    Ok((alignment << 5) | (combine << 2) | u8::from(segment.length == 0x1_0000) << 1)
}

fn validate_segment(index: usize, segment: &ObjectSegment) -> Result<(), WriteError> {
    if segment.length > 0x1_0000 {
        return Err(WriteError::SegmentTooLarge {
            segment: index,
            length: segment.length,
        });
    }
    let mut spans = segment.initialized.iter().collect::<Vec<_>>();
    spans.sort_by_key(|span| span.offset);
    let mut previous_end = 0;
    for span in spans {
        let byte_length =
            u32::try_from(span.bytes.len()).map_err(|_| WriteError::SpanOverflow {
                segment: index,
                offset: span.offset,
                length: span.bytes.len(),
            })?;
        let end = span
            .offset
            .checked_add(byte_length)
            .ok_or(WriteError::SpanOverflow {
                segment: index,
                offset: span.offset,
                length: span.bytes.len(),
            })?;
        if end > segment.length {
            return Err(WriteError::SpanOutsideSegment {
                segment: index,
                start: span.offset,
                end,
                length: segment.length,
            });
        }
        if span.offset < previous_end {
            return Err(WriteError::OverlappingSpans {
                segment: index,
                previous_end,
                next_start: span.offset,
            });
        }
        previous_end = end;
    }
    Ok(())
}

fn validate_public(module: &ObjectModule, public: &PublicSymbol) -> Result<(), WriteError> {
    let Some(segment) = public
        .segment_index
        .checked_sub(1)
        .and_then(|index| module.segments.get(usize::from(index)))
    else {
        return Err(WriteError::InvalidSegmentIndex {
            index: public.segment_index,
            count: module.segments.len(),
        });
    };
    if public.offset > segment.length || public.offset > u32::from(u16::MAX) {
        return Err(WriteError::PublicOutsideSegment {
            segment: public.segment_index,
            offset: public.offset,
            length: segment.length,
        });
    }
    Ok(())
}

fn data_records(
    segment_index: usize,
    segment: &ObjectSegment,
    segment_count: usize,
    external_count: usize,
) -> Result<Vec<Record>, WriteError> {
    let mut spans = segment.initialized.iter().collect::<Vec<_>>();
    spans.sort_by_key(|span| span.offset);
    let mut relocations = segment.relocations.clone();
    relocations.sort_by_key(|relocation| relocation.offset);
    validate_relocations(
        segment_index,
        &spans,
        &relocations,
        segment_count,
        external_count,
    )?;

    let mut out = Vec::new();
    let mut placed = 0;
    for span in spans {
        let span_end = span.offset + span.bytes.len() as u32;
        let mut start = span.offset;
        while start < span_end {
            let mut stop = span_end.min(start + DATA_CHUNK_SIZE as u32);
            for relocation in &relocations {
                let width = u32::from(location_width(segment_index, relocation.location)?);
                if relocation.offset < stop && stop < relocation.offset + width {
                    stop = stop.min(relocation.offset);
                }
            }
            if stop <= start {
                return Err(WriteError::RelocationCannotFit {
                    segment: segment_index,
                    offset: start,
                });
            }
            let low = (start - span.offset) as usize;
            let high = (stop - span.offset) as usize;
            let mut body = encode_index(segment_index as u16);
            body.extend_from_slice(&(start as u16).to_le_bytes());
            body.extend_from_slice(&span.bytes[low..high]);
            out.push(Record::new(LEDATA, body)?);

            let inside = relocations
                .iter()
                .filter(|relocation| start <= relocation.offset && relocation.offset < stop)
                .collect::<Vec<_>>();
            if !inside.is_empty() {
                let mut fixupp = Vec::new();
                for relocation in &inside {
                    fixupp.extend_from_slice(&encode_fixup(
                        segment_index,
                        relocation,
                        relocation.offset - start,
                    )?);
                }
                placed += inside.len();
                out.push(Record::new(FIXUPP, fixupp)?);
            }
            start = stop;
        }
    }
    if placed != relocations.len() {
        let relocation = relocations
            .iter()
            .find(|relocation| {
                !segment.initialized.iter().any(|span| {
                    let end = span.offset + span.bytes.len() as u32;
                    span.offset <= relocation.offset && relocation.offset < end
                })
            })
            .unwrap_or(&relocations[placed]);
        return Err(WriteError::RelocationOutsideData {
            segment: segment_index,
            offset: relocation.offset,
            width: location_width(segment_index, relocation.location)?,
        });
    }
    Ok(out)
}

fn validate_relocations(
    segment_index: usize,
    spans: &[&InitializedSpan],
    relocations: &[ObjectRelocation],
    segment_count: usize,
    external_count: usize,
) -> Result<(), WriteError> {
    let mut previous_end = 0;
    for relocation in relocations {
        let width = location_width(segment_index, relocation.location)?;
        let end = relocation.offset.checked_add(u32::from(width)).ok_or(
            WriteError::RelocationOutsideData {
                segment: segment_index,
                offset: relocation.offset,
                width,
            },
        )?;
        if relocation.offset < previous_end {
            return Err(WriteError::OverlappingRelocations {
                segment: segment_index,
                previous_end,
                next_start: relocation.offset,
            });
        }
        previous_end = end;
        if !spans.iter().any(|span| {
            let span_end = span.offset + span.bytes.len() as u32;
            span.offset <= relocation.offset && end <= span_end
        }) {
            return Err(WriteError::RelocationOutsideData {
                segment: segment_index,
                offset: relocation.offset,
                width,
            });
        }
        match relocation.target {
            RelocationTarget::Segment(index)
                if index == 0 || usize::from(index) > segment_count =>
            {
                return Err(WriteError::InvalidSegmentIndex {
                    index,
                    count: segment_count,
                });
            }
            RelocationTarget::External(index)
                if index == 0 || usize::from(index) > external_count =>
            {
                return Err(WriteError::InvalidExternalIndex {
                    index,
                    count: external_count,
                });
            }
            RelocationTarget::Segment(_) | RelocationTarget::External(_) => {}
        }
    }
    Ok(())
}

fn location_width(segment: usize, location: Location) -> Result<u8, WriteError> {
    match location {
        Location::LowByte | Location::HighByte => Ok(1),
        Location::Offset16 | Location::Base16 => Ok(2),
        Location::Pointer16_16 => Ok(4),
        Location::LoaderResolvedOffset16
        | Location::Offset32
        | Location::Pointer16_32
        | Location::LoaderResolvedOffset32 => {
            Err(WriteError::UnsupportedLocation { segment, location })
        }
    }
}

fn location_code(segment: usize, location: Location) -> Result<u8, WriteError> {
    match location {
        Location::LowByte => Ok(0),
        Location::Offset16 => Ok(1),
        Location::Base16 => Ok(2),
        Location::Pointer16_16 => Ok(3),
        Location::HighByte => Ok(4),
        _ => Err(WriteError::UnsupportedLocation { segment, location }),
    }
}

fn encode_fixup(
    segment: usize,
    relocation: &ObjectRelocation,
    data_offset: u32,
) -> Result<Vec<u8>, WriteError> {
    if data_offset >= 1024 {
        return Err(WriteError::RelocationCannotFit {
            segment,
            offset: relocation.offset,
        });
    }
    let mode = match relocation.mode {
        FixupMode::SelfRelative => 0,
        FixupMode::SegmentRelative => 0x40,
    };
    let location = location_code(segment, relocation.location)?;
    let (target_method, target_index) = match relocation.target {
        RelocationTarget::Segment(index) => (0, index),
        RelocationTarget::External(index) => (2, index),
    };
    let mut bytes = vec![
        0x80 | mode | (location << 2) | ((data_offset >> 8) as u8),
        data_offset as u8,
        0x50 | 0x04 | target_method,
    ];
    bytes.extend_from_slice(&encode_index(target_index));
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::omf::{data, declarations, fixups, segments, symbols};

    fn segment(bytes: Vec<u8>, relocations: Vec<ObjectRelocation>) -> ObjectSegment {
        ObjectSegment {
            name: b"text".to_vec(),
            class_name: b"code".to_vec(),
            alignment: Alignment::Word,
            combine: Combine::Public,
            length: bytes.len() as u32,
            initialized: vec![InitializedSpan { offset: 0, bytes }],
            relocations,
        }
    }

    fn module(segment: ObjectSegment) -> ObjectModule {
        ObjectModule {
            name: b"unit.c".to_vec(),
            segments: vec![segment],
            externals: vec![ExternalSymbol {
                name: b"callee".to_vec(),
            }],
            publics: vec![PublicSymbol {
                name: b"entry".to_vec(),
                segment_index: 1,
                offset: 0,
            }],
        }
    }

    #[test]
    fn constructed_records_decode_to_the_supplied_object_facts() {
        let relocation = ObjectRelocation {
            offset: 1,
            location: Location::Pointer16_16,
            mode: FixupMode::SegmentRelative,
            target: RelocationTarget::External(1),
        };
        let records = records(&module(segment(vec![0x9a, 3, 0, 0, 0], vec![relocation])))
            .expect("construct object records");

        let names = symbols::parse(&records).unwrap();
        assert_eq!(
            names.names[1..],
            [Vec::new(), b"code".to_vec(), b"text".to_vec()]
        );
        assert_eq!(names.externals[1].as_ref().unwrap().name, b"callee");
        let decoded_segments = segments::parse(&records).unwrap();
        assert_eq!(decoded_segments.segments[1].as_ref().unwrap().length, 5);
        assert_eq!(data::parse(&records).unwrap()[0].bytes, [0x9a, 3, 0, 0, 0]);
        assert_eq!(
            declarations::parse(&records).unwrap().publics[0].name,
            b"entry"
        );
        let decoded = fixups::parse(&records).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].patch_offset, 1);
        assert_eq!(decoded[0].location, Location::Pointer16_16);
        assert_eq!(decoded[0].target.method, fixups::TargetMethod::External);
        assert!(!decoded[0].target.displacement_present);
        assert_eq!(decoded[0].frame.method, fixups::FrameMethod::Target);
    }

    #[test]
    fn chunks_without_splitting_a_relocation_field() {
        let bytes = vec![0; DATA_CHUNK_SIZE * 3];
        let relocations = [DATA_CHUNK_SIZE - 1, DATA_CHUNK_SIZE * 2 - 2]
            .into_iter()
            .map(|offset| ObjectRelocation {
                offset: offset as u32,
                location: Location::Pointer16_16,
                mode: FixupMode::SegmentRelative,
                target: RelocationTarget::Segment(1),
            })
            .collect();
        let records = records(&module(segment(bytes, relocations))).unwrap();
        let blocks = data::parse(&records).unwrap();
        let cuts = blocks
            .iter()
            .map(|block| block.offset as usize + block.bytes.len())
            .collect::<Vec<_>>();
        for start in [DATA_CHUNK_SIZE - 1, DATA_CHUNK_SIZE * 2 - 2] {
            assert!(!cuts.iter().any(|cut| start < *cut && *cut < start + 4));
        }
        assert!(
            blocks
                .iter()
                .all(|block| block.bytes.len() <= DATA_CHUNK_SIZE)
        );
    }

    #[test]
    fn output_is_deterministic_and_every_fixup_is_explicit() {
        let object = module(segment(
            vec![0; 4],
            vec![ObjectRelocation {
                offset: 0,
                location: Location::Offset16,
                mode: FixupMode::SegmentRelative,
                target: RelocationTarget::Segment(1),
            }],
        ));
        assert_eq!(to_bytes(&object).unwrap(), to_bytes(&object).unwrap());
        let records = records(&object).unwrap();
        assert!(
            records
                .iter()
                .filter(|record| record.record_type() == FIXUPP)
                .all(|record| record.body().first().is_some_and(|lead| lead & 0x80 != 0))
        );
    }

    #[test]
    fn refuses_overlapping_or_uninitialized_relocation_fields() {
        let overlapping = module(segment(
            vec![0; 4],
            vec![
                ObjectRelocation {
                    offset: 0,
                    location: Location::Offset16,
                    mode: FixupMode::SegmentRelative,
                    target: RelocationTarget::Segment(1),
                },
                ObjectRelocation {
                    offset: 1,
                    location: Location::Offset16,
                    mode: FixupMode::SegmentRelative,
                    target: RelocationTarget::Segment(1),
                },
            ],
        ));
        assert!(matches!(
            records(&overlapping),
            Err(WriteError::OverlappingRelocations { .. })
        ));

        let mut outside = module(segment(
            Vec::new(),
            vec![ObjectRelocation {
                offset: 0,
                location: Location::Offset16,
                mode: FixupMode::SegmentRelative,
                target: RelocationTarget::Segment(1),
            }],
        ));
        outside.segments[0].length = 2;
        assert!(matches!(
            records(&outside),
            Err(WriteError::RelocationOutsideData { .. })
        ));
    }
}
