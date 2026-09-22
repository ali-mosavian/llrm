//! QB module-header serialization.
//!
//! The 48-byte `MODULE_CODE` prefix is runtime input, not executable code.
//! Its switch word must retain the exact compiler profile that produced it.

use std::error::Error;
use std::fmt;

use crate::old::hir::{ArrayOrder, FloatMode, Program, RuntimeProfile};

pub const MODULE_HEADER_SIZE: usize = 48;
pub const MODULE_NAME_SIZE: usize = 8;
pub const MODULE_SIGNATURE: [u8; 2] = *b"bl";

const DATA_OFFSET: u16 = 2;
const UNRESOLVED_WORD: [u8; 2] = [0xff, 0xff];

/// Why a QB module header cannot be represented faithfully.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleHeaderError {
    ModuleCount { count: usize },
    NonAsciiModuleName { name: String },
    AlternateFloatUnsupported { runtime: RuntimeProfile },
}

impl fmt::Display for ModuleHeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModuleCount { count } => {
                write!(
                    formatter,
                    "a QB module header requires exactly one module, found {count}"
                )
            }
            Self::NonAsciiModuleName { name } => {
                write!(formatter, "QB module name {name:?} is not ASCII")
            }
            Self::AlternateFloatUnsupported { runtime } => write!(
                formatter,
                "alternate floating-point runtime is not available for {runtime:?}"
            ),
        }
    }
}

impl Error for ModuleHeaderError {}

/// Return the linker spelling used for a module name.
pub fn object_name(name: &str) -> Result<String, ModuleHeaderError> {
    if !name.is_ascii() {
        return Err(ModuleHeaderError::NonAsciiModuleName {
            name: name.to_owned(),
        });
    }
    Ok(name
        .trim_end_matches(|character| matches!(character, '%' | '&' | '!' | '#' | '$'))
        .to_ascii_uppercase())
}

/// Return the exact runtime switch word for this QB compilation profile.
pub fn compiler_switches(program: &Program) -> Result<u16, ModuleHeaderError> {
    if program.float_mode == FloatMode::Alternate {
        return match program.runtime {
            RuntimeProfile::Pds71 => Ok(0x1088),
            runtime => Err(ModuleHeaderError::AlternateFloatUnsupported { runtime }),
        };
    }

    let mut flags = match program.runtime {
        RuntimeProfile::Qb45 => 0x1080,
        RuntimeProfile::Pds71 => 0x1084,
        RuntimeProfile::Vbdos => 0x12c4,
    };
    if program.runtime == RuntimeProfile::Vbdos && program.array_order == ArrayOrder::RowMajor {
        flags |= 0x0100;
    }
    Ok(flags)
}

/// Construct the measured 48-byte QB `MODULE_CODE` header.
pub fn module_header(program: &Program) -> Result<[u8; MODULE_HEADER_SIZE], ModuleHeaderError> {
    let [module] = program.modules.as_slice() else {
        return Err(ModuleHeaderError::ModuleCount {
            count: program.modules.len(),
        });
    };
    let name = object_name(&module.name)?;
    let mut header = [0; MODULE_HEADER_SIZE];
    header[..2].copy_from_slice(&MODULE_SIGNATURE);
    header[2..2 + MODULE_NAME_SIZE].fill(b' ');
    let copied = name.len().min(MODULE_NAME_SIZE);
    header[2..2 + copied].copy_from_slice(&name.as_bytes()[..copied]);
    header[12..14].copy_from_slice(&DATA_OFFSET.to_le_bytes());
    header[44..46].copy_from_slice(&UNRESOLVED_WORD);
    header[46..48].copy_from_slice(&compiler_switches(program)?.to_le_bytes());
    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::hir::{Dialect, FORMAT_VERSION, Module, ModuleId, TargetProfile};

    fn program(
        name: &str,
        runtime: RuntimeProfile,
        array_order: ArrayOrder,
        float_mode: FloatMode,
    ) -> Program {
        Program {
            version: FORMAT_VERSION,
            dialect: Dialect::Qb45,
            runtime,
            target: TargetProfile::I386RealMode,
            array_order,
            float_mode,
            modules: vec![Module {
                id: ModuleId::new(1),
                name: name.to_owned(),
                types: Vec::new(),
                functions: Vec::new(),
                data: Vec::new(),
                callables: Vec::new(),
            }],
        }
    }

    #[test]
    fn emission_header_is_the_exact_measured_48_bytes() {
        let header = module_header(&program(
            "emission.bas",
            RuntimeProfile::Qb45,
            ArrayOrder::ColumnMajor,
            FloatMode::Inline,
        ))
        .unwrap();
        assert_eq!(
            header,
            [
                b'b', b'l', b'E', b'M', b'I', b'S', b'S', b'I', b'O', b'N', 0, 0, 2, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff,
                0xff, 0x80, 0x10,
            ]
        );
    }

    #[test]
    fn vbdos_row_major_records_the_row_layout_switch() {
        let header = module_header(&program(
            "row",
            RuntimeProfile::Vbdos,
            ArrayOrder::RowMajor,
            FloatMode::Inline,
        ))
        .unwrap();
        assert_eq!(
            u16::from_le_bytes(header[46..48].try_into().unwrap()),
            0x13c4
        );
    }

    #[test]
    fn pds_alternate_and_inline_math_have_distinct_switches() {
        let inline = compiler_switches(&program(
            "math",
            RuntimeProfile::Pds71,
            ArrayOrder::ColumnMajor,
            FloatMode::Inline,
        ))
        .unwrap();
        let alternate = compiler_switches(&program(
            "math",
            RuntimeProfile::Pds71,
            ArrayOrder::ColumnMajor,
            FloatMode::Alternate,
        ))
        .unwrap();
        assert_eq!(inline, 0x1084);
        assert_eq!(alternate, 0x1088);
    }

    #[test]
    fn names_strip_suffixes_uppercase_truncate_and_pad() {
        let short = module_header(&program(
            "go$",
            RuntimeProfile::Qb45,
            ArrayOrder::ColumnMajor,
            FloatMode::Inline,
        ))
        .unwrap();
        assert_eq!(&short[2..10], b"GO      ");

        let long = module_header(&program(
            "abcdefghijkl!",
            RuntimeProfile::Qb45,
            ArrayOrder::ColumnMajor,
            FloatMode::Inline,
        ))
        .unwrap();
        assert_eq!(&long[2..10], b"ABCDEFGH");
    }

    #[test]
    fn non_ascii_names_wrong_module_counts_and_unsupported_math_refuse() {
        let non_ascii = program(
            "módulo",
            RuntimeProfile::Qb45,
            ArrayOrder::ColumnMajor,
            FloatMode::Inline,
        );
        assert!(matches!(
            module_header(&non_ascii),
            Err(ModuleHeaderError::NonAsciiModuleName { .. })
        ));

        let mut no_modules = program(
            "only",
            RuntimeProfile::Qb45,
            ArrayOrder::ColumnMajor,
            FloatMode::Inline,
        );
        no_modules.modules.clear();
        assert!(matches!(
            module_header(&no_modules),
            Err(ModuleHeaderError::ModuleCount { count: 0 })
        ));

        let mut two_modules = program(
            "first",
            RuntimeProfile::Qb45,
            ArrayOrder::ColumnMajor,
            FloatMode::Inline,
        );
        two_modules.modules.push(Module {
            id: ModuleId::new(2),
            name: "second".to_owned(),
            types: Vec::new(),
            functions: Vec::new(),
            data: Vec::new(),
            callables: Vec::new(),
        });
        assert!(matches!(
            module_header(&two_modules),
            Err(ModuleHeaderError::ModuleCount { count: 2 })
        ));

        let unsupported_qb = program(
            "math",
            RuntimeProfile::Qb45,
            ArrayOrder::ColumnMajor,
            FloatMode::Alternate,
        );
        assert!(matches!(
            module_header(&unsupported_qb),
            Err(ModuleHeaderError::AlternateFloatUnsupported {
                runtime: RuntimeProfile::Qb45
            })
        ));

        let unsupported_vbdos = program(
            "math",
            RuntimeProfile::Vbdos,
            ArrayOrder::ColumnMajor,
            FloatMode::Alternate,
        );
        assert!(matches!(
            module_header(&unsupported_vbdos),
            Err(ModuleHeaderError::AlternateFloatUnsupported {
                runtime: RuntimeProfile::Vbdos
            })
        ));
    }
}
