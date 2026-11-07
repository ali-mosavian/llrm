//! LLVM IR to an arm64 object: `llc`'s job over `opt`'s. Clang's front end
//! alone makes the IR (`-O2 -Xclang -disable-llvm-passes`); the shared mid-end
//! pipeline optimizes it for the machine; the arm64 backend selects, allocates
//! and encodes.

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_transforms::pipeline::{Applied, Options};

use crate::machine::{Arm64Machine, Platform};

/// `text`, LLVM IR, without the lines and words MIR's reader leaves out of
/// LLVM (`target triple`, comments, `dso_local`), and the triple it stated.
pub fn without_triple(text: &str) -> (Option<String>, String) {
    let mut triple = None;
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("target triple") {
            triple = rest.split('"').nth(1).map(str::to_owned);
            continue;
        }
        if line.starts_with(';') {
            continue;
        }
        // DEVIATE 4: newer clang's `ptrtoaddr` is read as `ptrtoint` (the same
        // where a pointer and its address are one width); MIR refuses the
        // instruction on purpose.
        kept.push_str(&line.replace(" dso_local", "").replace(" = ptrtoaddr ", " = ptrtoint "));
        kept.push('\n');
    }
    (triple, kept)
}

/// The platform whose triple `triple` is.
pub fn platform_of(triple: &str) -> Option<Platform> {
    if triple.starts_with("arm64-apple-") || triple.starts_with("aarch64-apple-") {
        Some(Platform::Darwin)
    } else if triple.starts_with("aarch64-") && triple.contains("linux") {
        Some(Platform::Linux)
    } else {
        None
    }
}

/// The platform of an AArch64 `triple` that names none (`aarch64--`): Linux,
/// the pin's default; `None` for another architecture.
pub fn platform_or_default(triple: &str) -> Option<Platform> {
    platform_of(triple).or_else(|| matches!(triple.split('-').next()?, "aarch64" | "arm64").then_some(Platform::Linux))
}

/// `text` read as a program for `platform`, its datalayout the platform's.
pub fn program(
    text: &str,
    platform: Platform,
) -> Result<Program, String> {
    let (_, text) = without_triple(text);
    let mut module = llrm_mir::parse::module(&text).map_err(|error| error.to_string())?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    // Atomics are calls the mid-end moves nothing across; the selector makes
    // them instructions.
    llrm_mir::atomic::hidden(&mut module)?;
    match module.datalayout.as_deref() {
        Some(stated) if stated == platform.datalayout() => {}
        Some(stated) => {
            return Err(format!(
                "the module's datalayout `{stated}` is not {:?}'s `{}`",
                platform,
                platform.datalayout()
            ));
        }
        None => {
            return Err(format!("the module states no datalayout; {:?}'s is `{}`", platform, platform.datalayout()));
        }
    }
    Program::new(vec![module], Rc::new(Arm64Machine { platform }))
}

/// How far the mid-end goes: gcc's and clang's `-O` levels, as `llc` takes
/// them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    O0,
    O1,
    O2,
    O3,
    Os,
    Oz,
}

impl Level {
    /// The level `-O<text>` names.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(match text {
            "0" => Self::O0,
            "" | "1" => Self::O1,
            "2" => Self::O2,
            "3" => Self::O3,
            "s" => Self::Os,
            "z" => Self::Oz,
            _ => return Err(format!("unknown optimization level -O{text}; choose -O0, -O1, -O2, -O3, -Os or -Oz")),
        })
    }

    fn options(self) -> Options {
        // The pin's AArch64 pass config adds the atomic tidy at every level but
        // -O0.
        Options { atomic_cfg_tidy: true, lower_switches: false, ..self.generic_options() }
    }

    fn generic_options(self) -> Options {
        match self {
            Self::O0 => Options::none(),
            Self::O1 => Options::basic(),
            Self::O2 => Options::default(),
            Self::O3 => Options::speed(),
            Self::Os => Options::size(),
            Self::Oz => Options::min_size(),
        }
    }
}

/// `true` is -O2 and `false` no mid-end, as the callers that predate levels
/// pass it.
impl From<bool> for Level {
    fn from(optimize: bool) -> Self {
        if optimize { Self::O2 } else { Self::O0 }
    }
}

/// `program` through the shared mid-end at -O2.
pub fn optimized(program: &mut Program) -> Result<(), String> {
    optimized_at(program, Level::O2)
}

/// The `-f[no-]PASS` switches of the command line (`-fno-unroll-loops`),
/// applied to every mid-end run of the process.
static PASS_FLAGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Records `flag` if it names a mid-end pass; false when it does not.
pub fn pass_flag(flag: &str) -> bool {
    if !llrm_codegen::driver::flags::apply_pass_flag(&mut Options::default(), flag) {
        return false;
    }
    PASS_FLAGS.lock().unwrap().push(flag.to_owned());
    true
}

/// `program` through the shared mid-end at `level`.
pub fn optimized_at(
    program: &mut Program,
    level: Level,
) -> Result<(), String> {
    let dump = std::env::var_os("LLRM_MIR_STAGES").filter(|one| !one.is_empty()).map(Into::into);
    let mut options = level.options();
    for flag in PASS_FLAGS.lock().unwrap().iter() {
        llrm_codegen::driver::flags::apply_pass_flag(&mut options, flag);
    }
    options.library = c_library();
    llrm_transforms::pipeline::applied(program, &Applied { dump, options, ..Applied::default() })?;
    // The pin's PreISelIntrinsicLowering: the selector never sees the
    // intrinsic.
    program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)?;
    // `addIRPasses`: GlobalMerge, after the mid-end, before selection.
    if let Some(merge) = program.target.global_merge(level == Level::O3) {
        for module in &mut program.modules {
            llrm_transforms::globalmerge::merged(module, &program.layout, &merge)?;
        }
    }
    Ok(())
}

/// The routines the C link of this target resolves, by their IR names (the
/// object writer decorates them): what `SimplifyLibCalls` may form.
fn c_library() -> Vec<llrm_mir::libfunc::Routine> {
    let names = ["puts", "putchar", "fwrite", "fputc", "fputs"].into_iter().chain(llrm_mir::libfunc::HOSTED_COMPARE);
    llrm_mir::libfunc::Routine::stated(names, str::to_owned, 0)
}

/// Which instruction selector makes the machine function (`--isel=`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IselMode {
    Hand,
    Dag,
    /// The DAG path, the hand selector where it refuses (counted on stderr).
    #[default]
    Shadow,
}

impl IselMode {
    /// The mode `--isel=NAME` names.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "hand" => Some(Self::Hand),
            "dag" => Some(Self::Dag),
            "shadow" => Some(Self::Shadow),
            _ => None,
        }
    }
}

static ISEL_MODE: std::sync::Mutex<IselMode> = std::sync::Mutex::new(IselMode::Shadow);

/// Chooses the selector for the compiles that follow (the process's, as the
/// pass flags are).
pub fn set_isel_mode(mode: IselMode) {
    *ISEL_MODE.lock().unwrap() = mode;
}

fn isel_mode() -> IselMode {
    *ISEL_MODE.lock().unwrap()
}

/// `name` through the DAG path: the pin's table and the shared stages.
fn dag_select(
    module: &llrm_mir::module::Module,
    name: &str,
    layout: &llrm_mir::datalayout::DataLayout,
    platform: Platform,
    lines: bool,
) -> Result<crate::mfunc::MFunc, String> {
    let blocks = crate::dag::function::select_function_in(module, name, layout, platform)?;
    crate::dag::emit::emit(name, &blocks, lines)
}

/// Every defined function of `program`'s one module, selected, allocated and
/// lowered.
pub fn functions(program: &Program) -> Result<Vec<crate::emit::Function>, String> {
    functions_with(program, false)
}

/// [`functions`], each carrying its source places where `lines`.
pub fn functions_with(
    program: &Program,
    lines: bool,
) -> Result<Vec<crate::emit::Function>, String> {
    let module = &program.modules[0];
    let platform = platform_of_machine(program);
    crate::lir_view::set_darwin(platform == Platform::Darwin);
    let mut out = Vec::new();
    for (id, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        let mut machine = match isel_mode() {
            IselMode::Hand => crate::isel::selected(module, id, &program.layout, platform, lines),
            IselMode::Dag => dag_select(module, &name, &program.layout, platform, lines),
            // A refusal of the DAG path falls back to the hand selector,
            // counted.
            IselMode::Shadow => dag_select(module, &name, &program.layout, platform, lines).or_else(|why| {
                eprintln!("isel-shadow: @{name}: {why}");
                crate::isel::selected(module, id, &program.layout, platform, lines)
            }),
        }
        .map_err(|why| format!("@{name}: {why}"))?;
        dump("Select", &machine);
        if llrm_support::env_flag!("LLRM_ARM64_VERIFY") {
            let undefined = crate::mfunc::undefined_reads(&machine);
            if !undefined.is_empty() {
                return Err(format!("@{name}: read before written: {undefined:?}"));
            }
        }
        machine.frame_pointer =
            llrm_codegen::backend::framefree::frame_pointer_policy(&function.attrs).map(str::to_owned);
        let optsize = function
            .attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    llrm_mir::opcode::Attribute::Flag(name) if name == "optsize" || name == "minsize"
                ),
            );
        crate::prepass::run(&mut machine, optsize);
        dump("Prepass", &machine);
        if !llrm_support::env_flag!("LLRM_ARM64_NO_COMBINE") {
            crate::combine::run(&mut machine, optsize);
            dump("Combine", &machine);
        }
        crate::cmpzero::run(&mut machine);
        dump("CmpZero", &machine);
        crate::flagcse::run(&mut machine);
        dump("FlagCse", &machine);
        crate::memcluster::run(&mut machine);
        dump("MemCluster", &machine);
        crate::sibcall::run(&mut machine);
        dump("SibCall", &machine);
        crate::prepass::order_copies(&mut machine);
        dump("OrderCopies", &machine);
        // The shared greedy allocator, unless `LLRM_ARM64_LINEAR` asks for the
        // linear scan; a function greedy refuses goes to the scan, reported.
        // `x16`/`x17` are the lowerer's scratch pair where an offset does not
        // encode; they are the allocator's while the frame stays small. A
        // spill slot is at most 16 B, a register mostly 8: between the two
        // bounds the allocation is kept only if the frame came out small.
        let (tight, loose) = crate::frame::scratch_estimate(&machine);
        let snapshot = (tight && !loose).then(|| machine.clone());
        crate::lir_view::set_frame_pointer_free(
            platform != Platform::Darwin
                && !(machine.calls
                    || machine.dynamic
                    || llrm_codegen::backend::framefree::keeps_frame_pointer(
                        machine.frame_pointer.as_deref(),
                        machine.calls,
                    )),
        );
        let allocate = |machine: &mut crate::mfunc::MFunc, scratch: bool| {
            crate::lir_view::set_scratch_free(scratch);
            // The shared greedy allocator, unless `LLRM_ARM64_LINEAR` asks for
            // the linear scan; a function greedy refuses goes to the scan,
            // reported.
            let greedy =
                (!llrm_support::env_flag!("LLRM_ARM64_LINEAR")).then(|| crate::lir_view::allocate(machine, optsize));
            match greedy {
                Some(Ok(allocation)) => allocation,
                Some(Err(why)) => {
                    eprintln!("greedy: @{name}: {why}");
                    GREEDY_FALLBACKS.with(|count| count.set(count.get() + 1));
                    crate::regalloc::allocate(machine)
                }
                None => crate::regalloc::allocate(machine),
            }
        };
        let mut allocation = allocate(&mut machine, tight);
        if let Some(before) = snapshot
            && crate::frame::frame_bound(&machine, 0) >= crate::frame::SCRATCH_FREE_BELOW
        {
            machine = before;
            allocation = allocate(&mut machine, false);
        }
        dump("RegAlloc", &machine);
        crate::copyprop::run(&mut machine);
        crate::postrasink::run(&mut machine);
        dump("PostRaSink", &machine);
        let variables = crate::frame::variable_offsets(&machine, &allocation, platform == Platform::Darwin, true);
        let blocks = crate::frame::lower(&machine, &allocation, platform == Platform::Darwin, true)
            .map_err(|why| format!("@{name}: {why}"))?;
        let mut tables = crate::jumptable::tables_of(&machine);
        let blocks = crate::layout::fold(
            blocks,
            &mut tables,
            &crate::layout::edge_odds(module, function, &program.layout, &machine),
            optsize,
            &crate::mfunc::returned_registers(&machine),
        );
        let blocks = crate::pairs::fold(blocks);
        let mut blocks = crate::ldst_update::fold(blocks, &crate::mfunc::returned_registers(&machine));
        crate::jumptable::compress(&mut blocks, &mut tables).map_err(|why| format!("@{name}: {why}"))?;
        out.push(crate::emit::Function {
            tables,
            symbol: crate::data::symbol_name(platform, &name),
            binding: crate::data::binding_of(global.linkage),
            blocks,
            variables,
            pool: machine.pool.clone(),
        });
    }
    Ok(out)
}

/// With `ISEL_DUMP=1`, the function's operations after machine phase `phase`,
/// one per line (`=== Phase @name`), as the x86 backend prints its LIR.
fn dump(
    phase: &str,
    machine: &crate::mfunc::MFunc,
) {
    if !llrm_support::env_flag!("ISEL_DUMP") {
        return;
    }
    eprintln!("=== {phase} @{}", machine.name);
    for block in &machine.blocks {
        eprintln!("{}:", block.label);
        for op in &block.ops {
            eprintln!("    {op:?}");
        }
    }
}

fn platform_of_machine(program: &Program) -> Platform {
    // The machine is the one `program` was made with; its datalayout names the
    // platform.
    match program.modules[0].datalayout.as_deref() {
        Some(text) if text == Platform::Linux.datalayout() => Platform::Linux,
        _ => Platform::Darwin,
    }
}

/// `text`, LLVM IR for `platform`, as an object: through the mid-end and the
/// backend.
pub fn object(
    text: &str,
    platform: Platform,
    name: &str,
    level: impl Into<Level>,
) -> Result<llrm_object::Object, String> {
    object_with(text, platform, name, level, false).map(|described| described.object)
}

/// [`object`], with the debug information (`-g`) where `debug`.
pub fn object_with(
    text: &str,
    platform: Platform,
    name: &str,
    level: impl Into<Level>,
    debug: bool,
) -> Result<llrm_debug::Described, String> {
    let mut program = program(text, platform)?;
    let level = level.into();
    if debug {
        llrm_mir::dbgimport::types_and_globals(&mut program.modules[0]);
    }
    if level != Level::O0 {
        optimized_at(&mut program, level)?;
    }
    let lowered = functions_with(&program, debug)?;
    let object = crate::emit::object(&program.modules[0], &program.layout, platform, name, &lowered)?;
    let debug = if debug { crate::debug::info(&program.modules[0], platform, &object, &lowered, name)? } else { None };
    Ok(llrm_debug::Described { object, debug })
}

/// The assembly of the lowered functions, one instruction a line.
thread_local! {
    static GREEDY_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many functions this thread has sent from greedy to the linear scan.
pub fn greedy_fallbacks() -> usize {
    GREEDY_FALLBACKS.with(std::cell::Cell::get)
}

pub fn assembly(
    text: &str,
    platform: Platform,
    level: impl Into<Level>,
) -> Result<String, String> {
    let mut program = program(text, platform)?;
    // The text carries no alias: refuse it rather than drop the symbol.
    program.modules[0].refuse_aliases("arm64")?;
    let level = level.into();
    if level != Level::O0 {
        optimized_at(&mut program, level)?;
    }
    let mut out = String::new();
    for function in functions(&program)? {
        out.push_str(&format!("{}:\n", function.symbol));
        for block in &function.blocks {
            out.push_str(&format!("{}:\n", block.label));
            // A frame mark is for the line table, not the assembler.
            for inst in block.insts.iter().filter(|inst| !matches!(inst, crate::mc::inst::Inst::Mark(_))) {
                out.push_str(&format!("\t{inst}\n"));
            }
        }
        for entry in &function.pool {
            out.push_str(&format!("{}:\n", entry.label));
            for chunk in entry.bytes.chunks(8) {
                let mut word = [0u8; 8];
                word[..chunk.len()].copy_from_slice(chunk);
                let directive = if chunk.len() == 4 { ".long" } else { ".quad" };
                out.push_str(&format!("\t{directive} {:#x}\n", u64::from_le_bytes(word)));
            }
        }
        for table in &function.tables {
            out.push_str(&table.assembly(platform));
        }
    }
    Ok(out)
}

/// The object file for `platform`: Mach-O or ELF.
pub fn object_file(
    text: &str,
    platform: Platform,
    name: &str,
    level: impl Into<Level>,
) -> Result<Vec<u8>, String> {
    object_file_with(text, platform, name, level, false)
}

/// [`object_file`], with the debug information (`-g`) where `debug`.
pub fn object_file_with(
    text: &str,
    platform: Platform,
    name: &str,
    level: impl Into<Level>,
    debug: bool,
) -> Result<Vec<u8>, String> {
    use llrm_target::object::Format;
    let object = object_with(text, platform, name, level, debug)?;
    use llrm_debug::DebugObjectTarget;
    let objects = crate::mc::Arm64Objects;
    let macho =
        || objects.write_described("macho", &object.object, object.debug.as_ref()).map_err(|error| error.to_string());
    let elf =
        || objects.write_described("elf", &object.object, object.debug.as_ref()).map_err(|error| error.to_string());
    if let Some(dir) = llrm_target::object::all_writers_dir() {
        for (format, written) in [(Format::MachO, macho()), (Format::Elf, elf())] {
            if let Ok(written) = written {
                llrm_target::object::record_writer(&dir, format, &written);
            }
        }
    }
    match platform {
        Platform::Darwin => macho(),
        Platform::Linux => elf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptrtoaddr_is_read_as_ptrtoint() {
        let (_, text) = without_triple("  %a = ptrtoaddr ptr %p to i64\n");
        assert_eq!(text, "  %a = ptrtoint ptr %p to i64\n");
    }

    #[test]
    fn a_triple_without_an_os_is_linux_only_for_aarch64() {
        assert_eq!(platform_or_default("aarch64--"), Some(Platform::Linux));
        assert_eq!(platform_or_default("arm64-apple-macos"), Some(Platform::Darwin));
        assert_eq!(platform_or_default("x86_64-unknown-linux-gnu"), None);
    }
}
