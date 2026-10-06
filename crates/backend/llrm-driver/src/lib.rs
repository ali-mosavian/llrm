//! The one place that names targets. A frontend asks for the target its flags
//! name, among those it supports; nothing else in the tree matches on a target's
//! name.

use std::rc::Rc;

use llrm_core::backend::isel::{self, Compiled};
use llrm_core::driver::flags::Flags;
use llrm_core::driver::Options;
use llrm_core::abi::machine::Machine;
use llrm_target::Target;

/// The target a driver builds for when no `-m` is given.
pub const DEFAULT: &str = "x86-m16";

/// Every target built in.
pub fn all() -> Vec<Rc<dyn Target>> {
    vec![Rc::new(llrm_x86_m16::M16), Rc::new(llrm_x86_m32::M32)]
}

/// A target and the instruction selector generated from its definitions.
pub struct Bound {
    pub target: Rc<dyn Target>,
    pub selection: &'static Compiled,
}

impl Bound {
    /// The driver's options for `machine`, selecting with this target's selector.
    pub fn options(&self, flags: &Flags, machine: Machine) -> Options {
        flags.driver(machine, Rc::clone(&self.target), self.selection)
    }
}

/// The options of the frontends that are built for 16-bit x86 only (BASIC, Nib, BC
/// objects) and of the tests of those: the one place that names it for them.
pub fn m16_options(machine: Machine) -> Options {
    let target: Rc<dyn Target> = Rc::new(llrm_x86_m16::M16);
    let selection = isel::selector(target.name()).expect("the 16-bit selector is built");
    Options::new(machine, target, selection)
}

/// The target `flags` name by its `-m` number (its `datalayout.toml` says which), or the default;
/// refused if none is built in or the frontend does not build for it: `supported` lists the
/// names of the ones it does, `None` any.
pub fn target(flags: &Flags, supported: Option<&[&str]>) -> Result<Bound, String> {
    let known = all();
    let found = match flags.mode() {
        Some(mode) => known.iter().find(|one| one.layout().mode == mode).ok_or_else(|| {
            format!("no target for -m{mode}; choose {}", known.iter().map(|one| format!("-m{}", one.layout().mode)).collect::<Vec<_>>().join(", "))
        })?,
        None => known.iter().find(|one| one.name() == DEFAULT).expect("the default target is built in"),
    };
    let name = found.name();
    if let Some(supported) = supported.filter(|list| !list.contains(&name)) {
        let modes: Vec<String> = known.iter().filter(|one| supported.contains(&one.name())).map(|one| format!("-m{}", one.layout().mode)).collect();
        return Err(format!("this compiler builds for {} only, not -m{}", modes.join(", "), found.layout().mode));
    }
    let selection = isel::selector(name).ok_or_else(|| format!("no instruction selector is built for {name}"))?;
    Ok(Bound { target: Rc::clone(found), selection })
}

#[cfg(test)]
mod tests;
