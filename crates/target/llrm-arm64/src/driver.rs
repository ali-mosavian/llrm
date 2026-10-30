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

/// `text` with each `va_arg` instruction written out, as the backend lowers it
/// for Apple's arm64: the list is a pointer to the next 8-byte slot of unnamed
/// arguments.
///
/// DEVIATE 2: MIR's subset has no `va_arg` instruction, which clang emits for
/// this ABI (AAPCS64 gets its list walked in the IR clang writes). Until MIR
/// has it the driver expands the instruction in the text: a load of the
/// pointer, the value read from it, the pointer advanced by one slot. Exit: MIR
/// reads `va_arg` and the selector lowers it
/// (`AArch64TargetLowering::LowerVAARG`).
pub fn without_va_arg(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let Some((result, rest)) = trimmed.split_once(" = va_arg ptr ") else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let Some((list, ty)) = rest.split_once(", ") else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let ty = ty.split(',').next().unwrap_or(ty).trim();
        let slot = format!("%va.{number}");
        out.push_str(&format!("  {slot}.cur = load ptr, ptr {list}, align 8\n"));
        out.push_str(&format!("  {slot}.next = getelementptr inbounds i8, ptr {slot}.cur, i64 8\n"));
        out.push_str(&format!("  store ptr {slot}.next, ptr {list}, align 8\n"));
        out.push_str(&format!("  {result} = load {ty}, ptr {slot}.cur, align 8\n"));
    }
    out
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
    let text = if platform == Platform::Darwin { without_va_arg(&text) } else { text };
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
        // The DAG builder lowers the switches (`visitSwitch`); the MIR pass would do it first.
        Options { lower_switches: false, ..self.generic_options() }
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

/// The `-f[no-]PASS` switches of the command line (`-fno-unroll-loops`), applied
/// to every mid-end run of the process.
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
    program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)
}

/// The routines the C link of this target resolves, by their IR names (the
/// object writer decorates them): what `SimplifyLibCalls` may form.
fn c_library() -> Vec<llrm_mir::libfunc::Routine> {
    ["puts", "putchar", "fwrite", "fputc", "fputs"]
        .into_iter()
        .map(|name| llrm_mir::libfunc::Routine { standard: name, symbol: name.to_owned(), calling_convention: 0 })
        .collect()
}

/// Which instruction selector makes the machine function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IselMode {
    Hand,
    Dag,
    Shadow,
}

fn isel_mode() -> IselMode {
    if llrm_support::env_flag!("LLRM_ISEL_DAG") {
        IselMode::Dag
    } else if llrm_support::env_flag!("LLRM_ISEL_SHADOW") {
        IselMode::Shadow
    } else {
        IselMode::Hand
    }
}

/// `name` through the DAG path: the pin's table and the shared stages.
fn dag_select(
    module: &llrm_mir::module::Module,
    name: &str,
    layout: &llrm_mir::datalayout::DataLayout,
    platform: Platform,
) -> Result<crate::mfunc::MFunc, String> {
    let blocks = crate::dag::function::select_function_in(module, name, layout, platform)?;
    crate::dag::emit::emit(name, &blocks)
}

/// Every defined function of `program`'s one module, selected, allocated and
/// lowered.
pub fn functions(program: &Program) -> Result<Vec<crate::emit::Function>, String> {
    let module = &program.modules[0];
    let platform = platform_of_machine(program);
    let mut out = Vec::new();
    for (id, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        let mut machine = match isel_mode() {
            IselMode::Hand => crate::isel::select(module, id, &program.layout, platform),
            IselMode::Dag => dag_select(module, &name, &program.layout, platform),
            // A refusal of the DAG path falls back to the hand selector, counted.
            IselMode::Shadow => dag_select(module, &name, &program.layout, platform).or_else(|why| {
                eprintln!("isel-shadow: @{name}: {why}");
                crate::isel::select(module, id, &program.layout, platform)
            }),
        }
        .map_err(|why| format!("@{name}: {why}"))?;
        dump("Select", &machine);
        crate::prepass::run(&mut machine);
        dump("Prepass", &machine);
        crate::flagcse::run(&mut machine);
        dump("FlagCse", &machine);
        crate::storecluster::run(&mut machine);
        dump("StoreCluster", &machine);
        crate::sibcall::run(&mut machine);
        dump("SibCall", &machine);
        crate::prepass::order_copies(&mut machine);
        dump("OrderCopies", &machine);
        let allocation = crate::regalloc::allocate(&mut machine);
        dump("RegAlloc", &machine);
        let blocks = crate::frame::lower(&machine, &allocation).map_err(|why| format!("@{name}: {why}"))?;
        let blocks = crate::layout::fold(blocks);
        let blocks = crate::pairs::fold(blocks);
        out.push(crate::emit::Function {
            symbol: crate::data::symbol_name(platform, &name),
            binding: crate::data::binding_of(global.linkage),
            blocks,
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
    let mut program = program(text, platform)?;
    let level = level.into();
    if level != Level::O0 {
        optimized_at(&mut program, level)?;
    }
    let lowered = functions(&program)?;
    crate::emit::object(&program.modules[0], &program.layout, platform, name, &lowered)
}

/// The assembly of the lowered functions, one instruction a line.
pub fn assembly(
    text: &str,
    platform: Platform,
    level: impl Into<Level>,
) -> Result<String, String> {
    let mut program = program(text, platform)?;
    let level = level.into();
    if level != Level::O0 {
        optimized_at(&mut program, level)?;
    }
    let mut out = String::new();
    for function in functions(&program)? {
        out.push_str(&format!("{}:\n", function.symbol));
        for block in &function.blocks {
            out.push_str(&format!("{}:\n", block.label));
            for inst in &block.insts {
                out.push_str(&format!("\t{inst}\n"));
            }
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
    use llrm_target::object::Format;
    let object = object(text, platform, name, level)?;
    use llrm_object::ObjectTarget;
    let objects = crate::mc::Arm64Objects;
    let macho = || objects.write_object("macho", &object).map_err(|error| error.to_string());
    let elf = || objects.write_object("elf", &object).map_err(|error| error.to_string());
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

    /// Clang writes Apple's variadic reads as `va_arg`, which MIR does not read
    /// ("`va_arg` is not an instruction of MIR's subset"): each becomes the
    /// load of the pointer, the value, and the pointer advanced by a slot.
    #[test]
    fn ptrtoaddr_is_read_as_ptrtoint() {
        let (_, text) = without_triple("  %a = ptrtoaddr ptr %p to i64\n");
        assert_eq!(text, "  %a = ptrtoint ptr %p to i64\n");
    }

    #[test]
    fn va_arg_is_written_out_as_loads_and_a_slot_step() {
        let text = "  %12 = va_arg ptr %3, i32\n  ret i32 %12\n";
        let out = without_va_arg(text);
        assert!(!out.contains("va_arg"), "{out}");
        assert!(out.contains("%12 = load i32, ptr %va.0.cur"), "{out}");
        assert!(out.contains("getelementptr inbounds i8, ptr %va.0.cur, i64 8"), "{out}");
        assert!(out.ends_with("  ret i32 %12\n"), "{out}");
    }

    #[test]
    fn a_triple_without_an_os_is_linux_only_for_aarch64() {
        assert_eq!(platform_or_default("aarch64--"), Some(Platform::Linux));
        assert_eq!(platform_or_default("arm64-apple-macos"), Some(Platform::Darwin));
        assert_eq!(platform_or_default("x86_64-unknown-linux-gnu"), None);
    }
}
