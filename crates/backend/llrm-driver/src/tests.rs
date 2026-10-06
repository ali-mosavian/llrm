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

/// The profile a target is compiled with is built from that target: the 16-bit
/// one's dword index is behind the address-size prefix and costs a prefix; the flat
/// one's is native. Both came from the 16-bit tables, whichever target was named.
#[test]
fn a_targets_cpu_profile_has_its_own_address_forms_and_registers() {
    let profile = |arguments: &[&str]| {
        let flags = flags(arguments);
        let bound = target(&flags, &["x86-code16", "x86-code32"]).unwrap();
        let options = bound.options(&flags, bound.target.machine());
        options.cpu().unwrap()
    };
    let real = profile(&[]);
    let flat = profile(&["--target", "x86-code32"]);
    assert!(real.dword_address_form().unwrap().secondary);
    assert!(!flat.dword_address_form().unwrap().secondary);
    assert_eq!((real.register_capacity, real.call_register_capacity), (6, 2));
    assert_eq!((flat.register_capacity, flat.call_register_capacity), (6, 3));
    assert_eq!(flat.name, real.name);
}

/// A flat target's profile prices its own forms: a near call and return, no 66h on a
/// dword. It had been priced by the 16-bit tables and mapping, a far call among them.
#[test]
fn a_targets_operations_are_priced_from_its_own_timings_and_mapping() {
    let profile = |arguments: &[&str]| {
        let flags = flags(arguments);
        let bound = target(&flags, &["x86-code16", "x86-code32"]).unwrap();
        bound.options(&flags, bound.target.machine()).cpu().unwrap()
    };
    let (real, flat) = (profile(&[]), profile(&["--target", "x86-code32"]));
    assert_eq!((real.operations.call, real.operations.return_), (18, 13));
    assert_eq!((flat.operations.call, flat.operations.return_), (3, 5));
    assert_eq!(flat.operations.multiply, 26);
    assert!(flat.cost("pop_seg").is_err() && real.cost("call_near").is_err());
}

/// The CPU a compile is priced for with none asked is the target's: llrm-c took the 386 for
/// every target, so a flat compile with no `-march` died on a CPU its tables do not have.
/// Asking for another names the target's own.
#[test]
fn a_target_states_its_default_cpu_and_names_its_cpus_when_asked_for_another() {
    for (arguments, default) in [(&[][..], "386"), (&["--target", "x86-code32"][..], "486")] {
        let flags = flags(arguments);
        let bound = target(&flags, &["x86-code16", "x86-code32"]).unwrap();
        assert_eq!(bound.target.default_cpu(), default);
        let options = bound.options(&flags, llrm_core::abi::machine::Machine { cpu: bound.target.default_cpu().to_owned(), ..bound.target.machine() });
        assert_eq!(options.cpu().unwrap().name, default);
    }
    let flags = flags(&["--target", "x86-code32", "--cpu", "386"]);
    let bound = target(&flags, &["x86-code32"]).unwrap();
    let machine = flags.machine(bound.target.machine()).unwrap();
    assert_eq!(bound.options(&flags, machine).cpu().err().unwrap(), "unknown CPU target: 386; x86-code32 has 486, P5");
}
