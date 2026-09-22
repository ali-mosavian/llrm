//! Lossless framing for Intel OMF records.
//!
//! OMF stores records as a type byte, a little-endian length, a body, and a
//! checksum. The length includes the checksum but not the three-byte header.
//! This module deliberately knows nothing about the meaning of a record body:
//! semantic OMF decoding belongs in later layers.

use std::fmt;

/// An OMF record type as it appears on disk.
///
/// Keeping this as a raw byte lets callers preserve unknown and vendor record
/// types without first expanding a central enum.
pub type RecordType = u8;

/// A framed OMF record.
///
/// Parsed records retain their on-disk checksum, including known producer
/// bugs. Consequently [`Record::to_bytes`] is lossless. Constructed or
/// deliberately changed records get a newly computed checksum instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    /// Byte offset of the record header in its parsed input.
    ///
    /// Constructed records have no source offset.
    offset: Option<usize>,
    record_type: RecordType,
    body: Vec<u8>,
    checksum: u8,
}

impl Record {
    /// Builds a new record with a checksum computed from its current contents.
    pub fn new(record_type: RecordType, body: Vec<u8>) -> Result<Self, RecordError> {
        Self::new_at(None, record_type, body)
    }

    /// Returns the record header's byte offset when this record was parsed.
    pub const fn offset(&self) -> Option<usize> {
        self.offset
    }

    /// Returns the raw OMF record type.
    pub const fn record_type(&self) -> RecordType {
        self.record_type
    }

    /// Returns the uninterpreted body, without its trailing checksum.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns the checksum byte exactly as stored or last computed.
    pub const fn checksum(&self) -> u8 {
        self.checksum
    }

    /// Returns whether this record's stored checksum makes its bytes sum to
    /// zero modulo 256.
    ///
    /// A false result is reported, but does not make an otherwise well-framed
    /// record unparseable: BC has emitted checksum-invalid FIXUPP records.
    pub fn is_checksum_valid(&self) -> bool {
        self.checksum == self.expected_checksum()
    }

    /// Returns an error when the retained checksum is not valid.
    ///
    /// Parsing intentionally does not call this method. Consumers which need
    /// strict checksum validation can opt into it after preserving the input.
    pub fn validate_checksum(&self) -> Result<(), RecordError> {
        let expected = self.expected_checksum();
        if self.checksum == expected {
            Ok(())
        } else {
            Err(RecordError::ChecksumMismatch {
                offset: self.offset,
                record_type: self.record_type,
                expected,
                actual: self.checksum,
            })
        }
    }

    /// Replaces the record body and recomputes its checksum.
    ///
    /// This is intentionally the only mutable body API, so changing a record
    /// cannot accidentally retain a checksum for its former contents.
    pub fn replace_body(&mut self, body: Vec<u8>) -> Result<(), RecordError> {
        Self::validate_body_len(body.len())?;
        self.body = body;
        self.recompute_checksum();
        Ok(())
    }

    /// Recomputes the checksum for this record's current type and body.
    pub fn recompute_checksum(&mut self) {
        self.checksum = self.expected_checksum();
    }

    /// Serializes the record with its retained checksum byte.
    ///
    /// In particular, an untouched checksum-invalid parsed record is emitted
    /// byte-identically. Use [`Record::recompute_checksum`] only when a caller
    /// deliberately wants canonical checksum output.
    pub fn to_bytes(&self) -> Vec<u8> {
        let length = u16::try_from(self.body.len() + 1)
            .expect("record bodies are length-checked on construction");
        let mut bytes = Vec::with_capacity(3 + usize::from(length));
        bytes.push(self.record_type);
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&self.body);
        bytes.push(self.checksum);
        bytes
    }

    fn new_at(
        offset: Option<usize>,
        record_type: RecordType,
        body: Vec<u8>,
    ) -> Result<Self, RecordError> {
        Self::validate_body_len(body.len())?;
        let mut record = Self {
            offset,
            record_type,
            body,
            checksum: 0,
        };
        record.recompute_checksum();
        Ok(record)
    }

    fn validate_body_len(body_length: usize) -> Result<(), RecordError> {
        if body_length > usize::from(u16::MAX) - 1 {
            return Err(RecordError::BodyTooLarge { body_length });
        }
        Ok(())
    }

    fn expected_checksum(&self) -> u8 {
        let length = u16::try_from(self.body.len() + 1)
            .expect("record bodies are length-checked on construction");
        let mut sum = self.record_type;
        for byte in length.to_le_bytes() {
            sum = sum.wrapping_add(byte);
        }
        for byte in &self.body {
            sum = sum.wrapping_add(*byte);
        }
        sum.wrapping_neg()
    }
}

/// Framing and checksum errors for OMF record streams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordError {
    /// Fewer than three bytes remain for a record header.
    TruncatedHeader { offset: usize, remaining: usize },
    /// The length field omitted the required checksum byte.
    InvalidLength {
        offset: usize,
        record_type: RecordType,
        declared_length: u16,
    },
    /// A complete header declared bytes not present in the input.
    TruncatedRecord {
        offset: usize,
        record_type: RecordType,
        declared_length: u16,
        available: usize,
    },
    /// An explicitly validated record has a checksum mismatch.
    ChecksumMismatch {
        offset: Option<usize>,
        record_type: RecordType,
        expected: u8,
        actual: u8,
    },
    /// A constructed record cannot be represented by OMF's 16-bit length.
    BodyTooLarge { body_length: usize },
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { offset, remaining } => write!(
                formatter,
                "truncated OMF record header at byte {offset}: {remaining} byte(s) remain"
            ),
            Self::InvalidLength {
                offset,
                record_type,
                declared_length,
            } => write!(
                formatter,
                "OMF record type {record_type:02x} at byte {offset} has invalid length {declared_length}"
            ),
            Self::TruncatedRecord {
                offset,
                record_type,
                declared_length,
                available,
            } => write!(
                formatter,
                "OMF record type {record_type:02x} at byte {offset} declares {declared_length} byte(s), only {available} remain"
            ),
            Self::ChecksumMismatch {
                offset,
                record_type,
                expected,
                actual,
            } => {
                write!(formatter, "OMF record type {record_type:02x}")?;
                if let Some(offset) = offset {
                    write!(formatter, " at byte {offset}")?;
                }
                write!(
                    formatter,
                    " has checksum {actual:02x}, expected {expected:02x}"
                )
            }
            Self::BodyTooLarge { body_length } => write!(
                formatter,
                "OMF record body has {body_length} bytes; the largest representable body is 65534 bytes"
            ),
        }
    }
}

impl std::error::Error for RecordError {}

/// Parses a complete OMF record stream without interpreting record bodies.
///
/// The parser validates framing and accepts checksum-invalid records so an
/// untouched stream can always be written back byte-for-byte. Call
/// [`Record::validate_checksum`] when strict validation is required.
pub fn parse(input: &[u8]) -> Result<Vec<Record>, RecordError> {
    parse_at(input, 0)
}

/// Parses a record stream whose first byte starts at `base_offset` in its
/// containing file.
pub(super) fn parse_at(input: &[u8], base_offset: usize) -> Result<Vec<Record>, RecordError> {
    let mut records = Vec::new();
    let mut offset = 0;

    while offset < input.len() {
        let remaining = input.len() - offset;
        if remaining < 3 {
            return Err(RecordError::TruncatedHeader {
                offset: base_offset + offset,
                remaining,
            });
        }

        let record_type = input[offset];
        let declared_length = u16::from_le_bytes([input[offset + 1], input[offset + 2]]);
        if declared_length == 0 {
            return Err(RecordError::InvalidLength {
                offset: base_offset + offset,
                record_type,
                declared_length,
            });
        }

        let payload_length = usize::from(declared_length);
        let payload_offset = offset + 3;
        let available = input.len() - payload_offset;
        if payload_length > available {
            return Err(RecordError::TruncatedRecord {
                offset: base_offset + offset,
                record_type,
                declared_length,
                available,
            });
        }

        let body_length = payload_length - 1;
        let body_end = payload_offset + body_length;
        records.push(Record {
            offset: Some(base_offset + offset),
            record_type,
            body: input[payload_offset..body_end].to_vec(),
            checksum: input[body_end],
        });
        offset = body_end + 1;
    }

    Ok(records)
}

/// Serializes records in their supplied order.
///
/// This preserves every retained checksum, including a checksum that an
/// optional strict validation would reject.
pub fn serialize(records: &[Record]) -> Vec<u8> {
    let size = records.iter().map(|record| record.body.len() + 4).sum();
    let mut bytes = Vec::with_capacity(size);
    for record in records {
        bytes.extend_from_slice(&record.to_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{RecordError, parse, serialize};

    #[test]
    fn parses_and_serializes_known_record_losslessly() {
        // THEADR "foo": 80 05 00 03 66 6f 6f 34 sums to zero modulo 256.
        let input = [0x80, 0x05, 0x00, 0x03, b'f', b'o', b'o', 0x34];

        let records = parse(&input).expect("record should parse");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].offset(), Some(0));
        assert_eq!(records[0].record_type(), 0x80);
        assert_eq!(records[0].body(), [0x03, b'f', b'o', b'o']);
        assert!(records[0].is_checksum_valid());
        assert_eq!(serialize(&records), input);
    }

    #[test]
    fn retains_checksum_invalid_record_without_accepting_it_as_valid() {
        // BC has emitted malformed FIXUPP checksums. Framing still has to
        // round-trip their raw bytes until a caller deliberately rewrites it.
        let input = [0x9c, 0x01, 0x00, 0x00];

        let records = parse(&input).expect("framed record should parse");

        assert!(!records[0].is_checksum_valid());
        assert_eq!(serialize(&records), input);
        assert_eq!(
            records[0].validate_checksum(),
            Err(RecordError::ChecksumMismatch {
                offset: Some(0),
                record_type: 0x9c,
                expected: 0x63,
                actual: 0,
            })
        );
    }

    #[test]
    fn rejects_truncated_header() {
        assert_eq!(
            parse(&[0x80, 0x01]),
            Err(RecordError::TruncatedHeader {
                offset: 0,
                remaining: 2,
            })
        );
    }

    #[test]
    fn rejects_truncated_record() {
        assert_eq!(
            parse(&[0x80, 0x05, 0x00, 0x03, b'f']),
            Err(RecordError::TruncatedRecord {
                offset: 0,
                record_type: 0x80,
                declared_length: 5,
                available: 2,
            })
        );
    }

    #[test]
    fn rejects_length_without_checksum() {
        assert_eq!(
            parse(&[0x80, 0x00, 0x00]),
            Err(RecordError::InvalidLength {
                offset: 0,
                record_type: 0x80,
                declared_length: 0,
            })
        );
    }
}
