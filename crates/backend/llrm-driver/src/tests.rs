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
    assert_eq!(target(&flags(&[]), &["x86-code16"]).unwrap().name(), DEFAULT);
    assert_eq!(target(&flags(&["--target", "x86-code16"]), &["x86-code16"]).unwrap().name(), "x86-code16");
}

#[test]
fn an_unknown_target_is_refused_with_the_known_ones() {
    let error = target(&flags(&["--target", "arm64"]), &["x86-code16"]).err().unwrap();
    assert_eq!(error, "unknown target arm64; choose x86-code16");
}

/// A frontend built for one target does not take another's flag.
#[test]
fn a_target_the_frontend_does_not_build_for_is_refused() {
    let error = target(&flags(&["--target", "x86-code16"]), &["x86-code32"]).err().unwrap();
    assert_eq!(error, "this compiler builds for x86-code32 only, not x86-code16");
}
