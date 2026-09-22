//! Decoding of QB-private statement-table metadata.
//!
//! The semantic frontend records source identities here so the QB object
//! adapter can build the runtime table after generic lowering and allocation.
//! These bytes are compiler metadata, not a portable-IR data object.

use std::error::Error;
use std::fmt;

use crate::old::hir::{self, AddressKind, Linkage};

pub const METADATA_OBJECT_NAME: &str = "$qb$statementTable";
const ROW_SIZE: usize = 14;

/// One frontend-recorded source statement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataRow {
    pub function: hir::FunctionId,
    pub block: hir::BlockId,
    pub instruction: hir::InstructionId,
    pub line: u16,
}

/// Verified metadata together with the HIR module generic lowering may see.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtractedMetadata {
    pub module: hir::Module,
    pub rows: Vec<MetadataRow>,
}

/// Why QB statement metadata cannot be consumed faithfully.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataError {
    ObjectCount { count: usize },
    InvalidStorage,
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectCount { count } => write!(
                formatter,
                "a QB module must carry exactly one statement-table metadata object, found {count}"
            ),
            Self::InvalidStorage => {
                write!(
                    formatter,
                    "the statement-table metadata object has an invalid storage contract"
                )
            }
        }
    }
}

impl Error for MetadataError {}

/// Decode and remove the frontend-private metadata object.
///
/// The storage contract and row layout match Python's `_statement_metadata`:
/// each row is `(function:u32, block:u32, instruction:u32, line:u16)` in
/// little-endian order. No source identity is reinterpreted at this boundary.
pub fn extract(module: &hir::Module) -> Result<ExtractedMetadata, MetadataError> {
    let objects = module
        .data
        .iter()
        .filter(|object| object.name == METADATA_OBJECT_NAME)
        .collect::<Vec<_>>();
    let [object] = objects.as_slice() else {
        return Err(MetadataError::ObjectCount {
            count: objects.len(),
        });
    };
    if object.linkage != Linkage::Internal
        || !object.readonly
        || !object.relocations.is_empty()
        || object.address != AddressKind::Near
        || object.bytes.len() % ROW_SIZE != 0
    {
        return Err(MetadataError::InvalidStorage);
    }

    let rows = object
        .bytes
        .chunks_exact(ROW_SIZE)
        .map(|row| MetadataRow {
            function: hir::FunctionId::new(u32::from_le_bytes(row[0..4].try_into().unwrap())),
            block: hir::BlockId::new(u32::from_le_bytes(row[4..8].try_into().unwrap())),
            instruction: hir::InstructionId::new(u32::from_le_bytes(
                row[8..12].try_into().unwrap(),
            )),
            line: u16::from_le_bytes(row[12..14].try_into().unwrap()),
        })
        .collect();
    let mut lowered = module.clone();
    lowered
        .data
        .retain(|object| object.name != METADATA_OBJECT_NAME);
    Ok(ExtractedMetadata {
        module: lowered,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::qb::{Dialect, compile_hir, parse};

    fn empty_module() -> hir::Module {
        let syntax = parse("", Dialect::VbDos).unwrap();
        compile_hir(&syntax, "empty", Dialect::VbDos, "vbdos")
            .unwrap()
            .modules
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn consumes_empty_frontend_metadata_before_generic_lowering() {
        let module = empty_module();
        let extracted = extract(&module).unwrap();

        assert!(extracted.rows.is_empty());
        assert!(
            extracted
                .module
                .data
                .iter()
                .all(|object| object.name != METADATA_OBJECT_NAME)
        );
        assert!(
            module
                .data
                .iter()
                .any(|object| object.name == METADATA_OBJECT_NAME)
        );
    }

    #[test]
    fn decodes_exact_source_identity_rows() {
        let mut module = empty_module();
        let object = module
            .data
            .iter_mut()
            .find(|object| object.name == METADATA_OBJECT_NAME)
            .unwrap();
        object.bytes = [
            0x04, 0x03, 0x02, 0x01, 0x08, 0x07, 0x06, 0x05, 0x0c, 0x0b, 0x0a, 0x09, 0x0e, 0x0d,
        ]
        .to_vec();

        assert_eq!(
            extract(&module).unwrap().rows,
            [MetadataRow {
                function: hir::FunctionId::new(0x0102_0304),
                block: hir::BlockId::new(0x0506_0708),
                instruction: hir::InstructionId::new(0x090a_0b0c),
                line: 0x0d0e,
            }]
        );
    }

    #[test]
    fn refuses_missing_duplicate_and_malformed_metadata() {
        let module = empty_module();
        let mut missing = module.clone();
        missing
            .data
            .retain(|object| object.name != METADATA_OBJECT_NAME);
        assert_eq!(
            extract(&missing),
            Err(MetadataError::ObjectCount { count: 0 })
        );

        let mut duplicate = module.clone();
        let object = duplicate
            .data
            .iter()
            .find(|object| object.name == METADATA_OBJECT_NAME)
            .unwrap()
            .clone();
        duplicate.data.push(object);
        assert_eq!(
            extract(&duplicate),
            Err(MetadataError::ObjectCount { count: 2 })
        );

        let mut malformed = module;
        malformed
            .data
            .iter_mut()
            .find(|object| object.name == METADATA_OBJECT_NAME)
            .unwrap()
            .bytes
            .push(0);
        assert_eq!(extract(&malformed), Err(MetadataError::InvalidStorage));
    }
}
