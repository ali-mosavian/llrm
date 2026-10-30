//! Compile with qbfront, linked in, and
//! decode its common-HIR document.

use std::fmt;
use std::path::{Path, PathBuf};

use llrm_codegen::hir::{codec, model};

#[allow(non_snake_case)]
pub fn ROOT() -> PathBuf {
    PathBuf::from(env!("LLRM_ROOT"))
}

/// `DIALECTS`: the QB-family `hir.Dialect` values.
pub const DIALECTS: [&str; 5] = ["qbasic11", "qb45", "pds71", "vbdos", "quickr"];
/// `RUNTIMES`: the QB-family `hir.RuntimeProfile` values.
pub const RUNTIMES: [&str; 4] = ["qb45", "pds71", "vbdos", "llrm"];
/// `ARRAY_ORDERS`: every `hir.ArrayOrder` value.
pub const ARRAY_ORDERS: [&str; 2] = ["column-major", "row-major"];

/// Source parsing or semantic analysis failed above HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendError(pub String);

impl fmt::Display for FrontendError {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FrontendError {}

/// How qbfront compiles a source: its options, as BC's switches name them.
#[derive(Clone, Debug)]
pub struct Frontend {
    pub dialect: String,
    pub runtime: String,
    pub array_order: String,
    pub huge_arrays: bool,
    /// What is stored to a SINGLE or DOUBLE variable is rounded to its type, as
    /// BC does: `-fexcess-precision=standard` or `-ffloat-store`.
    pub float_store: bool,
    pub checked_arrays: bool,
    /// Integer division raises error 11 in code:
    /// `-fsanitize=integer-divide-by-zero`.
    pub checked_division: bool,
    /// `-g`: each statement's source line.
    pub debug: bool,
    /// Integer arithmetic and narrowing raise error 6, Overflow:
    /// `-fsanitize=signed-integer-overflow`.
    pub checked_overflow: bool,
    /// The target's integer divide faults where the quotient overflows
    /// (`Target::idiv_overflow_traps`): INTEGER `\\` and MOD divide in 16 bits
    /// and leave `-32768 \\ -1` to the processor and the runtime's handler.
    pub idiv_overflow_traps: bool,
    /// Each procedure compares SP with the runtime's limit on entry:
    /// `-fsanitize=stack`.
    pub checked_stack: bool,
    pub mbf: bool,
    pub alternate_math: bool,
    /// Nothing outside the source calls its procedures: `--whole-program`.
    pub whole_program: bool,
    /// Lay out dynamic arrays read together in one allocation:
    /// `--array-merging`.
    pub array_merging: bool,
    /// Every procedure uses the runtime's frame entry and exit:
    /// `--runtime-frames`.
    pub runtime_frames: bool,
    /// Errors in a module handler report their BASIC line: `--error-lines`.
    pub error_lines: bool,
    /// `-g` in BC's CodeView layout, not standard CodeView 4: `--bc-codeview`.
    pub bc_codeview: bool,
    /// Built for the run-time module BRUN45, as BC without /O:
    /// `--run-time-module`.
    pub run_time_module: bool,
    /// The most bytes the target's data segment holds: its description's, set
    /// by the CLI that bound the target.
    pub segment_bytes: Option<usize>,
    /// The bytes of the target's near and far pointer, from its description (0:
    /// the 16-bit target's).
    pub near_bytes: usize,
    pub far_bytes: usize,
    pub includes: Vec<PathBuf>,
}

impl Frontend {
    /// `dialect` on `runtime`, column-major, every switch off.
    pub fn new(
        dialect: &str,
        runtime: &str,
    ) -> Self {
        Self {
            dialect: dialect.into(),
            runtime: runtime.into(),
            array_order: "column-major".into(),
            huge_arrays: false,
            float_store: false,
            checked_arrays: false,
            checked_division: false,
            debug: false,
            checked_overflow: false,
            idiv_overflow_traps: false,
            checked_stack: false,
            mbf: false,
            alternate_math: false,
            whole_program: false,
            array_merging: false,
            runtime_frames: false,
            error_lines: false,
            bc_codeview: false,
            run_time_module: false,
            segment_bytes: None,
            near_bytes: 0,
            far_bytes: 0,
            includes: Vec::new(),
        }
    }
}

fn _options(
    source: &Path,
    frontend: &Frontend,
) -> Result<crate::hir_driver::Args, FrontendError> {
    let Frontend { dialect, runtime, array_order, .. } = frontend;
    if !DIALECTS.contains(&dialect.as_str()) {
        return Err(FrontendError(format!("unknown QB dialect '{dialect}'")));
    }
    if !RUNTIMES.contains(&runtime.as_str()) {
        return Err(FrontendError(format!("unknown QB runtime '{runtime}'")));
    }
    if !ARRAY_ORDERS.contains(&array_order.as_str()) {
        return Err(FrontendError(format!("unknown QB array order '{array_order}'")));
    }
    let dialect =
        crate::Dialect::parse(dialect).ok_or_else(|| FrontendError(format!("unknown QB dialect '{dialect}'")))?;
    Ok(crate::hir_driver::Args {
        dialect,
        runtime: runtime.clone(),
        options: crate::semantic::Options {
            row_major: array_order == "row-major",
            huge_arrays: frontend.huge_arrays,
            float_store: frontend.float_store,
            checked_arrays: frontend.checked_arrays,
            checked_division: frontend.checked_division,
            checked_overflow: frontend.checked_overflow,
            idiv_overflow_traps: frontend.idiv_overflow_traps,
            mbf: frontend.mbf,
            alternate_math: frontend.alternate_math,
            whole_program: frontend.whole_program,
            array_merging: frontend.array_merging,
            runtime_frames: frontend.runtime_frames,
            error_lines: frontend.error_lines,
            segment_bytes: frontend.segment_bytes,
            near_bytes: frontend.near_bytes,
            far_bytes: frontend.far_bytes,
            bc_codeview: frontend.bc_codeview,
            run_time_module: frontend.run_time_module,
        },
        debug: frontend.debug,
        syntax: false,
        include_dirs: frontend.includes.clone(),
        dump_source: None,
        input: source.to_path_buf(),
    })
}

/// qbfront on `args`: its HIR text, or the refusal.
fn _run(args: crate::hir_driver::Args) -> Result<String, FrontendError> {
    let compiled = crate::hir_driver::compile(&args).map_err(FrontendError)?;
    // Warnings: the program still compiled.
    for warning in &compiled.warnings {
        eprintln!("{warning}");
    }
    Ok(compiled.text)
}

/// Run only source loading and parsing, independently of semantic HIR support.
pub fn syntax_checked(
    source: &Path,
    frontend: &Frontend,
) -> Result<(), FrontendError> {
    _run(crate::hir_driver::Args { syntax: true, .._options(source, frontend)? }).map(|_| ())
}

pub fn parsed(
    source: &Path,
    frontend: &Frontend,
    dump: Option<&Path>,
) -> Result<model::Program, FrontendError> {
    let stdout = _run(_options(source, frontend)?)?;
    if let Some(dump) = dump {
        if let Some(parent) = dump.parent() {
            std::fs::create_dir_all(parent).map_err(|error| FrontendError(error.to_string()))?;
        }
        std::fs::write(dump, &stdout).map_err(|error| FrontendError(error.to_string()))?;
    }
    let mut program = decoded(&stdout, frontend.checked_arrays, frontend.near_bytes)?;
    if frontend.checked_stack {
        let family = program.runtime.tables();
        program.stack_check = Some(
            llrm_codegen::abi::runtime::semantics::stack(family)
                .ok_or_else(|| FrontendError(format!("the {family} runtime states no stack limit")))?,
        );
    }
    Ok(program)
}

/// The program qbfront's HIR text `text` states, with what the runtime
/// adds: its entry, and what its routines promise. `checked` is the
/// program's `-fsanitize=bounds`.
pub fn decoded(
    text: &str,
    checked: bool,
    near_bytes: usize,
) -> Result<model::Program, FrontendError> {
    let mut program =
        codec::decode(text).map_err(|error| FrontendError(format!("qbfront produced invalid HIR: {error}")))?;
    let family = program.runtime.tables();
    for object_ in program.modules.iter_mut().flat_map(|module| &mut module.data) {
        if object_.linkage == model::DataLinkage::External
            && llrm_codegen::abi::runtime::named_only(&object_.name, family)
        {
            object_.addressed = false;
        }
    }
    // CLEAR zeroes the data segment the program's variables live in, and CHAIN
    // hands its COMMON segment to the next program: the optimiser must not
    // take what they reach for private.
    let calls = |routine: &str| {
        program
            .modules
            .iter()
            .flat_map(|module| &module.functions)
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|instruction| instruction.callee.as_deref() == Some(routine))
    };
    // Under the run-time module a chained-to program's COMMON arrives from the
    // program before it.
    let (clears, chains) = (calls("B$SCLR"), calls("B$SCHN"));
    for object_ in program.modules.iter_mut().flat_map(|module| &mut module.data) {
        let common = object_.segment.as_deref() == Some("COMMON");
        if object_.linkage == model::DataLinkage::Internal
            && !object_.readonly
            && object_.address == model::AddressKind::Near
            && (clears || ((chains || program.run_time_module) && common))
        {
            object_.exposed = true;
        }
    }
    // The runtime enters a module at its body, through the module header.
    program.entries = vec!["__main".to_owned()];
    // Where the program handles errors, any routine may run its handler.
    let handles =
        program.modules.iter().flat_map(|module| &module.functions).any(|function| function.error_handler.is_some());
    if !handles {
        program.promises = model::RuntimePromises::of(
            llrm_codegen::abi::runtime::ENTERS_USER_CODE.iter().copied(),
            llrm_codegen::abi::runtime::writers(family),
            [],
        );
    }
    // Where an error is handled the routine raises it, as the checks the
    // frontend writes do.
    program.promises.checked = checked || handles;
    program.promises.descriptor = llrm_codegen::abi::runtime::semantics::descriptor(
        program.runtime.value(),
        if near_bytes == 0 { 2 } else { near_bytes },
    );
    program.promises.routines = llrm_codegen::abi::runtime::semantics::routines();
    program.promises.nounwind = llrm_codegen::abi::runtime::CONTRACTS
        .iter()
        .filter(|(_, contract)| !contract.raises_error)
        .map(|(name, _)| name.clone())
        .collect();
    program.promises.no_retain =
        llrm_codegen::abi::runtime::captures_nothing().into_iter().map(str::to_owned).collect();
    program.promises.flags_results =
        llrm_codegen::abi::runtime::flags_results().into_iter().map(str::to_owned).collect();
    program.promises.no_return = llrm_codegen::abi::runtime::never_returning().into_iter().map(str::to_owned).collect();
    Ok(program)
}

/// The program's runtime calls made as the target calls any function it has not
/// marked: the target's own convention (`native_cc`, its description's `cc`),
/// the caller popping, the first argument the last pushed. The other runtimes
/// keep their stack ABI, which the Microsoft routines fix.
pub fn natively_called(
    program: &mut model::Program,
    native_cc: Option<&str>,
) {
    let named = native_cc.filter(|cc| *cc != "cdecl").map(str::to_owned);
    let hosted = program.promises.hosted;
    for function in program.modules.iter_mut().flat_map(|module| &mut module.functions) {
        let called: llrm_codegen::support::hash::HashMap<i64, String> = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|one| Some((one.id, one.callee.clone()?)))
            .collect();
        let native = |call: &model::CallAbi| {
            call.callee.is_none()
                && !called
                    .get(&call.instruction)
                    .is_some_and(|name| llrm_codegen::abi::runtime::semantics::keeps_stack_abi(name))
        };
        let natives: Vec<bool> = function.calls.iter().map(native).collect();
        // A host's stack-block routines are C varargs: they keep the order
        // BASIC pushes them in, and the caller removes them.
        if hosted {
            for call in function.calls.iter_mut().filter(|call| {
                call.callee.is_none()
                    && called
                        .get(&call.instruction)
                        .is_some_and(|name| llrm_codegen::abi::qb::HOST_STACK_BLOCKS.contains(&name.as_str()))
            }) {
                call.cleanup = model::StackCleanup::Caller;
                call.convention = named.clone();
            }
        }
        for call in function.calls.iter_mut().zip(natives).filter_map(|(call, native)| native.then_some(call)) {
            call.cleanup = model::StackCleanup::Caller;
            call.order.reverse();
            call.convention = named.clone();
        }
    }
}
