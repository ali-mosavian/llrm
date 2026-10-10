//! The one place that names targets. A frontend asks for the target its flags
//! name, among those it supports; nothing else in the tree matches on a
//! target's name.

use std::rc::Rc;

use llrm_core::abi::machine::Machine;
use llrm_core::backend::isel::Compiled;
use llrm_core::driver::Options;
use llrm_core::driver::flags::Flags;
use llrm_target::Target;

/// The target a driver builds for when no `-m` is given.
pub const DEFAULT: &str = "x86-m16";

/// Every target built in.
pub fn all() -> Vec<Rc<dyn Target>> {
    vec![Rc::new(llrm_x86_m16::M16), Rc::new(llrm_x86_m32::M32)]
}

/// The instruction selector of the target `name`, from its select crate. The
/// effect rows of its code width are bound with it.
pub fn selector(name: &str) -> Option<&'static Compiled> {
    match name {
        "x86-m16" => {
            llrm_core::backend::registerinfo::bind(&llrm_x86_m16::REGISTER_INFO);
            llrm_core::backend::effects::register(llrm_x86_m16_select::MODE, llrm_x86_m16_select::rows);
            Some(&llrm_x86_m16_select::SELECTOR)
        }
        "x86-m32" => {
            llrm_core::backend::registerinfo::bind(&llrm_x86_m32::REGISTER_INFO);
            llrm_core::backend::effects::register(llrm_x86_m32_select::MODE, llrm_x86_m32_select::rows);
            Some(&llrm_x86_m32_select::SELECTOR)
        }
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
