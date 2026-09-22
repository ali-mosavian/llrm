//! Lossless dispatch between standalone OMF objects and OMF libraries.

use std::fmt;

use super::archive::{Archive, ArchiveError};
use super::record::{self, Record, RecordError};

/// A framed OMF input file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum File {
    Object(Vec<Record>),
    Library(Archive),
}

impl File {
    /// Parses either a library or a standalone object record stream.
    pub fn parse(bytes: &[u8]) -> Result<Self, FileError> {
        match Archive::parse(bytes).map_err(FileError::Archive)? {
            Some(archive) => Ok(Self::Library(archive)),
            None => record::parse(bytes)
                .map(Self::Object)
                .map_err(FileError::Object),
        }
    }

    /// Writes an untouched input byte-for-byte.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Object(records) => record::serialize(records),
            Self::Library(archive) => archive.to_bytes(),
        }
    }
}

/// A framing failure in an OMF object or library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileError {
    Object(RecordError),
    Archive(ArchiveError),
}

impl fmt::Display for FileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Object(error) => error.fmt(formatter),
            Self::Archive(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            Self::Archive(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::File;
    use crate::old::object::omf::record::Record;

    #[test]
    fn standalone_object_round_trips_with_an_invalid_checksum() {
        let mut bytes = Record::new(0x80, vec![1, b'm']).unwrap().to_bytes();
        *bytes.last_mut().unwrap() = 0x5a;

        let file = File::parse(&bytes).unwrap();

        assert!(matches!(file, File::Object(_)));
        assert_eq!(file.to_bytes(), bytes);
    }
}
