//! Construction of segment images from ordered LEDATA writes.

use std::fmt;

use super::data::DataBlock;

/// A segment image and the record that last wrote each byte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentImage {
    pub bytes: Vec<u8>,
    pub last_writers: Vec<Option<usize>>,
}

impl SegmentImage {
    /// Applies matching LEDATA blocks in record order.
    ///
    /// Later blocks deliberately replace overlapping earlier bytes. BASIC
    /// compilers use those later writes to backpatch forward references.
    pub fn build(
        blocks: &[DataBlock<'_>],
        segment_index: u16,
        length: u64,
    ) -> Result<Self, ImageError> {
        let length = usize::try_from(length).map_err(|_| ImageError::SegmentTooLarge { length })?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ImageError::AllocationFailed { length })?;
        bytes.resize(length, 0);
        let mut last_writers = Vec::new();
        last_writers
            .try_reserve_exact(length)
            .map_err(|_| ImageError::AllocationFailed { length })?;
        last_writers.resize(length, None);
        let mut image = Self {
            bytes,
            last_writers,
        };
        for block in blocks
            .iter()
            .filter(|block| block.segment_index == segment_index)
        {
            let start =
                usize::try_from(block.offset).map_err(|_| ImageError::DataOutsideSegment {
                    record_index: block.record_index,
                    record_offset: block.record_offset,
                    data_offset: block.offset,
                    data_length: block.bytes.len(),
                    segment_length: length,
                })?;
            let Some(end) = start.checked_add(block.bytes.len()) else {
                return Err(ImageError::DataOutsideSegment {
                    record_index: block.record_index,
                    record_offset: block.record_offset,
                    data_offset: block.offset,
                    data_length: block.bytes.len(),
                    segment_length: length,
                });
            };
            if end > length {
                return Err(ImageError::DataOutsideSegment {
                    record_index: block.record_index,
                    record_offset: block.record_offset,
                    data_offset: block.offset,
                    data_length: block.bytes.len(),
                    segment_length: length,
                });
            }
            image.bytes[start..end].copy_from_slice(block.bytes);
            image.last_writers[start..end].fill(Some(block.record_index));
        }
        Ok(image)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageError {
    SegmentTooLarge {
        length: u64,
    },
    AllocationFailed {
        length: usize,
    },
    DataOutsideSegment {
        record_index: usize,
        record_offset: Option<usize>,
        data_offset: u32,
        data_length: usize,
        segment_length: usize,
    },
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SegmentTooLarge { length } => {
                write!(
                    formatter,
                    "OMF segment length {length} does not fit in memory"
                )
            }
            Self::AllocationFailed { length } => {
                write!(
                    formatter,
                    "cannot allocate {length} bytes for an OMF segment image"
                )
            }
            Self::DataOutsideSegment {
                record_index,
                record_offset,
                data_offset,
                data_length,
                segment_length,
            } => {
                write!(
                    formatter,
                    "OMF data record {record_index} writes {data_length} byte(s) at {data_offset:#x} outside a {segment_length}-byte segment"
                )?;
                if let Some(offset) = record_offset {
                    write!(formatter, " (record byte {offset})")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ImageError {}

#[cfg(test)]
mod tests {
    use super::SegmentImage;
    use crate::old::object::omf::data::DataBlock;

    #[test]
    fn later_overlapping_data_is_the_last_writer() {
        let first = [1, 2, 3, 4];
        let patch = [9, 8];
        let blocks = [
            DataBlock {
                record_index: 3,
                record_offset: Some(20),
                segment_index: 1,
                offset: 1,
                bytes: &first,
            },
            DataBlock {
                record_index: 7,
                record_offset: Some(40),
                segment_index: 1,
                offset: 2,
                bytes: &patch,
            },
        ];

        let image = SegmentImage::build(&blocks, 1, 6).unwrap();

        assert_eq!(image.bytes, [0, 1, 9, 8, 4, 0]);
        assert_eq!(
            image.last_writers,
            [None, Some(3), Some(7), Some(7), Some(3), None]
        );
    }
}
