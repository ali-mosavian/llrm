//! Port of `qbopt/frontend/qb/compile.py`: QB HIR through the existing
//! machine pipeline to a fresh OMF module.
//!
//! This emission boundary is a procedure module: it emits far Pascal
//! SUB/FUNCTION bodies and their data inside the BASIC module envelope.

use llrm_core::backend::stackusage::stack_to_add;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use llrm_core::support::hash::IndexMap;

use super::abi::AbiError;
use llrm_core::driver::{self, basic::{self, written_basic}};
use llrm_core::backend::{masm, objbuild};
use llrm_core::hir::{self, model};
use llrm_core::objectfile::module::Space;
use llrm_core::support::pyrepr::{self, Repr};

/// HIR is valid but does not yet have a truthful BASIC object spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmissionError(pub String);

impl fmt::Display for EmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EmissionError {}

/// Every exception the Python pipeline lets escape, by its message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    Emission(EmissionError),
    Abi(AbiError),
    /// Any other `ValueError` (and its subclasses) with Python's text.
    Value(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Emission(one) => one.fmt(formatter),
            Self::Abi(one) => one.fmt(formatter),
            Self::Value(one) => formatter.write_str(one),
        }
    }
}

impl std::error::Error for CompileError {}

impl From<EmissionError> for CompileError {
    fn from(one: EmissionError) -> Self {
        Self::Emission(one)
    }
}

impl From<AbiError> for CompileError {
    fn from(one: AbiError) -> Self {
        Self::Abi(one)
    }
}

impl From<String> for CompileError {
    fn from(one: String) -> Self {
        Self::Value(one)
    }
}

fn emission<T>(message: impl Into<String>) -> Result<T, CompileError> {
    Err(CompileError::Emission(EmissionError(message.into())))
}

/// Sees the verified HIR the compiler is about to emit, for `--dump`.
pub type HirObserver<'o> = dyn FnMut(&model::Program) -> Result<(), String> + 'o;

const _READ_DATA_OBJECT: &str = "$qb$readData";
const _STATEMENT_TABLE_OBJECT: &str = "$qb$statementTable";

/// The linker spelling BC uses: uppercase and without a type suffix.
pub fn _object_name(name: &str) -> String {
    name.trim_end_matches(['%', '&', '!', '#', '$']).to_uppercase()
}

/// The name a procedure links by: the frontend's, or BC's default.
fn _link_name(callable: &model::Callable) -> String {
    callable.symbol.clone().unwrap_or_else(|| _object_name(&callable.name))
}

/// BC keeps source globals typed; compiler-owned data keeps `$D<n>`, and so
/// does a global whose name is `taken`: a scalar and an array may share one.
fn _data_name(module: &model::Module, object_: &model::DataObject, taken: &BTreeSet<String>) -> String {
    let name = object_.name.to_uppercase();
    if object_.linkage == model::DataLinkage::Internal
        && !object_.name.starts_with('$')
        && !object_.name.ends_with("$static")
        && !object_.name.ends_with("$descriptor")
        && !taken.contains(&name)
    {
        return name;
    }
    format!("{}$D{}", _object_name(&module.name), object_.id)
}

fn _bytes_of(values: &[i64]) -> Vec<u8> {
    values.iter().map(|one| *one as u8).collect()
}

type Names = IndexMap<(Space, i64), String>;

/// The data objects BASIC lays down in its segments: the module's own, less
/// the DATA stream and statement table the envelope writes.
fn _placed(module: &model::Module) -> impl Iterator<Item = &model::DataObject> {
    let reserved = [_READ_DATA_OBJECT, _STATEMENT_TABLE_OBJECT];
    module.data.iter().filter(move |one| one.linkage == model::DataLinkage::Internal && !reserved.contains(&one.name.as_str()))
}

/// Each data object's symbol, by its HIR id: its own name where external.
fn _data_names(module: &model::Module) -> Names {
    let mut names: Names = llrm_core::hir::symbols::symbol_names();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for object_ in &module.data {
        if [_READ_DATA_OBJECT, _STATEMENT_TABLE_OBJECT].contains(&object_.name.as_str()) {
            continue;
        }
        let (key, name) = if object_.linkage == model::DataLinkage::External {
            ((Space::External, object_.id), object_.name.clone())
        } else {
            ((Space::Segment, object_.id), _data_name(module, object_, &taken))
        };
        taken.insert(name.to_uppercase());
        names.insert(key, name);
    }
    names
}

/// The BASIC segment a data object goes in: far ones FSL_CONST, read-only
/// ones BC_CN, the rest BC_DATA.
fn _segment(object_: &model::DataObject) -> &'static str {
    if matches!(object_.address, model::AddressKind::Far | model::AddressKind::Huge) {
        "FSL_CONST"
    } else if object_.readonly {
        "BC_CN"
    } else {
        "BC_DATA"
    }
}

/// Decode the frontend-owned DATA statement stream.
///
/// QB45 `rt/read.asm` starts at MODULE_CODE.OF_DS (`BC_DS+2`).  Each
/// NUL-terminated source-text line is followed by a two-byte code address;
/// `B$ReadVal` skips that word at end of line.  The semantic frontend keeps
/// only the text lines in this reserved object.  The OMF adapter supplies a
/// relocated, valid code address before every line and the measured
/// `FFFF 01` out-of-DATA sentinel after the last one.
fn _read_data_lines(module: &model::Module) -> Result<Vec<Vec<u8>>, CompileError> {
    let objects: Vec<&model::DataObject> = module.data.iter().filter(|one| one.name == _READ_DATA_OBJECT).collect();
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    if objects.len() != 1 {
        return emission("a module may contain only one READ/DATA stream");
    }
    let object_ = objects[0];
    if object_.linkage != model::DataLinkage::Internal
        || !object_.readonly
        || !object_.relocations.is_empty()
        || object_.address != model::AddressKind::Near
    {
        return emission("the READ/DATA stream has an invalid storage contract");
    }
    let payload = _bytes_of(&object_.bytes);
    if payload.last() != Some(&0) {
        return emission("the READ/DATA stream must contain NUL-terminated lines");
    }
    let lines: Vec<Vec<u8>> = payload[..payload.len() - 1].split(|byte| *byte == 0).map(<[u8]>::to_vec).collect();
    if lines.iter().any(|line| line.is_empty() || !line.is_ascii()) {
        return emission("READ/DATA lines must be nonempty ASCII source text");
    }
    Ok(lines)
}

/// Serialize the DATA rows, each after the key QB45's B$RSTB search compares.
///
/// B$RSTB takes the first row whose key is at least its argument
/// (rt/read.asm, RSTB_10), so keys need only ascend. BC's are the code
/// offsets of labeled NOPs; `_labeled_data_keys` gives those.
fn _read_data_items(module: &model::Module, keys: Vec<masm::Datum>) -> Result<Vec<masm::Datum>, CompileError> {
    let lines = _read_data_lines(module)?;
    if keys.len() != lines.len() {
        return emission("DATA keys do not match the serialized DATA rows");
    }
    Ok(keys.into_iter().zip(lines).flat_map(|(key, line)| [key, masm::Datum::Bytes([line, vec![0]].concat())]).collect())
}

/// `program` with each DATA row keyed by its position in the table rather
/// than a code label: no DATA marker, RESTORE passing the row to B$RSTB,
/// and no DATA block a code entry.
pub(super) fn _positional_data(program: &model::Program) -> Result<model::Program, CompileError> {
    let mut program = program.clone();
    for function in program.modules.iter_mut().flat_map(|module| &mut module.functions) {
        let mut rows = BTreeSet::new();
        for block in &mut function.blocks {
            let before = block.instructions.len();
            block.instructions.retain(|one| !one.callee.as_deref().is_some_and(|callee| callee.starts_with("$QB$DATA:")));
            if block.instructions.len() != before {
                rows.insert(block.id);
            }
            for instruction in &mut block.instructions {
                let Some(callee) = instruction.callee.as_deref().filter(|callee| callee.starts_with("$QB$RSTB:")) else { continue };
                let row = _parsed_target(callee, "$QB$RSTB:", "invalid RESTORE marker")?;
                let [model::Operand::Constant(key)] = instruction.operands.as_mut_slice() else {
                    return emission("RESTORE label lost its typed placeholder");
                };
                key.value = model::Number::Int(row);
                instruction.callee = Some("B$RSTB".to_owned());
            }
        }
        function.external_entries.retain(|entry| !rows.contains(entry));
        // A DATA marker's call goes with it, and so does its ABI.
        let kept: BTreeSet<i64> = function.blocks.iter().flat_map(|block| &block.instructions).map(|one| one.id).collect();
        function.calls.retain(|call| kept.contains(&call.instruction));
    }
    Ok(program)
}

/// `struct.pack_into("<H", buffer, at, value)`.
fn pack_into(buffer: &mut [u8], at: usize, value: i64) {
    buffer[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

/// Return the measured BC `U_FLAG` word for this compilation profile.
///
/// `U_FLAG` is consumed by the BASIC runtime, not descriptive metadata.
/// These are the exact optimized /FPi profiles emitted by each compatible
/// compiler; VBDOS additionally records its /R array layout choice.
fn _compiler_switches(program: &model::Program) -> Result<i64, CompileError> {
    if program.float_mode == model::FloatMode::Alternate {
        if program.runtime != model::RuntimeProfile::Pds71 {
            return emission("alternate floating-point runtime is only measured for PDS 7.1");
        }
        return Ok(0x1088); // BC /O /FPa /G2
    }
    let mut flags = match program.runtime {
        model::RuntimeProfile::Qb45 => 0x1080,  // BC /O /FPi
        model::RuntimeProfile::Pds71 => 0x1084, // BC /O /FPi /G2
        model::RuntimeProfile::Vbdos => 0x12C4, // BC /O /FPi /G3 /E
        model::RuntimeProfile::Freestanding => {
            return Err(CompileError::Value(format!("KeyError: {}", program.runtime.repr())));
        }
    };
    if program.runtime == model::RuntimeProfile::Vbdos && program.array_order == model::ArrayOrder::RowMajor {
        flags |= 0x0100; // /R
    }
    Ok(flags)
}

fn _header(program: &model::Program) -> Result<Vec<u8>, CompileError> {
    let module = &program.modules[0];
    let object_name = _object_name(&module.name);
    if !object_name.is_ascii() {
        return Err(CompileError::Value(format!("'ascii' codec can't encode {}", pyrepr::string(&object_name))));
    }
    let mut name = object_name.as_bytes()[..object_name.len().min(8)].to_vec();
    name.resize(8, b' ');
    // MODULE_CODE in runtime/inc/addr.inc. Every symbolic word is an offset,
    // framed through DGROUP by the shared writer.
    let mut out: Vec<u8> = [b"bl".to_vec(), name, vec![0; 34], vec![0xff, 0xff]].concat();
    out.extend((_compiler_switches(program)? as u16).to_le_bytes());
    if out.len() != 48 {
        return Err(CompileError::Value("MODULE_CODE is exactly O_ENT bytes".into()));
    }
    // The addends live in the image. The remaining words are zero.
    pack_into(&mut out, 12, 2); // OF_DS is BC_DS + 2.
    Ok(out)
}

/// Under `Frames::Own` a procedure frames itself where the runtime needs no
/// frame of its own (`Module::frames_itself`). The runtime's stack check goes
/// with it.
pub(super) fn _inline_frame(program: &model::Program, module: &model::Module, function: &model::Function) -> bool {
    module.frames_itself(program.frames, function)
}

/// A module-internal procedure that frames itself is called near, as the C
/// frontend does for a near function: no other module can call it, and every
/// caller shares its code segment. One on the runtime's frame stays far,
/// whose chain is only known to hold far returns.
fn _near_procedures(program: &model::Program) -> model::Program {
    let mut program = program.clone();
    for index in 0..program.modules.len() {
        let original = program.modules[index].clone();
        let near: BTreeSet<&str> = original
            .functions
            .iter()
            .filter(|function| {
                function.name != "__main"
                    && function.linkage == model::FunctionLinkage::Internal
                    && _inline_frame(&program, &original, function)
            })
            .map(|function| function.name.as_str())
            .collect();
        let callables: BTreeSet<i64> =
            original.callables.iter().filter(|one| near.contains(one.name.as_str())).map(|one| one.id).collect();
        for function in &mut program.modules[index].functions {
            if near.contains(function.name.as_str()) {
                if let Some(abi) = function.abi.as_mut() {
                    abi.distance = model::CallDistance::Near;
                }
            }
            for call in &mut function.calls {
                if call.callee.is_some_and(|callee| callables.contains(&callee)) {
                    call.distance = model::CallDistance::Near;
                }
            }
        }
    }
    program
}

fn _parsed_target(name: &str, prefix: &str, message: &str) -> Result<i64, CompileError> {
    name.strip_prefix(prefix)
        .unwrap_or(name)
        .trim()
        .parse::<i64>()
        .or_else(|_| emission(format!("{message} {}", pyrepr::string(name))))
}

#[allow(non_snake_case)]
fn _SCREEN_DRIVER(mode: i64) -> Option<&'static str> {
    Some(match mode {
        1 | 2 => "B$CGAUSED",
        3 => "B$HRCUSED",
        4 => "B$OLIUSED",
        7..=10 => "B$EGAUSED",
        11..=13 => "B$VGAUSED",
        _ => return None,
    })
}

/// Name the runtime graphics modules selected by source SCREEN calls.
///
/// Microsoft BC emits a reference to a mode-specific public for a constant
/// mode and B$GRPUSED for an expression.  The reference is a linker switch,
/// not a call: without it B$CSCN is present but has no device implementation
/// and reports BASIC error 5.
fn _graphics_dependencies(module: &model::Module) -> BTreeSet<String> {
    let mut required: BTreeSet<String> = BTreeSet::new();
    for function in &module.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if instruction.op != model::Op::Call || instruction.callee.as_deref() != Some("B$CSCN") {
                    continue;
                }
                let model::Operand::Constant(mode) = &instruction.operands[1] else {
                    required.insert("B$GRPUSED".to_owned());
                    continue;
                };
                let number = match mode.value {
                    model::Number::Int(one) => one,
                    model::Number::Float(one) => one.trunc() as i64,
                };
                if number != 0 {
                    required.insert(_SCREEN_DRIVER(number).unwrap_or("B$GRPUSED").to_owned());
                }
            }
        }
    }
    required
}

/// The rich route: the HIR emitted as MIR, optimized, and assembled by the
/// driver into the BASIC module object laid out here, each data object as
/// this names it and lays it down.
fn rich_assembled(program: &model::Program, codegen: &driver::Options) -> Result<masm::Module, CompileError> {
    let program = &llrm_core::support::debug::timed("hir positional data", || _positional_data(program))?;
    let module = &program.modules[0];
    let functions = module.functions.iter().map(|one| (one.name.clone(), _object_name(&one.name)));
    let symbols: BTreeMap<String, String> = functions.chain(module.callables.iter().map(|one| (one.name.clone(), _link_name(one)))).collect();
    let data = _data_names(module).into_iter().filter(|((space, _), _)| matches!(space, Space::Segment | Space::External)).map(|((_, id), name)| (id, name)).collect();
    let mut placed: IndexMap<&str, Vec<basic::Item>> = ["BC_DATA", "BC_CN", "FSL_CONST"].into_iter().map(|name| (name, Vec::new())).collect();
    for object_ in _placed(module) {
        placed[_segment(object_)].push(basic::Item::Object(object_.id));
    }
    let frames = module
        .functions
        .iter()
        .map(|function| {
            let frame = if _inline_frame(program, module, function) { basic::Frame::Own } else { basic::Frame::Runtime { strings: module.local_strings(function) } };
            (function.name.clone(), frame)
        })
        .collect();
    let vbdos = program.runtime == model::RuntimeProfile::Vbdos;
    if !vbdos && !placed["FSL_CONST"].is_empty() {
        return emission(format!("{} cannot place literals in VBDOS FSL_CONST", program.runtime.value()));
    }
    let rows = (0.._read_data_lines(module)?.len() as u16).map(|row| masm::Datum::Bytes(row.to_le_bytes().to_vec())).collect();
    let read_data = _read_data_items(module, rows)?;
    let graphics = _graphics_dependencies(module);
    let datum = basic::Item::Datum;
    let label = |name: &str| datum(masm::Datum::Label(masm::Label { name: name.to_owned() }));
    // The object's pointer cells are the target's: a far pointer's width and a near one's.
    let (near_bytes, far_bytes) = {
        let layout = codegen.arch.layout();
        let datalayout = llrm_mir::datalayout::DataLayout::parse(&layout.datalayout).map_err(CompileError::from)?;
        (datalayout.pointer(layout.spaces.near).bits / 8, datalayout.pointer(layout.spaces.far).bits / 8)
    };
    let mut segments: Vec<(&str, Vec<basic::Item>)> = vec![
        ("BR_DATA", vec![]),
        ("BR_SKYS", vec![]),
        ("COMMON", vec![]),
        ("BC_DATA", [vec![datum(masm::Datum::Bytes(vec![0; 6]))], placed.swap_remove("BC_DATA").unwrap_or_default()].concat()),
        ("NMALLOC", vec![]),
        ("ENMALLOC", vec![]),
        ("BC_FT", vec![]),
        ("BC_CN", placed.swap_remove("BC_CN").unwrap_or_default()),
        ("BC_DS", read_data.into_iter().chain([masm::Datum::Bytes(vec![0xff, 0xff, 0x01])]).map(datum).collect()),
        ("BC_SAB", vec![label("$QB$SAB")]),
        ("BC_SA", vec![label("$QB$SA"), datum(masm::Datum::Pointer(masm::Pointer { name: basic::HEADER.into(), offset: 0, far: true, bytes: far_bytes }))]),
    ];
    let mut private: BTreeSet<String> = BTreeSet::new();
    if vbdos {
        segments.push(("FDATA", vec![]));
        segments.push(("FSL_CONST", placed.swap_remove("FSL_CONST").unwrap_or_default()));
        private.extend(["FDATA".to_owned(), "FSL_CONST".to_owned()]);
    }
    if vbdos && !graphics.is_empty() {
        segments.push(("QB_LINK", graphics.iter().map(|name| datum(masm::Datum::Pointer(masm::Pointer { name: name.clone(), offset: 0, far: false, bytes: near_bytes }))).collect()));
        private.insert("QB_LINK".into());
    }
    let object = basic::Object {
        code: format!("{}_CODE", _object_name(&module.name)),
        header: _header(program)?,
        main: "__main".to_owned(),
        symbols,
        data,
        segments: segments.into_iter().map(|(name, items)| basic::Segment { name: name.to_owned(), items, size: None }).collect(),
        constants: "BC_CN".to_owned(),
        private,
        requests: graphics,
        frames,
        line_numbers: module.line_numbers.iter().copied().collect(),
        stack_check: program.stack_check.clone(),
    };
    let mut compiled = basic::compiled(program, &object, codegen)?;
    compiled.stack = stack_to_add(&compiled, STACK_BASE, STACK_RESERVE, llrm_core::backend::stackusage::stack_limit(codegen.arch.layout().segment_bytes()), &*codegen.arch)?;
    Ok(compiled)
}

/// The stack the BASIC runtime's crt0 links (`inc/stack2.inc`, STACK_SIZE), as the
/// link maps of BC's and llrm-qb's objects both show; the object's own adds to it.
pub const STACK_BASE: i64 = 0x800;

/// What the BASIC runtime's routines, DOS and an interrupt use below the deepest chain of frames.
pub const STACK_RESERVE: i64 = 512;

/// Compile one QB HIR module to the shared assembly model.
pub fn assembled(
    program: &model::Program,
    observer: Option<&mut HirObserver<'_>>,
    codegen: &driver::Options,
) -> Result<masm::Module, CompileError> {
    llrm_core::support::debug::timed("hir verify", || hir::verify::verify(program)).map_err(|error| CompileError::Value(error.0))?;
    if let Some(observe) = observer {
        observe(program)?;
    }
    let laid_out = llrm_core::support::debug::timed("hir zero fill", || super::zero_fill::laid_out(program, |module, function| !_inline_frame(program, module, function)));
    let program = &llrm_core::support::debug::timed("hir near procedures", || _near_procedures(&laid_out));
    if program.modules.len() != 1 {
        return emission("one OMF object represents exactly one QB module");
    }
    rich_assembled(program, codegen)
}

/// Emit a complete fresh BASIC-envelope OMF object.
pub fn object_bytes(
    program: &model::Program,
    source: &Path,
    observer: Option<&mut HirObserver<'_>>,
    codegen: &driver::Options,
) -> Result<Vec<u8>, CompileError> {
    let module = objbuild::live(&assembled(program, observer, codegen)?)
        .map_err(|error| CompileError::Value(error.to_string()))?;
    let name = source.file_name().map_or_else(String::new, |one| one.to_string_lossy().into_owned());
    Ok(written_basic(&module, _header(program)?, &name)?)
}
