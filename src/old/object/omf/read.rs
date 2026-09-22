//! Bounded primitive reads from an uninterpreted OMF record body.

use std::fmt;

/// A zero-copy reader for one OMF record body.
///
/// `base_offset` is the absolute byte offset at which `body` starts when it
/// is known. It is retained for diagnostics; positions are always relative to
/// the start of the body.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    body: &'a [u8],
    position: usize,
    base_offset: Option<usize>,
}

impl<'a> Reader<'a> {
    /// Starts reading `body` at its first byte.
    pub const fn new(body: &'a [u8], base_offset: Option<usize>) -> Self {
        Self {
            body,
            position: 0,
            base_offset,
        }
    }

    /// Returns the number of unread bytes in the record body.
    pub fn remaining(&self) -> usize {
        self.body.len() - self.position
    }

    /// Returns whether every byte in the record body has been consumed.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Returns the current byte position relative to the record body.
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Reads one byte.
    pub fn read_u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.read_bytes(1)?[0])
    }

    /// Reads a little-endian 16-bit integer.
    pub fn read_u16(&mut self) -> Result<u16, ReadError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Reads a little-endian 32-bit integer.
    pub fn read_u32(&mut self) -> Result<u32, ReadError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads `length` raw bytes without allocating.
    pub fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], ReadError> {
        let start = self.position;
        let remaining = self.remaining();
        let Some(end) = start
            .checked_add(length)
            .filter(|end| *end <= self.body.len())
        else {
            return Err(self.truncated_read(start, length, remaining));
        };

        self.position = end;
        Ok(&self.body[start..end])
    }

    /// Reads an Intel OMF variable-length index.
    ///
    /// Indices below `0x80` occupy one byte. Otherwise the high seven bits of
    /// the first byte and all eight bits of the second byte form the index.
    pub fn read_index(&mut self) -> Result<u16, ReadError> {
        let start = self.position;
        let remaining = self.remaining();
        if remaining == 0 {
            return Err(self.truncated_index(start, 1, remaining));
        }

        let first = self.body[start];
        if first < 0x80 {
            let Some(end) = start.checked_add(1) else {
                return Err(self.truncated_index(start, 1, remaining));
            };
            self.position = end;
            return Ok(u16::from(first));
        }

        if remaining < 2 {
            return Err(self.truncated_index(start, 2, remaining));
        }

        let Some(second_offset) = start.checked_add(1) else {
            return Err(self.truncated_index(start, 2, remaining));
        };
        let Some(end) = second_offset.checked_add(1) else {
            return Err(self.truncated_index(start, 2, remaining));
        };
        self.position = end;
        Ok((u16::from(first & 0x7f) << 8) | u16::from(self.body[second_offset]))
    }

    /// Reads an OMF length-prefixed name as raw bytes.
    ///
    /// OMF names are not required to be UTF-8, so this intentionally returns
    /// their original byte sequence.
    pub fn read_name(&mut self) -> Result<&'a [u8], ReadError> {
        let start = self.position;
        let remaining = self.remaining();
        if remaining == 0 {
            return Err(self.truncated_read(start, 1, remaining));
        }

        let length = usize::from(self.body[start]);
        let Some(payload_offset) = start.checked_add(1) else {
            return Err(self.truncated_read(start, 1, remaining));
        };
        let payload_remaining = remaining - 1;
        let Some(end) = payload_offset
            .checked_add(length)
            .filter(|end| *end <= self.body.len())
        else {
            return Err(self.truncated_name(payload_offset, length, payload_remaining));
        };

        self.position = end;
        Ok(&self.body[payload_offset..end])
    }

    fn truncated_read(&self, body_offset: usize, requested: usize, remaining: usize) -> ReadError {
        ReadError::TruncatedRead {
            body_offset,
            absolute_offset: self.absolute_offset(body_offset),
            requested,
            remaining,
        }
    }

    fn truncated_index(&self, body_offset: usize, requested: usize, remaining: usize) -> ReadError {
        ReadError::TruncatedIndex {
            body_offset,
            absolute_offset: self.absolute_offset(body_offset),
            requested,
            remaining,
        }
    }

    fn truncated_name(&self, body_offset: usize, requested: usize, remaining: usize) -> ReadError {
        ReadError::TruncatedNamePayload {
            body_offset,
            absolute_offset: self.absolute_offset(body_offset),
            requested,
            remaining,
        }
    }

    fn absolute_offset(&self, body_offset: usize) -> Option<usize> {
        self.base_offset
            .and_then(|base_offset| base_offset.checked_add(body_offset))
    }
}

/// Failures while reading primitive fields from an OMF record body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadError {
    /// A fixed-width integer or raw-byte read extends past the body.
    TruncatedRead {
        body_offset: usize,
        absolute_offset: Option<usize>,
        requested: usize,
        remaining: usize,
    },
    /// An OMF index has no first byte or lacks its required second byte.
    TruncatedIndex {
        body_offset: usize,
        absolute_offset: Option<usize>,
        requested: usize,
        remaining: usize,
    },
    /// An OMF name's declared payload extends past the body.
    TruncatedNamePayload {
        body_offset: usize,
        absolute_offset: Option<usize>,
        requested: usize,
        remaining: usize,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, body_offset, absolute_offset, requested, remaining) = match self {
            Self::TruncatedRead {
                body_offset,
                absolute_offset,
                requested,
                remaining,
            } => ("read", body_offset, absolute_offset, requested, remaining),
            Self::TruncatedIndex {
                body_offset,
                absolute_offset,
                requested,
                remaining,
            } => ("index", body_offset, absolute_offset, requested, remaining),
            Self::TruncatedNamePayload {
                body_offset,
                absolute_offset,
                requested,
                remaining,
            } => (
                "name payload",
                body_offset,
                absolute_offset,
                requested,
                remaining,
            ),
        };

        write!(
            formatter,
            "truncated OMF {kind} at body byte {body_offset}: requested {requested} byte(s), {remaining} remain"
        )?;
        if let Some(absolute_offset) = absolute_offset {
            write!(formatter, " (absolute byte {absolute_offset})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ReadError {}

#[cfg(test)]
mod tests {
    use super::{ReadError, Reader};

    #[test]
    fn reads_one_and_two_byte_indices() {
        let mut reader = Reader::new(&[0x7f, 0x81, 0x23], Some(40));

        assert_eq!(reader.read_index(), Ok(0x7f));
        assert_eq!(reader.read_index(), Ok(0x0123));
        assert!(reader.is_empty());
        assert_eq!(reader.position(), 3);
    }

    #[test]
    fn reads_zero_and_largest_index() {
        let mut reader = Reader::new(&[0x00, 0xff, 0xff], None);

        assert_eq!(reader.read_index(), Ok(0));
        assert_eq!(reader.read_index(), Ok(0x7fff));
    }

    #[test]
    fn preserves_non_utf8_name_bytes() {
        let mut reader = Reader::new(&[3, b'a', 0xff, 0], None);

        assert_eq!(reader.read_name(), Ok(&[b'a', 0xff, 0][..]));
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn reports_truncated_fixed_read_with_offsets_and_counts() {
        let mut reader = Reader::new(&[0x34], Some(100));

        assert_eq!(
            reader.read_u16(),
            Err(ReadError::TruncatedRead {
                body_offset: 0,
                absolute_offset: Some(100),
                requested: 2,
                remaining: 1,
            })
        );
    }

    #[test]
    fn reports_truncated_index_without_consuming_its_prefix() {
        let mut reader = Reader::new(&[0x80], Some(100));

        assert_eq!(
            reader.read_index(),
            Err(ReadError::TruncatedIndex {
                body_offset: 0,
                absolute_offset: Some(100),
                requested: 2,
                remaining: 1,
            })
        );
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn reports_truncated_name_payload_without_consuming_the_name() {
        let mut reader = Reader::new(&[3, b'a'], Some(100));

        assert_eq!(
            reader.read_name(),
            Err(ReadError::TruncatedNamePayload {
                body_offset: 1,
                absolute_offset: Some(101),
                requested: 3,
                remaining: 1,
            })
        );
        assert_eq!(reader.position(), 0);
    }
}
