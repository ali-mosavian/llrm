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
    assert_eq!(target(&flags(&[]), Some(&["x86-m16"])).unwrap().target.name(), DEFAULT);
    assert_eq!(target(&flags(&["-m16"]), Some(&["x86-m16"])).unwrap().target.name(), "x86-m16");
}

#[test]
fn an_unknown_target_is_refused_with_the_known_ones() {
    let error = target(&flags(&["-m128"]), Some(&["x86-m16"])).err().unwrap();
    assert_eq!(error, "no target for -m128; choose -m16, -m32, -m64");
}

/// A frontend built for one target does not take another's flag.
#[test]
fn a_target_the_frontend_does_not_build_for_is_refused() {
    let error = target(&flags(&["-m16"]), Some(&["x86-m32"])).err().unwrap();
    assert_eq!(error, "this compiler builds for -m32 only, not -m16");
}

/// A frontend that will build for another target says it does not yet, and
/// where the work is.
#[test]
fn a_target_the_frontend_will_build_for_is_refused_as_not_yet() {
    let error = planned(&flags(&["-m32"]), Some(&["x86-m16"]), Some("issue 1")).err().unwrap();
    assert_eq!(error, "this compiler does not build for -m32 yet (issue 1); it builds for -m16");
}

/// A target's options select with the selector built from its own definitions.
#[test]
fn a_target_is_bound_to_its_own_selector() {
    let bound = target(&flags(&[]), Some(&["x86-m16"])).unwrap();
    assert_eq!(bound.selection.name, "x86-m16");
    let options = bound.options(&flags(&[]), bound.target.machine());
    assert_eq!(options.selection.name, bound.target.name());
}

/// m32 is registered, flat, and bound to the selector made from its own
/// directory.
#[test]
fn m32_is_a_flat_target_with_its_own_selector() {
    let bound = target(&flags(&["-m32"]), Some(&["x86-m32"])).unwrap();
    assert_eq!(bound.selection.name, "x86-m32");
    assert_eq!(bound.target.machine().addressing, llrm_target::machine::Addressing::Flat);
}

/// The profile a target is compiled with is built from that target: the 16-bit
/// one's dword index is behind the address-size prefix and costs a prefix; the
/// flat one's is native. Both came from the 16-bit tables, whichever target was
/// named.
#[test]
fn a_targets_cpu_profile_has_its_own_address_forms_and_registers() {
    let profile = |arguments: &[&str]| {
        let flags = flags(arguments);
        let bound = target(&flags, Some(&["x86-m16", "x86-m32"])).unwrap();
        let options = bound.options(&flags, bound.target.machine());
        options.cpu().unwrap()
    };
    let real = profile(&[]);
    let flat = profile(&["-m32"]);
    assert!(real.dword_address_form().unwrap().secondary);
    assert!(!flat.dword_address_form().unwrap().secondary);
    assert_eq!((real.register_capacity, real.call_register_capacity), (6, 2));
    assert_eq!((flat.register_capacity, flat.call_register_capacity), (6, 5));
    assert_eq!(flat.name, real.name);
    assert_eq!((real.operand_bytes, flat.operand_bytes), (2, 4));
}

/// A flat target's profile prices its own forms: a near call and return, no 66h
/// on a dword. It had been priced by the 16-bit tables and mapping, a far call
/// among them.
#[test]
fn a_targets_operations_are_priced_from_its_own_timings_and_mapping() {
    let profile = |arguments: &[&str]| {
        let flags = flags(arguments);
        let bound = target(&flags, Some(&["x86-m16", "x86-m32"])).unwrap();
        bound.options(&flags, bound.target.machine()).cpu().unwrap()
    };
    let (real, flat) = (profile(&[]), profile(&["-m32"]));
    assert_eq!((real.operations.call, real.operations.return_), (18, 13));
    assert_eq!((flat.operations.call, flat.operations.return_), (3, 5));
    assert_eq!(flat.operations.multiply, 26);
    assert!(flat.cost("pop_seg").is_err() && real.cost("call_near").is_err());
}

/// The CPU a compile is priced for with none asked is the target's: llrm-c took
/// the 386 for every target, so a flat compile with no `-march` died on a CPU
/// its tables do not have. Asking for another names the target's own.
#[test]
fn a_target_states_its_default_cpu_and_names_its_cpus_when_asked_for_another() {
    for (arguments, default) in [(&[][..], "486"), (&["-m32"][..], "486")] {
        let flags = flags(arguments);
        let bound = target(&flags, Some(&["x86-m16", "x86-m32"])).unwrap();
        assert_eq!(bound.target.default_cpu(), default);
        let options = bound.options(&flags, bound.target.machine());
        assert_eq!(options.cpu().unwrap().name, default);
    }
    let flags = flags(&["-m32", "-march=i386"]);
    let bound = target(&flags, Some(&["x86-m32"])).unwrap();
    assert_eq!(
        flags.machine(&*bound.target, bound.target.machine()).unwrap_err(),
        "unknown -march=i386; x86-m32 has i486, pentium"
    );
}

/// Real-mode DOS said `cpu = "486"` and timings.times said `default_cpu 386`: a
/// target had two defaults. Nib and BASIC priced for the machine's, C for the
/// timings': one source went unread. A machine is priced for the one
/// `timings.times` names, whichever target.
#[test]
fn a_targets_machine_is_priced_for_its_default_cpu() {
    for target in crate::all() {
        assert_eq!(target.machine().cpu, target.default_cpu(), "{}", target.name());
        assert!(target.cpus().contains(&target.default_cpu()), "{}", target.name());
    }
}

/// The target is found by the `-m` number its own `datalayout.toml` declares:
/// the CLI holds no table of names to numbers, and a number no target declares
/// is refused.
#[test]
fn a_target_is_found_by_the_m_number_its_description_declares() {
    for (flag, name) in [("-m16", "x86-m16"), ("-m32", "x86-m32")] {
        assert_eq!(target(&flags(&[flag]), None).unwrap().target.name(), name);
    }
    assert_eq!(target(&flags(&[]), None).unwrap().target.name(), DEFAULT);
    for one in crate::all() {
        assert_eq!(target(&flags(&[&format!("-m{}", one.layout().mode)]), None).unwrap().target.name(), one.name());
    }
}

/// The registers an instruction pins come from the selected target's forms.
/// They came from 16-bit x86's whatever the target, so a flat compile pinned a
/// string store's operands as real mode has them (its selector among them) and,
/// once flat string ops are written, would have left its own unpinned.
#[test]
fn a_targets_pins_come_from_its_own_forms() {
    use llrm_core::backend::classes::RegisterClasses;
    use llrm_core::model::ir::{Held, Loc, Mem, Operation, Semantics};

    let held = |value: u32| Loc::Held(Held { value, width: 2 });
    // `stosd` as real mode lowers it: a placeholder cell and the pointer after;
    // the value, the pointer, the selector.
    let store = Semantics {
        name: Some("stosd".to_owned()),
        dests: vec![Loc::Mem(Mem::new(None, 0)), held(5)],
        sources: vec![held(1), held(2), held(3)],
        ..Semantics::new(Operation::Fill)
    };
    let real = RegisterClasses::of(&llrm_x86_m16::M16).requirements(&store);
    let flat = RegisterClasses::of(&llrm_x86_m32::M32).requirements(&store);
    assert_eq!(real.len(), 4, "{real:?}");
    assert!(flat.is_empty(), "{flat:?}");
}

/// A flat target's registers are its own, not real mode's: six values, ebp the
/// frame.
#[test]
fn a_targets_registers_come_from_its_description() {
    use llrm_core::backend::classes::RegisterClasses;
    use llrm_lir::registers::RegId;

    let flat = RegisterClasses::of(&llrm_x86_m32::M32);
    assert_eq!(flat.available, [RegId::EAX, RegId::ECX, RegId::EDX, RegId::EBX, RegId::ESI, RegId::EDI]);
    assert_eq!(flat.frame, RegId::EBP);
}

/// -Os prices a shift-and-add multiply by its bytes, and the 66h prefix is for
/// the size that is not the target's default. A flat dword was priced as real
/// mode's: a prefix on every one.
#[test]
fn a_flat_dword_has_no_operand_size_prefix_in_the_size_prices() {
    use llrm_x86::encoding::{register_bytes, shift_bytes};
    assert_eq!((register_bytes(4, 4), shift_bytes(3, 4, 4)), (2, 3));
    assert_eq!((register_bytes(4, 2), shift_bytes(3, 4, 2)), (3, 4));
}

/// A frontend that lists no targets takes every registered one: the list was a
/// second, hand-kept answer, and arm64 would have edited each frontend to add
/// itself.
#[test]
fn no_list_lets_any_registered_target_through() {
    assert_eq!(target(&flags(&["-m32"]), None).unwrap().target.name(), "x86-m32");
    assert_eq!(target(&flags(&["-m64"]), None).unwrap().target.name(), "x86-m64");
    assert!(target(&flags(&["-m128"]), None).err().unwrap().contains("no target for -m128"));
}

/// The audited multiply and divide bounds of a CPU are its description's: a
/// flat target prices the CPUs it has as the real-mode one does, and a CPU a
/// description has no row for has none (they were name matches in `timing.rs`,
/// which a new CPU or target would have had to edit).
#[test]
fn the_audited_bounds_come_from_the_targets_timings() {
    use llrm_core::backend::timing::{Clocks, signed_divide, signed_multiply};
    let profile = |arguments: &[&str]| {
        let flags = flags(arguments);
        let bound = target(&flags, Some(&["x86-m16", "x86-m32"])).unwrap();
        let machine = flags.machine(&*bound.target, bound.target.machine()).unwrap();
        bound.options(&flags, machine).cpu().unwrap()
    };
    for target in [&["-march=i486"][..], &["-m32", "-march=i486"][..]] {
        let cpu = profile(target);
        assert_eq!(signed_multiply(cpu, 4, true).unwrap(), Some(Clocks { minimum: 13, maximum: 42 }));
        assert_eq!(signed_divide(cpu, 2).unwrap(), Some(Clocks { minimum: 27, maximum: 27 }));
    }
    let pentium = profile(&["-march=pentium"]);
    assert_eq!(signed_multiply(pentium, 2, true).unwrap(), Some(Clocks { minimum: 11, maximum: 11 }));
    assert_eq!(signed_multiply(pentium, 2, false).unwrap(), Some(Clocks { minimum: 10, maximum: 10 }));
    assert_eq!(signed_multiply(profile(&["-march=k6"]), 4, true).unwrap(), None);
}

/// LLVM IR for a target is found by its `target triple` and run through the
/// mid-end as well as the backend: `llrm-llc` once compiled the module as
/// written (no pipeline), leaving `40 + 2` and a dead stack slot in the code.
#[test]
fn llvm_ir_is_found_by_its_triple_and_optimized() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define dso_local i32 @main() {\n  %1 = alloca i32, align 4\n  store i32 0, ptr %1, align 4\n  \
                %2 = add i32 40, 2\n  ret i32 %2\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&[]), text, "main").unwrap();
    let listing = llrm_core::backend::masm::text(&built).unwrap();
    let body: Vec<&str> =
        listing.lines().map(str::trim).filter(|line| line.starts_with("mov") || *line == "ret").collect();
    assert_eq!(body, ["mov eax, 42", "ret"], "{listing}");
}

/// A call of a variadic function tells the callee in `al` how many vector
/// registers carry arguments: SysV AMD64's callee saves them by it. It was
/// never set, so `printf` read whatever the register held.
#[test]
fn a_variadic_call_sets_al_to_the_vector_registers_it_uses() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @printf(ptr, ...)\n\
                define i32 @f(ptr %s, i32 %a) {\n  %r = call i32 (ptr, ...) @printf(ptr %s, i32 %a)\n  ret i32 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&[]), text, "f").unwrap();
    let listing = llrm_core::backend::masm::text(&built).unwrap();
    let call = listing.lines().position(|line| line.trim() == "call printf").expect("the call");
    let before: Vec<&str> = listing.lines().take(call).map(str::trim).collect();
    assert!(
        before.iter().any(|line| *line == "mov eax, 0" || *line == "xor eax, eax" || *line == "mov al, 0"),
        "{listing}"
    );
}

/// A struct of two quadwords returns in rax and rdx and arrives so:
/// `insertvalue` and the `ret` and `call` of an aggregate had no selection ("a
/// { i64, i64 } value").
#[test]
fn a_struct_of_two_quadwords_returns_in_rax_and_rdx() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare { i64, i64 } @mk(i64, i64)\n\
                define { i64, i64 } @pair(i64 %a, i64 %b) {\n\
                  %x = insertvalue { i64, i64 } poison, i64 %a, 0\n  %y = insertvalue { i64, i64 } %x, i64 %b, 1\n  ret { i64, i64 } %y\n}\n\
                define i64 @sum(i64 %a, i64 %b) {\n  %r = call { i64, i64 } @mk(i64 %a, i64 %b)\n\
                  %p = extractvalue { i64, i64 } %r, 0\n  %q = extractvalue { i64, i64 } %r, 1\n  %s = add i64 %p, %q\n  ret i64 %s\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1"]), text, "pair").unwrap();
    let listing = llrm_core::backend::masm::text(&built).unwrap();
    let mut lines = listing.lines().map(str::trim);
    let pair: Vec<&str> =
        lines.by_ref().skip_while(|line| *line != "pair proc near").take_while(|line| *line != "pair endp").collect();
    assert!(pair.contains(&"mov rax, rdi") && pair.contains(&"mov rdx, rsi"), "{listing}");
    let sum: Vec<&str> = listing.lines().map(str::trim).skip_while(|line| *line != "sum proc near").collect();
    assert!(
        sum.contains(&"call mk") && sum.iter().any(|line| line.starts_with("add ") && line.contains("rdx")),
        "{listing}"
    );
}

/// A frame register given to a value is a callee-saved register like rbx: `fib`
/// kept its argument in rbp across the recursive call and never saved it (it
/// listed `(rbp, rbp)` where the others list a root), so its caller's frame was
/// garbage.
#[test]
fn a_freed_frame_register_holding_a_value_is_saved() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @fib(i32 %n) {\n\
                  %c = icmp slt i32 %n, 2\n  br i1 %c, label %done, label %rec\n\
                done:\n  ret i32 %n\n\
                rec:\n  %a = sub i32 %n, 1\n  %x = call i32 @fib(i32 %a)\n  %b = sub i32 %n, 2\n  %y = call i32 @fib(i32 %b)\n  %s = add i32 %x, %y\n  ret i32 %s\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1"]), text, "fib").unwrap();
    let listing = llrm_core::backend::masm::text(&built).unwrap();
    let body: Vec<&str> = listing.lines().map(str::trim).collect();
    let uses_rbp = body.iter().any(|line| line.contains("ebp") && !line.starts_with("push"));
    let saved = body.contains(&"push rbp") && body.contains(&"pop rbp");
    assert!(uses_rbp, "premise: the frame register holds a value\n{listing}");
    assert!(saved, "{listing}");
}

/// The pin's one-operand `llvm.lifetime.start.p0(ptr)` was a call MIR could not
/// read, so its alloca escaped and nothing was promoted: every local of a
/// clang-front-end program stayed in the frame (`loop.c` kept its loop and four
/// stores for a constant).
#[test]
fn an_alloca_with_the_pins_lifetime_markers_is_promoted() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.lifetime.start.p0(ptr captures(none))\n\
                declare void @llvm.lifetime.end.p0(ptr captures(none))\n\
                define i32 @f() {\n  %p = alloca i32, align 4\n  call void @llvm.lifetime.start.p0(ptr %p)\n  store i32 5, ptr %p, align 4\n  %v = load i32, ptr %p, align 4\n  call void @llvm.lifetime.end.p0(ptr %p)\n  ret i32 %v\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&[]), text, "f").unwrap();
    let listing = llrm_core::backend::masm::text(&built).unwrap();
    let body: Vec<&str> =
        listing.lines().map(str::trim).filter(|line| line.starts_with("mov") || *line == "ret").collect();
    assert_eq!(body, ["mov eax, 5", "ret"], "{listing}");
}
