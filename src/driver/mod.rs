//! Whole-pipeline orchestration and diagnostics.

use std::fmt;

use crate::frontend::qb::{self, Dialect};
use crate::hir::{LowerError, Program, RuntimeProfile};
use crate::ir;
use crate::object::omf::file::{File as OmfFile, FileError};
use crate::target::x86::SelectionError;

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
    ExpectedSingleModule { actual: usize },
    Lower(LowerError),
    Selection(SelectionError),
    Omf(FileError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.message.fmt(formatter),
            Self::Semantic(error) => error.message.fmt(formatter),
            Self::ExpectedSingleModule { actual } => write!(
                formatter,
                "portable IR emission requires exactly one HIR module, got {actual}"
            ),
            Self::Lower(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
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

/// Lower one verified QB HIR program into portable SSA IR.
pub fn lower_qb_to_ir(program: &Program) -> Result<ir::Module, Error> {
    let [module] = program.modules.as_slice() else {
        return Err(Error::ExpectedSingleModule {
            actual: program.modules.len(),
        });
    };
    crate::hir::lower_to_ir(module).map_err(Error::Lower)
}

/// Select verified portable IR into initial x86 Machine IR.
pub fn lower_ir_to_machine(
    module: &ir::Module,
) -> Result<crate::codegen::machine::MachineModule, Error> {
    crate::target::x86::select_module(module).map_err(Error::Selection)
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
    use super::{Error, QbOptions, compile_qb, lower_ir_to_machine, lower_qb_to_ir, parse_omf};
    use crate::hir::{
        ArrayOrder, Dialect, FORMAT_VERSION, FloatMode, Program, RuntimeProfile, TargetProfile,
    };
    use crate::ir;
    use crate::object::omf::record::Record;

    #[test]
    fn untouched_omf_survives_the_driver_boundary_byte_for_byte() {
        let mut bytes = Record::new(0x80, vec![1, b'm']).unwrap().to_bytes();
        *bytes.last_mut().unwrap() = 0x5a;

        let file = parse_omf(&bytes).unwrap();

        assert_eq!(file.to_bytes(), bytes);
    }

    #[test]
    fn portable_ir_lowering_refuses_a_program_without_one_module() {
        let program = Program {
            version: FORMAT_VERSION,
            dialect: Dialect::Vbdos,
            runtime: RuntimeProfile::Vbdos,
            target: TargetProfile::I386RealMode,
            array_order: ArrayOrder::ColumnMajor,
            float_mode: FloatMode::Inline,
            modules: Vec::new(),
        };

        assert!(matches!(
            lower_qb_to_ir(&program),
            Err(Error::ExpectedSingleModule { actual: 0 })
        ));
    }

    #[test]
    fn qb_runtime_calls_reach_verified_portable_ir() {
        let program = compile_qb("screen 0\nend\n", "runtime", QbOptions::default())
            .expect("QB source compiles to HIR");

        let module = lower_qb_to_ir(&program).expect("runtime calls lower to portable IR");

        assert!(module.verify().is_ok());
        assert!(
            module
                .functions
                .iter()
                .any(|function| function.name == "B$CSCN")
        );
        assert!(
            module
                .functions
                .iter()
                .any(|function| function.name == "B$CEND")
        );
        assert!(matches!(
            module.functions[0].blocks[0].instructions[0].kind,
            ir::InstructionKind::Call {
                callee: ir::Callee::Direct(_),
                ..
            }
        ));
    }

    #[test]
    fn qb_string_relocations_reach_verified_portable_ir() {
        let program = compile_qb(
            "dim n as long\nprint \"A\"\n",
            "strings",
            QbOptions::default(),
        )
        .expect("QB string source compiles to HIR");

        let module = lower_qb_to_ir(&program).expect("QB data relocations lower to portable IR");

        assert!(module.verify().is_ok());
        assert!(module.globals.iter().any(|global| matches!(
            &global.initializer,
            Some(ir::Constant::RelocatableBytes { relocations, .. })
                if !relocations.is_empty()
        )));
        assert!(
            module.functions[0].blocks[0]
                .instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, ir::InstructionKind::Call { .. }))
        );
    }

    #[test]
    fn qb_defined_procedure_and_local_result_reach_portable_ir() {
        let program = compile_qb(
            include_str!("../../frontends/qb/fixtures/procedure.bas"),
            "procedure",
            QbOptions::default(),
        )
        .expect("QB procedure source compiles to HIR");

        let module = lower_qb_to_ir(&program).expect("QB procedure lowers to portable IR");

        assert!(module.verify().is_ok());
        let main = module
            .functions
            .iter()
            .find(|function| function.name == "__main")
            .expect("module entry function");
        let procedure = module
            .functions
            .iter()
            .find(|function| function.name.eq_ignore_ascii_case("twice&"))
            .expect("defined QB function");
        assert!(
            main.blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    ir::InstructionKind::Call {
                        callee: ir::Callee::Direct(target),
                        ..
                    } if target == procedure.id
                ))
        );
        assert!(
            procedure
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    ir::InstructionKind::StackAlloc { size: 4, .. }
                ))
        );
    }

    #[test]
    fn verified_integer_ir_reaches_x86_machine_ir() {
        let source = concat!(
            "qir 1\n",
            "module \"driver\"\n",
            "type 0 void\n",
            "type 1 integer 16\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc basic attributes []\n",
            "block 0\n",
            "inst 0 results [0:1] binary add const type 1 integer 2 const type 1 integer 3\n",
            "term return none\n",
            "endblock\n",
            "endfunction\n",
            "end\n",
        );
        let module = ir::parse_text(source).expect("test qir verifies");

        let machine = lower_ir_to_machine(&module).expect("integer qir selects");

        assert!(machine.verify().is_ok());
        assert_eq!(machine.functions.len(), 1);
        assert_eq!(
            machine.functions[0].blocks[0]
                .instructions
                .last()
                .unwrap()
                .opcode,
            crate::target::x86::X86Opcode::ReturnNear.machine_opcode()
        );
    }
}
