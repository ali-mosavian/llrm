//! Whole-pipeline orchestration and diagnostics.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::codegen::machine::{
    AllocationRewriteError, MachineFunctionId, MachineModule, MachineOperandKind, apply_assignment,
};
use crate::frontend::qb::{self, Dialect};
use crate::frontend::wcc;
use crate::hir::{LowerError, Program, RuntimeProfile, Storage};
use crate::ir;
use crate::object::omf::file::{File as OmfFile, FileError};
use crate::object::omf::write::WriteError as OmfWriteError;
use crate::support::diagnostic::Diagnostic;
use crate::target::x86::{
    BasicAbiError, BasicAbiExpansionError, BasicFramePlan, BasicRuntime, CAbiExpansionError,
    CFramePlan, CFramePlanError, CallClobberError, FrameIndexMaterializationError, SelectionError,
    X86AllocationError, X86JumpLayoutError, X86McModuleLowerError, X86OmfError,
};

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

/// Verified QB-specific Machine IR together with its target frame plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QbMachine {
    pub module: MachineModule,
    pub frames: BTreeMap<MachineFunctionId, BasicFramePlan>,
}

/// Verified C-family Machine IR together with its target frame plans.
///
/// This is deliberately frontend-neutral: WCC capture happens before portable
/// IR, and the driver only assembles that IR with the x86 C ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CMachine {
    pub module: MachineModule,
    pub frames: BTreeMap<MachineFunctionId, CFramePlan>,
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
    QbStatementMetadata(qb::statement_table::MetadataError),
    QbModuleHeader(qb::module_header::ModuleHeaderError),
    QbMc(qb::mc::ModuleMcError),
    QbStatementMc(qb::statement_mc::StatementMcError),
    QbObject(qb::object::ObjectEnvelopeError),
    QbStatementRowsUnsupported {
        count: usize,
    },
    QbTextSectionCount {
        count: usize,
    },
    QbStatementTableLayout(crate::mc::LayoutError),
    MissingQbStatementTableSymbol,
    QbStatementTableOffsetOverflow {
        offset: u64,
    },
    WccParse(wcc::ParseError),
    WccCapture(wcc::capture::BuildError),
    WccRaise(wcc::RaiseError),
    Hir(Vec<Diagnostic>),
    ExpectedSingleModule {
        actual: usize,
    },
    Lower(LowerError),
    Selection(SelectionError),
    BasicAbi {
        function: String,
        error: BasicAbiError,
    },
    CallClobber {
        function: String,
        error: CallClobberError,
    },
    Allocation {
        function: String,
        error: X86AllocationError,
    },
    AllocationRewrite {
        function: String,
        error: AllocationRewriteError,
    },
    FrameIndices {
        function: String,
        error: FrameIndexMaterializationError,
    },
    BasicAbiExpansion {
        function: String,
        error: BasicAbiExpansionError,
    },
    CFrame {
        function: String,
        error: CFramePlanError,
    },
    CAbiExpansion {
        function: String,
        error: CAbiExpansionError,
    },
    MissingFramePlan {
        function: String,
    },
    MissingCFramePlan {
        function: String,
    },
    MissingSourceFunction(MachineFunctionId),
    TemporaryStringCountTooLarge {
        function: String,
        count: usize,
    },
    Machine(Vec<Diagnostic>),
    Mc(X86McModuleLowerError),
    McEncoding(X86JumpLayoutError),
    X86Omf(X86OmfError),
    OmfWrite(OmfWriteError),
    Omf(FileError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.message.fmt(formatter),
            Self::Semantic(error) => error.message.fmt(formatter),
            Self::QbStatementMetadata(error) => error.fmt(formatter),
            Self::QbModuleHeader(error) => error.fmt(formatter),
            Self::QbMc(error) => error.fmt(formatter),
            Self::QbStatementMc(error) => error.fmt(formatter),
            Self::QbObject(error) => error.fmt(formatter),
            Self::QbStatementRowsUnsupported { count } => write!(
                formatter,
                "initial QB object emission cannot yet place {count} runtime statement row(s)"
            ),
            Self::QbTextSectionCount { count } => write!(
                formatter,
                "QB object emission requires exactly one MC text section, found {count}"
            ),
            Self::QbStatementTableLayout(error) => error.fmt(formatter),
            Self::MissingQbStatementTableSymbol => {
                formatter.write_str("QB statement-table symbol has no final definition")
            }
            Self::QbStatementTableOffsetOverflow { offset } => write!(
                formatter,
                "QB statement-table offset {offset:#x} cannot be represented in OMF"
            ),
            Self::WccParse(error) => error.fmt(formatter),
            Self::WccCapture(error) => error.fmt(formatter),
            Self::WccRaise(error) => error.fmt(formatter),
            Self::Hir(diagnostics) => {
                if let Some(diagnostic) = diagnostics.first() {
                    write!(formatter, "invalid HIR: {}", diagnostic.message)
                } else {
                    write!(formatter, "invalid HIR")
                }
            }
            Self::ExpectedSingleModule { actual } => write!(
                formatter,
                "portable IR emission requires exactly one HIR module, got {actual}"
            ),
            Self::Lower(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
            Self::BasicAbi { function, error } => {
                write!(formatter, "cannot expand BASIC ABI for {function}: {error}")
            }
            Self::CallClobber { function, error } => {
                write!(
                    formatter,
                    "cannot materialize call clobbers for {function}: {error}"
                )
            }
            Self::Allocation { function, error } => {
                write!(
                    formatter,
                    "cannot allocate x86 registers for {function}: {error}"
                )
            }
            Self::AllocationRewrite { function, error } => write!(
                formatter,
                "cannot apply x86 register allocation for {function}: {error}"
            ),
            Self::FrameIndices { function, error } => write!(
                formatter,
                "cannot materialize x86 frame indices for {function}: {error}"
            ),
            Self::BasicAbiExpansion { function, error } => write!(
                formatter,
                "cannot finalize the allocated BASIC x86 ABI for {function}: {error}"
            ),
            Self::CFrame { function, error } => {
                write!(
                    formatter,
                    "cannot plan the C x86 frame for {function}: {error}"
                )
            }
            Self::CAbiExpansion { function, error } => write!(
                formatter,
                "cannot finalize the allocated C x86 ABI for {function}: {error}"
            ),
            Self::MissingFramePlan { function } => write!(
                formatter,
                "machine function {function} uses frame indices without a BASIC frame plan"
            ),
            Self::MissingCFramePlan { function } => write!(
                formatter,
                "machine function {function} uses frame indices without a C frame plan"
            ),
            Self::MissingSourceFunction(function) => write!(
                formatter,
                "selected machine function {function} has no QB source function"
            ),
            Self::TemporaryStringCountTooLarge { function, count } => write!(
                formatter,
                "QB function {function} owns {count} local STRING descriptors; maximum is {}",
                u16::MAX
            ),
            Self::Machine(diagnostics) => {
                if let Some(diagnostic) = diagnostics.first() {
                    write!(formatter, "invalid x86 Machine IR: {}", diagnostic.message)
                } else {
                    write!(formatter, "invalid x86 Machine IR")
                }
            }
            Self::Mc(error) => write!(formatter, "cannot lower allocated x86 module: {error}"),
            Self::McEncoding(error) => write!(formatter, "cannot encode x86 MC module: {error}"),
            Self::X86Omf(error) => write!(formatter, "cannot lower x86 MC to OMF: {error}"),
            Self::OmfWrite(error) => write!(formatter, "cannot write OMF object: {error}"),
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

/// Parse a WCC capture and lower its verified source-neutral HIR to IR.
pub fn compile_wcc_capture(source: &str, module_name: &str) -> Result<ir::Module, Error> {
    let records = wcc::parse(source).map_err(Error::WccParse)?;
    let unit = wcc::capture::build(&records).map_err(Error::WccCapture)?;
    let module = wcc::raise_module(&unit, module_name).map_err(Error::WccRaise)?;
    module.verify().map_err(Error::Hir)?;
    crate::hir::lower_to_ir(&module).map_err(Error::Lower)
}

/// Lower one verified QB HIR program into portable SSA IR.
pub fn lower_qb_to_ir(program: &Program) -> Result<ir::Module, Error> {
    let [module] = program.modules.as_slice() else {
        return Err(Error::ExpectedSingleModule {
            actual: program.modules.len(),
        });
    };
    let extracted = qb::statement_table::extract(module).map_err(Error::QbStatementMetadata)?;
    crate::hir::lower_to_ir(&extracted.module).map_err(Error::Lower)
}

/// Select verified portable IR into initial x86 Machine IR.
pub fn lower_ir_to_machine(
    module: &ir::Module,
) -> Result<crate::codegen::machine::MachineModule, Error> {
    let machine = crate::target::x86::select_module(module).map_err(Error::Selection)?;
    crate::target::x86::verify_machine(&machine).map_err(Error::Machine)?;
    Ok(machine)
}

/// Lower verified QB HIR through the target-owned runtime ABI boundary.
pub fn lower_qb_to_machine(program: &Program) -> Result<QbMachine, Error> {
    let [source] = program.modules.as_slice() else {
        return Err(Error::ExpectedSingleModule {
            actual: program.modules.len(),
        });
    };
    let ir = lower_qb_to_ir(program)?;
    let mut machine = lower_ir_to_machine(&ir)?;
    let source_functions = source
        .functions
        .iter()
        .map(|function| (function.id.get(), function))
        .collect::<BTreeMap<_, _>>();
    let string_types = source
        .types
        .iter()
        .filter(|type_| type_.name == "string")
        .map(|type_| type_.id)
        .collect::<BTreeSet<_>>();
    let runtime = basic_runtime(program.runtime);
    let mut frames = BTreeMap::new();

    for function in &mut machine.functions {
        let source_function = source_functions
            .get(&function.id.get())
            .copied()
            .ok_or(Error::MissingSourceFunction(function.id))?;
        if source_function.name != "__main" {
            let temporary_strings = temporary_string_slots(source_function, &string_types)?;
            let expanded = crate::target::x86::expand_basic_runtime(
                function,
                runtime,
                u32::from(temporary_strings),
            )
            .map_err(|error| Error::BasicAbi {
                function: source_function.name.clone(),
                error,
            })?;
            frames.insert(function.id, expanded.frame);
            *function = expanded.function;
        }
        *function =
            crate::target::x86::materialize_far_call_clobbers(function).map_err(|error| {
                Error::CallClobber {
                    function: source_function.name.clone(),
                    error,
                }
            })?;
    }
    crate::target::x86::verify_machine(&machine).map_err(Error::Machine)?;
    Ok(QbMachine {
        module: machine,
        frames,
    })
}

/// Allocates one selected QB module and resolves its planned frame operands.
pub fn allocate_qb_machine(selected: &QbMachine) -> Result<QbMachine, Error> {
    let mut allocated = selected.clone();
    for function in &mut allocated.module.functions {
        let assignment = crate::target::x86::allocate_registers(function).map_err(|error| {
            Error::Allocation {
                function: function.name.clone(),
                error,
            }
        })?;
        *function =
            apply_assignment(function, &assignment).map_err(|error| Error::AllocationRewrite {
                function: function.name.clone(),
                error,
            })?;

        if let Some(frame) = allocated.frames.get(&function.id) {
            *function = crate::target::x86::materialize_frame_indices(function, frame).map_err(
                |error| Error::FrameIndices {
                    function: function.name.clone(),
                    error,
                },
            )?;
        } else if function.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| {
                instruction
                    .operands
                    .iter()
                    .any(|operand| matches!(operand.kind, MachineOperandKind::FrameIndex { .. }))
            })
        }) {
            return Err(Error::MissingFramePlan {
                function: function.name.clone(),
            });
        }
    }
    crate::target::x86::verify_machine(&allocated.module).map_err(Error::Machine)?;
    Ok(allocated)
}

/// Allocates selected QB Machine IR, finalizes its target ABI, and lowers it to MC.
pub fn lower_qb_machine_to_mc(selected: &QbMachine) -> Result<crate::mc::MCModule, Error> {
    let mut allocated = allocate_qb_machine(selected)?;
    for function in &mut allocated.module.functions {
        *function = crate::target::x86::expand_allocated_basic_abi(function).map_err(|error| {
            Error::BasicAbiExpansion {
                function: function.name.clone(),
                error,
            }
        })?;
    }
    crate::target::x86::verify_machine(&allocated.module).map_err(Error::Machine)?;
    crate::target::x86::lower_allocated_module(&allocated.module).map_err(Error::Mc)
}

/// Select portable IR into x86 Machine IR and plan its C-family stack frames.
///
/// WCC capture is one producer of this input, but C ABI selection intentionally
/// depends only on generic IR calling conventions and Machine IR facts.
pub fn lower_c_to_machine(module: &ir::Module) -> Result<CMachine, Error> {
    let machine = lower_ir_to_machine(module)?;
    let mut frames = BTreeMap::new();
    for function in &machine.functions {
        let plan = crate::target::x86::plan_c_frame(function).map_err(|error| Error::CFrame {
            function: function.name.clone(),
            error,
        })?;
        frames.insert(function.id, plan);
    }
    Ok(CMachine {
        module: machine,
        frames,
    })
}

/// Allocate C-family Machine IR and resolve its target-planned frame indices.
pub fn allocate_c_machine(selected: &CMachine) -> Result<CMachine, Error> {
    let mut allocated = selected.clone();
    for function in &mut allocated.module.functions {
        let assignment = crate::target::x86::allocate_registers(function).map_err(|error| {
            Error::Allocation {
                function: function.name.clone(),
                error,
            }
        })?;
        *function =
            apply_assignment(function, &assignment).map_err(|error| Error::AllocationRewrite {
                function: function.name.clone(),
                error,
            })?;

        if let Some(frame) = allocated.frames.get(&function.id) {
            *function =
                crate::target::x86::materialize_frame_indices_with_layout(function, frame.layout())
                    .map_err(|error| Error::FrameIndices {
                        function: function.name.clone(),
                        error,
                    })?;
        } else if function.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| {
                instruction
                    .operands
                    .iter()
                    .any(|operand| matches!(operand.kind, MachineOperandKind::FrameIndex { .. }))
            })
        }) {
            return Err(Error::MissingCFramePlan {
                function: function.name.clone(),
            });
        }
    }
    crate::target::x86::verify_machine(&allocated.module).map_err(Error::Machine)?;
    Ok(allocated)
}

/// Allocate selected C-family Machine IR, finalize its x86 ABI, and lower it
/// to target-neutral MC.
pub fn lower_c_machine_to_mc(selected: &CMachine) -> Result<crate::mc::MCModule, Error> {
    let mut allocated = allocate_c_machine(selected)?;
    for function in &mut allocated.module.functions {
        let frame = allocated
            .frames
            .get(&function.id)
            .ok_or_else(|| Error::MissingCFramePlan {
                function: function.name.clone(),
            })?;
        *function =
            crate::target::x86::expand_allocated_c_abi(function, frame).map_err(|error| {
                Error::CAbiExpansion {
                    function: function.name.clone(),
                    error,
                }
            })?;
    }
    crate::target::x86::verify_machine(&allocated.module).map_err(Error::Machine)?;
    crate::target::x86::lower_allocated_module(&allocated.module).map_err(Error::Mc)
}

/// Encodes and relaxes a symbolic physical x86 MC module without object policy.
pub fn encode_x86_mc(module: &crate::mc::MCModule) -> Result<crate::mc::MCModule, Error> {
    crate::target::x86::relax_and_encode_jumps(module).map_err(Error::McEncoding)
}

/// Writes an already encoded x86 MC module as a fresh, target-neutral OMF object.
///
/// Source-language envelopes are deliberately not inferred here. A frontend
/// adapter must add any runtime-owned header or entry convention before MC.
pub fn write_x86_omf(module_name: &[u8], module: &crate::mc::MCModule) -> Result<Vec<u8>, Error> {
    let object = crate::target::x86::lower_to_omf(module_name, module).map_err(Error::X86Omf)?;
    crate::object::omf::write::to_bytes(&object).map_err(Error::OmfWrite)
}

/// Compile the current QB slice into its measured BASIC OMF envelope.
///
/// The QB adapter owns runtime data placement and the BASIC object envelope;
/// generic x86 lowering remains unaware of those source-language conventions.
pub fn write_qb_omf(program: &Program, module_name: &[u8]) -> Result<Vec<u8>, Error> {
    let [source] = program.modules.as_slice() else {
        return Err(Error::ExpectedSingleModule {
            actual: program.modules.len(),
        });
    };
    let metadata = qb::statement_table::extract(source).map_err(Error::QbStatementMetadata)?;
    if !metadata.rows.is_empty() {
        return Err(Error::QbStatementRowsUnsupported {
            count: metadata.rows.len(),
        });
    }

    let selected = lower_qb_to_machine(program)?;
    let lowered = lower_qb_machine_to_mc(&selected)?;
    let text_sections = lowered
        .sections
        .iter()
        .filter(|section| section.kind == crate::mc::SectionKind::Text)
        .map(|section| section.id)
        .collect::<Vec<_>>();
    let [text_section] = text_sections.as_slice() else {
        return Err(Error::QbTextSectionCount {
            count: text_sections.len(),
        });
    };
    let placed = qb::mc::place_data(&lowered, &selected.module.data_objects, program.runtime)
        .map_err(Error::QbMc)?;
    let table = qb::statement_mc::append_statement_table(
        &placed,
        *text_section,
        &[],
        crate::target::x86::X86FixupKind::Absolute16.into(),
    )
    .map_err(Error::QbStatementMc)?;
    let header = qb::module_header::module_header(program).map_err(Error::QbModuleHeader)?;
    let prefixed =
        qb::mc::prepend_module_header(&table.module, *text_section, header).map_err(Error::QbMc)?;
    let encoded = encode_x86_mc(&prefixed)?;
    let layout = crate::mc::layout(&encoded, |_| None).map_err(Error::QbStatementTableLayout)?;
    let crate::mc::SymbolLayout::Defined { offset, .. } = layout
        .symbols
        .get(&table.table_symbol)
        .copied()
        .ok_or(Error::MissingQbStatementTableSymbol)?
    else {
        return Err(Error::MissingQbStatementTableSymbol);
    };
    let statement_table_offset =
        u32::try_from(offset).map_err(|_| Error::QbStatementTableOffsetOverflow { offset })?;
    let generic = crate::target::x86::lower_to_omf(module_name, &encoded).map_err(Error::X86Omf)?;
    let object = qb::object::add_object_envelope(&generic, program, statement_table_offset)
        .map_err(Error::QbObject)?;
    crate::object::omf::write::to_bytes(&object).map_err(Error::OmfWrite)
}

fn temporary_string_slots(
    function: &crate::hir::Function,
    string_types: &BTreeSet<crate::hir::TypeId>,
) -> Result<u16, Error> {
    let count = function
        .places
        .iter()
        .filter(|place| place.storage == Storage::Local && string_types.contains(&place.type_id))
        .count();
    u16::try_from(count).map_err(|_| Error::TemporaryStringCountTooLarge {
        function: function.name.clone(),
        count,
    })
}

fn runtime_name(runtime: RuntimeProfile) -> &'static str {
    match runtime {
        RuntimeProfile::Qb45 => "qb45",
        RuntimeProfile::Pds71 => "pds71",
        RuntimeProfile::Vbdos => "vbdos",
    }
}

fn basic_runtime(runtime: RuntimeProfile) -> BasicRuntime {
    match runtime {
        RuntimeProfile::Qb45 => BasicRuntime::Qb45,
        RuntimeProfile::Pds71 => BasicRuntime::Pds71,
        RuntimeProfile::Vbdos => BasicRuntime::Vbdos,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        Error, QbOptions, allocate_qb_machine, compile_qb, compile_wcc_capture, encode_x86_mc,
        lower_c_machine_to_mc, lower_c_to_machine, lower_ir_to_machine, lower_qb_machine_to_mc,
        lower_qb_to_ir, lower_qb_to_machine, parse_omf, write_qb_omf, write_x86_omf,
    };
    use crate::codegen::machine::{
        FrameObjectKind, MachineAddressSpace, MachineCallingConvention, MachineOperandKind,
        MachineRegister, MachineValueType, RegisterConstraint,
    };
    use crate::hir::{
        ArrayOrder, Dialect, FORMAT_VERSION, FloatMode, Program, RuntimeProfile, TargetProfile,
    };
    use crate::ir;
    use crate::mc::{MCFragment, MCOperand, SymbolDefinition};
    use crate::object::omf::file::File as OmfFile;
    use crate::object::omf::module::DecodedModule;
    use crate::object::omf::record::Record;
    use crate::target::x86::{X86FixupKind, X86Opcode, X86Register};

    #[test]
    fn untouched_omf_survives_the_driver_boundary_byte_for_byte() {
        let mut bytes = Record::new(0x80, vec![1, b'm']).unwrap().to_bytes();
        *bytes.last_mut().unwrap() = 0x5a;

        let file = parse_omf(&bytes).unwrap();

        assert_eq!(file.to_bytes(), bytes);
    }

    #[test]
    fn c_iparg_capture_reaches_mc_through_the_c_abi() {
        // iparg.cgs is the captured counterpart of C's simple scalar call
        // path: a near helper returns an i16 in AX and a far entry leaves its
        // pushed argument to the caller. This keeps the WCC-to-Machine-IR
        // boundary exercised without teaching the driver WCC details.
        let ir = compile_wcc_capture(include_str!("../../fixtures/c/iparg.cgs"), "iparg")
            .expect("real WCC capture lowers to portable IR");
        let selected = lower_c_to_machine(&ir).expect("portable C IR selects and plans frames");
        assert_eq!(selected.module.functions.len(), 2);
        assert!(
            selected
                .module
                .functions
                .iter()
                .all(|function| selected.frames.contains_key(&function.id))
        );
        assert_eq!(selected.module.functions[0].name, "_twice");
        assert_eq!(selected.module.functions[1].name, "_answer_from_argument");
        let mc = lower_c_machine_to_mc(&selected).expect("allocated C functions lower to MC");
        mc.verify().expect("C MC verifies");
        assert!(mc.symbols.iter().any(|symbol| symbol.name == "_twice"));
        assert!(
            mc.symbols
                .iter()
                .any(|symbol| symbol.name == "_answer_from_argument")
        );
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
    fn qb_statement_rows_do_not_become_portable_data() {
        let mut program = compile_qb("", "metadata", QbOptions::default()).unwrap();
        let metadata = program.modules[0]
            .data
            .iter_mut()
            .find(|object| {
                object.name == crate::frontend::qb::statement_table::METADATA_OBJECT_NAME
            })
            .unwrap();
        metadata.bytes.extend_from_slice(&1_u32.to_le_bytes());
        metadata.bytes.extend_from_slice(&2_u32.to_le_bytes());
        metadata.bytes.extend_from_slice(&3_u32.to_le_bytes());
        metadata.bytes.extend_from_slice(&100_u16.to_le_bytes());

        let lowered = lower_qb_to_ir(&program).unwrap();

        assert!(lowered.globals.iter().all(
            |global| global.name != crate::frontend::qb::statement_table::METADATA_OBJECT_NAME
        ));
    }

    #[test]
    fn qb_procedure_preserves_byref_frames_and_dx_ax_long_abi() {
        // The historical Python regressions establish four independent facts:
        // a default numeric formal is BYREF, its published load stays volatile,
        // a LONG result crosses a BASIC call in DX:AX, and `retf` pops the
        // pointer argument.  Keep them together because procedure.bas exercises
        // the complete source-to-Machine-IR boundary.
        let program = compile_qb(
            include_str!("../../frontends/qb/fixtures/procedure.bas"),
            "procedure",
            QbOptions::default(),
        )
        .expect("procedure fixture compiles to HIR");
        let lowered =
            lower_qb_to_machine(&program).expect("procedure fixture expands the BASIC x86 ABI");
        let machine = &lowered.module;

        assert_eq!(
            machine
                .data_objects
                .iter()
                .map(|object| object.name.as_str())
                .collect::<Vec<_>>(),
            ["RESULT&", "INPUTVALUE&"]
        );
        let main = machine
            .functions
            .iter()
            .find(|function| function.name == "__main")
            .expect("module entry function exists");
        let procedure = machine
            .functions
            .iter()
            .find(|function| function.name == "TWICE&")
            .expect("defined BASIC function exists");
        let source_procedure = program.modules[0]
            .functions
            .iter()
            .find(|function| function.name == "TWICE&")
            .expect("source BASIC function exists");

        assert_eq!(
            procedure.signature.calling_convention,
            MachineCallingConvention::FarPascal
        );
        assert_eq!(
            procedure.signature.result,
            Some(MachineValueType::Integer { bits: 32 })
        );
        assert_eq!(
            procedure.signature.parameters,
            [MachineValueType::Pointer {
                bits: 16,
                address_space: MachineAddressSpace::NearData,
            }]
        );
        assert!(matches!(
            procedure.frame_objects.as_slice(),
            [
                crate::codegen::machine::FrameObject {
                    size: 2,
                    kind: FrameObjectKind::IncomingArgument { parameter: 0 },
                    ..
                },
                crate::codegen::machine::FrameObject {
                    size: 4,
                    kind: FrameObjectKind::Local,
                    ..
                }
            ]
        ));
        let frame = lowered
            .frames
            .get(&procedure.id)
            .expect("the BASIC procedure retains its frame plan");
        assert_eq!(frame.header_bytes(), 20);
        assert_eq!(frame.local_bytes(), 4);
        assert_eq!(frame.parameter_bytes(), 2);
        assert_eq!(frame.offset(procedure.frame_objects[0].index), Some(6));
        assert_eq!(frame.offset(procedure.frame_objects[1].index), Some(-24));
        let entry = procedure
            .blocks
            .iter()
            .find(|block| block.id.get() == source_procedure.entry.get())
            .expect("procedure entry block survives selection");
        assert_eq!(
            entry.instructions[..3]
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            [
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::CallFar.machine_opcode(),
            ]
        );
        assert!(matches!(
            &entry.instructions[2].operands[0].kind,
            MachineOperandKind::ExternalSymbol { name, .. } if name == "B$ENRA"
        ));
        assert!(
            procedure
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    instruction.opcode == X86Opcode::Load.machine_opcode()
                        && instruction.flags.volatile
                        && matches!(
                            instruction.operands.get(1).map(|operand| &operand.kind),
                            Some(MachineOperandKind::Register(MachineRegister::Virtual(_)))
                        )
                })
        );

        let call = main
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|instruction| instruction.opcode == X86Opcode::CallFar.machine_opcode())
            .expect("caller contains a far defined call");
        assert!(
            matches!(call.operands[0].kind, MachineOperandKind::Function(id) if id == procedure.id)
        );
        assert_eq!(
            call.operands[1].constraint,
            Some(RegisterConstraint::Fixed(X86Register::Ax.physical()))
        );
        assert_eq!(
            call.operands[2].constraint,
            Some(RegisterConstraint::Fixed(X86Register::Dx.physical()))
        );

        let returned = procedure
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|instruction| instruction.opcode == X86Opcode::ReturnFar.machine_opcode())
            .expect("callee contains a far return");
        let (return_block, return_position) = procedure
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .instructions
                    .iter()
                    .position(|instruction| instruction.id == returned.id)
                    .map(|position| (block, position))
            })
            .expect("the far return remains in its selected block");
        assert!(matches!(
            &return_block.instructions[return_position - 1].operands[0].kind,
            MachineOperandKind::ExternalSymbol { name, .. } if name == "B$EXSA"
        ));
        assert_eq!(
            returned.operands[0].constraint,
            Some(RegisterConstraint::Fixed(X86Register::Ax.physical()))
        );
        assert_eq!(
            returned.operands[1].constraint,
            Some(RegisterConstraint::Fixed(X86Register::Dx.physical()))
        );
        assert!(matches!(
            returned.operands[2].kind,
            MachineOperandKind::Immediate(2)
        ));
    }

    #[test]
    fn qb_procedure_allocation_materializes_its_basic_frame_indices() {
        let program = compile_qb(
            include_str!("../../frontends/qb/fixtures/procedure.bas"),
            "procedure",
            QbOptions::default(),
        )
        .expect("procedure fixture compiles to HIR");
        let selected = lower_qb_to_machine(&program).expect("procedure reaches selected qmir");
        let allocated = allocate_qb_machine(&selected).expect("procedure qmir allocates");
        assert!(selected.module.functions.iter().any(|function| {
            !function.virtual_registers.is_empty()
                && function.blocks.iter().any(|block| {
                    block.instructions.iter().any(|instruction| {
                        instruction.operands.iter().any(|operand| {
                            matches!(operand.kind, MachineOperandKind::FrameIndex { .. })
                        })
                    })
                })
        }));
        let procedure = allocated
            .module
            .functions
            .iter()
            .find(|function| function.name == "TWICE&")
            .expect("defined procedure remains present");

        assert!(procedure.virtual_registers.is_empty());
        assert!(procedure.blocks.iter().all(|block| {
            block.instructions.iter().all(|instruction| {
                instruction.operands.iter().all(|operand| {
                    !matches!(
                        operand.kind,
                        MachineOperandKind::FrameIndex { .. }
                            | MachineOperandKind::Register(MachineRegister::Virtual(_))
                    )
                })
            })
        }));
        let displacements = procedure
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    X86Opcode::from_machine_opcode(instruction.opcode),
                    Some(X86Opcode::Load | X86Opcode::Store | X86Opcode::Lea)
                )
            })
            .flat_map(|instruction| &instruction.operands)
            .filter_map(|operand| match operand.kind {
                MachineOperandKind::Immediate(value) => Some(value),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert!(displacements.contains(&6));
        assert!(displacements.contains(&-24));
    }

    #[test]
    fn qb_procedure_reaches_deterministic_symbolic_mc() {
        let program = compile_qb(
            include_str!("../../frontends/qb/fixtures/procedure.bas"),
            "procedure",
            QbOptions::default(),
        )
        .expect("procedure fixture compiles to HIR");
        let selected = lower_qb_to_machine(&program).expect("procedure reaches selected qmir");
        let before = selected.clone();

        let first = lower_qb_machine_to_mc(&selected).expect("allocated procedure reaches MC");
        let second = lower_qb_machine_to_mc(&selected).expect("MC lowering is repeatable");

        assert_eq!(selected, before);
        assert_eq!(first, second);
        first.verify().expect("driver returns verified MC");
        assert_eq!(
            first
                .sections
                .iter()
                .map(|section| section.name.as_str())
                .collect::<Vec<_>>(),
            vec![".text", ".rodata", ".data"]
        );
        for defined in ["__main", "TWICE&"] {
            let symbol = first
                .symbols
                .iter()
                .find(|symbol| symbol.name == defined)
                .expect("defined QB function has an MC symbol");
            assert!(matches!(
                symbol.definition,
                SymbolDefinition::Fragment { .. }
            ));
        }
        for runtime in ["B$ENRA", "B$EXSA"] {
            let symbol = first
                .symbols
                .iter()
                .find(|symbol| symbol.name == runtime)
                .expect("BASIC frame runtime call has an MC symbol");
            assert_eq!(symbol.definition, SymbolDefinition::Undefined);
        }

        let instructions = first.sections[0]
            .fragments
            .iter()
            .filter_map(|fragment| match fragment {
                MCFragment::Instruction(fragment) => Some(&fragment.instruction),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(instructions.iter().all(|instruction| !matches!(
            X86Opcode::from_raw(instruction.opcode.get()),
            Some(X86Opcode::MergeWords | X86Opcode::LowWord | X86Opcode::HighWord)
        )));
        assert!(instructions
            .iter()
            .filter(|instruction| {
                X86Opcode::from_raw(instruction.opcode.get()) == Some(X86Opcode::CallFar)
            })
            .all(|instruction| matches!(
                instruction.operands.as_slice(),
                [MCOperand::Expression(_)]
            )));
        assert!(instructions.iter().any(|instruction| {
            X86Opcode::from_raw(instruction.opcode.get()) == Some(X86Opcode::ReturnFar)
                && instruction.operands == [MCOperand::Immediate(2)]
        }));
        assert!(instructions.iter().any(|instruction| {
            X86Opcode::from_raw(instruction.opcode.get()) == Some(X86Opcode::Jump)
        }));

        let encoded = encode_x86_mc(&first).expect("symbolic MC reaches encoded fragments");
        encoded.verify().expect("encoded MC remains verified");
        assert!(encoded.sections.iter().all(|section| {
            section
                .fragments
                .iter()
                .all(|fragment| !matches!(fragment, MCFragment::Instruction(_)))
        }));
        assert!(encoded.sections.iter().any(|section| {
            section
                .fragments
                .iter()
                .filter_map(|fragment| match fragment {
                    MCFragment::Data(data) => Some(data),
                    _ => None,
                })
                .flat_map(|data| &data.fixups)
                .any(|fixup| fixup.kind == X86FixupKind::FarPointer1616.into())
        }));

        let expected_fixups = encoded
            .sections
            .iter()
            .flat_map(|section| &section.fragments)
            .filter_map(|fragment| match fragment {
                MCFragment::Data(data) => Some(&data.fixups),
                _ => None,
            })
            .map(Vec::len)
            .sum::<usize>();
        let bytes = write_x86_omf(b"procedure.bas", &encoded)
            .expect("encoded x86 MC reaches a fresh generic OMF object");
        let OmfFile::Object(records) = parse_omf(&bytes).expect("fresh OMF parses") else {
            panic!("fresh output must be one object module");
        };
        let decoded = DecodedModule::parse(&records).expect("fresh OMF semantics decode");
        assert_eq!(decoded.fixups.len(), expected_fixups);
        for runtime in [b"B$ENRA".as_slice(), b"B$EXSA".as_slice()] {
            assert!(
                decoded
                    .symbols
                    .externals
                    .iter()
                    .flatten()
                    .any(|external| { external.name == runtime })
            );
        }
    }

    #[test]
    fn qb_emission_fixture_reaches_its_measured_object_envelope() {
        let options = QbOptions {
            dialect: crate::frontend::qb::Dialect::QuickBasic45,
            runtime: RuntimeProfile::Qb45,
            ..QbOptions::default()
        };
        let program = compile_qb(
            include_str!("../../frontends/qb/fixtures/emission.bas"),
            "emission",
            options,
        )
        .expect("established emission fixture compiles to HIR");
        let bytes = write_qb_omf(&program, b"emission.bas")
            .expect("data-free QB fixture reaches its BASIC object envelope");
        let OmfFile::Object(records) = parse_omf(&bytes).expect("fresh QB OMF parses") else {
            panic!("fresh QB output must be one object module");
        };
        let decoded = DecodedModule::parse(&records).expect("fresh QB OMF semantics decode");
        let segment_names = decoded
            .segments
            .segments
            .iter()
            .skip(1)
            .map(|segment| {
                let segment = segment.as_ref().unwrap();
                decoded.symbols.names[usize::from(segment.name_index)].clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            segment_names,
            [
                b"EMISSION_CODE".as_slice(),
                b"BR_DATA",
                b"BR_SKYS",
                b"COMMON",
                b"BC_DATA",
                b"NMALLOC",
                b"ENMALLOC",
                b"BC_FT",
                b"BC_CN",
                b"BC_DS",
                b"BC_SAB",
                b"BC_SA",
            ]
        );
        assert_eq!(
            decoded.declarations.groups[0]
                .members
                .iter()
                .map(|member| match member {
                    crate::object::omf::declarations::GroupMember::Segment { segment_index } => {
                        *segment_index
                    }
                })
                .collect::<Vec<_>>(),
            (2..=12).collect::<Vec<_>>()
        );
        assert!(decoded.declarations.publics.iter().any(|public| {
            public.name == b"ADDONE"
                && public.offset >= crate::frontend::qb::module_header::MODULE_HEADER_SIZE as u32
        }));

        let code_length = decoded.segments.segments[1].as_ref().unwrap().length as usize;
        let mut code = vec![0; code_length];
        for block in decoded.data.iter().filter(|block| block.segment_index == 1) {
            let start = block.offset as usize;
            code[start..start + block.bytes.len()].copy_from_slice(block.bytes);
        }
        assert_eq!(&code[..10], b"blEMISSION");
        let statement_at = usize::from(u16::from_le_bytes(code[10..12].try_into().unwrap()));
        assert_eq!(
            &code[statement_at - 3..statement_at + 2],
            &[0x55, 0x8b, 0xec, 0, 0]
        );
        assert!(decoded.fixups.iter().any(|fixup| {
            fixup.segment_index == 1
                && fixup.patch_offset == 10
                && fixup.location == crate::object::omf::fixups::Location::Offset16
                && fixup.frame.method == crate::object::omf::fixups::FrameMethod::Target
                && fixup.target.method == crate::object::omf::fixups::TargetMethod::Segment
                && fixup.target.datum == 1
        }));
        assert!(decoded.fixups.iter().any(|fixup| {
            fixup.segment_index == 12
                && fixup.location == crate::object::omf::fixups::Location::Pointer16_16
        }));
    }

    #[test]
    fn qb_runtime_frame_counts_owned_string_places_not_expression_temporaries() {
        // Nested LTRIM$/RTRIM$ was once counted as two frame handles even
        // though raw VBDOS emits BX=1 for SHOWCOMMAND's one local STRING.
        let program = compile_qb(
            include_str!("../../frontends/qb/fixtures/managed-temporaries.bas"),
            "managed-temporaries",
            QbOptions::default(),
        )
        .expect("managed temporary fixture compiles to HIR");
        let module = &program.modules[0];
        let function = module
            .functions
            .iter()
            .find(|function| function.name == "SHOWCOMMAND")
            .expect("fixture defines SHOWCOMMAND");
        let string_types = module
            .types
            .iter()
            .filter(|type_| type_.name == "string")
            .map(|type_| type_.id)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            super::temporary_string_slots(function, &string_types).unwrap(),
            1
        );
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
            "qir 5\n",
            "module \"driver\"\n",
            "type 0 void\n",
            "type 1 integer 16\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc far_pascal attributes []\n",
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
