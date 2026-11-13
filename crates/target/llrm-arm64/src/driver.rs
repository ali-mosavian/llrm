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
    // The words are one line's, so the text is rewritten once, and only if one
    // is there.
    let rewritten;
    let text = if text.contains(" dso_local") || text.contains(" = ptrtoaddr ") {
        rewritten = text.replace(" dso_local", "").replace(" = ptrtoaddr ", " = ptrtoint ");
        rewritten.as_str()
    } else {
        text
    };
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
        kept.push_str(line);
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
            Self::Omax => Options::aggressive(),
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

/// The `-f` switches of the command line (`-fno-unroll-loops`,
/// `-fno-inline=NAME`, `-fexcess-precision=`), applied to every mid-end
/// run of the process: the same parser as every other backend's `llc`.
static POLICY: std::sync::Mutex<llrm_codegen::driver::policy::Policy> =
    std::sync::Mutex::new(llrm_codegen::driver::policy::Policy::new());

/// Records `flag` if the driver passes it on to a backend; false when it is not
/// one.
pub fn pass_flag(flag: &str) -> bool {
    POLICY.lock().unwrap().take(flag)
}

/// What the pin runs before selection at every level, -O0 included: the
/// selector never sees `memset_pattern` (PreISelIntrinsicLowering), nor
/// `llvm.is.constant` and `llvm.objectsize` (LowerConstantIntrinsics).
pub fn lowered_before_selection(program: &mut Program) -> Result<(), String> {
    program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)?;
    program.modules.iter_mut().try_for_each(llrm_transforms::lowerconstantintrinsics::module_lowered)
}

/// `program` through the shared mid-end at `level`.
pub fn optimized_at(
    program: &mut Program,
    level: Level,
) -> Result<(), String> {
    let dump = std::env::var_os("LLRM_MIR_STAGES").filter(|one| !one.is_empty()).map(Into::into);
    let mut options = level.options();
    {
        let policy = POLICY.lock().unwrap();
        policy.mark(program)?;
        policy.apply(&mut options);
    }
    options.library = c_library();
    llrm_transforms::pipeline::applied(program, &Applied { dump, options, ..Applied::default() })?;
    lowered_before_selection(program)?;
    // `addIRPasses`: GlobalMerge, after the mid-end, before selection.
    if let Some(mut merge) = program.target.global_merge(level == Level::O3) {
        // DEVIATE GM-TRIAL (`crate::trial`): the trial compile merges as the
        // pin does, nothing at -Os.
        merge.optsize_counts &= !crate::trial::unmerged();
        for module in &mut program.modules {
            let before = module.globals.len();
            if llrm_transforms::globalmerge::merged(module, &program.layout, &merge)? && merge.optsize_counts {
                // Each merged global leaves; one struct stays.
                crate::trial::note_merged(before + 1 - module.globals.len());
            }
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

/// Whether calls are priced by what their callee writes (`crate::ipra`):
/// the target opts in, `LLRM_ARM64_NO_IPRA` not set.
fn interprocedural_register_use(platform: Platform) -> bool {
    use llrm_mir::tti::Tti;
    crate::tti::AArch64::of_cpu(platform.cpu()).is_some_and(|tti| tti.interprocedural_register_use())
        && !llrm_support::env_flag!("LLRM_ARM64_NO_IPRA")
}

/// One defined function of `program`'s module, selected, allocated and
/// lowered, with the registers a call to it destroys (`crate::ipra`).
fn lower_function(
    program: &Program,
    id: llrm_mir::context::GlobalId,
    platform: Platform,
    lines: bool,
) -> Result<(crate::emit::Function, crate::ipra::Written), String> {
    let module = &program.modules[0];
    let global = module.global(id);
    let function = global.function().ok_or("not a function")?;
    let name = global.name.owned().ok_or("an unnamed function")?;
    let mut machine =
        dag_select(module, &name, &program.layout, platform, lines).map_err(|why| format!("@{name}: {why}"))?;
    dump("Select", &machine);
    crate::mfunc::remove_unreachable_blocks(&mut machine);
    if let Some(mode) = crate::ifcvt::mode_from_env() {
        crate::ifcvt::run(&mut machine, mode);
        dump("EarlyIfConversion", &machine);
    }
    // The shared phases that need SSA see the body as selection made it,
    // phis and all; the private phases below see it after phi elimination.
    if llrm_support::env_flag!("LLRM_ARM64_STAGES") {
        crate::lir_body::stages::observe(&machine)?;
    }
    crate::mfunc::lower_phis(&mut machine).map_err(|why| format!("@{name}: {why}"))?;
    dump("PhiElimination", &machine);
    if llrm_support::env_flag!("LLRM_ARM64_VERIFY") {
        let undefined = crate::mfunc::undefined_reads(&machine);
        if !undefined.is_empty() {
            return Err(format!("@{name}: read before written: {undefined:?}"));
        }
    }
    machine.frame_pointer = llrm_codegen::backend::framefree::frame_pointer_policy(&function.attrs).map(str::to_owned);
    machine.unwind = crate::cfi::Unwind::of(function);
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
    if crate::lir_body::checking() {
        // The body carries the edge odds by block position; no pass between
        // here and the conversion adds or removes a block.
        machine.alloc_odds = Some(crate::layout::edge_odds(module, function, &program.layout, &machine));
        machine = crate::lir_body::round_trip(&machine)?;
    }
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
            && !(machine.dynamic
                || machine.outgoing > crate::frame::SAFE_SP_DISPLACEMENT
                || llrm_codegen::backend::framefree::keeps_frame_pointer(
                    machine.frame_pointer.as_deref(),
                    machine.calls,
                )),
    );
    machine.alloc_odds = Some(crate::layout::edge_odds(module, function, &program.layout, &machine));
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
    crate::trial::add(&machine);
    let variables = crate::frame::variable_offsets(&machine, &allocation, platform == Platform::Darwin);
    let blocks = crate::frame::lower(&machine, &allocation, platform == Platform::Darwin)
        .map_err(|why| format!("@{name}: {why}"))?;
    let mut tables = crate::jumptable::tables_of(&machine);
    let blocks = crate::layout::fold(
        blocks,
        &mut tables,
        &crate::layout::odds_by_label_number(
            &crate::layout::edge_odds(module, function, &program.layout, &machine),
            &machine,
        ),
        optsize,
        &crate::mfunc::returned_registers(&machine),
    );
    let blocks = crate::pairs::fold(blocks);
    let mut blocks = crate::ldst_update::fold(blocks, &crate::mfunc::returned_registers(&machine));
    crate::jumptable::compress(&mut blocks, &mut tables).map_err(|why| format!("@{name}: {why}"))?;
    let written = crate::ipra::written(&machine);
    Ok((
        crate::emit::Function {
            tables,
            symbol: crate::data::symbol_name(platform, &name),
            binding: crate::data::binding_of(global.linkage),
            blocks,
            variables,
            pool: machine.pool.clone(),
            unwind: machine.unwind != crate::cfi::Unwind::None,
        },
        written,
    ))
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
    crate::ipra::clear();
    if !interprocedural_register_use(platform) {
        let mut out = Vec::new();
        for (id, _, function) in module.functions() {
            if function.is_declaration() {
                continue;
            }
            out.push(lower_function(program, id, platform, lines)?.0);
        }
        return Ok(out);
    }
    // Callees first, so a call is priced by what its callee wrote.
    let mut lowered: std::collections::HashMap<llrm_mir::context::GlobalId, crate::emit::Function> = Default::default();
    let graph = llrm_mir::callgraph::CallGraph::new(module);
    for (component, _) in graph.bottom_up_components() {
        for id in component.iter().copied() {
            let global = module.global(id);
            let Some(function) = global.function() else { continue };
            if function.is_declaration() {
                continue;
            }
            let name = global.name.owned().ok_or("an unnamed function")?;
            let optnone = function
                .attrs
                .iter()
                .any(
                    |attr| matches!(
                        attr,
                        llrm_mir::opcode::Attribute::Flag(flag) if flag == "optnone"
                    ),
                );
            let usable = !optnone && !global.linkage.is_interposable();
            let (done, written) = lower_function(program, id, platform, lines)?;
            if usable {
                crate::ipra::record(&name, written);
            }
            lowered.insert(id, done);
        }
    }
    let mut out = Vec::new();
    for (id, _, function) in module.functions() {
        if !function.is_declaration() {
            out.push(lowered.remove(&id).ok_or("a function was not lowered")?);
        }
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
    let (program, lowered) = compiled(text, platform, level.into(), debug, false)?;
    let object = crate::emit::object(&program.modules[0], &program.layout, platform, name, &lowered)?;
    let debug = if debug { crate::debug::info(&program.modules[0], platform, &object, &lowered, name)? } else { None };
    Ok(llrm_debug::Described { object, debug })
}

/// `text` through the mid-end and the backend. At -Os on a target with the
/// rotation trial, DEVIATE ROTATION-TRIAL and ROTATION-HOT (see
/// [`crate::trial`]): a module whose pass noted a loop it rotated or
/// left is compiled again with the decision reversed, and the finished code
/// decides.
fn compiled(
    text: &str,
    platform: Platform,
    level: Level,
    debug: bool,
    refuse_aliases: bool,
) -> Result<(Program, Vec<crate::emit::Function>), String> {
    /// What the mid-end noted for a trial to reverse.
    #[derive(Default)]
    struct Notes {
        left: Vec<String>,
        left_loops: usize,
        rotated: Vec<String>,
        loops: usize,
        merged: usize,
    }
    type Built = (Program, Vec<crate::emit::Function>, crate::trial::Price, Notes);
    let size = matches!(level, Level::Os | Level::Oz);
    let asked = crate::trial::asked(platform, size);
    let once = |(unrotated, more, unmerged): (bool, bool, bool)| -> Result<Built, String> {
        let mut program = program(text, platform)?;
        if refuse_aliases {
            // The text carries no alias: refuse it rather than drop the symbol.
            program.modules[0].refuse_aliases("arm64")?;
        }
        if debug {
            llrm_mir::dbgimport::types_and_globals(&mut program.modules[0]);
        }
        if level != Level::O0 {
            let run = |program: &mut Program| optimized_at(program, level);
            let go = |program: &mut Program| match (unrotated, more) {
                (true, _) => llrm_transforms::trial::with_unrotated(|| run(program)),
                (_, true) => llrm_transforms::trial::with_more_rotation(|| run(program)),
                _ => run(program),
            };
            if unmerged { crate::trial::without_merge(|| go(&mut program))? } else { go(&mut program)? }
        } else {
            lowered_before_selection(&mut program)?;
        }
        let mut notes = Notes::default();
        if asked.is_some() {
            use llrm_transforms::trial;
            (notes.left, notes.left_loops) = trial::take_left();
            (notes.rotated, notes.loops) = (trial::take_rotated(), trial::rotated_loops());
            notes.merged = crate::trial::take_merged();
            // A module nothing was noted in is compiled once, unpriced: the
            // pricing is a second walk of every function.
            let noted = !(notes.left.is_empty() && notes.rotated.is_empty() && notes.merged == 0);
            if noted || (unrotated, more, unmerged) != (false, false, false) {
                crate::trial::start();
            }
        }
        let lowered = functions_with(&program, debug)?;
        let cost = crate::trial::price(&lowered);
        Ok((program, lowered, cost, notes))
    };
    let Some(hot) = asked else {
        let (program, lowered, _, _) = once((false, false, false))?;
        return Ok((program, lowered));
    };
    use llrm_transforms::trial;
    let _ = (trial::take_left(), trial::take_rotated(), crate::trial::take_merged());
    trial::clear_rotated_loops();
    let mut best = once((false, false, false))?;
    let notes = std::mem::take(&mut best.3);
    let mut state = (false, false, false);
    let mut try_one = |best: &mut Built,
                       other: (bool, bool, bool),
                       what: &str,
                       pays: &dyn Fn(crate::trial::Price, crate::trial::Price) -> bool|
     -> Result<bool, String> {
        trial::clear_rotated_loops();
        let other_built = once(other)?;
        let (kept, now) = (best.2, other_built.2);
        let keeps = pays(kept, now);
        if llrm_support::debug::enabled("trial") {
            eprintln!("trial {what}: {kept:?} -> {now:?}: {}", if keeps { "kept" } else { "put back" });
        }
        if keeps {
            *best = other_built;
        }
        Ok(keeps)
    };
    if !notes.rotated.is_empty()
        && try_one(&mut best, (true, false, false), "unrotated", &|kept, now| {
            crate::trial::keeps_unrotated(kept, now, hot, notes.loops)
        })?
    {
        state.0 = true;
    }
    if hot
        && !notes.left.is_empty()
        && try_one(&mut best, (state.0, true, false), "rotation-hot", &|kept, now| {
            crate::trial::keeps_rotated(kept, now, notes.left_loops)
        })?
    {
        state = (false, true, false);
    }
    if notes.merged > 0 {
        try_one(&mut best, (state.0, state.1, true), "unmerged", &|kept, now| {
            crate::trial::keeps_unmerged(kept, now, notes.merged)
        })?;
    }
    Ok((best.0, best.1))
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
    let (_, lowered) = compiled(text, platform, level.into(), false, true)?;
    let mut out = String::new();
    for function in lowered {
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
