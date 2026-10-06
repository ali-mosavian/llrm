//! The one place that names targets. A frontend asks for the target its flags
//! name, among those it supports; nothing else in the tree matches on a target's
//! name.

use std::rc::Rc;

use llrm_core::backend::isel::{self, Compiled};
use llrm_core::driver::flags::Flags;
use llrm_core::driver::Options;
use llrm_core::abi::machine::Machine;
use llrm_target::Target;

/// The target a driver builds for when `--target` is absent.
pub const DEFAULT: &str = "x86-code16";

/// Every target built in.
fn all() -> Vec<Rc<dyn Target>> {
    vec![Rc::new(llrm_x86_code16::Code16), Rc::new(llrm_x86_code32::Code32)]
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
pub fn code16_options(machine: Machine) -> Options {
    let target: Rc<dyn Target> = Rc::new(llrm_x86_code16::Code16);
    let selection = isel::selector(target.name()).expect("the 16-bit selector is built");
    Options::new(machine, target, selection)
}

/// The target `flags` name, or the default; refused if it is not built in or
/// the frontend (`supported`) does not build for it.
pub fn target(flags: &Flags, supported: &[&str]) -> Result<Bound, String> {
    let name = flags.target().unwrap_or(DEFAULT);
    let known = all();
    let found = known.iter().find(|one| one.name() == name).ok_or_else(|| {
        format!("unknown target {name}; choose {}", known.iter().map(|one| one.name()).collect::<Vec<_>>().join(", "))
    })?;
    if !supported.contains(&name) {
        return Err(format!("this compiler builds for {} only, not {name}", supported.join(", ")));
    }
    let selection = isel::selector(name).ok_or_else(|| format!("no instruction selector is built for {name}"))?;
    Ok(Bound { target: Rc::clone(found), selection })
}

#[cfg(test)]
mod tests;
