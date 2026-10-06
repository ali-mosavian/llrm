//! The one place that names targets. A frontend asks for the target its flags
//! name, among those it supports; nothing else in the tree matches on a target's
//! name.

use std::rc::Rc;

use llrm_core::driver::flags::Flags;
use llrm_target::Target;

/// The target a driver builds for when `--target` is absent.
pub const DEFAULT: &str = "x86-code16";

/// Every target built in.
fn all() -> Vec<Rc<dyn Target>> {
    vec![Rc::new(llrm_x86_code16::Code16)]
}

/// The target `flags` name, or the default; refused if it is not built in or
/// the frontend (`supported`) does not build for it.
pub fn target(flags: &Flags, supported: &[&str]) -> Result<Rc<dyn Target>, String> {
    let name = flags.target().unwrap_or(DEFAULT);
    let known = all();
    let found = known.iter().find(|one| one.name() == name).ok_or_else(|| {
        format!("unknown target {name}; choose {}", known.iter().map(|one| one.name()).collect::<Vec<_>>().join(", "))
    })?;
    if !supported.contains(&name) {
        return Err(format!("this compiler builds for {} only, not {name}", supported.join(", ")));
    }
    Ok(Rc::clone(found))
}

#[cfg(test)]
mod tests;
