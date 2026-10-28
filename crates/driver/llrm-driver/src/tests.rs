use llrm_codegen::driver::flags::Flags;

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
    use llrm_codegen::backend::classes::RegisterClasses;
    use llrm_codegen::model::ir::{Held, Loc, Mem, Operation, Semantics};

    let held = |value: u32| Loc::Held(Held { value, width: 2 });
    // `stosd` as real mode lowers it: a placeholder cell and the pointer after;
    // the value, the pointer, the selector.
    let store = Semantics {
        name: Some("stosd".into()),
        dests: vec![Loc::Mem(Mem::new(None, 0)), held(5)],
        sources: vec![held(1), held(2), held(3)],
        ..Semantics::new(Operation::Fill)
    };
    let real = RegisterClasses::of(&llrm_x86_m16::M16).requirements(&store);
    let flat = RegisterClasses::of(&llrm_x86_m32::M32).requirements(&store);
    assert_eq!(real.len(), 4, "{real:?}");
    assert!(flat.is_empty(), "{flat:?}");
}

/// A flat target's registers are its own, not real mode's: six values in the
/// pin's allocation order (`GR32`: ebx last, as the callee-saved one), ebp the
/// frame.
#[test]
fn a_targets_registers_come_from_its_description() {
    use llrm_codegen::backend::classes::RegisterClasses;
    use llrm_lir::registers::RegId;

    let flat = RegisterClasses::of(&llrm_x86_m32::M32);
    assert_eq!(flat.available, [RegId::EAX, RegId::ECX, RegId::EDX, RegId::ESI, RegId::EDI, RegId::EBX]);
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
    use llrm_codegen::backend::timing::{Clocks, signed_divide, signed_multiply};
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
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let body: Vec<&str> =
        listing.lines().map(str::trim).filter(|line| line.starts_with("mov") || *line == "ret").collect();
    assert_eq!(body, ["mov eax, 42", "ret"], "{listing}");
}

/// An add of a constant to a dword that stays live is a three-address `lea` at
/// m64 (`LEA64_32r`). It was off there, so each such add cost a `mov` and an
/// `add`.
#[test]
fn a_dword_add_that_keeps_its_source_is_a_lea_at_m64() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @g = global i32 0\n@h = global i32 0\n\
                define i32 @f() {\n  %a = load volatile i32, ptr @g\n  %r = add i32 %a, 5\n  store volatile i32 %r, ptr @h\n  %s = mul i32 %r, %a\n  ret i32 %s\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("lea e") && line.contains("+5]")), "{listing}");
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
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
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
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
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

/// A frame register given to a value is a callee-saved register like rbx: a
/// function with more values live across a call than rbx and r12-r15 hold kept
/// one in rbp and never saved it (it listed `(rbp, rbp)` where the others list
/// a root), so its caller's frame was garbage.
#[test]
fn a_freed_frame_register_holding_a_value_is_saved() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @g(i32)\n\
                define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e, i32 %h) {\n\
                  %x = call i32 @g(i32 %a)\n\
                  %1 = add i32 %x, %a\n  %2 = add i32 %1, %b\n  %3 = add i32 %2, %c\n  %4 = add i32 %3, %d\n  %5 = add i32 %4, %e\n  %6 = add i32 %5, %h\n  ret i32 %6\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
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
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let body: Vec<&str> =
        listing.lines().map(str::trim).filter(|line| line.starts_with("mov") || *line == "ret").collect();
    assert_eq!(body, ["mov eax, 5", "ret"], "{listing}");
}

/// A variadic definition saves the argument registers it does not name and
/// `va_start` fills the SysV list: "va_start with no parameter before the
/// variadic arguments" refused every `va_list` of x86-64.
#[test]
fn va_start_saves_the_unnamed_argument_registers_and_fills_the_list() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.va_start.p0(ptr)\n\
                declare void @llvm.va_end.p0(ptr)\n\
                define i32 @f(i32 %n, ...) {\n  %ap = alloca [24 x i8], align 8\n  call void @llvm.va_start.p0(ptr %ap)\n  %g = load i32, ptr %ap, align 8\n  call void @llvm.va_end.p0(ptr %ap)\n  ret i32 %g\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // One argument is named: rsi, rdx, rcx, r8 and r9 are saved; the list
    // starts with gp_offset 8.
    for register in ["rsi", "rdx", "rcx", "r8", "r9"] {
        assert!(
            listing.lines().any(|line| line.trim().starts_with("mov qword ptr") && line.trim().ends_with(register)),
            "{register}\n{listing}"
        );
    }
    let rdi_saved =
        listing.lines().any(|line| line.trim().starts_with("mov qword ptr") && line.trim().ends_with("rdi"));
    assert!(!rdi_saved, "the named argument is not saved\n{listing}");
}

/// An `alloca` whose element count is a value took a static slot of one
/// element: `alloca i32, i64 %n` was `lea rdi, [rsp+12]`, four bytes for `n` of
/// them. It lowers the stack pointer when it runs and keeps the frame register.
#[test]
fn an_alloca_with_a_variable_count_lowers_the_stack_pointer() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f(i64 %n) {\n  %p = alloca i32, i64 %n, align 16\n  call void @use(ptr %p)\n  ret void\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    assert!(lines.iter().any(|line| line.starts_with("sub rsp, r")), "the run-time size is subtracted\n{listing}");
    assert!(lines.contains(&"push rbp") && lines.contains(&"leave"), "the frame register is kept\n{listing}");
}

/// A constant count above one reserved one element's bytes.
#[test]
fn an_alloca_with_a_constant_count_reserves_all_its_elements() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f() {\n  %p = alloca i64, i32 8, align 8\n  call void @use(ptr %p)\n  ret void\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let reserved: i64 =
        listing.lines().map(str::trim).find_map(|line| line.strip_prefix("sub rsp, ")?.parse().ok()).expect("a frame");
    assert!(reserved >= 64, "{reserved} bytes for eight quadwords\n{listing}");
}

/// A `select` of a loaded value stopped the matcher at m64 ("node transform
/// `GetOppositeBranchCondition` is not translated"): the table folds the load
/// into `cmov` with the condition inverted (`inv_cond_XFORM`).
#[test]
fn a_select_of_a_loaded_value_is_a_cmov_from_memory() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(ptr %p, i64 %x, i64 %y, i64 %z) {\n  %a = load i64, ptr %p\n  %c = icmp slt i64 %x, %y\n  %r = select i1 %c, i64 %z, i64 %a\n  ret i64 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("cmovge") && line.contains('[')), "{listing}");
}

/// Setting a bit of an `i64` by a variable index stopped the emitter
/// (`Unmapped("BTS64rr")`): the x86-64 file had `bt` and `btc` but not `bts`,
/// which the table selects from `or` with a shifted bit.
#[test]
fn a_variable_bit_set_of_a_quadword_is_bts() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @set(i64 %x, i64 %n) {\n  %b = shl i64 1, %n\n  %r = or i64 %x, %b\n  ret i64 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), text, "set").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("bts")), "{listing}");
}

/// `x == 0xFFFFFFF7` on an `i64` stopped the encoder at `cmp rdi, 0xfffffff7`:
/// `foldImmediate` put a zero-extended 32-bit constant in a 64-bit immediate,
/// which the CPU sign-extends. It stays in a register.
#[test]
fn a_64_bit_constant_past_the_signed_32_bit_range_is_not_folded() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i1 @f(i64 %x) {\n  %r = icmp eq i64 %x, 4294967287\n  ret i1 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("4294967287") || listing.contains("mov"), "{listing}");
}

/// A `double` argument came in `xmm0` and went to `rax` and back
/// (`mov rax, xmm0; mov xmm0, rax; addsd xmm0, xmm1`): the value delivered in
/// an xmm register named no xmm operand, so the allocator took a
/// general-purpose register for it, which the object writer cannot encode.
#[test]
fn a_double_argument_stays_in_its_xmm_register() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define double @f(double %a, double %b) {\n  %r = fadd double %a, %b\n  ret double %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("rax"), "no general register holds a double\n{listing}");
}

/// A `double` returned by a call was popped from `st(0)`, which the SysV
/// convention does not use: the compile stopped "a floating value used as an
/// integer". It is read from `xmm0`, and a `double` returned from another
/// register is copied into `xmm0` first.
#[test]
fn a_double_result_travels_in_xmm0() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare double @g(double)\n\
                define double @f(double %a, double %b) {\n  %r = call double @g(double %b)\n  %s = fadd double %r, %a\n  ret double %s\n}\n\
                define double @second(double %a, double %b) {\n  ret double %b\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(
        !listing.lines().any(|line| ["fld", "fst", "fxch"].iter().any(|op| line.trim().starts_with(op))),
        "no x87 instruction\n{listing}"
    );
    let (built, _) = crate::llc::compiled(&flags(&["-O1", "--isel=dag"]), text, "second").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().ends_with("xmm0, xmm1")), "the result is copied to xmm0\n{listing}");
}

/// A `double` returned by a call and passed straight to the return went
/// `rax` -> `xmm0` (`mov rax, xmm0; mov xmm0, rax`): the copies around a value
/// pinned in `xmm0` are plain moves, which named no class, so the value between
/// them took a general register and the object writer refused it.
#[test]
fn a_double_copied_between_pinned_xmm_registers_stays_in_xmm() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare double @g(double)\n\
                define double @f(double %a) {\n  %r = call double @g(double %a)\n  ret double %r\n}\n";
    let (built, _) =
        crate::llc::compiled(&flags(&["-O1", "--isel=dag", "-fno-optimize-sibling-calls"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // The call's one-slot frame is `push rax` / `pop rcx`, as the pin's: only a
    // copy touching an xmm register counts.
    let through_rax = |line: &&str| line.contains("rax") && line.contains("xmm");
    assert!(!listing.lines().any(|line| through_rax(&line)), "no general register holds a double\n{listing}");
}

/// `fcmp` of two doubles at m64 selected the x87 compare (`fcom`), which the
/// emitter drops: `fcmp olt` was a lone `seta al` on whatever flags were live.
/// The CPU has CMOV, so the table's `UCOMISD` pattern selects it.
#[test]
fn a_double_compare_is_ucomisd() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i1 @f(double %a, double %b) {\n  %r = fcmp olt double %a, %b\n  ret i1 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("ucomisd")), "{listing}");
    assert!(
        !listing.lines().any(|line| line.trim().starts_with("fcom") || line.trim().starts_with("fucom")),
        "{listing}"
    );
}

/// A `select` was expanded into a branch diamond (`cmp; jl; mov; jmp`). It is
/// one `cmov` on the compare's condition, or on `test` for a plain `i1`.
#[test]
fn a_select_is_one_conditional_move() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @min(i32 %a, i32 %b) {\n  %c = icmp slt i32 %a, %b\n  %r = select i1 %c, i32 %a, i32 %b\n  ret i32 %r\n}\n\
                define i64 @pick(i1 %c, i64 %a, i64 %b) {\n  %r = select i1 %c, i64 %a, i64 %b\n  ret i64 %r\n}\n";
    for (name, cmov) in [("min", "cmovl"), ("pick", "cmovne")] {
        let (built, _) = crate::llc::compiled(&flags(&["-O2"]), text, name).unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(listing.lines().any(|line| line.trim().starts_with(cmov)), "{name}: {cmov}\n{listing}");
        assert!(!listing.lines().any(|line| line.trim().starts_with('j')), "{name}: no branch\n{listing}");
    }
}

/// The listing (`-S`, stage dumps) panicked "a pointer of 8 bytes" on a global
/// holding a pointer, so a program with one could not be dumped at x86-64.
#[test]
fn a_global_holding_a_pointer_lists_as_a_quadword() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @x = global i32 0\n@p = global ptr @x\n\
                define ptr @f() {\n  %v = load ptr, ptr @p\n  ret ptr %v\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("dq x"), "{listing}");
}

/// Every access to a global array made its own `lea reg,[rip+sym]` and `add
/// reg,index`: two stores were four instructions more than `llc`'s one `lea`
/// and `[base+index*4]` stores.
#[test]
fn accesses_to_one_global_share_its_address_and_take_the_index_in_the_address() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @a = global [64 x i32] zeroinitializer\n\
                define void @f(i64 %i) {\n  %p = getelementptr [64 x i32], ptr @a, i64 0, i64 %i\n  store i32 1, ptr %p\n  %q = getelementptr [64 x i32], ptr @a, i64 1, i64 %i\n  store i32 2, ptr %q\n  ret void\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let leas = lines.iter().filter(|line| line.ends_with("offset a")).count();
    let adds = lines.iter().filter(|line| line.starts_with("add")).count();
    assert_eq!((leas, adds), (1, 0), "{listing}");
}

/// The x86-64 timings still carried a 386's `imul` (10 clocks), so `x * 100`
/// was five shifts and adds where `llc` has one `imul r,r,100`.
#[test]
fn a_multiply_by_a_constant_is_one_imul_at_x86_64() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(i32 %x) {\n  %r = mul i32 %x, 100\n  ret i32 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("imul")), "{listing}");
    assert!(!listing.lines().any(|line| line.trim().starts_with("shl")), "{listing}");
}

/// A call was said to destroy the upper half of every callee-saved register
/// (`rbx` is not `ebx`, so the convention "kept only the pushed part"): values
/// live across it fought over `ebp`, and `f` spilled three of four to the
/// stack.
#[test]
fn values_live_across_a_call_take_the_callee_saved_registers() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @g(i32)\n\
                define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d) {\n  %1 = call i32 @g(i32 %a)\n  %2 = add i32 %1, %b\n  %3 = add i32 %2, %c\n  %4 = add i32 %3, %d\n  ret i32 %4\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("[rsp"), "no value goes to the stack\n{listing}");
}

/// A quadword zero stayed `mov rax, 0` (7 bytes): the zero idiom took only
/// words and dwords. It is `xor eax, eax`.
#[test]
fn a_quadword_zero_is_a_dword_xor() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i64 @g(i64)\n\
                define i64 @f() {\n  %r = call i64 @g(i64 0)\n  ret i64 0\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("xor edi, edi") && !listing.contains(", 0\n"), "{listing}");
}

/// x86-64 describes sixteen xmm registers in a class of their own: none of them
/// is a general register, and the 32-bit targets have none.
#[test]
fn xmm_registers_are_a_class_apart_from_the_general_ones() {
    use llrm_codegen::backend::classes::RegisterClasses;
    let m64 = RegisterClasses::of(&*crate::by_triple("x86_64-unknown-linux-gnu").unwrap());
    assert_eq!(m64.xmm.len(), 16);
    assert!(m64.xmm.iter().all(|one| !m64.available.contains(one)));
    assert_eq!(m64.available.len(), 14);
    assert!(RegisterClasses::of(&llrm_x86_m16::M16).xmm.is_empty());
}

/// An operand an SSE form fills is in an SSE register, whatever its width: with
/// no class for it a `double` would be offered the general registers. An
/// integer form's 16-byte operand is not. A target without xmm registers
/// confines nothing.
#[test]
fn an_operand_of_an_sse_form_is_confined_to_xmm_registers() {
    use llrm_codegen::backend::classes::RegisterClasses;
    use llrm_codegen::backend::regclass::{Confinement, Sets, confinement_of};
    use llrm_codegen::model::ir::{Held, Loc, Operation, Semantics};
    use llrm_codegen::model::lir::Insn;
    let held = |value, width| Loc::Held(Held { value, width });
    let add = |name: &str, width| {
        let what = Semantics {
            name: Some(name.into()),
            dests: vec![held(1, width)],
            sources: vec![held(1, width), held(2, width)],
            ..Semantics::new(Operation::Binary)
        };
        std::sync::Arc::new(Insn::new(1, None, Some(what), vec![1], vec![1, 2]))
    };
    let m64 = RegisterClasses::of(&*crate::by_triple("x86_64-unknown-linux-gnu").unwrap());
    let sets = Sets::of(&m64);
    let regs = m64.registers;
    let class = confinement_of(regs, std::iter::once(&*add("addsd", 16)), 2, &sets);
    assert!(matches!(class, Confinement::Only(only) if only == m64.xmm_set));
    let narrow = confinement_of(regs, std::iter::once(&*add("addsd", 8)), 2, &sets);
    assert!(matches!(narrow, Confinement::Only(only) if only == m64.xmm_set));
    let integer = confinement_of(regs, std::iter::once(&*add("add", 16)), 2, &sets);
    assert!(matches!(integer, Confinement::Free));
}

/// An OMF object from LLVM IR named `main` and `report` plain: the m32 start-up
/// calls `_main` and the OS layer defines `_report`, so the link failed with
/// both undefined. Names take the convention's spelling for the format.
#[test]
fn llvm_ir_symbols_are_decorated_as_the_object_format_spells_them() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @report(i32)\n\
                define i32 @main() {\n  call void @report(i32 7)\n  ret i32 0\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=sysv"]);
    let (built, _) = crate::llc::compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("_main") && listing.contains("_report"), "{listing}");
}

/// The i8086 clang writes Pascal functions as `cc232` (8 bits hold no 1000);
/// llrm-llc refused the module with "calling convention 232", so no QCport
/// module that calls qgl compiled.
#[test]
fn clangs_pascal_number_is_llrms_basic_convention() {
    let text = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                declare cc232 i16 @QGL(i16) addrspace(1)\n\
                define i16 @f() addrspace(1) {\n  %1 = call cc232 addrspace(1) i16 @QGL(i16 7)\n  ret i16 %1\n}\n",
    );
    crate::llc::compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
}

/// Inline-code intrinsics (`llrm.ia16.code.*`) were decorated to
/// `_llrm.ia16.code.*` and then "no assembler symbol": no QCport module with an
/// asm block compiled.
#[test]
fn inline_code_intrinsics_keep_their_names() {
    let text = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                declare i32 @\"llrm.ia16.code.89a60170c3.2p0\"(ptr)\n\
                define i16 @f() addrspace(1) {\n  %a = alloca i16\n  %r = call i32 @\"llrm.ia16.code.89a60170c3.2p0\"(ptr %a)\n  ret i16 0\n}\n",
    );
    crate::llc::compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
}

/// A global defined by clang IR was written as `_tbl label byte` with no
/// `public`, so a module that used it linked with "_mon_facts is an undefined
/// reference" (QCport).
#[test]
fn defined_globals_are_public_and_internal_ones_are_not() {
    let text = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                @a = dso_local global i16 5\n\
                @b = dso_local addrspace(1) constant i16 7\n\
                @c = internal global i16 9\n\
                define i16 @f() addrspace(1) {\n  ret i16 0\n}\n",
    );
    let (built, _) = crate::llc::compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(
        listing.contains("public _a") && listing.contains("public _b") && !listing.contains("public _c"),
        "{listing}"
    );
}

/// Borland's `outportb`/`inportb` are the library's
/// `__outportb__`/`__inportb__`; they were left as calls to a symbol
/// nothing defines ("___outportb__ is an undefined reference"), where llrm has
/// the `llrm.ia16.in`/`out` intrinsics.
#[test]
fn borlands_port_functions_are_the_port_intrinsics() {
    let text = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                declare cc1000 i8 @__outportb__(i16, i8) addrspace(1)\n\
                declare i8 @__inportb__(i16) addrspace(1)\n\
                define i8 @f(i8 %v) addrspace(1) {\n  %1 = call addrspace(1) i8 @__outportb__(i16 noundef 968, i8 noundef zeroext %v)\n  %2 = call addrspace(1) i8 @__inportb__(i16 noundef 969)\n  ret i8 %2\n}\n",
    );
    let (built, _) = crate::llc::compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(
        listing.contains("out dx") && listing.contains("in al, dx") && !listing.contains("__outportb__"),
        "{listing}"
    );
}

/// With `-mabi=watcom` a clang function (no stated convention) was still
/// spelled `_main`, so Open Watcom's start-up, which calls `main_`, could not
/// find it.
#[test]
fn the_chosen_abi_spells_clang_functions_without_a_convention() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @main() {\n  ret i32 0\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=watcom"]);
    let (built, _) = crate::llc::compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("public main_") && !listing.contains("_main"), "{listing}");
}

/// `-mabi=watcom` renamed clang's functions but still passed their arguments on
/// the stack, so a callback handed to Open Watcom's `qsort` (which calls it in
/// registers) was never sorted by.
#[test]
fn the_chosen_abi_passes_arguments_of_functions_without_a_convention() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare i32 @ext(i32, i32)\n\
                define i32 @main() {\n  %1 = call i32 @ext(i32 11, i32 22)\n  ret i32 %1\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=watcom"]);
    let (built, _) = crate::llc::compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(
        listing.contains("mov eax, 11") && listing.contains("mov edx, 22") && !listing.contains("pushd"),
        "{listing}"
    );
}

/// Under `-mabi=watcom` a variable was spelled `__iob_` like a function; Open
/// Watcom's data symbols take only the leading underscore (`__iob`), so
/// `stdout` was an undefined reference.
#[test]
fn variables_keep_the_leading_underscore_under_the_watcom_abi() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @counter = global i32 5\n\
                @__iob = external global i32\n\
                define i32 @main() {\n  %1 = load i32, ptr @__iob\n  ret i32 %1\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=watcom"]);
    let (built, _) = crate::llc::compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(
        listing.contains("public _counter") && listing.contains("__iob") && !listing.contains("__iob_"),
        "{listing}"
    );
}

/// An asm label (`\01NAME`, what `#pragma aux NAME "NAME"` becomes) was
/// decorated like any other symbol (`_NAME`), so a runtime routine spelled
/// `B$SINCOS` was an undefined reference.
#[test]
fn an_asm_label_is_the_symbol_as_written() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @\"\\01B$SINCOS\"()\n\
                define void @main() {\n  call void @\"\\01B$SINCOS\"()\n  ret void\n}\n";
    let flags = flags(&["-fobject-format=omf"]);
    let (built, _) = crate::llc::compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("B$SINCOS") && !listing.contains("_B$SINCOS"), "{listing}");
}

/// `section "NAME"` (`#pragma data_seg`, `#pragma code_seg`) stopped the import
/// ("sections are outside MIR's subset"), then would have put the object in
/// `_DATA` and the module's own code segment.
#[test]
fn a_section_names_the_segment_of_a_variable_and_a_function() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @in_xib = global i32 3, section \"XIB\", align 4\n\
                define i32 @hot() section \"HOT_TEXT\" {\n  %1 = load i32, ptr @in_xib\n  ret i32 %1\n}\n\
                define i32 @main() {\n  %1 = call i32 @hot()\n  ret i32 %1\n}\n";
    let flags = flags(&["-fobject-format=omf"]);
    let (built, _) = crate::llc::compiled(&flags, text, "main").unwrap();
    assert!(built.data.iter().any(|(name, items)| name == "XIB" && !items.is_empty()), "{:?}", built.data);
    assert_eq!(built.code_segments.values().collect::<Vec<_>>(), ["HOT_TEXT"], "{:?}", built.code_segments);
}

/// `@llvm.global_ctors` panicked the x86 backends (`no entry found for key`: an
/// intrinsic-named global has no symbol): each target now lays out the function
/// pointers, lowest priority first, in an init-function segment.
#[test]
fn global_ctors_become_init_function_pointers_at_x86_64_and_m32() {
    for (layout, triple) in [
        ("e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128", "x86_64-unknown-linux-gnu"),
        ("e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32", "i486-unknown-linux-gnu"),
    ] {
        let text = format!(
            "target datalayout = \"{layout}\"\ntarget triple = \"{triple}\"\n\
             @llvm.global_ctors = appending global [3 x {{ i32, ptr, ptr }}] [{{ i32, ptr, ptr }} {{ i32 200, ptr @late, ptr null }}, \
             {{ i32, ptr, ptr }} {{ i32 100, ptr @early, ptr null }}, {{ i32, ptr, ptr }} {{ i32 150, ptr null, ptr null }}]\n\
             define void @early() {{\n  ret void\n}}\ndefine internal void @late() {{\n  ret void\n}}\n"
        );
        let (built, _) =
            crate::llc::compiled(&flags(&[]), &text, "ctors").unwrap_or_else(|error| panic!("{triple}: {error}"));
        let (_, items) = built
            .data
            .iter()
            .find(|(name, _)| name == ".init_array")
            .unwrap_or_else(|| panic!("{triple}: {:?}", built.data));
        let names: Vec<&str> = items
            .iter()
            .filter_map(|item| match item {
                llrm_codegen::backend::masm::Datum::Pointer(pointer) => Some(pointer.name.trim_matches('_')),
                _ => None,
            })
            .collect();
        assert_eq!(names, ["early", "late"], "{triple}");
        let width = if triple.starts_with("x86_64") { 8 } else { 4 };
        assert!(
            matches!(items.first(), Some(llrm_codegen::backend::masm::Datum::Align(align)) if align.to == width),
            "{triple}: entries are {width}-byte aligned: {items:?}"
        );
    }
}

/// A 16-bit DOS start-up runs no constructor table: the list is refused, not
/// emitted for nothing.
#[test]
fn global_ctors_are_refused_at_m16() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
                @llvm.global_ctors = appending global [1 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 1, ptr @f, ptr null }]\n\
                define void @f() {\n  ret void\n}\n";
    let error = crate::llc::compiled(&flags(&["-m16"]), text, "ctors").expect_err("refused");
    assert!(error.contains("llvm.global_ctors") && error.contains("start-up"), "{error}");
}

/// `__builtin_constant_p(x)` of a parameter reached the backend as
/// `llvm.is.constant.i32` and was refused (`@f_: @llvm.is.constant.i16`): it is
/// false, and the branch on it is gone.
#[test]
fn is_constant_of_a_parameter_is_lowered_before_selection() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                declare i1 @llvm.is.constant.i32(i32)\n\
                define i32 @f(i32 %x) {\n  %c = call i1 @llvm.is.constant.i32(i32 %x)\n  %r = select i1 %c, i32 1, i32 2\n  ret i32 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-O0"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("is.constant"), "{listing}");
}

/// `floor(x)` of a C program reached the backend as `llvm.floor.f64` and was
/// refused (`@f_: @llvm.floor.f64`): x87 has no instruction for it, the pin
/// calls `floor`.
#[test]
fn a_floor_intrinsic_is_a_call_of_floor() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                declare double @llvm.floor.f64(double)\n\
                define double @f(double %x) {\n  %a = call double @llvm.floor.f64(double %x)\n  ret double %a\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m32"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // the DAG default, as the pin does, jumps to the libcall at a return
    assert!(
        listing
            .lines()
            .any(|line| ["call ", "jmp "].iter().any(|op| line.trim().starts_with(op)) && line.contains("floor")),
        "{listing}"
    );
}

/// `__builtin_prefetch` is `llvm.prefetch`; the 386/486 have no instruction for
/// it and `llrm-llc` refused the module ("@f_: @llvm.prefetch.p0"), so
/// `builtins_table.c` never compiled through `llrm-cc`. The hint compiles to
/// nothing.
#[test]
fn a_prefetch_hint_compiles_to_no_code() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @llvm.prefetch.p0(ptr, i32, i32, i32)\n\
                define void @f(ptr %p) {\n  call void @llvm.prefetch.p0(ptr %p, i32 0, i32 3, i32 1)\n  ret void\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m32"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("prefetch"), "{listing}");
}

/// `-Og` makes clang write `llvm.fake.use` (a debugger keeps a value live);
/// `llrm-llc` refused the module, "@f_: @llvm.fake.use", and `-Og` compiled
/// nothing through `llrm-cc`.
#[test]
fn a_fake_use_compiles_to_no_code() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @llvm.fake.use(...)\n\
                define i32 @f(i32 %a) {\n  %b = add i32 %a, 1\n  call void (...) @llvm.fake.use(i32 %b)\n  ret i32 %a\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m32"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("fake"), "{listing}");
}

/// A variable of zeros in LLVM IR (`@c = internal global i32 0`) went into
/// `_DATA` as `db 0,0,0,0`: every zero static stored its bytes in the image,
/// and `-mbss-begin` found no `_BSS` to open. It is uninitialised data, as
/// LLVM's `getKindForGlobal` makes it; a constant of zeros and a nonzero
/// initial value stay in `_DATA`.
#[test]
fn a_zero_static_in_llvm_ir_is_uninitialised_data() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @zero = internal global i32 0, align 4\n\
                @three = internal global i32 3, align 4\n\
                @ro = internal constant i32 0, align 4\n\
                define i32 @f() {\n  store volatile i32 1, ptr @zero\n  %a = load volatile i32, ptr @zero\n  store volatile i32 1, ptr @three\n  %b = load volatile i32, ptr @three\n  %c = load volatile i32, ptr @ro\n  %s = add i32 %a, %b\n  %t = add i32 %s, %c\n  ret i32 %t\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m32"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let bss = listing.split(".data?").nth(1).unwrap_or_default().split(".code").next().unwrap();
    assert!(bss.contains("_zero") && bss.contains("dup (?)"), "{listing}");
    assert!(!bss.contains("_three") && !bss.contains("_ro"), "{listing}");
    assert!(listing.split(".data?").next().unwrap().contains("_three"), "{listing}");
}

/// `__attribute__((weak))` left no trace in the listing, so a weak definition
/// linked as a strong one and could clash with, or hide, the strong one it was
/// written to yield to. It is refused, naming the symbol.
#[test]
fn a_weak_symbol_is_refused_where_the_object_format_cannot_say_so() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define weak i32 @hook() {\n  ret i32 1\n}\n\
                declare extern_weak i32 @maybe()\n";
    let refused = crate::llc::compiled(&flags(&["-m32"]), text, "w").map(|_| ()).unwrap_err();
    assert!(refused.contains("@hook") && refused.contains("weak"), "{refused}");
    let strong = text.replace("define weak", "define").replace("declare extern_weak", "declare");
    assert!(crate::llc::compiled(&flags(&["-m32"]), &strong, "w").is_ok());
}

/// A store into the video buffer (`*(char *)0xB8000 = 1`) was taken for a
/// write that may reach the program's own `g`, so `g` was read back after it
/// instead of forwarded: the target's foreign ranges reached only the other
/// frontends, not LLVM IR.
#[test]
fn a_store_to_video_memory_leaves_a_global_forwarded_in_llvm_ir() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @g = global i32 0, align 4\n\
                define i32 @f() {\n  store i32 5, ptr @g, align 4\n  store i8 1, ptr inttoptr (i32 753664 to ptr), align 1\n  %a = load i32, ptr @g, align 4\n  ret i32 %a\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("mov eax, dword ptr _g"), "{listing}");
    let ordinary = text.replace("753664", "4096");
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-O2"]), &ordinary, "f").unwrap();
    assert!(llrm_codegen::backend::masm::text(&built).unwrap().contains("mov eax, dword ptr _g"));
}

const REGCALL_DATALAYOUT: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                                  target triple = \"i486-unknown-linux-gnu\"\n";

/// `__cdecl` on a Watcom target was the register convention: clang's `ccc`
/// took the ABI's default, so `int __cdecl a(int, int)` read EAX/EDX and was
/// named `a_`. With `-mregcall-default` an unmarked (regcall) function and
/// `main` take the ABI's, a variadic one too, and `ccc` stays C's: `_a`, on the
/// stack.
#[test]
fn an_explicit_cdecl_stays_cdecl_when_regcall_is_the_default() {
    let text = format!(
        "{REGCALL_DATALAYOUT}\
         define i32 @a(i32 %x, i32 %y) {{\n  %s = add i32 %x, %y\n  ret i32 %s\n}}\n\
         define x86_regcallcc i32 @__regcall3__b(i32 %x, i32 %y) {{\n  %s = sub i32 %x, %y\n  ret i32 %s\n}}\n\
         define i32 @main() {{\n  ret i32 0\n}}\n\
         declare i32 @pr(ptr, ...)\n\
         define x86_regcallcc i32 @__regcall3__use() {{\n  %p = call i32 (ptr, ...) @pr(ptr null, i32 1)\n  ret i32 %p\n}}\n"
    );
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-mabi=watcom", "-mregcall-default"]), &text, "c").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let body =
        |name: &str| listing.split(&format!("{name} proc")).nth(1).unwrap_or_default().split("endp").next().unwrap();
    assert!(body("_a").contains("[esp+4]") || body("_a").contains("[ebp+8]"), "{listing}");
    for name in ["b_", "use_", "main_"] {
        assert!(listing.contains(&format!("{name} proc")), "{name}: {listing}");
    }
    assert!(listing.contains("call pr_"), "{listing}");
    assert!(!listing.contains("__regcall3__"), "{listing}");
}

/// clang keeps the C convention on the library functions it knows, so under
/// regcall `strlen` and `atan` were called as `_strlen` and `_atan` and the
/// Watcom library (`strlen_`, `atan_`) did not link; a user's own declaration
/// of another name stays what it says.
#[test]
fn a_known_library_function_is_called_in_the_default_convention() {
    let text = format!(
        "{REGCALL_DATALAYOUT}\
         declare i32 @strlen(ptr)\n\
         declare double @atan(double)\n\
         declare i32 @mine(i32)\n\
         define x86_regcallcc double @__regcall3__use(ptr %s, double %x) {{\n  \
         %n = call i32 @strlen(ptr %s)\n  %a = call double @atan(double %x)\n  %m = call i32 @mine(i32 %n)\n  \
         %f = sitofp i32 %m to double\n  %r = fadd double %a, %f\n  ret double %r\n}}\n"
    );
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-mabi=watcom", "-mregcall-default"]), &text, "c").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("call strlen_") && listing.contains("call atan_"), "{listing}");
    assert!(listing.contains("call _mine"), "{listing}");
}

/// A definition of a library function the pin knows (floatm.c's `sqrtf`) kept
/// C's convention while its calls took the default: `sqrtf_` was called and
/// `_sqrtf` defined, and the link failed with `sqrtf_` undefined.
#[test]
fn a_defined_library_function_takes_the_default_convention_too() {
    let text = format!(
        "{REGCALL_DATALAYOUT}\
         define float @sqrtf(float %x) {{\n  ret float %x\n}}\n\
         define x86_regcallcc float @__regcall3__use(float %x) {{\n  %r = call float @sqrtf(float %x)\n  ret float %r\n}}\n"
    );
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-mabi=watcom", "-mregcall-default"]), &text, "c").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("sqrtf_ proc"), "{listing}");
    assert!(!listing.contains("_sqrtf proc"), "{listing}");
}

/// `-m32 -gcodeview -fobject-format=elf` wrote an ELF object with no CodeView
/// in it and said nothing; a debug flavor the object format cannot carry is an
/// error that says so.
#[test]
fn a_debug_flavor_the_object_format_cannot_carry_is_refused() {
    let text = format!("{REGCALL_DATALAYOUT}define i32 @f(i32 %x) {{\n  ret i32 %x\n}}\n");
    let refused = |arguments: &[&str]| crate::llc::compiled(&flags(arguments), &text, "c").map(|_| ()).unwrap_err();
    assert!(refused(&["-m32", "-gcodeview", "-fobject-format=elf"]).contains("cannot carry CodeView"));
    assert!(refused(&["-m32", "-gtd", "-fobject-format=elf"]).contains("Turbo Debugger"));
    assert!(refused(&["-m32", "-gtd"]).contains("16-bit"));
    assert!(crate::llc::compiled(&flags(&["-m32", "-g", "-fobject-format=elf"]), &text, "c").is_ok());
}

/// `__stdcall` and `__fastcall` reached the backend as "calling convention
/// 64": no target here has them, and the message did not say which function or
/// what to write instead.
#[test]
fn stdcall_is_refused_by_name() {
    let text = format!("{REGCALL_DATALAYOUT}define x86_stdcallcc i32 @c(i32 %x) {{\n  ret i32 %x\n}}\n");
    let refused = crate::llc::compiled(&flags(&["-m32", "-mabi=watcom"]), &text, "c").map(|_| ()).unwrap_err();
    assert!(refused.contains("@c") && refused.contains("stdcall") && refused.contains("__cdecl"), "{refused}");
}

/// At m16 an unmarked function takes the register-size convention (`_f@3`,
/// callee clobbers CX and DX), as llrm-c's did, and `main` is Watcom's `main_`.
/// `-mabi=watcom` forced on the Watcom row made every function save BX, CX and
/// DX and call `main_`: the m16 -Os programs came out 16% larger.
#[test]
fn m16_defaults_to_the_register_size_convention_with_a_watcom_main() {
    let text = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                define i16 @f(i16 %x) {\n  %d = sdiv i16 %x, 7\n  ret i16 %d\n}\n\
                define i16 @main() {\n  ret i16 0\n}\n",
    );
    let (built, _) = crate::llc::compiled(&flags(&["-m16", "-Os"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("_f@3 proc") && listing.contains("main_ proc"), "{listing}");
    let body = listing.split("_f@3 proc").nth(1).unwrap().split("endp").next().unwrap();
    assert!(!body.contains("push cx") && !body.contains("push dx"), "{listing}");
}

/// At m16 the default is regparm3 (`_f@3`), but the Watcom library is the
/// register convention (`atan2_`, `strlen_`, `printf_`): clang leaves the
/// library functions it knows in C's convention and marks the headers'
/// `__watcall` ones `regcall`, so every program that called the library
/// linked against `_atan2@3`, which the library does not have.
#[test]
fn m16_library_calls_are_the_watcom_register_convention() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                declare double @atan2(double, double)\n\
                declare i16 @strlen(ptr)\n\
                declare i16 @printf(ptr, ...)\n\
                declare x86_regcallcc i16 @__regcall3__getch()\n\
                declare i16 @mine(i16)\n\
                declare double @llvm.ceil.f64(double)\n\
                define i16 @f(ptr %s) {\n  %n = call i16 @strlen(ptr %s)\n  %a = call double @atan2(double 1.0, double 2.0)\n  %c = call double @llvm.ceil.f64(double %a)\n  store double %c, ptr %s\n  \
                %p = call i16 (ptr, ...) @printf(ptr %s, i16 %n)\n  %g = call i16 @__regcall3__getch()\n  %m = call i16 @mine(i16 %g)\n  ret i16 %m\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m16"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    for name in ["atan2_", "strlen_", "printf_", "getch_", "ceil_"] {
        assert!(listing.contains(&format!("call {name}")), "{name}: {listing}");
    }
    // `mine` is in return position: the DAG default jumps to it, as the pin
    // does
    assert!(listing.contains("call _mine@3") || listing.contains("jmp _mine@3"), "{listing}");
}

/// A C `static` function at m16 stayed a far procedure (`call far ptr`,
/// `retf`) where llrm-c entered it by a near call: 3 bytes a call site, 1 a
/// return, the growth of the programs that call a static helper often. One
/// whose address is taken stays far.
#[test]
fn a_static_function_is_entered_by_a_near_call() {
    let text = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                @keep = global ptr addrspace(1) @taken\n\
                define internal i16 @h(i16 %x) addrspace(1) {\n  %y = mul i16 %x, 3\n  ret i16 %y\n}\n\
                define internal i16 @taken(i16 %x) addrspace(1) {\n  %y = mul i16 %x, 5\n  ret i16 %y\n}\n\
                define i16 @f(i16 %x) addrspace(1) {\n  %a = call addrspace(1) i16 @h(i16 %x)\n  %b = call addrspace(1) i16 @taken(i16 %a)\n  ret i16 %b\n}\n",
    );
    let (built, _) = crate::llc::compiled(&flags(&["-m16", "-O0"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("call _h@3\n"), "{listing}");
    assert!(listing.contains("call far ptr _taken@3"), "{listing}");
}

/// COFF spells `f` as `_f`: the debug info imported before the rename named a
/// symbol that no longer existed, no function was described, and the writer
/// refused "the line table's lines are in no function".
#[test]
fn a_clang_unit_with_decorated_symbols_keeps_its_functions_in_the_debug_info() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/dbg.ll")
        .lines()
        .map(|line| {
            if line.starts_with("target datalayout") {
                "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\""
                    .to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-g", "-fobject-format=coff"]), &text, "dbgfix").unwrap();
    assert!(!built.debug.expect("debug info").procedures.is_empty());
}

/// `llrm-llc` left the object's debug format at its default whatever `-g...`
/// asked: `-gtd` and `-gdwarf-2` at m16 OMF both wrote CodeView (class 0xA1),
/// and `-gtd` was never Borland's.
#[test]
fn the_debug_flavor_asked_for_reaches_the_object() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/dbg.ll")
        .lines()
        .map(|line| {
            if line.starts_with("target datalayout") {
                "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\""
                    .to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let asked = |flag: &str| {
        let (built, _) = crate::llc::compiled(&flags(&["-m32", flag, "-fobject-format=omf"]), &text, "dbgfix").unwrap();
        built.debug.expect("debug info").format
    };
    assert_eq!(asked("-gdwarf-2"), llrm_object::debug::Format::Dwarf { version: 2 });
    assert_eq!(asked("-gcodeview"), llrm_object::debug::Format::CodeView);
}

/// Clang's file MD5 reached a `-gdwarf-2` object and the writer refused it
/// ("a file checksum needs DWARF 5"): no m16 DWARF object came out at all.
#[test]
fn a_dwarf_2_object_from_clang_leaves_out_the_files_md5() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/dbg.ll")
        .lines()
        .map(|line| {
            if line.starts_with("target datalayout") {
                "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\""
                    .to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, format) =
        crate::llc::compiled(&flags(&["-m32", "-gdwarf-2", "-fobject-format=elf"]), &text, "dbgfix").unwrap();
    assert_eq!(built.debug.as_ref().expect("debug info").source.as_ref().map(|(_, md5)| md5.is_none()), Some(true));
    llrm_codegen::backend::objbuild::written_in(
        &built,
        "dbgfix.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
    )
    .unwrap();
}

/// A 64-bit variable kept in `esi:edx` is two register pieces, which CodeView
/// writes as subfields; the writer refused the unit ("a location list holds
/// only registers and frame cells") and no object came out.
#[test]
fn a_variable_in_two_registers_is_written_to_coff_as_subfields() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/dbg_pieces.ll")
        .lines()
        .map(|line| {
            if line.starts_with("target datalayout") {
                "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\""
                    .to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, format) =
        crate::llc::compiled(&flags(&["-m32", "-O2", "-g", "-fobject-format=coff"]), &text, "dbgpieces").unwrap();
    llrm_codegen::backend::objbuild::written_in(
        &built,
        "dbgpieces.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
    )
    .unwrap();
}

/// A `-g` object from clang's IR names clang's source, `dbg.c`, with its MD5,
/// not the `.ll` file: the debugger could not find the text.
#[test]
fn a_clang_units_source_file_reaches_the_debug_info() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/dbg.ll")
        .lines()
        .map(|line| {
            if line.starts_with("target datalayout") {
                "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\""
                    .to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-g"]), &text, "dbgfix").unwrap();
    let source = built.debug.expect("debug info").source;
    let md5 = source.as_ref().and_then(|(_, md5)| md5.as_ref()).map(|bytes| bytes.len());
    assert_eq!((source.map(|(name, _)| name), md5), (Some("dbg.c".to_owned()), Some(16)));
}

/// An array of `long double` at m32 was laid out 10 bytes apart (the store
/// size) while the code indexed it 12 apart (the alloc size): `table[1]` read
/// the middle of the first two elements, and long_double_layout.c printed
/// -2147483645 for 15.
#[test]
fn an_array_of_long_doubles_is_laid_out_at_the_alloc_size() {
    let text = format!(
        "{REGCALL_DATALAYOUT}@t = global [2 x x86_fp80] [x86_fp80 0xK3FFF8000000000000000, x86_fp80 0xK40008000000000000000], align 4\n\
         define i32 @main() {{\n  %v = load x86_fp80, ptr getelementptr inbounds (i8, ptr @t, i32 12), align 4\n  %i = fptosi x86_fp80 %v to i32\n  ret i32 %i\n}}\n"
    );
    let (built, _) = crate::llc::compiled(&flags(&["-m32"]), &text, "t").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let data: usize = listing
        .lines()
        .filter(|line| line.starts_with("db "))
        .map(|line| line.trim_start_matches("db ").split(',').count())
        .sum();
    assert_eq!(data, 24, "{listing}");
}

const SMALL_STRUCT_BY_VALUE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
    target triple = \"i386-unknown-unknown-code16\"\n\
    %struct.T = type { i8 }\n\
    declare i16 @wc_small(ptr byval(%struct.T) align 2, i16) addrspace(1)\n\
    define i16 @f() addrspace(1) {\n  %t = alloca %struct.T, align 1\n  store i8 5, ptr %t, align 1\n  \
    %r = call addrspace(1) i16 @wc_small(ptr byval(%struct.T) align 2 %t, i16 6)\n  ret i16 %r\n}\n\
    define i16 @g(ptr byval(%struct.T) align 2 %t, i16 %a) addrspace(1) {\n  %x = load i8, ptr %t, align 1\n  \
    %w = zext i8 %x to i16\n  %r = add i16 %w, %a\n  ret i16 %r\n}\n";

/// clang passes `struct { char x; }` `byval`, in memory; Open Watcom's register
/// convention takes it in AL with the next argument in DX. The call pushed it
/// and the callee read memory, so wc16_mixed.c's `wc_small(t, 6)` was 50, not
/// 56, against Watcom's own object.
#[test]
fn a_one_byte_struct_is_passed_in_a_register_under_the_watcom_convention() {
    let (built, _) = crate::llc::compiled(&flags(&["-mabi=watcom", "-m16"]), SMALL_STRUCT_BY_VALUE, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("mov al, 5") && listing.contains("mov dx, 6"), "{listing}");
    assert!(!listing.contains("sub sp") && !listing.contains("push 6") && !listing.contains("pushw 6"), "{listing}");
    assert!(listing.contains("movzx ax, al\n    add ax, dx"), "the callee reads AL and DX\n{listing}");
}

/// `-fno-inline=sum5` named the function as the source does, but the module
/// is decorated (`_sum5` under sysv, `sum5_` under watcom) before the flag is
/// looked up: "the program defines no function of that name", so
/// tail_calls_edges.c failed to compile under llrm-cc at every ABI.
#[test]
fn no_inline_takes_the_source_name_under_every_abi() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @sum5(i32 %a) {\n  %r = add i32 %a, 5\n  ret i32 %r\n}\n";
    for abi in ["sysv", "watcom"] {
        let flags = flags(&["-fobject-format=omf", &format!("-mabi={abi}"), "-fno-inline=sum5"]);
        crate::llc::compiled(&flags, text, "sum5").unwrap_or_else(|why| panic!("{abi}: {why}"));
    }
}

/// `-fsanitize=stack` reached llrm-llc from llrm-cc and did nothing for IR:
/// llrm-c marked the functions it compiled, an IR module's were never marked,
/// so `stackover.c` ran off the stack instead of ending in "Stack Overflow!".
#[test]
fn fsanitize_stack_makes_every_function_compare_sp_on_entry() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @deep(i32 %a) {\n  %r = add i32 %a, 5\n  ret i32 %r\n}\n";
    let listing = |extra: &[&str]| {
        let mut arguments = vec!["-fobject-format=omf", "-mabi=sysv"];
        arguments.extend(extra);
        let (built, _) = crate::llc::compiled(&flags(&arguments), text, "deep").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    assert!(!listing(&[]).contains("STKOVERFLOW"), "unchecked");
    assert!(listing(&["-fsanitize=stack"]).contains("STKOVERFLOW"), "checked");
}

/// An x86-64 target had no C runtime description, so no library routine: a
/// `memmove` whose direction no pass proved was refused under `--isel=dag`
/// ("memmove with no memmove in the runtime") and runtime/qb/dev/host/video.c
/// (`gd_move_rows`) did not compile. The pin calls libc's.
#[test]
fn an_unproven_memmove_is_a_call_at_m64() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @mv(ptr %d, ptr %s, i64 %n) {\n  \
                call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)\n  ret void\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m64", "--isel=dag"]), text, "mv").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("memmove"), "{listing}");
}

/// `text` with `@m16-datalayout@` replaced by the m16 target's datalayout
/// string (its `datalayout.toml`, stated once), kept for the life of the test
/// run.
fn m16_ir(text: &str) -> &'static str {
    Box::leak(
        text.replace(
            "@m16-datalayout@",
            &llrm_x86_m16::DATALAYOUT_TOML
                .lines()
                .find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"'))
                .expect("datalayout.toml states a datalayout"),
        )
        .into_boxed_str(),
    )
}

/// A 64-bit function whose frame is one slot (the alignment of its call)
/// adjusted the stack with `sub rsp, 8` / `add rsp, 8`, six bytes where the pin
/// has `push rax` / `pop rcx`.
#[test]
fn a_one_slot_frame_at_m64_is_push_and_pop() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @g()\n\
                define i32 @f() {\n  call void @g()\n  ret i32 0\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    assert!(lines.contains(&"push rax") && lines.contains(&"pop rcx"), "{listing}");
    assert!(!lines.iter().any(|line| line.starts_with("sub rsp") || line.starts_with("add rsp")), "{listing}");
}

/// Two byte stores of a value and of its high byte stopped the encoder at
/// m64 (`mov [bp+13] width 4, dil`): the high byte's store is `MOV8mr_NOREX`
/// and its cell was taken for a dword.
#[test]
fn a_store_of_the_high_byte_is_a_byte_store() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f(i32 %o) {\n  %h = alloca [7 x i8], align 1\n  %lo = trunc i32 %o to i8\n  %p3 = getelementptr i8, ptr %h, i64 3\n  store i8 %lo, ptr %p3\n  %s = lshr i32 %o, 8\n  %hi = trunc i32 %s to i8\n  %p4 = getelementptr i8, ptr %h, i64 4\n  store i8 %hi, ptr %p4\n  call void @use(ptr %h)\n  ret void\n}\n";
    crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), text, "f").unwrap();
}

/// Clearing a bit of an `i64` by a variable index was `mov rax,-1; btc rax,n;
/// and`: the pin folds `xor (shl 1, n), -1` to `rotl -2, n` (DAGCombiner),
/// which the table selects with the `and` as `btr`.
#[test]
fn a_variable_bit_clear_of_a_quadword_is_btr() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @clear(i64 %x, i64 %n) {\n  %b = shl i64 1, %n\n  %m = xor i64 %b, -1\n  %r = and i64 %x, %m\n  ret i64 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), text, "clear").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("btr")), "{listing}");
}

/// The pin pushes the callee-saved registers `ebp, ebx, edi, esi`
/// (`clang -target i386-unknown-linux-gnu -O2` on this function, with and
/// without `-fno-omit-frame-pointer`): the last of CSR_32 (`esi, edi, ebx,
/// ebp`) first. llrm pushed `esi, edi, ebp` here, the order of its own list
/// (`ebx, esi, edi, ebp`). The allocator takes no `ebx` for this function, so
/// the three it pushes are those of the pin's order.
#[test]
fn m32_callee_saved_registers_are_pushed_in_the_pins_order() {
    let text = "\
         target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
         target triple = \"i386-unknown-linux-gnu\"\n\
         declare i32 @g(i32)\n\
         define i32 @six(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e, i32 %f) {\n\
           %s1 = call i32 @g(i32 %a)\n\
           %s2 = call i32 @g(i32 %b)\n\
           %s3 = call i32 @g(i32 %c)\n\
           %s4 = call i32 @g(i32 %d)\n\
           %s5 = call i32 @g(i32 %e)\n\
           %s6 = call i32 @g(i32 %f)\n\
           %p1 = call i32 @g(i32 %s1)\n\
           %p2 = call i32 @g(i32 %s2)\n\
           %p3 = call i32 @g(i32 %s3)\n\
           %p4 = call i32 @g(i32 %s4)\n\
           %p5 = call i32 @g(i32 %s5)\n\
           %p6 = call i32 @g(i32 %s6)\n\
           %t0 = add i32 %s1, %p1\n\
           %x2 = add i32 %s2, %p2\n\
           %t1 = add i32 %t0, %x2\n\
           %x3 = add i32 %s3, %p3\n\
           %t2 = add i32 %t1, %x3\n\
           %x4 = add i32 %s4, %p4\n\
           %t3 = add i32 %t2, %x4\n\
           %x5 = add i32 %s5, %p5\n\
           %t4 = add i32 %t3, %x5\n\
           %x6 = add i32 %s6, %p6\n\
           %t5 = add i32 %t4, %x6\n\
           ret i32 %t5\n\
         }\n\
         ";
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-O2"]), text, "six").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let pushed: Vec<&str> = listing
        .lines()
        .map(str::trim)
        .filter(|line| matches!(*line, "push ebp" | "push ebx" | "push edi" | "push esi"))
        .collect();
    assert_eq!(pushed, ["push ebp", "push edi", "push esi"], "{listing}");
}

/// The `.eh_frame` of `text` (an x86-64 module) written as ELF.
fn eh_frame_of(text: &str) -> Vec<u8> {
    let (built, format) = crate::llc::compiled(&flags(&["-m64", "-O2"]), text, "f").unwrap();
    let bytes = llrm_codegen::backend::objbuild::written_in(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
    )
    .unwrap();
    let (u16_at, u32_at, u64_at) = (
        |at: usize| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize,
        |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize,
        |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize,
    );
    let (headers, count, names) = (u64_at(0x28), u16_at(0x3C), u16_at(0x3E));
    let header = |index: usize| headers + index * u16_at(0x3A);
    let strings = u64_at(header(names) + 0x18);
    (0..count)
        .map(header)
        .find(|&at| {
            let from = strings + u32_at(at);
            bytes[from..].starts_with(b".eh_frame\0")
        })
        .map(|at| bytes[u64_at(at + 0x18)..u64_at(at + 0x18) + u64_at(at + 0x20)].to_vec())
        .unwrap_or_default()
}

const SYSV: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                    target triple = \"x86_64-unknown-linux-gnu\"\n";

fn hex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
}

/// An x86-64 ELF object had no `.eh_frame`, so a debugger or an exception
/// could not unwind through its functions. The bytes are what the pin's clang
/// writes (`-target x86_64-linux-gnu -O1 -c`) for `push rax; call; pop; ret`
/// and for `push rbx; ...; pop rbx; ret`, where the code is the same length:
/// the offset of `rbx` is declared after the prologue.
#[test]
fn a_function_with_a_push_and_a_pushed_register_has_the_pins_eh_frame() {
    let one_slot = format!(
        "{SYSV}declare void @g()\ndefine void @f() \"disable-tail-calls\"=\"true\" {{\n  call void @g()\n  ret void\n}}\n"
    );
    assert_eq!(
        eh_frame_of(&one_slot),
        hex("1400000000000000017a5200017810011b0c070890010000140000001c000000000000000800000000410e10460e0800")
    );
    let saved = format!(
        "{SYSV}declare i32 @g()\ndefine i32 @f(i32 %a) {{\n  %r = call i32 @g()\n  %s = add i32 %r, %a\n  ret i32 %s\n}}\n"
    );
    assert_eq!(
        eh_frame_of(&saved),
        hex(
            "1400000000000000017a5200017810011b0c0708900100001c0000001c000000000000000c00000000410e1083024a0e0800000000000000"
        )
    );
}

/// With a frame register the address is rebased on it and goes back to `rsp`
/// at the `leave`: `def_cfa_register rbp` (0x0D 0x06) after the `mov`, and
/// `def_cfa rsp, 8` (0x0C 0x07 0x08) after it.
#[test]
fn a_frame_register_function_rebases_the_frame_address_on_it_and_back() {
    let text = format!(
        "{SYSV}declare void @g(ptr)\ndefine void @f(i32 %n) \"disable-tail-calls\"=\"true\" {{\n  %p = alloca i32, i32 %n\n  call void @g(ptr %p)\n  ret void\n}}\n"
    );
    let frame = eh_frame_of(&text);
    let has = |pattern: &[u8]| frame.windows(pattern.len()).any(|window| window == pattern);
    assert!(has(&[0x0D, 0x06]) && has(&[0x0C, 0x07, 0x08]) && has(&[0x86, 0x02]), "{frame:02x?}");
}

/// `tot += r(&slot)` in a loop: `liverangeshrink` took the call's result for a
/// value defined before the call's frame barrier and hoisted the add to the
/// top of the loop. The first pass added garbage and the last never added
/// (bq32 switch.bas -Os: "value is read but never defined").
#[test]
fn a_sum_with_a_call_result_stays_below_the_call() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                declare cc 1000 i32 @r(ptr)\n\
                define cc 1000 i32 @f(ptr %p) {\n\
                b1:\n  %slot = alloca i32\n  %n = load i16, ptr %p\n  %g = icmp slt i16 %n, 1\n  br i1 %g, label %b6, label %b5\n\
                b5:\n  %cnt = phi i16 [ %n, %b1 ], [ %cnt1, %b5 ]\n  %st = phi i32 [ %st1, %b5 ], [ 1, %b1 ]\n  %tot = phi i32 [ %tot1, %b5 ], [ 0, %b1 ]\n  %d = and i32 %st, 10\n  store i32 %d, ptr %slot\n  %c = call cc 1000 i32 @r(ptr nonnull %slot)\n  %tot1 = add i32 %c, %tot\n  %cnt1 = add i16 %cnt, -1\n  %st1 = add i32 %st, 77\n  %e = icmp eq i16 %cnt1, 0\n  br i1 %e, label %b6, label %b5\n\
                b6:\n  %res = phi i32 [ 0, %b1 ], [ %tot1, %b5 ]\n  ret i32 %res\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m32", "-march=i486", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let call = lines.iter().position(|line| line.starts_with("call")).expect("the call");
    let top = lines.iter().rposition(|line| line.ends_with(':') && lines[..call].contains(line)).unwrap_or(0);
    assert!(
        !lines[top..call].iter().any(|line| line.starts_with("add ") && line.ends_with(", eax")),
        "an add of the call's eax result sits above the call:\n{listing}"
    );
}

/// Constant stores to one array interleaved with variable stores to another
/// stayed word and dword stores: `GatherAllAliases` gave up after 6 chain hops
/// where the pin allows 18 (`TargetLoweringBase`), so the four constants were
/// not one quadword store (matmul +560 B at m64).
#[test]
fn constant_stores_interleaved_with_stores_to_another_object_are_one_quadword_store() {
    let mut text = String::from(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
         target triple = \"x86_64-unknown-linux-gnu\"\n\
         declare void @use(ptr, ptr)\n@g = global i16 0\n\
         define void @f() {\n  %x = load i16, ptr @g\n  %a = alloca [4 x i16], align 2\n  %b = alloca [4 x i16], align 2\n",
    );
    for (i, value) in [2, 1, 0, 1].iter().enumerate() {
        text += &format!(
            "  %a{i} = getelementptr inbounds [4 x i16], ptr %a, i64 0, i64 {i}\n  %b{i} = getelementptr inbounds [4 x i16], ptr %b, i64 0, i64 {i}\n  %x{i} = add i16 %x, {i}\n  store i16 %x{i}, ptr %a{i}\n  store i16 {value}, ptr %b{i}\n"
        );
    }
    text += "  call void @use(ptr %a, ptr %b)\n  ret void\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("mov qword ptr [rsp")), "{listing}");
}

/// The `.xdata` of an x86-64 module written as COFF, found by `.pdata`'s
/// layout: the frames the compiler declared (`.seh_*`) against the bytes read
/// back from the same code agree.
#[test]
fn the_declared_win64_prologue_is_what_the_code_bytes_say() {
    let text = format!(
        "{SYSV}declare i32 @g(i32)\ndefine i32 @f(i32 %a, i32 %b) \"disable-tail-calls\"=\"true\" {{\n  %x = call i32 @g(i32 %a)\n  %y = call i32 @g(i32 %b)\n  %s = add i32 %x, %y\n  %t = add i32 %s, %a\n  ret i32 %t\n}}\n"
    );
    let (built, _) = crate::llc::compiled(&flags(&["-m64", "-O2"]), &text, "f").unwrap();
    let object =
        llrm_codegen::backend::objbuild::built(&built, "f.ll", llrm_codegen::backend::objbuild::CodeLayout::OneSegment)
            .unwrap();
    let frame = object.frames.first().expect("a declared frame");
    assert!(!frame.seh.is_empty(), "no .seh_* directive reached the frame");
    let section = &object.sections[frame.section];
    let body = &section.image[frame.offset..frame.offset + frame.length];
    let declared = llrm_x86::mc::win64unwind::from_seh("f", &frame.seh).unwrap();
    let read = llrm_x86::mc::win64unwind::from_prologue("f", body).unwrap();
    assert_eq!(declared, read, "{:?}", frame.seh);
}
