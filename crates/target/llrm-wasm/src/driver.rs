//! LLVM IR to wasm assembly text: `llc -O0 -fast-isel=false`'s job for the
//! functions of a module, one after another.

use crate::Width;
use crate::dag::{ModuleFacts, Options, compile_function_with};
use crate::print;

/// The width `-mtriple=` names.
pub fn width_of(triple: &str) -> Result<Width, String> {
    match triple {
        "wasm32" | "wasm32-unknown-unknown" => Ok(Width::W32),
        "wasm64" | "wasm64-unknown-unknown" => Ok(Width::W64),
        _ => Err(format!("unknown wasm triple `{triple}`; choose wasm32 or wasm64")),
    }
}

/// The pin clang's datalayout for `--target=wasm32` and `wasm64`, from the
/// machine description.
pub fn datalayout(width: Width) -> String {
    crate::layout(width).datalayout
}

/// `text`, LLVM IR, as a module: the lines and words MIR's reader leaves out of
/// LLVM dropped.
/// How far the mid-end goes: gcc's and clang's `-O` levels, as `llc` takes
/// them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    O0,
    O1,
    O2,
    O3,
    /// -Omax: `Options::aggressive()`.
    Omax,
    Os,
    Oz,
}

impl Level {
    /// The level `-O<text>` names.
    pub fn parse(text: &str) -> Result<Self, String> {
        use llrm_transforms::pipeline::LevelName as Name;
        Ok(match Name::parse(text)? {
            Name::O0 => Self::O0,
            Name::O1 => Self::O1,
            Name::O2 => Self::O2,
            Name::O3 => Self::O3,
            Name::Omax => Self::Omax,
            Name::Os => Self::Os,
            Name::Oz => Self::Oz,
        })
    }

    fn mid_end(self) -> llrm_transforms::pipeline::Options {
        use llrm_transforms::pipeline::Options as MidEnd;
        // `SelectionDAGBuilder::visitSwitch` lowers the switches (`br_table`);
        // the mid-end's `switchlower` would leave isel a compare tree.
        let options = match self {
            Self::O0 => MidEnd::none(),
            Self::O1 => MidEnd::basic(),
            Self::O2 => MidEnd::default(),
            Self::O3 => MidEnd::speed(),
            Self::Omax => MidEnd::aggressive(),
            Self::Os => MidEnd::size(),
            Self::Oz => MidEnd::min_size(),
        };
        // The DAG's `mergeConsecutiveStores` widens the stores, as in the pin.
        // The pin's -O1 has no GVN: a global's store is not forwarded across
        // blocks.
        MidEnd { lower_switches: false, merge_stores: false, write_through: false, ..options }
    }
}

/// The `-f` switches of the command line (`-fno-inline=NAME`, `-fexcess-precision=`), applied to every mid-end run of the process: the
/// same parser as every other backend's `llc`.
static POLICY: std::sync::Mutex<llrm_codegen::driver::policy::Policy> =
    std::sync::Mutex::new(llrm_codegen::driver::policy::Policy::new());

/// Records `flag` if the driver passes it on to a backend; false when it is not one.
pub fn pass_flag(flag: &str) -> bool {
    POLICY.lock().unwrap().take(flag)
}

/// `module` through the shared mid-end at `level`.
fn optimized(
    module: llrm_mir::module::Module,
    level: Level,
) -> Result<llrm_mir::module::Module, String> {
    use llrm_transforms::pipeline::{Applied, Options as MidEnd};
    let machine: std::rc::Rc<dyn llrm_mir::target::Machine> = std::rc::Rc::new(crate::machine::WasmMachine);
    let mut program = llrm_mir::program::Program::new(vec![module], machine)?;
    // `LLRM_WASM_BACKEND_ONLY=1`: no mid-end, so an IR that already went
    // through it (`LLRM_WASM_IR`) is compared with the pin's `llc` as it
    // stands. A second run is no fixed point (it inlined `gcd` in recgcd
    // -O1).
    let mut options = if std::env::var_os("LLRM_WASM_BACKEND_ONLY").is_some() {
        MidEnd::none()
    } else {
        MidEnd { library: c_library(), ..level.mid_end() }
    };
    {
        let policy = POLICY.lock().unwrap();
        policy.mark(&mut program)?;
        policy.apply(&mut options);
    }
    // `LLRM_WASM_DUMP=DIR`: every mid-end step that changed a body, written to
    // DIR.
    let dump = std::env::var_os("LLRM_WASM_DUMP").map(std::path::PathBuf::from);
    llrm_transforms::pipeline::applied(&mut program, &Applied { options, dump, ..Applied::default() })?;
    program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)?;
    let module = program.modules.remove(0);
    // `LLRM_WASM_IR=1`: the module the backend gets, on standard error.
    if std::env::var_os("LLRM_WASM_IR").is_some() {
        eprintln!("{}", llrm_mir::print::module(&module));
    }
    Ok(module)
}

pub fn module(
    text: &str,
    width: Width,
) -> Result<llrm_mir::module::Module, String> {
    let mut module = parsed(text, width)?;
    // No `atomics` feature: the pin's `LowerAtomicPass` makes the accesses
    // plain.
    llrm_mir::atomic::lowered_single_thread(&mut module)?;
    Ok(module)
}

/// `text` as MIR, its layout checked against wasm's.
fn parsed(
    text: &str,
    width: Width,
) -> Result<llrm_mir::module::Module, String> {
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with("target triple") || line.starts_with(';') {
            continue;
        }
        kept.push_str(&line.replace(" dso_local", ""));
        kept.push('\n');
    }
    let mut module = llrm_mir::parse::module(&kept).map_err(|error| error.to_string())?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    // `llc` takes the target's layout when the module states none.
    match module.datalayout.as_deref() {
        Some(stated) if *stated != datalayout(width) => {
            return Err(format!("the module's datalayout `{stated}` is not wasm's `{}`", datalayout(width)));
        }
        Some(_) => {}
        None => module.datalayout = Some(datalayout(width)),
    }
    Ok(module)
}

/// [`module`], with a landing pad that branches back into its function made
/// enterable from a `catch` when the module unwinds.
fn prepared(
    text: &str,
    width: Width,
    options: Options,
) -> Result<llrm_mir::module::Module, String> {
    let mut module = parsed(text, width)?;
    if options.level != Level::O0 {
        module = optimized(module, options.level)?;
    }
    // `PreISelIntrinsicLowering` runs `LowerConstantIntrinsics` at every level; at -O0 the mid-end has not.
    llrm_transforms::lowerconstantintrinsics::module_lowered(&mut module)?;
    llrm_mir::atomic::lowered_single_thread(&mut module)?;
    crate::fix_function_bitcasts::fix(&mut module)?;
    if options.exception_model == crate::dag::ExceptionModel::Wasm {
        crate::eh_reentry::reentry(&mut module)?;
    }
    Ok(module)
}

/// Each defined function of `text`: its label and instructions.
pub fn assembly(
    text: &str,
    width: Width,
    options: Options,
) -> Result<String, String> {
    let module = prepared(text, width, options)?;
    let mut out = String::new();
    let mut labels = print::Labels::default();
    let facts = ModuleFacts::of(&module);
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        let f =
            compile_function_with(&module, &name, width, options, &facts).map_err(|why| format!("@{name}: {why}"))?;
        out.push_str(&print::function_in(&f, &mut labels));
    }
    Ok(out)
}

/// The relocatable object of each defined function of `text`: `llc
/// -filetype=obj`.
pub fn object(
    text: &str,
    width: Width,
    options: Options,
) -> Result<Vec<u8>, String> {
    object_with_debug(text, width, options, &[])
}

/// [`object`] with custom debug sections and their relocations.
pub fn object_with_debug(
    text: &str,
    width: Width,
    options: Options,
    debug: &[crate::object::DebugSection],
) -> Result<Vec<u8>, String> {
    let module = prepared(text, width, options)?;
    object_of(&module, width, options, |_| Ok(debug.to_vec()))
}

/// [`object`] with DWARF (`-g`) from the module's `!DI*` nodes; `source` names
/// the unit where the module names none.
pub fn object_g(
    text: &str,
    width: Width,
    options: Options,
    source: &str,
) -> Result<Vec<u8>, String> {
    let mut module = prepared(text, width, options)?;
    llrm_mir::dbgimport::types_and_globals(&mut module);
    object_of(&module, width, options, |functions| crate::debug::sections(&module, width, functions, source))
}

fn object_of(
    module: &llrm_mir::module::Module,
    width: Width,
    options: Options,
    debug: impl FnOnce(&[crate::object::Encoded]) -> Result<Vec<crate::object::DebugSection>, String>,
) -> Result<Vec<u8>, String> {
    let mut functions = Vec::new();
    // `EmitTargetFeatures` reads the subtarget of the last function compiled.
    let mut features = crate::features::Features::default();
    let facts = ModuleFacts::of(module);
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        features = crate::features::Features::of(function).map_err(|why| format!("@{name}: {why}"))?;
        let f =
            compile_function_with(module, &name, width, options, &facts).map_err(|why| format!("@{name}: {why}"))?;
        functions.push(crate::object::encode_function(&f));
    }
    if options.exception_model == crate::dag::ExceptionModel::Wasm {
        features = features.with("exception-handling", true)?;
    }
    let debug = debug(&functions)?;
    Ok(crate::object::write_with_debug(&functions, &data(module, width)?, features, &debug))
}

/// The assembler's name for `global`: `.L` ahead of a private one (`Mangler`
/// with the private prefix `.L`).
pub fn symbol_name(global: &llrm_mir::module::GlobalValue) -> String {
    let name = global.name.as_deref().unwrap_or("");
    match global.linkage {
        llrm_mir::module::Linkage::Private => format!(".L{name}"),
        _ => name.to_string(),
    }
}

/// The variables of `module`: the defined ones as data segments, in module
/// order, and the names of the declared ones.
fn data(
    module: &llrm_mir::module::Module,
    width: Width,
) -> Result<crate::object::Data, String> {
    use llrm_codegen::backend::{globals, masm::Datum};
    use llrm_mir::module::{GlobalKind, Linkage, UnnamedAddr};

    use crate::object::{Data, DataKind, DataObject, DataReloc};

    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().ok_or("no datalayout")?)?;
    let mut names = llrm_codegen::support::hash::IndexMap::default();
    for (index, global) in module.globals.iter().enumerate() {
        let id = llrm_mir::GlobalId(index as u32);
        names.insert((globals::space(module, id), i64::from(id.0)), symbol_name(global));
    }
    let mut out = Data::default();
    for alias in &module.aliases {
        let target = module.global(alias.aliasee);
        if target.function().is_none_or(|one| one.is_declaration()) {
            return Err(format!("@{} aliases something that is no defined function", alias.name));
        }
        out.aliases.push(crate::object::Alias {
            name: alias.name.clone(),
            target: symbol_name(target),
            binding: crate::mir::Binding::of(alias.linkage),
            hidden: alias.visibility == llrm_mir::Visibility::Hidden,
        });
    }
    for (index, global) in module.globals.iter().enumerate() {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let id = llrm_mir::GlobalId(index as u32);
        let name = symbol_name(global);
        let Some(initializer) = variable.initializer else {
            out.undefined.push(name);
            continue;
        };
        let constructors = name == "llvm.global_ctors" && global.linkage == Linkage::Appending;
        if !constructors
            && !matches!(
                global.linkage,
                Linkage::External
                    | Linkage::Internal
                    | Linkage::Private
                    | Linkage::Weak
                    | Linkage::WeakOdr
                    | Linkage::LinkOnce
                    | Linkage::LinkOnceOdr
            )
        {
            return Err(format!("@{name}: {:?} linkage is not supported yet", global.linkage));
        }
        let mut bytes = Vec::new();
        let mut relocs = Vec::new();
        for datum in globals::datums(module, id, &names)? {
            match datum {
                Datum::Label(_) => {}
                Datum::Bytes(more) => bytes.extend(more),
                Datum::Align(align) => bytes.resize(bytes.len().next_multiple_of(align.to as usize), 0),
                Datum::Fill(fill) => bytes.resize(bytes.len() + fill.size as usize, fill.byte.unwrap_or(0)),
                Datum::Pointer(pointer) if !pointer.far && pointer.bytes == 4 => {
                    relocs.push(DataReloc { offset: bytes.len(), symbol: pointer.name, addend: pointer.offset });
                    bytes.extend([0; 4]);
                }
                other => return Err(format!("@{name}: {other:?} in an initializer is not supported yet")),
            }
        }
        for reloc in &relocs {
            let declared = out.functions.iter().any(|(one, ..)| *one == reloc.symbol);
            if !declared && module.named(&reloc.symbol).is_some_and(|id| module.global(id).function().is_some()) {
                let (params, results) = crate::dag::signature(module, &reloc.symbol, width)?;
                out.functions.push((reloc.symbol.clone(), params, results));
            }
        }
        if constructors {
            // The shared reader's order and its drop of an entry without a
            // function.
            out.init_funcs = globals::constructor_entries(module, &layout, initializer)?
                .into_iter()
                .map(|(priority, function, _)| (priority as u16, symbol_name(module.global(function))))
                .collect();
            continue;
        }
        let align = globals::alignment(module, Some(&layout), variable, false, false).unwrap_or(1);
        let kind = match (variable.constant, bytes.iter().all(|&b| b == 0) && relocs.is_empty()) {
            (false, true) => DataKind::Bss,
            (true, _) => DataKind::Rodata,
            _ => DataKind::Data,
        };
        // `getKindForGlobal`: a constant whose address does not matter, and
        // that is one C string, merges.
        let is_string = matches!(
            module.context.get(initializer).kind,
            llrm_mir::ConstantKind::Bytes(_)
        )
            && bytes.last() == Some(&0)
            && !bytes[..bytes.len() - 1].contains(&0);
        out.objects.push(DataObject {
            name,
            kind,
            align: align.trailing_zeros(),
            bytes,
            binding: crate::mir::Binding::of(global.linkage),
            hidden: global.visibility == llrm_mir::Visibility::Hidden,
            strings: variable.constant && global.unnamed_addr == UnnamedAddr::Global && is_string,
            relocs,
        });
    }
    Ok(out)
}

/// The compare routines of the platform's C library, by their IR names:
/// what `SimplifyLibCalls` may form.
fn c_library() -> Vec<llrm_mir::libfunc::Routine> {
    llrm_mir::libfunc::Routine::stated(llrm_mir::libfunc::HOSTED_COMPARE, str::to_owned, 0)
}
