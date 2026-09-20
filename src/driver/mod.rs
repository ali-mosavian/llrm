//! Whole-pipeline orchestration and diagnostics.

use std::fmt;

use crate::frontend::qb::{self, Dialect};
use crate::hir::{Program, RuntimeProfile};
use crate::object::omf::file::{File as OmfFile, FileError};

/// Configuration that affects QB source semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QbOptions {
    pub dialect: Dialect,
    pub runtime: RuntimeProfile,
    pub row_major: bool,
    pub huge_arrays: bool,
    pub checked_arrays: bool,
    pub mbf: bool,
    pub alternate_math: bool,
}

impl Default for QbOptions {
    fn default() -> Self {
        Self {
            dialect: Dialect::VbDos,
            runtime: RuntimeProfile::Vbdos,
            row_major: false,
            huge_arrays: false,
            checked_arrays: false,
            mbf: false,
            alternate_math: false,
        }
    }
}

/// A pipeline input failure. Printing belongs to the CLI boundary.
#[derive(Debug)]
pub enum Error {
    Parse(qb::ParseError),
    Semantic(qb::SemanticError),
    Omf(FileError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.message.fmt(formatter),
            Self::Semantic(error) => error.message.fmt(formatter),
            Self::Omf(error) => error.fmt(formatter),
        }
    }
}

/// Parse an OMF object or library for the rewrite pipeline.
pub fn parse_omf(bytes: &[u8]) -> Result<OmfFile, Error> {
    OmfFile::parse(bytes).map_err(Error::Omf)
}

impl std::error::Error for Error {}

/// Compile QB source through the verified typed-HIR boundary.
pub fn compile_qb(source: &str, module_name: &str, options: QbOptions) -> Result<Program, Error> {
    let module = qb::parse(source, options.dialect).map_err(Error::Parse)?;
    qb::semantic::compile_hir_with_options(
        &module,
        module_name,
        options.dialect,
        runtime_name(options.runtime),
        options.row_major,
        options.huge_arrays,
        options.checked_arrays,
        options.mbf,
        options.alternate_math,
    )
    .map_err(Error::Semantic)
}

fn runtime_name(runtime: RuntimeProfile) -> &'static str {
    match runtime {
        RuntimeProfile::Qb45 => "qb45",
        RuntimeProfile::Pds71 => "pds71",
        RuntimeProfile::Vbdos => "vbdos",
    }
}

#[cfg(test)]
mod tests {
    use super::parse_omf;
    use crate::object::omf::record::Record;

    #[test]
    fn untouched_omf_survives_the_driver_boundary_byte_for_byte() {
        let mut bytes = Record::new(0x80, vec![1, b'm']).unwrap().to_bytes();
        *bytes.last_mut().unwrap() = 0x5a;

        let file = parse_omf(&bytes).unwrap();

        assert_eq!(file.to_bytes(), bytes);
    }
}
