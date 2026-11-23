//! LLVM IR to m68k assembly text: `llc`'s job over `opt`'s. Clang's front end
//! alone makes the IR (`-O2 -Xclang -disable-llvm-passes`); the shared mid-end
//! optimizes it; the m68k backend selects, allocates and prints.

use std::rc::Rc;

use llrm_mir::Module;
use llrm_mir::program::Program;
use llrm_mir::target::Machine;
use llrm_transforms::pipeline::{Applied, Options as MidEnd};

use crate::compile::{CodeModel, Item, Options, items_with};
use crate::dagisel::hooks::Cpu;
use crate::machine::M68kMachine;

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

    fn mid_end(self) -> MidEnd {
        match self {
            Self::O0 => MidEnd::none(),
            Self::O1 => MidEnd::basic(),
            Self::O2 => MidEnd::default(),
            Self::O3 => MidEnd::speed(),
            Self::Omax => MidEnd::aggressive(),
            Self::Os => MidEnd::size(),
            Self::Oz => MidEnd::min_size(),
        }
    }
}

pub use llrm_codegen::driver::policy::Policy;

/// The CPU `-mcpu=` names.
pub fn cpu_of(name: &str) -> Result<Cpu, String> {
    match name {
        "68000" | "m68000" => Ok(Cpu::M68000),
        "68010" | "m68010" => Ok(Cpu::M68010),
        "68020" | "m68020" => Ok(Cpu::M68020),
        "68030" | "m68030" => Ok(Cpu::M68030),
        "68040" | "m68040" => Ok(Cpu::M68040),
        "68060" | "m68060" => Ok(Cpu::M68060),
        _ => Err(format!("unknown m68k cpu `{name}`; choose 68000, 68010, 68020, 68030, 68040 or 68060")),
    }
}

/// The machine the mid-end is handed: its cost model prices the passes that
/// need one (loop strength reduction, the CRC recognizer).
pub fn machine() -> Rc<dyn Machine> {
    machine_for(Cpu::default())
}

/// [`machine`] for `cpu`.
pub fn machine_for(cpu: Cpu) -> Rc<dyn Machine> {
    Rc::new(M68kMachine { cpu })
}

/// `text`, LLVM IR from clang, as a program: the lines and words MIR's reader
/// leaves out of LLVM dropped, the datalayout checked against the target's.
pub fn program(text: &str) -> Result<Program, String> {
    program_for(text, Cpu::default())
}

/// [`program`] for `cpu`.
pub fn program_for(
    text: &str,
    cpu: Cpu,
) -> Result<Program, String> {
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with("target triple") || line.starts_with(';') {
            continue;
        }
        kept.push_str(&line.replace(" dso_local", ""));
        kept.push('\n');
    }
    let mut module = llrm_mir::parse::module(&kept).map_err(|error| error.to_string())?;
    module.refuse_aliases("m68k")?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let expected = crate::kernels::DATALAYOUT;
    match module.datalayout.as_deref() {
        Some(stated) if stated == expected => {}
        Some(stated) => return Err(format!("the module's datalayout `{stated}` is not m68k's `{expected}`")),
        None => return Err(format!("the module states no datalayout; m68k's is `{expected}`")),
    }
    Program::new(vec![module], machine_for(cpu))
}

/// `text` through the shared mid-end at `level` and the m68k backend, as
/// assembly: each defined function's label and instructions.
pub fn assembly(
    text: &str,
    cpu: Cpu,
    level: Level,
) -> Result<String, String> {
    assembly_with_debug(text, cpu, level, false)
}

/// [`assembly`], with each instruction's source place (`.loc`) where `debug`.
pub fn assembly_with_debug(
    text: &str,
    cpu: Cpu,
    level: Level,
    debug: bool,
) -> Result<String, String> {
    let dump = llrm_support::env_var("LLRM_MIR_STAGES").map(std::path::PathBuf::from);
    assembly_with(text, cpu, level, debug, dump.as_deref())
}

/// [`assembly`], writing the mid-end's stages to `dump` (`N/NNN-stage.ll`, as
/// every other target's `--dump DIR` and `LLRM_MIR_STAGES` do).
pub fn assembly_dumping(
    text: &str,
    cpu: Cpu,
    level: Level,
    dump: Option<&std::path::Path>,
) -> Result<String, String> {
    assembly_with(text, cpu, level, false, dump)
}

/// The size table's estimate of each defined function of the module the backend
/// gets at `level`, before the `function_scale` residue: the numerator
/// `tools/m68k_size_fit.py` divides the objects' symbol sizes by.
pub fn size_estimates(
    text: &str,
    cpu: Cpu,
    level: Level,
) -> Result<Vec<(String, i64)>, String> {
    use llrm_mir::datalayout::DataLayout;
    let module = lowered_for(text, cpu, level)?;
    let layout = DataLayout::parse(module.datalayout.as_deref().ok_or("no datalayout")?)?;
    let costs = llrm_mir::target::OperationCosts { function_scale: 100, ..M68kMachine { cpu }.size_costs() };
    Ok(module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .map(|(_, global, function)| {
            (
                global.name.owned().unwrap_or_default(),
                llrm_transforms::walk::function_bytes(&module, &layout, &costs, function),
            )
        })
        .collect())
}

/// [`lowered`] with the default flags: the module the backend gets at `level`,
/// what a size estimate is of.
pub fn lowered_for(
    text: &str,
    cpu: Cpu,
    level: Level,
) -> Result<Module, String> {
    lowered(text, cpu, level, None, &Policy::default(), default_backend(level)).map(|lowered| lowered.module)
}

/// The backend options the assembly and object paths build for `level`.
fn default_backend(level: Level) -> Options {
    Options {
        size: matches!(level, Level::Os | Level::Oz),
        strict: level == Level::O1,
        mid_end: false,
        ..Options::default()
    }
}

thread_local! {
    static LOWERINGS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many functions this thread has lowered to items since it began: what a
/// test counts to see that the trial's winner is not lowered again.
pub fn lowerings() -> usize {
    LOWERINGS.with(std::cell::Cell::get)
}

/// A module the mid-end finished, and the items of the functions the -Os/-O1/
/// nest trial already lowered for the side it kept: lowered once, for the
/// price, and not again for the output.
struct Lowered {
    module: Module,
    kept: Option<Kept>,
}

/// The items of the trial's differing functions, as `function_items` builds
/// them for `module` (pc-relative calls) under `backend`: used only where the
/// output is built from the same module with the same options and libcalls.
struct Kept {
    libcalls: u8,
    items: Vec<(String, Vec<Item>)>,
}

/// The unrolled size, in the unroll units, of the refused loops of a module
/// above which -Os does not compile it again: the second compile is the
/// module's whole mid-end. Over the corpus (116 of 434 cells run the trial,
/// 34 keep it) none of the 11 modules above 700 kept it, and they took 9.9 of
/// the 29.4 s the trial added (run/fixed_point 3,628 units: 0.95 s -> 3.7 s).
const TRIAL_MAX_UNITS: u64 = 700;

/// [`lowered_once`], then at -Os DEVIATE UNROLL-OS-TRIAL (the shared
/// `llrm_transforms::trial`, as x86's driver): the module compiled again with
/// the loops the -Os threshold refused unrolled, kept if its functions that
/// differ are smaller, bytes then clocks (`Price::Size`). At -O1 the same
/// trial (DEVIATE UNROLL-O1-TRIAL, `Tti::unroll_trial_threshold`) keeps them
/// where the functions are cheaper in clocks (`Price::Rate`). A module
/// with no refused loop is compiled once.
fn lowered(
    text: &str,
    cpu: Cpu,
    level: Level,
    dump: Option<&std::path::Path>,
    policy: &Policy,
    backend: Options,
) -> Result<Lowered, String> {
    use llrm_transforms::trial;
    let _ = trial::take();
    let _ = trial::take_refused();
    let mut base = lowered_once(text, cpu, level, dump, policy)?;
    let backend = Options { code_model: model_of(&base, backend.code_model)?, ..backend };
    let (mut noted, mut units) = trial::take();
    let mut refused_sites = trial::take_refused();
    let mut accepted_sites = trial::take_accepted();
    let small = matches!(level, Level::Os | Level::Oz);
    // Each side's differing functions are lowered in the side's own module,
    // the build the output makes of them: what the price sees is what is
    // printed, and the kept side's items are not built twice.
    let lower = |module: &Module, cut: &std::collections::BTreeSet<String>| -> Result<Kept, String> {
        let libcalls = backend.libcalls.unwrap_or_else(|| crate::compile::libcalls_for(module, cpu, backend));
        // `differing` also names globals and a function one side lost
        let defined: Vec<String> = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .filter_map(|(_, global, _)| global.name.owned().filter(|name| cut.contains(name)))
            .collect();
        let items = lower_functions(module, &defined, cpu, Options { libcalls: Some(libcalls), ..backend }, true)?;
        Ok(Kept { libcalls, items })
    };
    let flat = |kept: &Kept| -> Vec<Item> { kept.items.iter().flat_map(|(_, items)| items.iter().cloned()).collect() };
    // DEVIATE OS-INLINE-TRIAL-CLOCKS (`Tti::opt_size_inline_finished_trial`):
    // the calls the -Os size bound alone refused, inlined in a second
    // compile, kept where the finished code is faster within the bytes ceiling
    // (`trial::keeps_within_headroom`). A module with no such call is compiled
    // once. DEVIATE INLINEHINT-GROWTH-TRIAL (gcc `ipa-inline.cc`
    // `want_inline_small_function_p` over `estimate_edge_growth`;
    // `Tti::opt_size_inline_guard_yields_to_inlinehint` and
    // `..._negative_cost`): the size bound stood down for the pin's
    // `inlinehint` threshold and its negative cost, which the units cannot
    // price. The module compiled again with those calls kept is kept where
    // its differing functions are fewer bytes.
    let mut base_kept: Option<Kept> = None;
    let yielded = !trial::take_hint_yields().is_empty() | !trial::take_negative_yields().is_empty();
    if small && yielded {
        let other = trial::with_kept_hinted_calls(|| {
            trial::with_kept_negative_calls(|| lowered_once(text, cpu, level, None, policy))
        });
        if let Ok(other) = other {
            let other_state = (trial::take(), trial::take_refused(), trial::take_accepted());
            let _ = (trial::take_hint_yields(), trial::take_negative_yields());
            let cut = trial::differing(&base, &other);
            if !cut.is_empty() {
                let (calls, yields) = (lower(&other, &cut)?, lower(&base, &cut)?);
                let rest_calls_mulsi3 = || {
                    let rest: std::collections::BTreeSet<String> = other
                        .functions()
                        .filter(|(_, _, function)| !function.is_declaration())
                        .filter_map(|(_, global, _)| global.name.owned().filter(|name| !cut.contains(name)))
                        .collect();
                    lower(&base, &rest).map_or(true, |kept| crate::trial::calls_mulsi3(&kept.items))
                };
                let keeps = crate::trial::keeps_fewer_bytes(&calls.items, &yields.items, rest_calls_mulsi3, cpu);
                if llrm_support::debug::enabled("trial") {
                    eprintln!(
                        "trial hint {level:?}: {} functions differ: calls {}",
                        cut.len(),
                        if keeps { "kept" } else { "put back" }
                    );
                }
                if keeps {
                    base = other;
                    base_kept = Some(calls);
                    ((noted, units), refused_sites, accepted_sites) = other_state;
                } else {
                    base_kept = Some(yields);
                }
            }
        }
    }
    let sites = refused_sites;
    // A module with refused sites keeps their trial as it was: the callers they
    // share with the accepted sites would make the accepted sites' bytes
    // verdict cover them (sfm16_float_cmp: the four refused copies lost).
    let accepted = accepted_sites;
    let outlined = if sites.is_empty() { accepted } else { Vec::new() };
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial inline {level:?}: {} refused site(s), {} accepted site(s)", sites.len(), outlined.len());
    }
    let mut inlined: Vec<trial::Site> = Vec::new();
    let mut kept_outlined: Vec<trial::Site> = Vec::new();
    // only a program: a library unit has no entry to weigh the clocks against,
    // and its functions run an unknown number of times (the runtime's -Os
    // objects grew up to 18 % for the same clocks)
    let is_program = base
        .functions()
        .any(|(_, global, function)| !function.is_declaration() && global.name.owned().as_deref() == Some("main"));
    // DEVIATE OS-OUTLINE-TRIAL (`Tti::opt_size_inline_outline_trial`): the cold
    // sites the units accept are built as calls in the same second compile.
    // Each group of sites that share a function is judged on its own functions:
    // a refused site by the clocks within the bytes ceiling, a group with
    // an accepted site by the bytes alone. A mixed verdict takes a third
    // compile of the kept groups.
    if small && is_program && (!sites.is_empty() || !outlined.is_empty()) {
        let built = |refused: &[trial::Site], accepted: &[trial::Site]| {
            let module = trial::with_inlined(refused, || {
                trial::with_outlined(accepted, || lowered_once(text, cpu, level, None, policy))
            });
            let notes = trial::take();
            let _ = (trial::take_refused(), trial::take_accepted());
            module.map(|module| (module, notes))
        };
        if let Ok((other, other_notes)) = built(&sites, &outlined) {
            let cut = trial::differing(&base, &other);
            if !cut.is_empty() {
                let (with, without) = (lower(&other, &cut)?, lower(&base, &cut)?);
                let rest = std::cell::OnceCell::new();
                let rest_calls_mulsi3 = || {
                    *rest.get_or_init(|| {
                        let names: std::collections::BTreeSet<String> = other
                            .functions()
                            .filter(|(_, _, function)| !function.is_declaration())
                            .filter_map(|(_, global, _)| global.name.owned().filter(|name| !cut.contains(name)))
                            .collect();
                        lower(&base, &names).map_or(true, |kept| crate::trial::calls_mulsi3(&kept.items))
                    })
                };
                // refused sites alone keep the one verdict over every function
                // that differs
                let groups = if outlined.is_empty() {
                    vec![trial::Group { names: cut.clone(), refused: (0..sites.len()).collect(), accepted: Vec::new() }]
                } else {
                    trial::groups(&sites, &outlined)
                };
                let only = |items: &[(String, Vec<Item>)],
                            names: &std::collections::BTreeSet<String>|
                 -> Vec<(String, Vec<Item>)> {
                    items.iter().filter(|(name, _)| names.contains(name)).cloned().collect()
                };
                let verdicts: Vec<bool> = groups
                    .iter()
                    .map(|group| {
                        let (with, without) = (only(&with.items, &group.names), only(&without.items, &group.names));
                        if group.accepted.is_empty() {
                            crate::trial::keeps_within_headroom(&with, &without, rest_calls_mulsi3, cpu)
                        } else {
                            crate::trial::keeps_fewer_bytes(&with, &without, rest_calls_mulsi3, cpu)
                        }
                    })
                    .collect();
                let kept_groups = verdicts.iter().filter(|&&one| one).count();
                if llrm_support::debug::enabled("trial") {
                    eprintln!(
                        "trial inline {level:?}: {} functions differ: {kept_groups} of {} groups kept",
                        cut.len(),
                        groups.len()
                    );
                }
                let pick = |index: fn(&trial::Group) -> &Vec<usize>, all: &[trial::Site]| -> Vec<trial::Site> {
                    groups
                        .iter()
                        .zip(&verdicts)
                        .filter(|(_, kept)| **kept)
                        .flat_map(|(group, _)| index(group).iter().map(|&at| all[at].clone()))
                        .collect()
                };
                if kept_groups == groups.len() {
                    (base, base_kept, inlined, kept_outlined) = (other, Some(with), sites, outlined);
                    (noted, units) = other_notes;
                } else if kept_groups == 0 {
                    base_kept = Some(without);
                } else {
                    let (chosen, chosen_outlined) =
                        (pick(|group| &group.refused, &sites), pick(|group| &group.accepted, &outlined));
                    if let Ok((third, third_notes)) = built(&chosen, &chosen_outlined) {
                        let cut = trial::differing(&base, &third);
                        if !cut.is_empty() {
                            let kept = lower(&third, &cut)?;
                            (base, base_kept, inlined, kept_outlined) = (third, Some(kept), chosen, chosen_outlined);
                            (noted, units) = third_notes;
                        }
                    }
                }
            }
        }
    }
    let limit = if matches!(level, Level::O1 | Level::Os | Level::Oz) {
        M68kMachine { cpu }
            .tti()
            .and_then(
                |tti| {
                    let size = tti.unroll_trial_at_size();
                    // DEVIATE UNROLL-NEST-O1: the pin's threshold for a loop
                    // with a subloop.
                    if level == Level::O1 && tti.unroll_trial_nests_at_o1() {
                        size.max(Some(llrm_transforms::loopunrollpass::pin_full_unroll_threshold(1)))
                    } else {
                        size
                    }
                },
            )
    } else {
        // DEVIATE UNROLL-NEST-TRIAL (-O2, -O3): the pin's threshold, for the
        // loops with a subloop
        M68kMachine { cpu }
            .tti()
            .filter(|tti| tti.unroll_trial_nests_at_speed())
            .map(|_| llrm_transforms::loopunrollpass::pin_full_unroll_threshold(if level == Level::O3 { 3 } else { 2 }))
    };
    let plain = |module: Module, kept: Option<Kept>| Lowered { module, kept };
    let Some(limit) = limit.filter(|_| !noted.is_empty() && units <= TRIAL_MAX_UNITS) else {
        return Ok(plain(base, base_kept));
    };
    let Ok(other) = trial::with_inlined(&inlined, || {
        trial::with_outlined(&kept_outlined, || trial::with(limit, || lowered_once(text, cpu, level, None, policy)))
    }) else {
        return Ok(plain(base, base_kept));
    };
    let _ = (trial::take(), trial::take_accepted());
    let cut = trial::differing(&base, &other);
    if cut.is_empty() {
        return Ok(plain(base, base_kept));
    }
    // A lowering that fails is the compile's error, not a verdict: it was once
    // swallowed here and every trial put the unroll back.
    let (with, without) = (lower(&other, &cut)?, lower(&base, &cut)?);
    let rest_calls_mulsi3 = || {
        let rest: std::collections::BTreeSet<String> = other
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .filter_map(|(_, global, _)| global.name.owned().filter(|name| !cut.contains(name)))
            .collect();
        lower(&base, &rest).map_or(true, |kept| crate::trial::calls_mulsi3(&kept.items))
    };
    let pays = crate::trial::keeps_linking(
        &with.items,
        &without.items,
        rest_calls_mulsi3,
        cpu,
        crate::timing::Price::of(small),
    );
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial unroll {level:?}: {} functions differ: {}", cut.len(), if pays { "kept" } else { "put back" });
    }
    Ok(if pays { Lowered { module: other, kept: Some(with) } } else { Lowered { module: base, kept: Some(without) } })
}

/// `text` through the shared mid-end, the expansions m68k needs, and the
/// module's own mid-end, once: what both the assembly and the object print.
fn lowered_once(
    text: &str,
    cpu: Cpu,
    level: Level,
    dump: Option<&std::path::Path>,
    policy: &Policy,
) -> Result<Module, String> {
    let mut program = program_for(text, cpu)?;
    policy.mark(&mut program)?;
    let mut options =
        MidEnd { library: c_library(), lower_switches: !crate::dagisel::function::SWITCHES_IN_DAG, ..level.mid_end() };
    policy.apply(&mut options);
    if let Some(dump) = dump {
        std::fs::create_dir_all(dump).map_err(|error| format!("{}: {error}", dump.display()))?;
    }
    let applied = Applied { options, dump: dump.map(Into::into), ..Applied::default() };
    // The pipeline sees an atomic access as a call it cannot move; then the target's own form: the pin selects a
    // plain `move` for an atomic load or store of at most 32 bits (M68kInstrAtomics.td).
    // DEVIATE M68K-ATOMIC-INLINE-FRONTEND (owner w9, review 2027-01-31): see `atomic::generic_accesses_plain`.
    program.modules.iter_mut().try_for_each(|module| llrm_mir::atomic::generic_accesses_plain(module, 32))?;
    program.modules.iter_mut().try_for_each(llrm_mir::atomic::hidden)?;
    llrm_transforms::pipeline::applied(&mut program, &applied)?;
    // DEVIATE M68K-ATOMIC-SYNC: a 68020 expands a read-modify-write to a `cas` loop (`shouldExpandAtomicRMWInIR`,
    // AtomicExpandPass, not translated): it calls the `__sync_*` routine as a 68000 does. Exit: 2026-12-15, w8.
    program.modules.iter_mut().try_for_each(|module| llrm_mir::atomic::sync_libcalled(module, 32))?;
    program.modules.iter_mut().try_for_each(|module| llrm_mir::atomic::ordered_accesses_plain(module, 32))?;
    program.modules.iter_mut().try_for_each(|module| llrm_mir::atomic::lowered(module, 32, false, false, false))?;
    program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)?;
    // `llvm.is.constant` and `llvm.objectsize` left by -O0 (the pin runs
    // `LowerConstantIntrinsics` at every level)
    program.modules.iter_mut().try_for_each(llrm_transforms::lowerconstantintrinsics::module_lowered)?;
    // `llvm.scmp` and `llvm.ucmp` have no m68k instruction: the generic
    // expansion
    program.modules.iter_mut().try_for_each(|module| llrm_codegen::backend::compares::expanded(module, |_| true))?;
    // The mid-end is the module's, run once: `text_with` runs it over the whole
    // module for each function it is given.
    let module = crate::compile::mid_end_for_narrowing(
        &program.modules[0],
        cpu,
        crate::compile::NarrowMultiplies::of(
            cpu,
            level != Level::O0,
            matches!(level, Level::O2 | Level::O3 | Level::Omax),
        ),
        matches!(level, Level::O2 | Level::O3 | Level::Omax),
    )?;
    // The target's LSR (with the m68k TTI), exit-test and multiply narrowing
    // edit the IR after the shared pipeline's last stage: the dump ends
    // with what selection reads.
    if let Some(dump) = dump {
        let file = dump.join(TARGET_MID_END_STAGE);
        std::fs::write(&file, llrm_mir::print::module(&module))
            .map_err(|error| format!("{}: {error}", file.display()))?;
    }
    Ok(module)
}

/// The dump's last stage: the IR after the module's own mid-end, as selection
/// reads it.
pub const TARGET_MID_END_STAGE: &str = "99-target-midend.ll";

/// [`assembly`] with the debug lines and the stage dump chosen.
pub fn assembly_with(
    text: &str,
    cpu: Cpu,
    level: Level,
    debug: bool,
    dump: Option<&std::path::Path>,
) -> Result<String, String> {
    assembly_in(text, cpu, level, debug, dump, CodeModel::Small)
}

/// [`assembly_with`] in the code model `model` (`-mcmodel`).
pub fn assembly_in(
    text: &str,
    cpu: Cpu,
    level: Level,
    debug: bool,
    dump: Option<&std::path::Path>,
    model: CodeModel,
) -> Result<String, String> {
    assembly_flagged(text, cpu, level, debug, dump, model, &Policy::default())
}

/// The items of each function `module` defines, in order. Calls of the
/// module's own functions are pc-relative when all its code fits in
/// [`crate::pccall::REACH`], else absolute.
fn function_items(
    module: &Module,
    cpu: Cpu,
    backend: Options,
    kept: Option<&Kept>,
) -> Result<Vec<(String, Vec<Item>)>, String> {
    let mut names = Vec::new();
    for (_, global, function) in module.functions() {
        if !function.is_declaration() {
            names.push(global.name.owned().ok_or("an unnamed function")?);
        }
    }
    // chosen once for the module, not per function (M68K-SIZE-LIBCALL)
    let libcalls = backend.libcalls.unwrap_or_else(|| crate::compile::libcalls_for(module, cpu, backend));
    let backend = Options { libcalls: Some(libcalls), ..backend };
    // The trial's items stand for the functions they were built for: the same
    // module, options and libcalls, pc-relative calls.
    let kept = kept.filter(|one| one.libcalls == libcalls);
    let near = match kept {
        Some(kept) => {
            let rest: Vec<String> =
                names.iter().filter(|name| !kept.items.iter().any(|(held, _)| held == *name)).cloned().collect();
            let mut built = lower_functions(module, &rest, cpu, backend, true)?;
            built.extend(kept.items.iter().cloned());
            names
                .iter()
                .map(|name| {
                    built
                        .iter()
                        .position(|(held, _)| held == name)
                        .map(|at| built.swap_remove(at))
                        .ok_or_else(|| format!("@{name}: lost"))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        None => lower_functions(module, &names, cpu, backend, true)?,
    };
    let calls =
        near.iter().flat_map(|(_, items)| items).any(|item| matches!(item, Item::Inst(inst) if inst.opcode == "CALLq"));
    let bytes: usize = near.iter().map(|(_, items)| crate::compile::items_bytes(items)).sum();
    let mut functions = if calls && bytes > crate::pccall::REACH {
        lower_functions(module, &names, cpu, backend, false)?
    } else {
        near
    };
    crate::fallthrough::drop_jumps_to_next(&mut functions);
    Ok(functions)
}

/// The items of the functions `names` of `module`, in that order, with
/// pc-relative calls to the module's own functions or absolute ones.
fn lower_functions(
    module: &Module,
    names: &[String],
    cpu: Cpu,
    backend: Options,
    pc_calls: bool,
) -> Result<Vec<(String, Vec<Item>)>, String> {
    names
        .iter()
        .map(|name| {
            LOWERINGS.with(|count| count.set(count.get() + 1));
            // `LLRM_DEBUG=time` charges the function by name (the largest
            // functions are the first to look at)
            llrm_support::debug::timed_by(
                || format!("m68k fn {name}"),
                || items_with(module, name, cpu, Options { pc_calls, ..backend }),
            )
            .map(|items| (name.clone(), items))
            .map_err(|why| format!("@{name}: {why}"))
        })
        .collect()
}

/// [`assembly_in`] with the mid-end flags in `policy`.
pub fn assembly_flagged(
    text: &str,
    cpu: Cpu,
    level: Level,
    debug: bool,
    dump: Option<&std::path::Path>,
    model: CodeModel,
    policy: &Policy,
) -> Result<String, String> {
    let backend = Options { debug, code_model: model, ..default_backend(level) };
    let optimized = lowered(text, cpu, level, dump, policy, backend)?;
    let module = &optimized.module;
    let backend = Options { code_model: model_of(module, model)?, ..backend };
    let mut out = String::new();
    for (name, items) in function_items(module, cpu, backend, optimized.kept.as_ref())? {
        let global = module.global(module.named(&name).ok_or("a function with no global")?);
        match llrm_mir::image::Export::of(global.linkage) {
            llrm_mir::image::Export::Public => out.push_str(&format!("\t.globl\t{name}\n")),
            llrm_mir::image::Export::Weak => out.push_str(&format!("\t.weak\t{name}\n")),
            llrm_mir::image::Export::Local => {}
        }
        out.push_str(&crate::compile::printed(items, backend.debug).map_err(|why| format!("@{name}: {why}"))?);
    }
    let layout =
        llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    out.push_str(&crate::data::assembly(module, &layout)?);
    Ok(out)
}

/// `model` with [`CodeModel::Auto`] settled by the bytes of globals `module`
/// defines.
fn model_of(
    module: &llrm_mir::module::Module,
    model: CodeModel,
) -> Result<CodeModel, String> {
    if model != CodeModel::Auto {
        return Ok(model);
    }
    let layout =
        llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    let variables = llrm_mir::image::variables_with_blocks(
        module,
        &layout,
        &|name| crate::data::symbol(name),
        Some(&llrm_codegen::backend::blockaddress::symbol),
    )?;
    Ok(model.resolved(variables.iter().map(|one| one.bytes.len()).sum()))
}

/// `text` as an ELF32 big-endian relocatable object: the functions' code in
/// `.text`, the globals in `.data`, `.rodata` and `.bss`, the pointers between
/// them as relocations.
pub fn object(
    text: &str,
    cpu: Cpu,
    level: Level,
    name: &str,
) -> Result<Vec<u8>, String> {
    object_in(text, cpu, level, name, CodeModel::Small)
}

/// [`object`] in the code model `model` (`-mcmodel`).
pub fn object_in(
    text: &str,
    cpu: Cpu,
    level: Level,
    name: &str,
    model: CodeModel,
) -> Result<Vec<u8>, String> {
    object_flagged(text, cpu, level, name, model, &Policy::default(), None)
}

/// [`object_in`] with the mid-end flags in `policy`.
pub fn object_flagged(
    text: &str,
    cpu: Cpu,
    level: Level,
    name: &str,
    model: CodeModel,
    policy: &Policy,
    dump: Option<&std::path::Path>,
) -> Result<Vec<u8>, String> {
    // The file symbol is the module's own `source_filename`, as the pin's: the
    // path of a scratch IR file would differ between two compiles of one
    // source.
    let name = text
        .lines()
        .find_map(|line| line.strip_prefix("source_filename = \"")?.strip_suffix('"'))
        .map_or_else(|| name.rsplit('/').next().unwrap_or(name), |one| one);
    if let Some(dump) = dump {
        std::fs::create_dir_all(dump).map_err(|error| format!("{}: {error}", dump.display()))?;
    }
    let backend = Options { code_model: model, ..default_backend(level) };
    let lowered = lowered(text, cpu, level, dump, policy, backend)?;
    let module = lowered.module;
    let backend = Options { code_model: model_of(&module, model)?, ..backend };
    let mut items = Vec::new();
    let mut exports = llrm_support::hash::HashMap::default();
    for (function_name, one) in function_items(&module, cpu, backend, lowered.kept.as_ref())? {
        let global = module.global(module.named(&function_name).ok_or("a function with no global")?);
        exports.insert(function_name, llrm_mir::image::Export::of(global.linkage));
        items.extend(one);
    }
    let layout =
        llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    let variables = crate::data::with_constructors(llrm_mir::image::variables_with_blocks(
        &module,
        &layout,
        &|name| crate::data::symbol(name),
        Some(&llrm_codegen::backend::blockaddress::symbol),
    )?)?;
    crate::elf::relocatable_with_branches(name, &items, &exports, &variables, cpu.at_least(Cpu::M68020))
}

/// The compare routines of the platform's C library, by their IR names:
/// what `SimplifyLibCalls` may form.
fn c_library() -> Vec<llrm_mir::libfunc::Routine> {
    llrm_mir::libfunc::Routine::stated(llrm_mir::libfunc::HOSTED_COMPARE, str::to_owned, 0)
}
