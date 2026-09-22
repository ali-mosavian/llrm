//! Typed OMF group and public-symbol declarations.
//!
//! Group names are LNAMES references and remain indices here; resolving them
//! belongs to the name-table consumer. Public names, in contrast, are stored
//! in their records and are retained as raw bytes because OMF does not require
//! UTF-8.

use std::fmt;

use super::read::{ReadError, Reader};
use super::record::Record;

const PUBDEF: u8 = 0x90;
const PUBDEF32: u8 = 0x91;
const GRPDEF: u8 = 0x9a;
const LPUBDEF: u8 = 0xb6;
const LPUBDEF32: u8 = 0xb7;
const SEGMENT_COMPONENT: u8 = 0xff;

/// All group and public declarations in an OMF record stream.
///
/// Both vectors preserve input-record order. `record_index` is the zero-based
/// index in the input slice, while `record_offset` is the optional absolute
/// byte offset of the record header in its source stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declarations {
    /// GRPDEF declarations in record-stream order.
    pub groups: Vec<GroupDefinition>,
    /// PUBDEF and LPUBDEF declarations in record-stream order.
    pub publics: Vec<PublicDefinition>,
}

impl Declarations {
    /// Decodes GRPDEF, PUBDEF, PUBDEF32, LPUBDEF, and LPUBDEF32 records.
    pub fn parse(records: &[Record]) -> Result<Self, DeclarationError> {
        parse(records)
    }
}

/// One GRPDEF declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupDefinition {
    /// Zero-based position of this record in the source record stream.
    pub record_index: usize,
    /// Absolute byte offset of the record header, if the record was parsed.
    pub record_offset: Option<usize>,
    /// One-based position of this group among GRPDEF records.
    pub group_index: usize,
    /// One-based index into the module's LNAMES table.
    pub name_index: u16,
    /// The group's component segment references, in record order.
    pub members: Vec<GroupMember>,
}

/// One member of a GRPDEF declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupMember {
    /// A one-based reference to a SEGDEF record.
    Segment { segment_index: u16 },
}

/// Whether a public definition is externally visible or local to its module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicScope {
    /// A PUBDEF/PUBDEF32 definition, usable to satisfy an external reference.
    External,
    /// An LPUBDEF/LPUBDEF32 definition, local to this object module.
    Local,
}

/// The address base shared by public definitions in one declaration record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicBase {
    /// A possibly ungrouped segment. A zero group index means ungrouped.
    GroupSegment {
        /// One-based GRPDEF index, or zero for no group.
        group_index: u16,
        /// One-based SEGDEF index.
        segment_index: u16,
    },
    /// An absolute frame, encoded when the segment index is zero.
    Frame {
        /// The group index retained exactly as encoded.
        group_index: u16,
        frame: u16,
    },
}

/// One symbol declared by a PUBDEF or LPUBDEF record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicDefinition {
    /// Zero-based position of this record in the source record stream.
    pub record_index: usize,
    /// Absolute byte offset of the record header, if the record was parsed.
    pub record_offset: Option<usize>,
    /// Whether this record is public outside its module.
    pub scope: PublicScope,
    /// The group/segment or absolute-frame address base for this symbol.
    pub base: PublicBase,
    /// Symbol name bytes exactly as stored by OMF.
    pub name: Vec<u8>,
    /// Offset from `base`; 16-bit records are widened to `u32`.
    pub offset: u32,
    /// OMF type index associated with this symbol.
    pub type_index: u16,
}

/// Decodes GRPDEF, PUBDEF, PUBDEF32, LPUBDEF, and LPUBDEF32 records.
pub fn parse(records: &[Record]) -> Result<Declarations, DeclarationError> {
    let mut declarations = Declarations {
        groups: Vec::new(),
        publics: Vec::new(),
    };

    for (record_index, record) in records.iter().enumerate() {
        match record.record_type() {
            GRPDEF => declarations.groups.push(parse_group(
                record_index,
                declarations.groups.len() + 1,
                record,
            )?),
            PUBDEF => parse_publics(
                record_index,
                record,
                PublicScope::External,
                false,
                &mut declarations.publics,
            )?,
            PUBDEF32 => parse_publics(
                record_index,
                record,
                PublicScope::External,
                true,
                &mut declarations.publics,
            )?,
            LPUBDEF => parse_publics(
                record_index,
                record,
                PublicScope::Local,
                false,
                &mut declarations.publics,
            )?,
            LPUBDEF32 => parse_publics(
                record_index,
                record,
                PublicScope::Local,
                true,
                &mut declarations.publics,
            )?,
            _ => {}
        }
    }

    Ok(declarations)
}

fn parse_group(
    record_index: usize,
    group_index: usize,
    record: &Record,
) -> Result<GroupDefinition, DeclarationError> {
    let mut reader = reader_for(record);
    let name_index = read(record, &mut reader, Reader::read_index)?;
    let mut members = Vec::new();
    while !reader.is_empty() {
        let body_offset = reader.position();
        let component_type = read(record, &mut reader, Reader::read_u8)?;
        if component_type != SEGMENT_COMPONENT {
            return Err(context(
                record,
                DeclarationErrorKind::UnsupportedGroupMember {
                    body_offset,
                    absolute_offset: absolute_body_offset(record, body_offset),
                    component_type,
                },
            ));
        }
        let segment_index = read(record, &mut reader, Reader::read_index)?;
        members.push(GroupMember::Segment { segment_index });
    }

    Ok(GroupDefinition {
        record_index,
        record_offset: record.offset(),
        group_index,
        name_index,
        members,
    })
}

fn parse_publics(
    record_index: usize,
    record: &Record,
    scope: PublicScope,
    wide: bool,
    publics: &mut Vec<PublicDefinition>,
) -> Result<(), DeclarationError> {
    let mut reader = reader_for(record);
    let group_index = read(record, &mut reader, Reader::read_index)?;
    let segment_index = read(record, &mut reader, Reader::read_index)?;
    let base = if segment_index == 0 {
        PublicBase::Frame {
            group_index,
            frame: read(record, &mut reader, Reader::read_u16)?,
        }
    } else {
        PublicBase::GroupSegment {
            group_index,
            segment_index,
        }
    };

    while !reader.is_empty() {
        let name = read(record, &mut reader, Reader::read_name)?.to_vec();
        let offset = if wide {
            read(record, &mut reader, Reader::read_u32)?
        } else {
            u32::from(read(record, &mut reader, Reader::read_u16)?)
        };
        let type_index = read(record, &mut reader, Reader::read_index)?;
        publics.push(PublicDefinition {
            record_index,
            record_offset: record.offset(),
            scope,
            base,
            name,
            offset,
            type_index,
        });
    }

    Ok(())
}

fn reader_for(record: &Record) -> Reader<'_> {
    Reader::new(
        record.body(),
        record.offset().and_then(|offset| offset.checked_add(3)),
    )
}

fn read<'a, T>(
    record: &Record,
    reader: &mut Reader<'a>,
    operation: impl FnOnce(&mut Reader<'a>) -> Result<T, ReadError>,
) -> Result<T, DeclarationError> {
    operation(reader).map_err(|source| context(record, DeclarationErrorKind::Read(source)))
}

fn absolute_body_offset(record: &Record, body_offset: usize) -> Option<usize> {
    record
        .offset()
        .and_then(|offset| offset.checked_add(3))
        .and_then(|offset| offset.checked_add(body_offset))
}

fn context(record: &Record, kind: DeclarationErrorKind) -> DeclarationError {
    DeclarationError {
        record_offset: record.offset(),
        record_type: record.record_type(),
        kind,
    }
}

/// A malformed group or public declaration record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationError {
    /// Absolute byte offset of the malformed record header, if known.
    pub record_offset: Option<usize>,
    /// Raw OMF record type that failed to decode.
    pub record_type: u8,
    /// The specific malformed declaration detail.
    pub kind: DeclarationErrorKind,
}

/// The semantic or primitive failure in a declaration record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationErrorKind {
    /// A bounded primitive field read ran past the record body.
    Read(ReadError),
    /// GRPDEF used a component form other than an OMF segment reference.
    UnsupportedGroupMember {
        /// Position of the component-type byte within the record body.
        body_offset: usize,
        /// Absolute position of the component-type byte, if known.
        absolute_offset: Option<usize>,
        /// The unsupported component-type byte.
        component_type: u8,
    },
}

impl fmt::Display for DeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "OMF declaration record type {:02x}",
            self.record_type
        )?;
        if let Some(offset) = self.record_offset {
            write!(formatter, " at byte {offset}")?;
        }
        write!(formatter, ": ")?;
        match &self.kind {
            DeclarationErrorKind::Read(source) => source.fmt(formatter),
            DeclarationErrorKind::UnsupportedGroupMember {
                body_offset,
                absolute_offset,
                component_type,
            } => {
                write!(
                    formatter,
                    "unsupported GRPDEF component {component_type:02x} at body byte {body_offset}"
                )?;
                if let Some(offset) = absolute_offset {
                    write!(formatter, " (absolute byte {offset})")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for DeclarationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            DeclarationErrorKind::Read(source) => Some(source),
            DeclarationErrorKind::UnsupportedGroupMember { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DeclarationErrorKind, GroupMember, PublicBase, PublicScope, parse};
    use crate::object::omf::read::ReadError;
    use crate::object::omf::record::Record;

    #[test]
    fn decodes_sixteen_and_thirty_two_bit_public_offsets() {
        let records = [
            Record::new(0x90, vec![1, 2, 3, b'f', b'o', b'o', 0x34, 0x12, 7]).unwrap(),
            Record::new(
                0x91,
                vec![
                    3, 4, 3, b'b', b'a', b'r', 0x78, 0x56, 0x34, 0x12, 0x81, 0x23,
                ],
            )
            .unwrap(),
        ];

        let declarations = parse(&records).unwrap();

        assert_eq!(declarations.publics[0].offset, 0x1234);
        assert_eq!(declarations.publics[1].offset, 0x1234_5678);
        assert_eq!(declarations.publics[0].type_index, 7);
        assert_eq!(declarations.publics[1].type_index, 0x0123);
        assert_eq!(declarations.publics[0].scope, PublicScope::External);
        assert_eq!(
            declarations.publics[1].base,
            PublicBase::GroupSegment {
                group_index: 3,
                segment_index: 4,
            }
        );
        assert_eq!(
            (
                declarations.publics[0].record_index,
                declarations.publics[1].record_index
            ),
            (0, 1)
        );
    }

    #[test]
    fn decodes_an_absolute_frame_public_base() {
        let record = Record::new(0xb6, vec![0, 0, 0x34, 0x12, 1, b'x', 0x78, 0x56, 0]).unwrap();

        let declarations = parse(&[record]).unwrap();

        assert_eq!(declarations.publics[0].scope, PublicScope::Local);
        assert_eq!(
            declarations.publics[0].base,
            PublicBase::Frame {
                group_index: 0,
                frame: 0x1234,
            }
        );
        assert_eq!(declarations.publics[0].offset, 0x5678);
    }

    #[test]
    fn consumes_a_frame_whenever_the_segment_index_is_zero() {
        let record = Record::new(0x90, vec![5, 0, 0x34, 0x12, 1, b'x', 0x78, 0x56, 0]).unwrap();

        let declarations = parse(&[record]).unwrap();

        assert_eq!(
            declarations.publics[0].base,
            PublicBase::Frame {
                group_index: 5,
                frame: 0x1234,
            }
        );
        assert_eq!(declarations.publics[0].offset, 0x5678);
    }

    #[test]
    fn decodes_group_segment_members() {
        let record = Record::new(0x9a, vec![0x81, 0x23, 0xff, 2, 0xff, 0x81, 0x24]).unwrap();

        let declarations = parse(&[record]).unwrap();

        assert_eq!(declarations.groups[0].group_index, 1);
        assert_eq!(declarations.groups[0].name_index, 0x0123);
        assert_eq!(
            declarations.groups[0].members,
            [
                GroupMember::Segment { segment_index: 2 },
                GroupMember::Segment {
                    segment_index: 0x0124
                },
            ]
        );
    }

    #[test]
    fn preserves_raw_non_utf8_public_names() {
        let record = Record::new(0x90, vec![0, 1, 2, b'a', 0xff, 0, 0, 0]).unwrap();

        let declarations = parse(&[record]).unwrap();

        assert_eq!(declarations.publics[0].name, [b'a', 0xff]);
    }

    #[test]
    fn rejects_a_truncated_public_offset() {
        let record = Record::new(0x90, vec![0, 1, 1, b'x', 0x34]).unwrap();

        let error = parse(&[record]).unwrap_err();

        assert_eq!(error.record_type, 0x90);
        assert_eq!(
            error.kind,
            DeclarationErrorKind::Read(ReadError::TruncatedRead {
                body_offset: 4,
                absolute_offset: None,
                requested: 2,
                remaining: 1,
            })
        );
    }
}
