use llrm_core::driver::flags::Flags;

use super::*;

fn flags(arguments: &[&str]) -> Flags {
    let argv: Vec<String> = arguments.iter().map(|one| one.to_string()).collect();
    let mut flags = Flags::default();
    let mut at = 0;
    while at < argv.len() {
        assert!(flags.take(&argv, &mut at).unwrap());
        at += 1;
    }
    flags
}

#[test]
fn no_flag_is_the_default_target() {
    assert_eq!(target(&flags(&[]), &["x86-code16"]).unwrap().target.name(), DEFAULT);
    assert_eq!(target(&flags(&["--target", "x86-code16"]), &["x86-code16"]).unwrap().target.name(), "x86-code16");
}

#[test]
fn an_unknown_target_is_refused_with_the_known_ones() {
    let error = target(&flags(&["--target", "arm64"]), &["x86-code16"]).err().unwrap();
    assert_eq!(error, "unknown target arm64; choose x86-code16, x86-code32");
}

/// A frontend built for one target does not take another's flag.
#[test]
fn a_target_the_frontend_does_not_build_for_is_refused() {
    let error = target(&flags(&["--target", "x86-code16"]), &["x86-code32"]).err().unwrap();
    assert_eq!(error, "this compiler builds for x86-code32 only, not x86-code16");
}

/// A target's options select with the selector built from its own definitions.
#[test]
fn a_target_is_bound_to_its_own_selector() {
    let bound = target(&flags(&[]), &["x86-code16"]).unwrap();
    assert_eq!(bound.selection.name, "x86-code16");
    let options = bound.options(&flags(&[]), bound.target.machine());
    assert_eq!(options.selection.name, bound.target.name());
}

/// code32 is registered, flat, and bound to the selector made from its own
/// directory.
#[test]
fn code32_is_a_flat_target_with_its_own_selector() {
    let bound = target(&flags(&["--target", "x86-code32"]), &["x86-code32"]).unwrap();
    assert_eq!(bound.selection.name, "x86-code32");
    assert_eq!(bound.target.machine().addressing, llrm_target::machine::Addressing::Flat);
}
