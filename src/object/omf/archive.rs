//! Lossless framing for Intel OMF libraries.
//!
//! A library is not a normal OMF record stream.  Its `LIBHDR` record fills
//! the first page, object modules begin on subsequent page boundaries, and
//! the `LIBDIC` dictionary following the modules is opaque data rather than
//! OMF records.  This module therefore keeps the original library bytes and
//! only parses the record sequences which make up its modules.

use std::fmt;

use super::record::{parse, parse_at, Record, RecordError};

const LIBHDR: u8 = 0xf0;
const LIBDIC: u8 = 0xf1;
const THEADR: u8 = 0x80;
const MODEND: u8 = 0x8a;
const MODEND32: u8 = 0x8b;

/// An Intel OMF library, retained byte-for-byte until an editing API exists.
///
/// [`Archive::to_bytes`] deliberately returns the input bytes instead of
/// rebuilding the library.  In particular this preserves page padding and
/// the opaque dictionary, neither of which belongs to a module record stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Archive {
    bytes: Vec<u8>,
    header: Record,
    page_size: usize,
    modules: Vec<Module>,
    dictionary_offset: Option<usize>,
}

impl Archive {
    /// Parses `input` when it begins with an OMF `LIBHDR` record.
    ///
    /// A non-library input is not an error: it returns `Ok(None)` so callers
    /// can first attempt library framing before parsing a standalone object.
    pub fn parse(input: &[u8]) -> Result<Option<Self>, ArchiveError> {
        if input.first().copied() != Some(LIBHDR) {
            return Ok(None);
        }

        let (header, page_size) = parse_header(input)?;
        let mut modules = Vec::new();
        let mut at = page_size;
        let mut dictionary_offset = None;

        while at < input.len() {
            if input[at] == LIBDIC {
                dictionary_offset = Some(at);
                break;
            }

            if input[at] == MODEND || input[at] == MODEND32 {
                return Err(ArchiveError::ModendWithoutModule { offset: at });
            }
            if input[at] != THEADR {
                return Err(ArchiveError::RecordBeforeTheadr {
                    offset: at,
                    record_type: input[at],
                });
            }

            let module_start = at;
            let (header_record, mut next) = parse_record(input, at)?;
            let name = theadr_name(&header_record, module_start)?;
            let mut records = vec![header_record];
            loop {
                if next >= input.len() {
                    return Err(ArchiveError::MissingModend {
                        module_offset: module_start,
                        offset: next,
                    });
                }
                if next % page_size == 0 && input[next] == LIBDIC {
                    return Err(ArchiveError::MissingModend {
                        module_offset: module_start,
                        offset: next,
                    });
                }

                let (record, record_end) = parse_record(input, next)?;
                let record_type = record.record_type();
                if record_type == THEADR {
                    return Err(ArchiveError::NestedTheadr { offset: next });
                }

                records.push(record);
                next = record_end;

                if record_type == MODEND || record_type == MODEND32 {
                    modules.push(Module {
                        name,
                        records,
                        offset: module_start,
                        end_offset: next,
                    });
                    at = next_page_boundary(next, page_size)?;
                    break;
                }
            }
        }

        if modules.is_empty() {
            return Err(ArchiveError::NoModules);
        }

        Ok(Some(Self {
            bytes: input.to_vec(),
            header,
            page_size,
            modules,
            dictionary_offset,
        }))
    }

    /// Returns the parsed `LIBHDR` record.
    pub const fn header(&self) -> &Record {
        &self.header
    }

    /// Returns the library's page size, including the three-byte record
    /// header that precedes the `LIBHDR` payload.
    pub const fn page_size(&self) -> usize {
        self.page_size
    }

    /// Returns the object modules in their original library order.
    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    /// Returns the absolute offset of the opaque dictionary, when present.
    pub const fn dictionary_offset(&self) -> Option<usize> {
        self.dictionary_offset
    }

    /// Returns the untouched opaque dictionary bytes, including its `0xf1`
    /// marker, when the library has a dictionary.
    pub fn dictionary_bytes(&self) -> Option<&[u8]> {
        self.dictionary_offset.map(|offset| &self.bytes[offset..])
    }

    /// Returns the complete source library bytes.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Serializes an untouched archive byte-for-byte.
    ///
    /// There is intentionally no editing API yet.  Returning the retained
    /// source also preserves padding and dictionary bytes exactly.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// One `THEADR` through `MODEND` object module in an OMF library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    name: Vec<u8>,
    records: Vec<Record>,
    offset: usize,
    end_offset: usize,
}

impl Module {
    /// Returns the module name as its raw OMF bytes.
    ///
    /// OMF names are not necessarily UTF-8.
    pub fn name(&self) -> &[u8] {
        &self.name
    }

    /// Returns the module records, from `THEADR` through `MODEND`.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Returns the absolute offset at which this module's `THEADR` begins.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the exclusive absolute end offset of this module's `MODEND`.
    pub const fn end_offset(&self) -> usize {
        self.end_offset
    }
}

/// Failures while framing an Intel OMF library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveError {
    /// The `LIBHDR` length cannot describe a complete non-empty first page.
    InvalidPageSize {
        offset: usize,
        declared_length: u16,
        input_length: usize,
    },
    /// A page-boundary calculation overflowed.
    InvalidPageCalculation { offset: usize, page_size: usize },
    /// An ordinary OMF record has invalid framing at its absolute offset.
    Record(RecordError),
    /// A module area begins with a record other than `THEADR`.
    RecordBeforeTheadr { offset: usize, record_type: u8 },
    /// A `THEADR` occurred before the current module's `MODEND`.
    NestedTheadr { offset: usize },
    /// A `MODEND` occurred where a new module header was required.
    ModendWithoutModule { offset: usize },
    /// A module reached the end of the archive or its dictionary without a
    /// closing `MODEND` record.
    MissingModend { module_offset: usize, offset: usize },
    /// The `THEADR` body does not contain exactly one complete length-prefixed
    /// raw module name.
    InvalidTheadrName {
        offset: usize,
        declared_length: u8,
        body_length: usize,
    },
    /// The library contains no object modules before its dictionary or end.
    NoModules,
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPageSize {
                offset,
                declared_length,
                input_length,
            } => write!(
                formatter,
                "invalid OMF library page size at byte {offset}: header length {declared_length}, input is {input_length} byte(s)"
            ),
            Self::InvalidPageCalculation { offset, page_size } => write!(
                formatter,
                "invalid OMF library page calculation after byte {offset} with page size {page_size}"
            ),
            Self::Record(error) => error.fmt(formatter),
            Self::RecordBeforeTheadr {
                offset,
                record_type,
            } => write!(
                formatter,
                "OMF library record type {record_type:02x} at byte {offset} precedes THEADR"
            ),
            Self::NestedTheadr { offset } => {
                write!(formatter, "nested OMF library THEADR at byte {offset}")
            }
            Self::ModendWithoutModule { offset } => {
                write!(formatter, "OMF library MODEND at byte {offset} has no module")
            }
            Self::MissingModend {
                module_offset,
                offset,
            } => write!(
                formatter,
                "OMF library module at byte {module_offset} has no MODEND before byte {offset}"
            ),
            Self::InvalidTheadrName {
                offset,
                declared_length,
                body_length,
            } => write!(
                formatter,
                "invalid OMF library THEADR name at byte {offset}: declares {declared_length} byte(s), body has {body_length} byte(s)"
            ),
            Self::NoModules => write!(formatter, "OMF library contains no modules"),
        }
    }
}

impl std::error::Error for ArchiveError {}

fn parse_header(input: &[u8]) -> Result<(Record, usize), ArchiveError> {
    if input.len() < 3 {
        return Err(ArchiveError::Record(RecordError::TruncatedHeader {
            offset: 0,
            remaining: input.len(),
        }));
    }

    let declared_length = u16::from_le_bytes([input[1], input[2]]);
    let Some(page_size) = usize::from(declared_length).checked_add(3) else {
        return Err(ArchiveError::InvalidPageCalculation {
            offset: 0,
            page_size: usize::from(declared_length),
        });
    };
    if declared_length == 0 || page_size > input.len() {
        return Err(ArchiveError::InvalidPageSize {
            offset: 0,
            declared_length,
            input_length: input.len(),
        });
    }

    let mut records = parse(&input[..page_size]).map_err(ArchiveError::Record)?;
    let Some(header) = records.pop() else {
        return Err(ArchiveError::InvalidPageCalculation {
            offset: 0,
            page_size,
        });
    };
    Ok((header, page_size))
}

fn parse_record(input: &[u8], offset: usize) -> Result<(Record, usize), ArchiveError> {
    let remaining =
        input
            .len()
            .checked_sub(offset)
            .ok_or(ArchiveError::InvalidPageCalculation {
                offset,
                page_size: 0,
            })?;
    if remaining < 3 {
        return Err(ArchiveError::Record(RecordError::TruncatedHeader {
            offset,
            remaining,
        }));
    }

    let record_type = input[offset];
    let declared_length = u16::from_le_bytes([input[offset + 1], input[offset + 2]]);
    if declared_length == 0 {
        return Err(ArchiveError::Record(RecordError::InvalidLength {
            offset,
            record_type,
            declared_length,
        }));
    }

    let payload_offset = offset
        .checked_add(3)
        .ok_or(ArchiveError::InvalidPageCalculation {
            offset,
            page_size: 0,
        })?;
    let end = payload_offset
        .checked_add(usize::from(declared_length))
        .ok_or(ArchiveError::InvalidPageCalculation {
            offset,
            page_size: 0,
        })?;
    if end > input.len() {
        return Err(ArchiveError::Record(RecordError::TruncatedRecord {
            offset,
            record_type,
            declared_length,
            available: input.len() - payload_offset,
        }));
    }

    let mut records = parse_at(&input[offset..end], offset).map_err(ArchiveError::Record)?;
    let Some(record) = records.pop() else {
        return Err(ArchiveError::InvalidPageCalculation {
            offset,
            page_size: 0,
        });
    };
    Ok((record, end))
}

fn theadr_name(record: &Record, offset: usize) -> Result<Vec<u8>, ArchiveError> {
    let body = record.body();
    let Some(&length) = body.first() else {
        return Err(ArchiveError::InvalidTheadrName {
            offset,
            declared_length: 0,
            body_length: 0,
        });
    };
    let expected_body_length =
        usize::from(length)
            .checked_add(1)
            .ok_or(ArchiveError::InvalidTheadrName {
                offset,
                declared_length: length,
                body_length: body.len(),
            })?;
    if body.len() != expected_body_length {
        return Err(ArchiveError::InvalidTheadrName {
            offset,
            declared_length: length,
            body_length: body.len(),
        });
    }
    Ok(body[1..].to_vec())
}

fn next_page_boundary(offset: usize, page_size: usize) -> Result<usize, ArchiveError> {
    let remainder = offset % page_size;
    if remainder == 0 {
        return Ok(offset);
    }
    offset
        .checked_add(page_size - remainder)
        .ok_or(ArchiveError::InvalidPageCalculation { offset, page_size })
}

#[cfg(test)]
mod tests {
    use super::super::record::Record;
    use super::{Archive, ArchiveError};

    fn record(record_type: u8, body: &[u8]) -> Vec<u8> {
        Record::new(record_type, body.to_vec())
            .expect("small test record")
            .to_bytes()
    }

    fn header(page_size: usize) -> Vec<u8> {
        let body_length = page_size
            .checked_sub(4)
            .expect("a page needs room for a record header and checksum");
        record(0xf0, &vec![0; body_length])
    }

    #[test]
    fn non_library_is_not_an_error() {
        assert_eq!(Archive::parse(&[0x80, 0x01, 0x00, 0]), Ok(None));
    }

    #[test]
    fn page_aligned_module_round_trips_with_opaque_dictionary() {
        let mut input = header(16);
        let module = [record(0x80, &[1, b'\xff']), record(0x8a, &[0])].concat();
        input.extend_from_slice(&module);
        input.resize(32, 0);
        input.extend_from_slice(&[0xf1, 0xa5, 0x5a]);

        let archive = Archive::parse(&input)
            .expect("library framing should parse")
            .expect("input is a library");

        assert_eq!(archive.page_size(), 16);
        assert_eq!(archive.modules().len(), 1);
        assert_eq!(archive.modules()[0].name(), [0xff]);
        assert_eq!(archive.modules()[0].records()[0].offset(), Some(16));
        assert_eq!(archive.modules()[0].records()[1].offset(), Some(22));
        assert_eq!(archive.dictionary_offset(), Some(32));
        assert_eq!(archive.dictionary_bytes(), Some(&[0xf1, 0xa5, 0x5a][..]));
        assert_eq!(archive.to_bytes(), input);
    }

    #[test]
    fn rejects_invalid_library_page_header() {
        let error = Archive::parse(&[0xf0, 0, 0]).expect_err("zero page is invalid");
        assert_eq!(
            error,
            ArchiveError::InvalidPageSize {
                offset: 0,
                declared_length: 0,
                input_length: 3,
            }
        );
    }

    #[test]
    fn rejects_module_without_modend() {
        let mut input = header(16);
        input.extend_from_slice(&record(0x80, &[1, b'a']));

        let error = Archive::parse(&input).expect_err("module has no MODEND");
        assert_eq!(
            error,
            ArchiveError::MissingModend {
                module_offset: 16,
                offset: 22,
            }
        );
    }
}
