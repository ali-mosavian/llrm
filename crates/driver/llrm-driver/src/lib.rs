//! The one place that names targets. A frontend asks for the target its flags
//! name, among those it supports; nothing else in the tree matches on a
//! target's name.

mod coerce;
pub mod llc;

use std::rc::Rc;

use llrm_codegen::abi::machine::Machine;
use llrm_codegen::backend::isel::Compiled;
use llrm_codegen::driver::Options;
use llrm_codegen::driver::flags::Flags;
use llrm_target::Target;

/// The target a driver builds for when no `-m` is given.
pub const DEFAULT: &str = "x86-m16";

/// Every target built in.
pub fn all() -> Vec<Rc<dyn Target>> {
    vec![Rc::new(llrm_x86_m16::M16), Rc::new(llrm_x86_m32::M32), Rc::new(llrm_x86_m64::M64)]
}

/// The target whose description states `triple` as its `llvm_triple`: the one a
/// module's `target triple` names.
pub fn by_triple(triple: &str) -> Option<Rc<dyn Target>> {
    all().into_iter().find(|one| one.layout().llvm_triple.as_deref() == Some(triple))
}

/// What a tool's `--mode N` stands for: the datalayout of the target whose
/// `datalayout.toml` says `mode = N`, and the machine of its default CPU, so
/// that pointer-width answers and the passes that price with the target's TTI
/// (unroll, licm, rotate, indvars, loopidiom, unswitch) read the target's.
pub struct ModeTarget {
    pub datalayout: String,
    pub machine: Rc<dyn llrm_mir::target::Machine>,
    /// One line for the tool to say what it ran on.
    pub described: String,
}

/// The `ModeTarget` of mode `mode`, tuned for size where `size`.
pub fn mode_target(
    mode: &str,
    size: bool,
) -> Result<ModeTarget, String> {
    let target = mode
        .parse::<u32>()
        .ok()
        .and_then(|mode| all().into_iter().find(|one| one.layout().mode == mode))
        .ok_or("--mode N: no target with that mode")?;
    let cpu = target.default_cpu();
    let machine = llrm_codegen::backend::cpu::tuned_for(&*target, cpu, size)?.target();
    // m16 states no TTI: llrm-c prices there by the generic rules, and so does
    // this.
    let tti = machine.tti().map_or("none", |tti| tti.name());
    let described = format!("{} cpu {cpu} tti {tti} datalayout {}", target.name(), target.layout().datalayout);
    Ok(ModeTarget { datalayout: target.layout().datalayout.clone(), machine, described })
}

/// The instruction selector of the target `name`, from its select crate.
pub fn selector(name: &str) -> Option<&'static Compiled> {
    match name {
        "x86-m16" => Some(&llrm_x86_m16_select::SELECTOR),
        "x86-m32" => Some(&llrm_x86_m32_select::SELECTOR),
        "x86-m64" => Some(&llrm_x86_m64_select::SELECTOR),
        _ => None,
    }
}

/// A target and the instruction selector generated from its definitions.
pub struct Bound {
    pub target: Rc<dyn Target>,
    pub selection: &'static Compiled,
}

impl Bound {
    /// The driver's options for `machine`, selecting with this target's
    /// selector.
    pub fn options(
        &self,
        flags: &Flags,
        machine: Machine,
    ) -> Options {
        flags.driver(machine, Rc::clone(&self.target), self.selection)
    }
}

/// The options of the frontends that are built for 16-bit x86 only (BASIC, Nib,
/// BC objects) and of the tests of those: the one place that names it for them.
pub fn m16_options(machine: Machine) -> Options {
    let target: Rc<dyn Target> = Rc::new(llrm_x86_m16::M16);
    let selection = selector(target.name()).expect("the 16-bit selector is built");
    Options::new(machine, target, selection)
}

/// `m16_options` with the old selector: for the tests that pin its output
/// (the DAG selector is the default).
pub fn m16_options_old(machine: Machine) -> Options {
    Options { isel: llrm_codegen::backend::shadow::Selector::Old, ..m16_options(machine) }
}

/// BASIC's machine: real mode's, with its stack in the data group, which its
/// programs run on.
pub fn m16_machine() -> Machine {
    use llrm_target::Target;
    llrm_x86_m16::M16.machine().with_stack_in_data()
}

/// The target `flags` name by its `-m` number (its `datalayout.toml` says
/// which), or the default; refused if none is built in or the frontend does not
/// build for it: `supported` lists the names of the ones it does, `None` any.
pub fn target(
    flags: &Flags,
    supported: Option<&[&str]>,
) -> Result<Bound, String> {
    planned(flags, supported, None)
}

/// `target`, for a frontend that will build for the other targets and does not
/// yet: it says so, and where the work is tracked (`tracked`), instead of
/// saying it builds for another only.
pub fn planned(
    flags: &Flags,
    supported: Option<&[&str]>,
    tracked: Option<&str>,
) -> Result<Bound, String> {
    let known = all();
    let found = match flags.mode() {
        Some(mode) => known.iter().find(|one| one.layout().mode == mode).ok_or_else(|| {
            format!(
                "no target for -m{mode}; choose {}",
                known.iter().map(|one| format!("-m{}", one.layout().mode)).collect::<Vec<_>>().join(", ")
            )
        })?,
        None => known.iter().find(|one| one.name() == DEFAULT).expect("the default target is built in"),
    };
    let name = found.name();
    if let Some(supported) = supported.filter(|list| !list.contains(&name)) {
        if let Some(tracked) = tracked {
            let modes: Vec<String> = known
                .iter()
                .filter(|one| supported.contains(&one.name()))
                .map(|one| format!("-m{}", one.layout().mode))
                .collect();
            return Err(format!(
                "this compiler does not build for -m{} yet ({tracked}); it builds for {}",
                found.layout().mode,
                modes.join(", ")
            ));
        }
        let modes: Vec<String> = known
            .iter()
            .filter(|one| supported.contains(&one.name()))
            .map(|one| format!("-m{}", one.layout().mode))
            .collect();
        return Err(format!("this compiler builds for {} only, not -m{}", modes.join(", "), found.layout().mode));
    }
    flags.format(&**found)?;
    flags.convention(&**found)?;
    let selection = selector(name).ok_or_else(|| format!("no instruction selector is built for {name}"))?;
    Ok(Bound { target: Rc::clone(found), selection })
}

#[cfg(test)]
mod tests;
