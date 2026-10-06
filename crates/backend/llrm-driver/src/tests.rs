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
    assert_eq!(target(&flags(&[]), Some(&["x86-code16"])).unwrap().target.name(), DEFAULT);
    assert_eq!(target(&flags(&["--target", "x86-code16"]), Some(&["x86-code16"])).unwrap().target.name(), "x86-code16");
}

#[test]
fn an_unknown_target_is_refused_with_the_known_ones() {
    let error = target(&flags(&["--target", "arm64"]), Some(&["x86-code16"])).err().unwrap();
    assert_eq!(error, "unknown target arm64; choose x86-code16, x86-code32");
}

/// A frontend built for one target does not take another's flag.
#[test]
fn a_target_the_frontend_does_not_build_for_is_refused() {
    let error = target(&flags(&["--target", "x86-code16"]), Some(&["x86-code32"])).err().unwrap();
    assert_eq!(error, "this compiler builds for x86-code32 only, not x86-code16");
}

/// A target's options select with the selector built from its own definitions.
#[test]
fn a_target_is_bound_to_its_own_selector() {
    let bound = target(&flags(&[]), Some(&["x86-code16"])).unwrap();
    assert_eq!(bound.selection.name, "x86-code16");
    let options = bound.options(&flags(&[]), bound.target.machine());
    assert_eq!(options.selection.name, bound.target.name());
}

/// code32 is registered, flat, and bound to the selector made from its own
/// directory.
#[test]
fn code32_is_a_flat_target_with_its_own_selector() {
    let bound = target(&flags(&["--target", "x86-code32"]), Some(&["x86-code32"])).unwrap();
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
        let bound = target(&flags, Some(&["x86-code16", "x86-code32"])).unwrap();
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
    assert_eq!((real.operand_bytes, flat.operand_bytes), (2, 4));
}

/// A flat target's profile prices its own forms: a near call and return, no 66h on a
/// dword. It had been priced by the 16-bit tables and mapping, a far call among them.
#[test]
fn a_targets_operations_are_priced_from_its_own_timings_and_mapping() {
    let profile = |arguments: &[&str]| {
        let flags = flags(arguments);
        let bound = target(&flags, Some(&["x86-code16", "x86-code32"])).unwrap();
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
        let bound = target(&flags, Some(&["x86-code16", "x86-code32"])).unwrap();
        assert_eq!(bound.target.default_cpu(), default);
        let options = bound.options(&flags, llrm_core::abi::machine::Machine { cpu: bound.target.default_cpu().to_owned(), ..bound.target.machine() });
        assert_eq!(options.cpu().unwrap().name, default);
    }
    let flags = flags(&["--target", "x86-code32", "--cpu", "386"]);
    let bound = target(&flags, Some(&["x86-code32"])).unwrap();
    let machine = flags.machine(bound.target.machine()).unwrap();
    assert_eq!(bound.options(&flags, machine).cpu().err().unwrap(), "unknown CPU target: 386; x86-code32 has 486, P5");
}

/// The registers an instruction pins come from the selected target's forms. They came from
/// 16-bit x86's whatever the target, so a flat compile pinned a string store's operands as
/// real mode has them (its selector among them) and, once flat string ops are written, would
/// have left its own unpinned.
#[test]
fn a_targets_pins_come_from_its_own_forms() {
    use llrm_core::backend::classes::RegisterClasses;
    use llrm_core::model::ir::{Held, Loc, Mem, Operation, Semantics};

    let held = |value: u32| Loc::Held(Held { value, width: 2 });
    // `stosd` as real mode lowers it: a placeholder cell and the pointer after; the value, the pointer, the selector.
    let store = Semantics {
        name: Some("stosd".to_owned()),
        dests: vec![Loc::Mem(Mem::new(None, 0)), held(5)],
        sources: vec![held(1), held(2), held(3)],
        ..Semantics::new(Operation::Fill)
    };
    let real = RegisterClasses::of(&llrm_x86_code16::Code16).requirements(&store);
    let flat = RegisterClasses::of(&llrm_x86_code32::Code32).requirements(&store);
    assert_eq!(real.len(), 4, "{real:?}");
    assert!(flat.is_empty(), "{flat:?}");
}

/// A flat target's registers are its own, not real mode's: six values, ebp the frame.
#[test]
fn a_targets_registers_come_from_its_description() {
    use iced_x86::Register;
    use llrm_core::backend::classes::RegisterClasses;

    let flat = RegisterClasses::of(&llrm_x86_code32::Code32);
    assert_eq!(flat.available, [Register::EAX, Register::ECX, Register::EDX, Register::EBX, Register::ESI, Register::EDI]);
    assert_eq!(flat.frame, Register::EBP);
}

/// -Os prices a shift-and-add multiply by its bytes, and the 66h prefix is for the size that is
/// not the target's default. A flat dword was priced as real mode's: a prefix on every one.
#[test]
fn a_flat_dword_has_no_operand_size_prefix_in_the_size_prices() {
    use llrm_x86::encoding::{register_bytes, shift_bytes};
    assert_eq!((register_bytes(4, 4), shift_bytes(3, 4, 4)), (2, 3));
    assert_eq!((register_bytes(4, 2), shift_bytes(3, 4, 2)), (3, 4));
}

/// A frontend that lists no targets takes every registered one: the list was a second, hand-kept
/// answer, and arm64 would have edited each frontend to add itself.
#[test]
fn no_list_lets_any_registered_target_through() {
    assert_eq!(target(&flags(&["--target", "x86-code32"]), None).unwrap().target.name(), "x86-code32");
    assert!(target(&flags(&["--target", "arm64"]), None).err().unwrap().contains("unknown target arm64"));
}
