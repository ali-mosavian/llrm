//! The one compiler past the frontends, as clang's CodeGen and llc: a HIR
//! program emitted as MIR, or a lifter's MIR modules, linked against the
//! runtime and optimized by the pipeline for the machine. It reads what
//! the program states and never asks which frontend made it.

pub mod basic;
mod data;
pub mod flags;

use std::path::PathBuf;

use data::Placed;
use llrm_lir::registers::Regs;
use llrm_mir::program::Program;
use llrm_mir::{GlobalId, Module};
use llrm_support::debug::timed;

use crate::abi::machine::Machine;
use crate::abi::qb::HirAbi;
use crate::backend::cpu::{self, Profile, ProfileOrName};
use crate::backend::masm;
use crate::backend::target::Segments;
use crate::backend::{assemble, executed};
use crate::hir::model;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir;
use crate::support::hash::HashMap;

/// The assembled listing and the executed-cost summary beside the stage dumps,
/// where `options` names a directory: every front end's compile writes them
/// here, whatever path it took to the assembled module.
pub(crate) fn stage_files(
    options: &Options,
    suffix: &str,
    assembled: &masm::Module,
) -> Result<(), String> {
    let Some(directory) = &options.dump else { return Ok(()) };
    let written = |name: &str, text: String| {
        std::fs::write(directory.join(format!("{name}{suffix}")), text).map_err(|error| error.to_string())
    };
    written("listing.asm", masm::text(assembled).map_err(|error| error.to_string())?)?;
    let cpu = options.cpu()?;
    written("cost", assembled.procedures.iter().map(|one| executed::summary(&one.body, cpu) + "\n").collect())
}

/// What a compile is for: the machine, whose CPU prices the choices, the
/// passes that run, and where the pipeline writes each stage.
#[derive(Clone)]
pub struct Options {
    pub machine: Machine,
    pub pipeline: llrm_transforms::pipeline::Options,
    pub dump: Option<PathBuf>,
    /// `-fstack-usage`: print each procedure's frame and what it can reach.
    pub stack_usage: bool,
    /// `-Wstack-usage=N`: warn of each entry that can reach more than N bytes.
    pub stack_limit: Option<i64>,
    /// The target's instruction selector, which `llrm-driver` binds to it.
    pub selection: &'static crate::backend::isel::Compiled,
    /// The target `selection` is for.
    pub arch: std::rc::Rc<dyn llrm_target::Target>,
    /// The object format the symbols are spelled for: `omf`, `elf` or `macho`,
    /// which the target's conventions decorate.
    pub object_format: &'static str,
    /// `-mabi=`: the ABI family an unmarked function has (calling.toml's
    /// `[abi.*]`), the target's default without.
    pub abi: Option<String>,
    /// `-gcodeview`, `-gdwarf`...: the debug format asked for, where `-g`
    /// writes any.
    pub debug_format: llrm_debug::Format,
    /// `-fexcess-precision=standard` or `-ffloat-store`: a float conversion
    /// rounds to its type.
    pub float_store: bool,
    /// Whether a spill of a float rounds to the float's type (`Profile`'s
    /// `rounds_float_spills`): set by llrm-c, which follows the pin.
    pub rounds_float_spills: bool,
    /// Whether a spill of an x87 value keeps its 80 bits (`Profile`'s
    /// `spills_extended`): set by llrm-qb, which follows real BC.
    pub spills_extended: bool,
    /// `-mcode-segment=`: the segment every module's code is in, where it is
    /// not its own `<MODULE>_TEXT`.
    pub code_segment: Option<String>,
    /// `-fno-inline=NAME`: the functions named are `noinline` (the pin's
    /// attribute), whichever frontend made them.
    pub no_inline: Vec<String>,
    /// `-fsanitize=stack`: every function compares SP with the runtime's
    /// limit on entry (`stack.toml` of the target's C runtime).
    pub stack_check: bool,
    /// `--isel=`: the instruction selector.
    pub isel: crate::backend::shadow::Selector,
    /// The library routines the target's runtime has, each with the calling
    /// convention (LLVM's number) a call of it is made in: `memcpy` is a call
    /// where it is named, the selector's `rep movs` where not; and the symbol
    /// the routine has in the object (Open Watcom's `memcpy_`, C's `_memcpy`).
    pub libcalls: Vec<(String, u32, String)>,
}

impl Options {
    /// For `machine` on the target `arch` with its selector, at -O2, the stages
    /// written where `LLRM_MIR_STAGES` names.
    pub fn new(
        machine: Machine,
        arch: std::rc::Rc<dyn llrm_target::Target>,
        selection: &'static crate::backend::isel::Compiled,
    ) -> Self {
        let isel = crate::backend::shadow::Selector::for_target(&*arch);
        Self {
            machine,
            pipeline: Default::default(),
            dump: llrm_support::env_var("LLRM_MIR_STAGES").map(Into::into),
            stack_usage: false,
            stack_limit: None,
            selection,
            arch,
            object_format: "omf",
            debug_format: Default::default(),
            abi: None,
            float_store: false,
            rounds_float_spills: false,
            spills_extended: false,
            code_segment: None,
            no_inline: Vec::new(),
            stack_check: false,
            isel,
            libcalls: Vec::new(),
        }
    }

    /// For 16-bit x86, which the tests of this crate are written for.
    #[cfg(test)]
    pub fn m16(machine: Machine) -> Self {
        Self::new(machine, std::rc::Rc::new(llrm_x86_m16::M16), crate::backend::isel::m16())
    }

    /// The debug format asked for, and what its writer needs said, on
    /// `assembled`'s debug information.
    pub fn describe_debug(
        &self,
        assembled: &mut masm::Module,
    ) {
        if let Some(debug) = assembled.debug.as_mut() {
            debug.format = self.debug_format;
            debug.cfa = self.cfa_locations();
            debug.ranges = self.location_ranges();
            // Only DWARF 5 has a place for a file's MD5; LLVM leaves it out of
            // older ones.
            if matches!(
                debug.format,
                llrm_debug::Format::Dwarf { version } if version < 5
            ) && let Some((_, md5)) = debug.source.as_mut()
            {
                *md5 = None;
            }
        }
    }

    /// Whether the debug writer this object format and `-g` flavor pick says
    /// where a value is over a range of code.
    pub fn location_ranges(&self) -> bool {
        self.debug_caps().location_ranges
    }

    /// Whether that writer places a frame cell from the canonical frame
    /// address, so `-g` need not keep a frame register.
    pub fn cfa_locations(&self) -> bool {
        self.debug_caps().cfa_locations
    }

    /// What the debug writer asked for in this object format can say.
    fn debug_caps(&self) -> llrm_debug::DebugCaps {
        self.arch.objects().debug_caps(self.object_format, self.debug_format)
    }

    pub fn cpu(&self) -> Result<&'static Profile, String> {
        cpu::tuned_optimizing(
            &*self.arch,
            &self.machine.cpu,
            self.pipeline.prefers_size(),
            self.pipeline.searches(),
            self.pipeline.searches_all(),
            self.pipeline.optimize,
            self.float_store,
            self.pipeline.aggressive_codegen(),
            self.rounds_float_spills,
            self.spills_extended,
            self.pipeline.stable_loads,
            self.pipeline.has_min_size,
        )
    }
}

/// Whether the global `have` is the function `asked` for: the same name in
/// either case, or with a BASIC type suffix (`%&!#$`) that `asked` leaves
/// off (`BENCHFIB&` is `BenchFib`).
fn is_named(
    have: &str,
    asked: &str,
) -> bool {
    have.eq_ignore_ascii_case(asked)
        || have.strip_suffix(['%', '&', '!', '#', '$']).is_some_and(|bare| bare.eq_ignore_ascii_case(asked))
}

/// Each function of `names` defined in `program` marked `noinline`, as the
/// pin's attribute does; a name that no function has is an error, not a flag
/// that did nothing. A name matches as `is_named` says.
fn no_inlined(
    program: &mut Program,
    names: &[String],
) -> Result<(), String> {
    for name in names {
        let mut found = false;
        for module in &mut program.modules {
            for global in &mut module.globals {
                if !global.name.as_deref().is_some_and(|have| is_named(have, name)) {
                    continue;
                }
                if let llrm_mir::GlobalKind::Function(function) = &mut global.kind
                    && !function.is_declaration()
                {
                    function.attrs.push(llrm_mir::facts::Fact::Inline(llrm_mir::facts::Inlining::Never).carrier());
                    found = true;
                }
            }
        }
        if !found {
            return Err(format!("-fno-inline={name}: the program defines no function of that name"));
        }
    }
    Ok(())
}

/// Each function `program` defines marked to check its stack on entry.
fn stack_checked(program: &mut Program) {
    for module in &mut program.modules {
        for global in &mut module.globals {
            if let llrm_mir::GlobalKind::Function(function) = &mut global.kind
                && !function.is_declaration()
            {
                function.attrs.push(llrm_mir::facts::Fact::StackCheck.carrier());
            }
        }
    }
}

/// Each function `program` defines marked "no-jump-tables"="true", as clang
/// writes it for `-fno-jump-tables`.
fn without_jump_tables(program: &mut Program) {
    for module in &mut program.modules {
        for global in &mut module.globals {
            if let llrm_mir::GlobalKind::Function(function) = &mut global.kind
                && !function.is_declaration()
            {
                function.attrs.push(llrm_mir::Attribute::Str("no-jump-tables".to_owned(), Some("true".to_owned())));
            }
        }
    }
}

/// What C's runtime says of its stack on `arch`: the `stack.toml` its
/// description names.
fn runtime_stack_check(arch: &dyn llrm_target::Target) -> Result<crate::hir::model::StackCheck, String> {
    let description = arch.runtime("c").ok_or("-fsanitize=stack: this target has no C runtime")?;
    let name = description.string("stack").map_err(|error| format!("-fsanitize=stack: {error}"))?;
    let file = description.file(&name).ok_or_else(|| format!("-fsanitize=stack: {name} is not shipped"))?;
    let row = toml::Value::Table(file.parse().map_err(|error| format!("-fsanitize=stack: {name}: {error}"))?);
    crate::hir::model::StackCheck::from_toml(&row)
}

/// `program` compiled for the machine: emitted, optimized, then each module
/// selected and assembled, its code in `<MODULE>_TEXT`, each function linked
/// by its symbol and its data where the frontend put it. Each module's
/// listing and executed costs go beside the stages.
pub fn compiled(
    program: &model::Program,
    options: &Options,
) -> Result<Vec<masm::Module>, String> {
    let (mut mir, data) = emitted(program, options)?;
    let placed: Vec<Placed> = timed("data placement", || {
        mir.modules
            .iter()
            .zip(&program.modules)
            .zip(&data)
            .map(|((module, hir), data)| Placed::of(module, hir, data))
            .collect()
    });
    optimized(&mut mir, options)?;
    let abi = HirAbi::of(program)?;
    let segments = Segments::of(
        &options.machine,
        Regs(options.arch.registers()),
        crate::backend::target::offset_bytes(&*options.arch),
    );
    let mut out = Vec::new();
    for ((module, hir), placed) in mir.modules.iter().zip(&program.modules).zip(&placed) {
        let mut assembled = timed("assemble", || {
            assemble::assembled_by(
                module,
                &abi,
                &options.code_segment.clone().unwrap_or_else(|| format!("{}_TEXT", hir.name.to_uppercase())),
                ProfileOrName::Profile(options.cpu()?),
                &segments,
                options.selection,
                options.isel,
                &*options.arch,
                &options.libcalls,
            )
        })?;
        // What a frontend asks the linker for though nothing names it.
        for name in &hir.requests {
            assembled.requests.insert(name.clone());
            if !assembled.externs.iter().any(|(one, _)| one == name) {
                assembled.externs.push((name.clone(), "near".to_owned()));
            }
        }
        options.describe_debug(&mut assembled);
        timed("data layout", || {
            placed.lay_out(
                &mut assembled,
                module,
                mir.segments.data_space,
                program.constant_segment.as_deref(),
                &program.bracketed_segments,
                options.machine.far_bss,
                options.machine.bss_begin,
                options.arch.layout().segment_bytes(),
            )
        })?;
        let suffix = if program.modules.len() > 1 { format!("-{}", hir.name) } else { String::new() };
        stage_files(options, &suffix, &assembled)?;
        out.push(assembled);
    }
    if options.stack_usage || options.stack_limit.is_some() {
        let usage = crate::backend::stackusage::Usage::of(&out, &*options.arch);
        if options.stack_usage {
            eprint!("{}", usage.report());
        }
        for warning in options.stack_limit.into_iter().flat_map(|limit| usage.warnings(limit)) {
            eprintln!("{warning}");
        }
    }
    Ok(out)
}

/// `module`, read from LLVM IR (clang's front end alone, `-O2 -Xclang
/// -disable-llvm-passes`), run through the whole pipeline and selected, as
/// `opt` then `llc` do: its code in `<NAME>_TEXT`, each function linked under
/// its own name. The target is `options.arch`; the module states its
/// datalayout, which must be the target's.
pub fn compiled_module(
    module: Module,
    name: &str,
    options: &Options,
) -> Result<masm::Module, String> {
    let layout = options.arch.layout().datalayout;
    match module.datalayout.as_deref() {
        Some(stated) if stated == layout => {}
        Some(stated) => {
            return Err(format!("the module's datalayout `{stated}` is not {}'s `{layout}`", options.arch.name()));
        }
        None => return Err(format!("the module states no datalayout; {}'s is `{layout}`", options.arch.name())),
    }
    let cpu = options.cpu()?;
    let mut program = Program::new(vec![module], cpu.target())?;
    if llrm_support::debug::verifying() {
        timed("mir verify frontend", || verified(&program, "the frontend"))?;
    }
    optimized(&mut program, options)?;
    let segments = Segments::of(
        &options.machine,
        Regs(options.arch.registers()),
        crate::backend::target::offset_bytes(&*options.arch),
    );
    let assembled = timed("assemble", || {
        assemble::assembled_by(
            &program.modules[0],
            &crate::abi::llvm::LlvmAbi {
                stack_check: if options.stack_check { Some(runtime_stack_check(&*options.arch)?) } else { None },
            },
            &format!("{}_TEXT", name.to_uppercase()),
            ProfileOrName::Profile(cpu),
            &segments,
            options.selection,
            options.isel,
            &*options.arch,
            &options.libcalls,
        )
    })?;
    let mut assembled = assembled;
    options.describe_debug(&mut assembled);
    data::zero_to_bss(&mut assembled, &program.modules[0], program.segments.data_space, options.machine.bss_begin);
    if options.machine.far_bss {
        data::zero_far_to_bss(&mut assembled, &program.modules[0]);
    }
    stage_files(options, "", &assembled)?;
    Ok(assembled)
}

/// What the spill model (`llrm_transforms::spill`) forecasts for each function
/// the pipeline hands to instruction selection: the `spillmodel` channel, to
/// set beside the spills the allocator makes (`cost` channel, `executed`).
fn spill_model(program: &Program) {
    use llrm_transforms::{profit, spill};
    for module in &program.modules {
        let Some(text) = module.datalayout.as_deref() else { continue };
        let Ok(layout) = llrm_mir::datalayout::DataLayout::parse(text) else { continue };
        let outer = llrm_mir::passes::Outer::of(module, Some(program.target.clone()));
        let room = spill::Room::of(&outer);
        let costs = program.target.costs();
        for global in &module.globals {
            let Some(function) = global.function().filter(|one| !one.is_declaration()) else { continue };
            let mut analyses = llrm_mir::passes::Analyses::new(std::rc::Rc::new(outer.clone()));
            let registers = analyses.get::<llrm_analysis::manager::Registers>(&module.context, &layout, function);
            let shape = analyses.get::<llrm_analysis::cfg::Shape>(&module.context, &layout, function);
            let unit = llrm_analysis::memory::Unit::of(module, &layout, function)
                .with_spaces(program.target.spaces())
                .with_registers(&registers)
                .with_shape(&shape);
            let trips = profit::proven_trips(&unit);
            let Some(frequency) =
                profit::_frequencies(&module.context, &module.metadata, &module.globals, function, Some(&trips))
            else {
                continue;
            };
            let across = |inst| spill::kept_across(&outer, &module.context, function, inst);
            if let Some(forecast) =
                profit::spill_forecast(&module.context, &layout, function, &costs, room, &across, &frequency)
            {
                llrm_support::debug!(
                    "spillmodel",
                    "{} peak {} spilled {} price {}",
                    global.name.as_deref().unwrap_or("?"),
                    forecast.peak,
                    forecast.spilled.len(),
                    forecast.cost
                );
                if let Some(name) = global.name.as_deref() {
                    let per_entry = forecast.cost as f64 / profit::UNIT as f64;
                    crate::backend::executed::predict(
                        name,
                        crate::backend::executed::Predicted {
                            peak: forecast.peak,
                            spilled: forecast.spilled.len(),
                            price: per_entry,
                            load: costs.load,
                            store: costs.store,
                        },
                    );
                }
            }
        }
    }
}

/// `program` as MIR, a module per HIR module, linked against the runtime
/// its promises describe; and each module's data objects' globals, by the
/// objects' ids.
pub fn emitted(
    program: &model::Program,
    options: &Options,
) -> Result<(Program, Vec<HashMap<i64, GlobalId>>), String> {
    // Whichever frontend made it, a program is checked before it is lowered.
    if llrm_support::debug::verifying() {
        crate::support::debug::timed("hir verify", || llrm_hir::verify::verify(program)).map_err(|why| why.0)?;
    }
    let emitted = timed("hir to mir", || llrm_hir::mir::emit(program, &options.arch.layout()));
    if let Some((name, why)) = emitted.iter().find_map(|one| one.refused.first()) {
        return Err(format!("@{name}: {why}"));
    }
    let runtime = timed("mir runtime", || {
        crate::hir::mir::runtime(&emitted.iter().zip(&program.modules).collect::<Vec<_>>(), &program.promises)
    })?;
    let (modules, data) = emitted.into_iter().map(|one| (one.module, one.data)).unzip();
    let target =
        std::rc::Rc::new(crate::abi::qb::LoweredTarget::of(options.cpu()?, crate::abi::qb::HirAbi::of(program)?));
    let mut linked = timed("mir link", || linked(modules, runtime, target))?;
    linked.exports.entries = program.entries.iter().cloned().collect();
    if crate::support::debug::enabled("mir") {
        let bodies = || {
            linked.modules.iter().flat_map(|module| {
                module.globals.iter().filter_map(|global| global.function()).filter(|one| !one.is_declaration())
            })
        };
        let instructions: usize =
            bodies().map(|one| one.layout().iter().map(|&block| one.block(block).len()).sum::<usize>()).sum();
        llrm_support::debug!("mir", "functions {} instructions {}", bodies().count(), instructions);
    }
    Ok((linked, data))
}

/// `modules` as one program for `target`, linked against `runtime`, a
/// module of declarations alone; each verified.
pub fn linked(
    modules: Vec<Module>,
    runtime: Module,
    target: std::rc::Rc<dyn llrm_mir::target::Machine>,
) -> Result<Program, String> {
    let program = Program::new(modules, target)?.with_runtime(runtime)?;
    if llrm_support::debug::verifying() {
        timed("mir verify frontend", || verified(&program, "the frontend"))?;
    }
    Ok(program)
}

/// `program` through the pipeline, then each module prepared for
/// instruction selection: a landing pad made one the runtime enters. Each
/// module verified after.
pub fn optimized(
    program: &mut Program,
    options: &Options,
) -> Result<(), String> {
    no_inlined(program, &options.no_inline)?;
    if options.stack_check {
        stack_checked(program);
    }
    if !options.pipeline.jump_tables {
        without_jump_tables(program);
    }
    let mut pipeline = options.pipeline.clone();
    // The DAG builder lowers the switches that remain, as the pin's
    // SelectionDAGBuilder does; the shadow run keeps the old selector's
    // lowering for the functions the DAG path refuses.
    pipeline.lower_switches &= options.isel != crate::backend::shadow::Selector::Dag;
    let applied =
        llrm_transforms::pipeline::Applied { options: pipeline, dump: options.dump.clone(), ..Default::default() };
    // The pipeline sees an atomic access as a call it cannot move.
    program.modules.iter_mut().try_for_each(llrm_mir::atomic::hidden)?;
    timed("mir pipeline", || llrm_transforms::pipeline::applied(program, &applied))?;
    // The widest atomic access: a 64-bit one needs `cmpxchg8b` below 64 bits.
    let native = if llrm_target::Target::layout(&*options.arch).mode == 64 { 64 } else { 32 };
    let tso = options.arch.name().starts_with("x86");
    program.modules.iter_mut().try_for_each(|module| llrm_mir::atomic::lowered(module, native, tso, tso))?;
    timed("mir markers", || program.modules.iter_mut().for_each(llrm_transforms::dead::markers_dropped));
    timed("mir ehprepare", || {
        program
            .modules
            .iter_mut()
            .try_for_each(
                |module| crate::backend::ehprepare::prepared(module, &llrm_target::Target::layout(&*options.arch)),
            )
    })?;
    // The DAG route expands it itself (`expandFP_TO_UINT`), after the combines.
    if options.arch.expands("fptoui.i64") && options.isel != crate::backend::shadow::Selector::Dag {
        timed("mir fp to unsigned", || program.modules.iter_mut().try_for_each(crate::backend::fpconvert::expanded))?;
    }
    timed("mir libm calls", || {
        program.modules.iter_mut().try_for_each(|module| crate::backend::libmcalls::expanded(module, &options.libcalls))
    })?;
    if let Some((_, convention, symbol)) = options.libcalls.iter().find(|(name, ..)| name == "memcpy") {
        timed("mir memcpy calls", || {
            program
                .modules
                .iter_mut()
                .try_for_each(|module| crate::backend::memcpycall::expanded(module, *convention, symbol))
        })?;
    }
    timed("mir constant intrinsics", || {
        program.modules.iter_mut().try_for_each(llrm_transforms::lowerconstantintrinsics::module_lowered)
    })?;
    if crate::backend::memsetpattern::expands_for(options.isel) {
        timed("mir memset patterns", || {
            program.modules.iter_mut().try_for_each(crate::backend::memsetpattern::expanded)
        })?;
    }
    timed("mir bit counts", || {
        program
            .modules
            .iter_mut()
            .try_for_each(
                |module| crate::backend::bitcounts::expanded(module, |operation| options.arch.expands(operation)),
            )
    })?;
    timed("mir byte swaps", || {
        program
            .modules
            .iter_mut()
            .try_for_each(|module| crate::backend::bytes::expanded(module, |operation| options.arch.expands(operation)))
    })?;
    if options.isel == crate::backend::shadow::Selector::Old {
        timed("mir bit reversals", || {
            program
                .modules
                .iter_mut()
                .try_for_each(|module| crate::backend::bytes::bits_reversed(module, |op| options.arch.expands(op)))
        })?;
    }
    timed("mir funnel shifts", || {
        program
            .modules
            .iter_mut()
            .try_for_each(
                |module| crate::backend::funnel::expanded(
                    module,
                    |operation| options.arch.expands(operation),
                    |bits| crate::backend::funnel::kept_for_isel(options.isel, &format!("i{bits}")),
                ),
            )
    })?;
    timed("mir absolute values", || {
        program
            .modules
            .iter_mut()
            .try_for_each(
                |module| {
                    crate::backend::absolute::expanded(module, |operation| {
                        // the DAG lowers `llvm.abs` itself (`LowerABS`, `neg;
                        // cmov`)
                        options.arch.expands(operation) && options.isel != crate::backend::shadow::Selector::Dag
                    })
                },
            )
    })?;
    timed("mir compares", || {
        program
            .modules
            .iter_mut()
            .try_for_each(
                |module| crate::backend::compares::expanded(module, |operation| options.arch.expands(operation)),
            )
    })?;
    if options.isel != crate::backend::shadow::Selector::Dag {
        timed("mir legalize", || program.modules.iter_mut().try_for_each(crate::backend::legalize::legalized))?;
    }
    let layout = options.arch.layout();
    if layout.expands_some("select") {
        let tti = program.target.tti().unwrap_or_else(|| std::rc::Rc::new(llrm_mir::tti::Basic));
        let profile = options.cpu()?;
        timed("mir selects", || {
            program.modules.iter_mut().try_for_each(|module| {
                let wanted = |ty: &str| {
                    layout.expands_for("select", ty) && !crate::backend::selects::kept_for_isel(options.isel, ty)
                };
                crate::backend::selects::priced(module, &*tti, &wanted, &|name| profile.cost(name).ok())
            })
        })?;
    }
    if llrm_support::debug::enabled("spillmodel") || llrm_support::debug::enabled("pressure") {
        timed("mir spill model", || spill_model(program));
    }
    if !llrm_support::debug::verifying() {
        return Ok(());
    }
    timed("mir verify pipeline", || verified(program, "the pipeline"))
}

/// Refuses `program` where a module does not verify, `stage` having made it.
fn verified(
    program: &Program,
    stage: &str,
) -> Result<(), String> {
    match program.modules.iter().find_map(|module| llrm_mir::verify::verify(module).into_iter().next()) {
        Some(first) => Err(format!("{stage} left invalid MIR: {first}")),
        None => Ok(()),
    }
}

/// The data no frontend lays out, which emission and the pipeline made --
/// ON ERROR's ERL table and the ERR its landing keeps -- each defined
/// variable `laid` does not claim, for a DGROUP segment.
pub fn added_data(
    module: &Module,
    laid: &dyn Fn(GlobalId) -> bool,
    names: &crate::support::hash::IndexMap<(crate::model::ir::Space, i64), String>,
) -> Result<Vec<masm::Datum>, String> {
    let added = module
        .globals
        .iter()
        .enumerate()
        .map(|(at, _)| GlobalId(at as u32))
        .filter(
            |&id| matches!(
                &module.global(id).kind,
                llrm_mir::GlobalKind::Variable(variable) if variable.initializer.is_some()
            ) && !laid(id),
        );
    added
        .map(|id| crate::backend::globals::datums(module, id, names))
        .collect::<Result<Vec<_>, _>>()
        .map(|all| all.concat())
}

/// Whether a driver frames `function`: a naked one, as the landing stub,
/// runs on the frame the runtime made.
pub fn framed(
    module: &Module,
    function: GlobalId,
) -> bool {
    !module
        .global(function)
        .function()
        .is_some_and(
            |one| one.attrs.iter().any(|attr| matches!(attr, llrm_mir::Attribute::Flag(flag) if flag == "naked")),
        )
}

/// The statement-table row RESUME NEXT reaches a landing pad by: the pad's
/// block `landing`, in the procedure assembled `number`th.
pub fn landing_row(
    number: usize,
    landing: i64,
) -> (i64, i64, String, i64) {
    (number as i64, 0, masm::label(number, landing), 0)
}

/// The statement-table row at the procedure assembled `number`th's first
/// block `entry`, at line 0: the runtime reports an error's line from the
/// last row before it, and no numbered line precedes this one's code.
pub fn entry_row(
    number: usize,
    entry: i64,
) -> (i64, i64, String, i64) {
    (number as i64, -1, masm::label(number, entry), 0)
}

/// OF_STA's table: each row a statement's offset and BASIC line, ended by
/// a zero word, as a procedure of inline data.
pub fn statement_table(
    rows: &[(i64, i64, String, i64)],
    registers: llrm_target::FrameRegisters,
    word: usize,
    regs: Regs,
) -> masm::Procedure {
    // A row is two words: where the statement is, and its line; the table ends
    // with a word of 0. The word is the target's near pointer.
    let mut code: Vec<masm::InlinePart> = Vec::new();
    for (_procedure, _order, label, line) in rows {
        code.push(masm::InlinePart::Fixup("offset".into(), label.clone(), 0));
        code.push(masm::InlinePart::Bytes((*line as u32).to_le_bytes()[..word].to_vec()));
    }
    code.push(masm::InlinePart::Bytes(vec![0; word]));
    let what = Semantics { name: Some("statement-table".into()), ..Semantics::new(Operation::Call) };
    let instruction = lir::Insn::new(1, None, Some(what), vec![], vec![]);
    let mut body = lir::LirBody::new(
        "$QB$STAT",
        1,
        vec![lir::LirBlock::new(1, vec![std::sync::Arc::new(instruction)]).into()],
        Default::default(),
        Default::default(),
    );
    body.registers = Some(regs);
    masm::Procedure {
        name: "$QB$STAT".into(),
        public: false,
        far: false,
        body,
        reserve: 0,
        callees: crate::support::hash::IndexMap::from_iter([(
            1,
            masm::Callee { name: "$statement-table".into(), far: false, pops: 0, code, tail: false },
        )]),
        interrupt: None,
        size: false,
        entry: 0,
        stack_check: None,
        probe_stack: None,
        registers,
    }
}

#[cfg(test)]
mod lifetimes_tests;

#[cfg(test)]
mod location_ranges_tests {
    use llrm_debug::Format;

    use super::Options;

    /// Whether a debug format says where a value is over a range of code is the
    /// writer's fact: location lists in DWARF and C13's ranges do; CodeView
    /// 4, its BASIC-era dialect and Turbo Debugger's records name one place for
    /// a scope. OMF is the last, ELF and Mach-O the first, whatever `-g`
    /// flavor is asked where one can be written.
    #[test]
    fn a_debug_format_says_ranges_as_its_writer_does() {
        let of = |object_format, debug_format| {
            Options { object_format, debug_format, ..Options::m16(llrm_x86_m16::machine::BUILT_IN.clone()) }
                .location_ranges()
        };
        assert!(!of("omf", Format::Default) && !of("omf", Format::CodeView) && !of("omf", Format::TurboDebugger));
        assert!(of("elf", Format::Default) && of("macho", Format::Default) && of("elf", Format::Dwarf { version: 4 }));
        assert!(of("coff", Format::Default) && of("coff", Format::CodeView));
    }

    /// The statement table's body is made here, not by instruction selection:
    /// it carries its target's register file too, or listing it panicked (every
    /// BASIC module: "the body has no register file").
    #[test]
    fn the_statement_table_is_listed_with_its_targets_register_file() {
        use llrm_target::Target;
        let table = super::statement_table(
            &[],
            llrm_x86_m16::M16.frame_registers(),
            2,
            llrm_lir::registers::Regs(llrm_x86_m32::M32.registers()),
        );
        // m32's file, which a body made with no target (m16's, in tests) is
        // not.
        assert_eq!(table.body.registers, Some(llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO)));
    }
}
