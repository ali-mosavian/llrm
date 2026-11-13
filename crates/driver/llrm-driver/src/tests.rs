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

/// The CPU a compile is priced for with none asked is the target's: llrm-cc took
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

    let held = |value: u32| Loc::Held(Held { value, width: 2, offset: 0 });
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define dso_local i32 @main() {\n  %1 = alloca i32, align 4\n  store i32 0, ptr %1, align 4\n  \
                %2 = add i32 40, 2\n  ret i32 %2\n}\n";
    let (built, _) = compiled(&flags(&[]), text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let body: Vec<&str> =
        listing.lines().map(str::trim).filter(|line| line.starts_with("mov") || *line == "ret").collect();
    assert_eq!(body, ["mov eax, 42", "ret"], "{listing}");
}

/// `__builtin_parity(unsigned short)` at m32 -O1 was the whole popcount
/// expansion (21 instructions, 66 B) while m64 had `xor cl, dil; setnp`: the
/// m32 `expand` list expanded `ctpop` in MIR before selection, so the DAG
/// combiner never saw a CTPOP to fold to PARITY. The pin keeps it through the
/// combiner.
#[test]
fn the_parity_of_a_word_is_the_parity_flag_at_m32() {
    let text = "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i386-unknown-linux-gnu\"\n\
                declare i32 @llvm.ctpop.i32(i32)\n\
                define i32 @f(i16 zeroext %x) {\n  %z = zext i16 %x to i32\n  %c = call i32 @llvm.ctpop.i32(i32 %z)\n  %b = and i32 %c, 1\n  ret i32 %b\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-O1", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("setnp") && !listing.contains("sub "), "{listing}");
}

/// An add of a constant to a dword that stays live is a three-address `lea` at
/// m64 (`LEA64_32r`). It was off there, so each such add cost a `mov` and an
/// `add`.
#[test]
fn a_dword_add_that_keeps_its_source_is_a_lea_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @g = global i32 0\n@h = global i32 0\n\
                define i32 @f() {\n  %a = load volatile i32, ptr @g\n  %r = add i32 %a, 5\n  store volatile i32 %r, ptr @h\n  %s = mul i32 %r, %a\n  ret i32 %s\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("lea e") && line.contains("+5]")), "{listing}");
}

/// `sadd.sat(load a, load b)`: the flag add was cloned for the shift that reads
/// the sum, and the load of `b` stayed folded in one of them (`add rcx, [rsi]`)
/// so the cell was read twice and the copy of the sum took a `mov`
/// (sat_builtins m64 -O2, +32 B). The pin's scheduler unfolds the load first,
/// then clones: one load of each cell, a `lea` for the sum, the flag add on
/// registers.
#[test]
fn a_saturating_add_of_two_loads_reads_each_cell_once_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(ptr %a, ptr %b) {\n  %x = load i64, ptr %a\n  %y = load i64, ptr %b\n\
                \x20 %r = call i64 @llvm.sadd.sat.i64(i64 %x, i64 %y)\n  ret i64 %r\n}\n\
                declare i64 @llvm.sadd.sat.i64(i64, i64)\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert_eq!(listing.matches("[rsi]").count(), 1, "{listing}");
    assert!(listing.lines().any(|line| line.trim().starts_with("lea r") && line.contains("+")), "{listing}");
    assert!(!listing.lines().any(|line| line.trim().starts_with("add") && line.contains('[')), "{listing}");
}

/// A symbol's address passed to a call, read by a loop and passed to a call
/// again is made once for the first call and the loop, and again for the last
/// call, as the pin has it (`lea` into a callee-saved register, then a `lea`
/// into `rdi` at the end). It was made three times: CSE4 refused the share.
/// Since #6878 (cpu-model unroll preferences) the pin runtime-unrolls this
/// loop by 8: the address is made for the first call, the unrolled loop, the
/// remainder loop and the last call. Since #7955 (DEVIATE PIE64, exit
/// 2026-12-15) `@sym + reg` is not one address here, as it is to the pin, so
/// the unroll is by 4: 4 + 1 loads. Back to 9 when PIE64 goes.
#[test]
fn a_symbol_address_before_a_loop_is_shared_with_it_and_made_again_after_a_call_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @g = global [64 x i8] zeroinitializer\n\
                declare void @h(ptr)\n\
                define i32 @f(i64 %n) {\nentry:\n  call void @h(ptr @g)\n  br label %loop\n\
                loop:\n  %i = phi i64 [ 0, %entry ], [ %j, %loop ]\n  %s = phi i32 [ 0, %entry ], [ %t, %loop ]\n\
                  %p = getelementptr i8, ptr @g, i64 %i\n  %b = load volatile i8, ptr %p\n  %w = zext i8 %b to i32\n\
                  %t = add i32 %s, %w\n  %j = add i64 %i, 1\n  %c = icmp ult i64 %j, %n\n  br i1 %c, label %loop, label %done\n\
                done:\n  call void @h(ptr @g)\n  ret i32 %t\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // The first call's argument and the loop's address, then the last call's.
    let made = listing.lines().filter(|line| line.contains("offset g")).count();
    assert_eq!(made, 4, "{listing}");
    assert_eq!(listing.matches("movzx").count(), 5, "unrolled by 4 (DEVIATE PIE64) plus the remainder loop: {listing}");
}

/// A private callee called six times (`unpack`, three callers with two calls each) stayed out of line for the
/// by-size guard, but the -Os trial of the first callers' sites measured -22 B a copy (the objects grew +5), accepted,
/// and the guard of the last caller then saw two calls left and let its own through: all six inlined. gcc refuses at
/// -Os where the growth over every call left is above zero (`estimate_growth`). m64 -Os sfm16_float_mul +39 B over
/// the guard's verdict. DEVIATE OS-INLINE-GROWTH (gcc-bar only): the copies are 523 B against 553 B out of line here.
#[test]
fn a_callee_the_size_guard_keeps_out_of_line_is_not_inlined_by_the_trial_at_m64_os() {
    let text = include_str!("testdata/shared_unpack.ll");
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f1").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let calls = listing.lines().filter(|line| line.trim().starts_with("call unpack")).count();
    assert_eq!(calls, 6, "{listing}");
}

/// A symbol's address passed to four calls in one block was made four times
/// (`mov rdi, offset src` before each): the greedy input held one def per use,
/// where the pin's DAG selects the `LEA64r` once and its users share it, so the
/// allocator never saw a value worth a callee-saved register. memory_expansion
/// m64 -Os `main` was 920 bytes over the pin, 709 address defs to its 72.
#[test]
fn a_symbol_address_passed_to_calls_in_one_block_is_made_once_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @src = global [64 x i8] zeroinitializer\n\
                declare void @g(ptr)\n\
                define void @f() {\n  call void @g(ptr @src)\n  call void @g(ptr @src)\n  call void @g(ptr @src)\n\
                  call void @g(ptr @src)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let made = listing.lines().filter(|line| line.contains("offset src")).count();
    assert_eq!(made, 1, "{listing}");
}

/// The same symbol address hoisted twice out of a loop stayed two registers
/// (`mov r14, offset a+32; mov r15, offset a+32`), both callee-saved across the
/// calls: the pin's MachineLICM makes it once (`EliminateCSE`). scroll m64 -O2
/// was 56 bytes over the pin with five pushes instead of three. Here the loop
/// reads through the address, so hoisting it pays.
#[test]
fn a_symbol_address_hoisted_twice_out_of_a_loop_is_made_once_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @a = global [4096 x i8] zeroinitializer\n\
                declare void @h()\n\
                define i32 @f(i64 %n) {\nentry:\n  br label %loop\n\
                loop:\n  %i = phi i64 [ 0, %entry ], [ %j, %loop ]\n  %s = phi i32 [ 0, %entry ], [ %t, %loop ]\n\
                  %b1 = getelementptr i8, ptr @a, i64 160\n  %p1 = getelementptr i8, ptr %b1, i64 %i\n\
                  %v1 = load volatile i8, ptr %p1\n  call void @h()\n\
                  %b2 = getelementptr i8, ptr @a, i64 160\n  %p2 = getelementptr i8, ptr %b2, i64 %i\n\
                  %v2 = load volatile i8, ptr %p2\n  %w1 = zext i8 %v1 to i32\n  %w2 = zext i8 %v2 to i32\n\
                  %u = add i32 %w1, %w2\n  %t = add i32 %s, %u\n\
                  %j = add i64 %i, 1\n  %c = icmp ult i64 %j, %n\n  br i1 %c, label %loop, label %done\n\
                done:\n  ret i32 %t\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let made = listing.lines().filter(|line| line.contains("offset a")).count();
    assert_eq!(made, 1, "{listing}");
}

/// A symbol address that every use in a call-bearing loop copies into an
/// argument register is made once before the loop and held in a callee-saved
/// register, as the pin has it (r14/r15 beside the loop counter). It was made
/// at each call in the loop, by the fixed-register copy that remade a constant.
#[test]
fn a_symbol_address_only_passed_to_calls_in_a_loop_is_made_before_it_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @a = global [4096 x i8] zeroinitializer\n\
                declare void @h(ptr, ptr, i64)\n\
                define void @f(i64 %n) {\nentry:\n  br label %loop\n\
                loop:\n  %i = phi i64 [ 0, %entry ], [ %j, %loop ]\n  %b = getelementptr i8, ptr @a, i64 160\n\
                  call void @h(ptr @a, ptr %b, i64 3840)\n  call void @h(ptr %b, ptr @a, i64 3840)\n\
                  %j = add i64 %i, 1\n  %c = icmp ult i64 %j, %n\n  br i1 %c, label %loop, label %done\n\
                done:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let loop_start = listing.find("L0_2:").expect("the loop");
    let made = listing[loop_start..].lines().filter(|line| line.contains("offset a")).count();
    assert_eq!(made, 0, "{listing}");
    assert_eq!(listing[..loop_start].lines().filter(|line| line.contains("offset a")).count(), 2, "{listing}");
}

/// A constant used in a loop whose call keeps the caller-saved registers
/// (a local function's known clobbers) is held in one of them: no callee-saved
/// register, so hoisting costs nothing. div_constants_ulong m64 -Os made the
/// divisor in the loop, 32 bytes over release/next.
#[test]
fn a_constant_across_a_call_that_keeps_a_caller_saved_register_is_made_before_the_loop_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @s = internal global i64 1\n\
                define internal i64 @next() noinline {\n  %v = load i64, ptr @s\n  %w = add i64 %v, 7\n  store i64 %w, ptr @s\n  ret i64 %w\n}\n\
                define i64 @f(i64 %n) {\nentry:\n  br label %loop\n\
                loop:\n  %i = phi i64 [ 0, %entry ], [ %j, %loop ]\n  %acc = phi i64 [ 0, %entry ], [ %t, %loop ]\n\
                  %x = call i64 @next()\n  %q = udiv i64 %x, %n\n  %r = urem i64 %x, 3\n  %t0 = add i64 %acc, %q\n  %t = add i64 %t0, %r\n\
                  %j = add i64 %i, 1\n  %c = icmp ult i64 %j, %n\n  br i1 %c, label %loop, label %done\n\
                done:\n  ret i64 %t\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let loop_start = listing.find("L1_2:").expect("the loop");
    let made = listing[loop_start..].lines().filter(|line| line.trim_end().ends_with(", 3")).count();
    assert_eq!(made, 0, "{listing}");
}

/// A symbol address made before a call and again after it, once each, is made
/// once and kept in a callee-saved register across the call, as the pin does
/// (`lea r14, [rip + a]` before the call, no second one). CSE4 made it twice.
#[test]
fn a_symbol_address_made_before_and_after_a_call_is_made_once_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @a = global [4096 x i8] zeroinitializer\n\
                declare void @h()\n\
                define void @f(i64 %k, i1 %c) {\nentry:\n  %p = getelementptr i8, ptr @a, i64 %k\n  store volatile i8 0, ptr %p\n\
                  call void @h()\n  br i1 %c, label %then, label %done\n\
                then:\n  %q = getelementptr i8, ptr @a, i64 %k\n  store volatile i8 1, ptr %q\n  br label %done\n\
                done:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let made = listing.lines().filter(|line| line.contains("offset a")).count();
    assert_eq!(made, 1, "{listing}");
}

/// A `zeroext` argument compared for (in)equality with a constant is compared
/// in the register it arrived in: the pin's `combineSetCC` turns
/// `cmpeq(trunc(x), C)` into `cmpeq(x, C)` where the upper bits of `x` are
/// zero. sieve m64 -O2 had `test di, di` (a 66 prefix) where the pin has `test
/// edi, edi`.
#[test]
fn a_zeroext_argument_equality_is_compared_in_its_register_at_m64() {
    let compare = |ty: &str, param: &str, predicate: &str, constant: i64| {
        let text = format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             define i32 @f({ty} {param} %a) {{\nentry:\n  %c = icmp {predicate} {ty} %a, {constant}\n\
               br i1 %c, label %t, label %e\nt:\n  ret i32 1\ne:\n  ret i32 2\n}}\n"
        );
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    for (ty, narrow) in [("i16", "di"), ("i8", "dil")] {
        let listing = compare(ty, "zeroext", "eq", 5);
        assert!(listing.contains("cmp edi, 5"), "{ty}: {listing}");
        assert!(!listing.contains(&format!("cmp {narrow}, 5")), "{ty}: {listing}");
        // No assertion about the upper bits: the argument is not extended.
        let plain = compare(ty, "", "eq", 5);
        assert!(plain.contains(&format!("cmp {narrow}, 5")), "{ty} plain: {plain}");
    }
    // The pin leaves the unsigned orders at the narrow type.
    let listing = compare("i16", "zeroext", "ult", 5);
    assert!(listing.contains("cmp di, 5"), "{listing}");
}

/// A byte or word add that keeps its source live was a copy and a narrow add
/// at m64 (`mov r10d, edx; add r10b, 0xec`); the pin makes it a three-address
/// `lea r9d, [r8 - 20]` (`convertToThreeAddressWithLEA`, `LEA64_32r`).
/// copy_runs m64 -O2 had the extra instruction. Straight-line code: in a loop
/// the strength reduction turned the add into an induction variable and the
/// test lost its premise on release/next.
#[test]
fn a_byte_and_a_word_add_that_keep_their_source_are_leas_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define void @f(i8 %t, i16 %u, ptr %p, ptr %q) {\nentry:\n  %a = add i8 %t, -20\n  store volatile i8 %a, ptr %p\n\
                  %b = add i16 %u, 300\n  store volatile i16 %b, ptr %q\n\
                  %pt = getelementptr i8, ptr %p, i64 1\n  store volatile i8 %t, ptr %pt\n\
                  %qu = getelementptr i8, ptr %q, i64 2\n  store volatile i16 %u, ptr %qu\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("lea e") && line.contains("-20]")), "{listing}");
    assert!(listing.lines().any(|line| line.trim().starts_with("lea e") && line.contains("+300]")), "{listing}");
}

/// A `{ i64, i48 }` (a packed struct of 14 bytes) crossed the DAG selector as
/// an `i48`, which nothing legalized: torture 20131127-1 was refused at -m64,
/// "Unsupported CopyToReg Int(48) promote operand".
#[test]
fn a_struct_with_an_i48_member_is_returned_loaded_and_stored_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare { i64, i48 } @g()\n\
                define { i64, i48 } @get(ptr %p) {\n  %v = load { i64, i48 }, ptr %p\n  ret { i64, i48 } %v\n}\n\
                define void @put(ptr %p) {\n  %v = call { i64, i48 } @g()\n  store { i64, i48 } %v, ptr %p\n  ret void\n}\n";
    for function in ["get", "put"] {
        let made = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, function);
        assert!(made.is_ok(), "{function}: {:?}", made.err());
    }
}

/// The verifier read a 64-bit use of a register whose low word the entry
/// delivers as 64 bits unwritten: `foo(unsigned short)` at m64 was
/// "floatalloc: foo reads edi before every path writes it" under LLRM_VERIFY,
/// ~50 torture m64 builds. A register with some lane defined is read whole, and
/// "whole" is the register, not the view that was read.
#[test]
fn a_short_argument_read_as_a_quadword_is_no_undefined_read_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @foo(i16 zeroext %0) {\n\
                  %2 = alloca i16\n  %3 = alloca i16\n  store i16 %0, ptr %2\n  %4 = load i16, ptr %2\n  %5 = zext i16 %4 to i32\n\
                  %6 = icmp sgt i32 %5, 32767\n  br i1 %6, label %7, label %11\n7:\n  %8 = load i16, ptr %2\n  %9 = zext i16 %8 to i32\n\
                  %10 = sub nsw i32 %9, 32768\n  br label %12\n11:\n  br label %12\n12:\n  %13 = phi i32 [ %10, %7 ], [ 0, %11 ]\n\
                  %14 = trunc i32 %13 to i16\n  store i16 %14, ptr %3\n  %15 = load i16, ptr %3\n  %16 = zext i16 %15 to i32\n  ret i32 %16\n}\n";
    for isel in ["--isel=old", "--isel=dag"] {
        let made = compiled(&flags(&["-m64", "-O2", "-fverify-ir", isel]), text, "foo");
        assert!(made.is_ok(), "{isel}: {:?}", made.err());
    }
}

/// `(float)u` read `rdi` whole: the dword move that zeroes its upper half was
/// taken for a copy and coalesced away, so garbage above `edi` was converted.
#[test]
fn an_unsigned_dword_converted_at_m64_is_zero_extended_first() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define float @f(i32 %u) {\n  %r = uitofp i32 %u to float\n  ret float %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let convert = lines.iter().position(|line| line.starts_with("cvtsi2ss")).expect("the convert");
    assert!(lines[..convert].iter().any(|line| line.starts_with("movzx r")), "{listing}");
}

/// A `zeroext` argument arrives with its high bits clear, so `x / 10` of an
/// `unsigned short` multiplies the register as it is; ours `movzx`ed it first
/// (3 instructions where the pin has 2 at -m64). A `signext` one is its own
/// `movsx`.
#[test]
fn a_zeroext_or_signext_argument_is_not_extended_again_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i16 @d(i16 zeroext %x) {\n  %r = udiv i16 %x, 10\n  ret i16 %r\n}\n\
                define i32 @s(i8 signext %c) {\n  %r = sext i8 %c to i32\n  ret i32 %r\n}\n";
    for function in ["d", "s"] {
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, function).unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(!listing.contains("movzx") && !listing.contains("movsx"), "{function}: {listing}");
    }
}

/// `x >> (64 - n)` masked the count with 31, the dword's mask: a rotate by
/// 1..31 of a quadword came out wrong (`rotl64(x, 1)` gave `x` with the top bit
/// lost) at -m64, 32 of 63 counts.
#[test]
fn a_quadword_shift_by_64_minus_n_is_not_masked_to_31_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @rotl64(i64 %x, i32 %n) {\n  %a = zext nneg i32 %n to i64\n  %l = shl i64 %x, %a\n\
                  %s = sub nsw i32 64, %n\n  %b = zext nneg i32 %s to i64\n  %r = lshr i64 %x, %b\n  %o = or i64 %r, %l\n  ret i64 %o\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "rotl64").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("and cl, 31"), "{listing}");
}

/// `(x << n) | (x >> (64 - n))` is one `rol r, cl` in the pin; `MatchRotate`
/// took constant amounts only, so it stayed shl/neg/shr/or (8 instructions
/// against 4 at -m64).
#[test]
fn a_rotate_by_a_variable_amount_is_one_rotate_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @rotl64(i64 %x, i32 %n) {\n  %a = zext nneg i32 %n to i64\n  %l = shl i64 %x, %a\n\
                  %s = sub nsw i32 64, %n\n  %b = zext nneg i32 %s to i64\n  %r = lshr i64 %x, %b\n  %o = or i64 %r, %l\n  ret i64 %o\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "rotl64").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rol rax, cl"), "{listing}");
}

/// `x < 0 ? -x : x` is `neg; cmovs` in the pin (`LowerABS`, custom wherever
/// `canUseCMOV`). The DAG path was built with `cmov: false` at -m64 as well, so
/// it expanded to `sar; xor; sub`, one instruction more.
///
/// C's `abs` is `llvm.abs(x, true)` (ABS_MIN_POISON): it had no legalization
/// and failed with `CannotSelect("AbsMinPoison")`.
#[test]
fn an_abs_is_a_negate_and_a_cmov_at_m64() {
    for poison in ["false", "true"] {
        let text = format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             declare i32 @llvm.abs.i32(i32, i1)\n\
             define i32 @f(i32 %x) {{\n  %r = call i32 @llvm.abs.i32(i32 %x, i1 {poison})\n  ret i32 %r\n}}\n"
        );
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(listing.contains("neg ") && listing.contains("cmov") && !listing.contains("sar "), "{listing}");
    }
}

/// The pin's TwoAddress commutes a `cmov` (swapping its sources and taking the
/// opposite condition) so the result reuses the source that dies: `abs` is
/// `mov eax, edi; neg eax; cmovs eax, edi`. Only add/and/or/xor/imul commuted,
/// so `abs` was `mov; mov; neg; cmovns`, one copy more.
#[test]
fn a_cmov_commutes_so_abs_needs_one_copy() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @llvm.abs.i32(i32, i1)\n\
                define i32 @f(i32 %x) {\n  %r = call i32 @llvm.abs.i32(i32 %x, i1 true)\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let copies = listing.lines().filter(|line| line.trim_start().starts_with("mov ")).count();
    assert!(copies == 1 && listing.contains("cmovs"), "{listing}");
}

/// `add` overflow selecting 0 is `xor; add; cmovno`: the commute lets the
/// zero stay in the tied register. It was one `mov` more.
#[test]
fn an_overflow_select_needs_no_copy() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare {i32, i1} @llvm.sadd.with.overflow.i32(i32, i32)\n\
                define i32 @f(i32 %a, i32 %b) {\n  %p = call {i32, i1} @llvm.sadd.with.overflow.i32(i32 %a, i32 %b)\n\
                  %v = extractvalue {i32, i1} %p, 0\n  %o = extractvalue {i32, i1} %p, 1\n\
                  %r = select i1 %o, i32 0, i32 %v\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.lines().any(|line| line.trim_start().starts_with("mov ")), "{listing}");
}

/// A call of a variadic function tells the callee in `al` how many vector
/// registers carry arguments: SysV AMD64's callee saves them by it. It was
/// never set, so `printf` read whatever the register held.
#[test]
fn a_variadic_call_sets_al_to_the_vector_registers_it_uses() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @printf(ptr, ...)\n\
                define i32 @f(ptr %s, i32 %a) {\n  %r = call i32 (ptr, ...) @printf(ptr %s, i32 %a)\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&[]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // a sibling call, as the pin makes it, is a `jmp`
    let call = listing.lines().position(|line| matches!(line.trim(), "call printf" | "jmp printf")).expect("the call");
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare { i64, i64 } @mk(i64, i64)\n\
                define { i64, i64 } @pair(i64 %a, i64 %b) {\n\
                  %x = insertvalue { i64, i64 } poison, i64 %a, 0\n  %y = insertvalue { i64, i64 } %x, i64 %b, 1\n  ret { i64, i64 } %y\n}\n\
                define i64 @sum(i64 %a, i64 %b) {\n  %r = call { i64, i64 } @mk(i64 %a, i64 %b)\n\
                  %p = extractvalue { i64, i64 } %r, 0\n  %q = extractvalue { i64, i64 } %r, 1\n  %s = add i64 %p, %q\n  ret i64 %s\n}\n";
    let (built, _) = compiled(&flags(&["-O1"]), text, "pair").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @g(i32)\n\
                define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e, i32 %h) {\n\
                  %x = call i32 @g(i32 %a)\n\
                  %1 = add i32 %x, %a\n  %2 = add i32 %1, %b\n  %3 = add i32 %2, %c\n  %4 = add i32 %3, %d\n  %5 = add i32 %4, %e\n  %6 = add i32 %5, %h\n  ret i32 %6\n}\n";
    let (built, _) = compiled(&flags(&["-O1"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.lifetime.start.p0(ptr captures(none))\n\
                declare void @llvm.lifetime.end.p0(ptr captures(none))\n\
                define i32 @f() {\n  %p = alloca i32, align 4\n  call void @llvm.lifetime.start.p0(ptr %p)\n  store i32 5, ptr %p, align 4\n  %v = load i32, ptr %p, align 4\n  call void @llvm.lifetime.end.p0(ptr %p)\n  ret i32 %v\n}\n";
    let (built, _) = compiled(&flags(&[]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.va_start.p0(ptr)\n\
                declare void @llvm.va_end.p0(ptr)\n\
                define i32 @f(i32 %n, ...) {\n  %ap = alloca [24 x i8], align 8\n  call void @llvm.va_start.p0(ptr %ap)\n  %g = load i32, ptr %ap, align 8\n  call void @llvm.va_end.p0(ptr %ap)\n  ret i32 %g\n}\n";
    let (built, _) = compiled(&flags(&["-O1"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f(i64 %n) {\n  %p = alloca i32, i64 %n, align 16\n  call void @use(ptr %p)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    // the pin subtracts in a scratch register and moves the result to `rsp`
    assert!(lines.iter().any(|line| line.starts_with("sub r")), "the run-time size is subtracted\n{listing}");
    assert!(lines.iter().any(|line| line.starts_with("mov rsp, r")), "the stack pointer is lowered\n{listing}");
    // The pin takes the stack back through the frame register and a pop, not
    // `leave`.
    let at = lines.iter().position(|line| *line == "mov rsp, rbp");
    assert!(
        lines.contains(&"push rbp") && at.is_some_and(|at| lines[at + 1] == "pop rbp"),
        "the frame register is kept\n{listing}"
    );
    assert!(!lines.contains(&"leave"), "{listing}");
}

/// A constant count above one reserved one element's bytes.
#[test]
fn an_alloca_with_a_constant_count_reserves_all_its_elements() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f() {\n  %p = alloca i64, i32 8, align 8\n  call void @use(ptr %p)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let reserved: i64 =
        listing.lines().map(str::trim).find_map(|line| line.strip_prefix("sub rsp, ")?.parse().ok()).expect("a frame");
    assert!(reserved >= 64, "{reserved} bytes for eight quadwords\n{listing}");
}

/// A `select` of a loaded value stopped the matcher at m64 ("node transform
/// `GetOppositeBranchCondition` is not translated"): the table folds the load
/// into `cmov` with the condition inverted (`inv_cond_XFORM`). The pin's
/// X86CmovConversion then turns a `cmov` with a memory operand into a branch
/// over the load (`x86-cmov-converter-force-mem-operand` is on), outside a loop
/// too: `mov rax, rcx; cmp; jl; mov rax, [rdi]; ret`, as the pin's llc has it.
#[test]
fn a_select_of_a_loaded_value_is_a_branch_over_the_load() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(ptr %p, i64 %x, i64 %y, i64 %z) {\n  %a = load i64, ptr %p\n  %c = icmp slt i64 %x, %y\n  %r = select i1 %c, i64 %z, i64 %a\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    assert!(!lines.iter().any(|line| line.starts_with("cmov")), "{listing}");
    assert!(lines.iter().any(|line| line.starts_with("jl ")), "{listing}");
    assert!(lines.contains(&"mov rax, qword ptr [rdi]"), "{listing}");
}

/// `c ? 48 : 32` (a difference of 16) was two `mov`s and a `cmov`: the pin's
/// `combineSelectOfTwoConstants` is `zext(c) * 16 + 32`, a `setl`, a `shl` and
/// an `or` (word_compare_branches -Os m64: +37 B against the pin).
#[test]
fn a_select_of_two_constants_a_power_of_two_apart_is_a_shifted_setcc() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(i32 %x, i32 %y) {\n  %c = icmp slt i32 %x, %y\n  %r = select i1 %c, i32 48, i32 32\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("shl")), "{listing}");
    assert!(!listing.lines().any(|line| line.trim().starts_with("cmov")), "{listing}");
}

/// A compare folded a volatile load into its memory operand, and the flags made
/// again for a later reader re-emitted it: `vv[j]` read once in the source was
/// read three times (`cmp ecx, [rdx+rax*4]` each), word_compare_branches -Os.
/// The cells must be read exactly twice, once per volatile load.
#[test]
fn a_volatile_load_folded_into_a_compare_is_read_once() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @vv = global [16 x i32] zeroinitializer, align 16\n\
                define i32 @f(i32 %0, i32 %1) {\n\
                  %3 = sext i32 %0 to i64\n  %4 = getelementptr inbounds [4 x i8], ptr @vv, i64 %3\n\
                  %5 = load volatile i32, ptr %4, align 4\n  %6 = sext i32 %1 to i64\n\
                  %7 = getelementptr inbounds [4 x i8], ptr @vv, i64 %6\n  %8 = load volatile i32, ptr %7, align 4\n\
                  %9 = icmp ult i32 %5, %8\n  %10 = zext i1 %9 to i32\n  %11 = icmp ugt i32 %5, %8\n\
                  %12 = select i1 %11, i32 4, i32 2\n  %13 = or disjoint i32 %12, %10\n  %14 = or disjoint i32 %13, 8\n\
                  %15 = select i1 %9, i32 %13, i32 %14\n  %16 = icmp slt i32 %5, %8\n  %17 = icmp sgt i32 %5, %8\n\
                  %18 = select i1 %16, i32 48, i32 32\n  %19 = select i1 %17, i32 0, i32 %18\n\
                  %20 = or disjoint i32 %15, %19\n  %21 = or disjoint i32 %20, 64\n  %22 = select i1 %17, i32 %21, i32 %20\n\
                  %23 = add nuw nsw i32 %22, 128\n  %24 = select i1 %16, i32 %20, i32 %23\n  ret i32 %24\n}\n";
    let (built, _) = compiled(&flags(&["-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let reads = listing.lines().filter(|line| line.contains("ptr [")).count();
    assert_eq!(reads, 2, "{listing}");
}

/// Setting a bit of an `i64` by a variable index stopped the emitter
/// (`Unmapped("BTS64rr")`): the x86-64 file had `bt` and `btc` but not `bts`,
/// which the table selects from `or` with a shifted bit.
#[test]
fn a_variable_bit_set_of_a_quadword_is_bts() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @set(i64 %x, i64 %n) {\n  %b = shl i64 1, %n\n  %r = or i64 %x, %b\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-O2", "--isel=dag"]), text, "set").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("bts")), "{listing}");
}

/// `x == 0xFFFFFFF7` on an `i64` stopped the encoder at `cmp rdi, 0xfffffff7`:
/// `foldImmediate` put a zero-extended 32-bit constant in a 64-bit immediate,
/// which the CPU sign-extends. It stays in a register.
#[test]
fn a_64_bit_constant_past_the_signed_32_bit_range_is_not_folded() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i1 @f(i64 %x) {\n  %r = icmp eq i64 %x, 4294967287\n  ret i1 %r\n}\n";
    let (built, _) = compiled(&flags(&["-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("4294967287") || listing.contains("mov"), "{listing}");
}

/// A `double` argument came in `xmm0` and went to `rax` and back
/// (`mov rax, xmm0; mov xmm0, rax; addsd xmm0, xmm1`): the value delivered in
/// an xmm register named no xmm operand, so the allocator took a
/// general-purpose register for it, which the object writer cannot encode.
#[test]
fn a_double_argument_stays_in_its_xmm_register() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define double @f(double %a, double %b) {\n  %r = fadd double %a, %b\n  ret double %r\n}\n";
    let (built, _) = compiled(&flags(&["-O1", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("rax"), "no general register holds a double\n{listing}");
}

/// A `double` returned by a call was popped from `st(0)`, which the SysV
/// convention does not use: the compile stopped "a floating value used as an
/// integer". It is read from `xmm0`, and a `double` returned from another
/// register is copied into `xmm0` first.
#[test]
fn a_double_result_travels_in_xmm0() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare double @g(double)\n\
                define double @f(double %a, double %b) {\n  %r = call double @g(double %b)\n  %s = fadd double %r, %a\n  ret double %s\n}\n\
                define double @second(double %a, double %b) {\n  ret double %b\n}\n";
    let (built, _) = compiled(&flags(&["-O1", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(
        !listing.lines().any(|line| ["fld", "fst", "fxch"].iter().any(|op| line.trim().starts_with(op))),
        "no x87 instruction\n{listing}"
    );
    let (built, _) = compiled(&flags(&["-O1", "--isel=dag"]), text, "second").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().ends_with("xmm0, xmm1")), "the result is copied to xmm0\n{listing}");
}

/// A `double` returned by a call and passed straight to the return went
/// `rax` -> `xmm0` (`mov rax, xmm0; mov xmm0, rax`): the copies around a value
/// pinned in `xmm0` are plain moves, which named no class, so the value between
/// them took a general register and the object writer refused it.
#[test]
fn a_double_copied_between_pinned_xmm_registers_stays_in_xmm() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare double @g(double)\n\
                define double @f(double %a) {\n  %r = call double @g(double %a)\n  ret double %r\n}\n";
    let (built, _) = compiled(&flags(&["-O1", "--isel=dag", "-fno-optimize-sibling-calls"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i1 @f(double %a, double %b) {\n  %r = fcmp olt double %a, %b\n  ret i1 %r\n}\n";
    let (built, _) = compiled(&flags(&["-O1", "--isel=dag"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @min(i32 %a, i32 %b) {\n  %c = icmp slt i32 %a, %b\n  %r = select i1 %c, i32 %a, i32 %b\n  ret i32 %r\n}\n\
                define i64 @pick(i1 %c, i64 %a, i64 %b) {\n  %r = select i1 %c, i64 %a, i64 %b\n  ret i64 %r\n}\n";
    for (name, cmov) in [("min", "cmovl"), ("pick", "cmovne")] {
        let (built, _) = compiled(&flags(&["-O2"]), text, name).unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(listing.lines().any(|line| line.trim().starts_with(cmov)), "{name}: {cmov}\n{listing}");
        assert!(!listing.lines().any(|line| line.trim().starts_with('j')), "{name}: no branch\n{listing}");
    }
}

/// The listing (`-S`, stage dumps) panicked "a pointer of 8 bytes" on a global
/// holding a pointer, so a program with one could not be dumped at x86-64.
#[test]
fn a_global_holding_a_pointer_lists_as_a_quadword() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @x = global i32 0\n@p = global ptr @x\n\
                define ptr @f() {\n  %v = load ptr, ptr @p\n  ret ptr %v\n}\n";
    let (built, _) = compiled(&flags(&["-O1"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("dq x"), "{listing}");
}

/// Every access to a global array made its own `lea reg,[rip+sym]` and `add
/// reg,index`: two stores were four instructions more than `llc`'s one `lea`
/// and `[base+index*4]` stores.
#[test]
fn accesses_to_one_global_share_its_address_and_take_the_index_in_the_address() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @a = global [64 x i32] zeroinitializer\n\
                define void @f(i64 %i) {\n  %p = getelementptr [64 x i32], ptr @a, i64 0, i64 %i\n  store i32 1, ptr %p\n  %q = getelementptr [64 x i32], ptr @a, i64 1, i64 %i\n  store i32 2, ptr %q\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-O2"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(i32 %x) {\n  %r = mul i32 %x, 100\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-O2"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @g(i32)\n\
                define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d) {\n  %1 = call i32 @g(i32 %a)\n  %2 = add i32 %1, %b\n  %3 = add i32 %2, %c\n  %4 = add i32 %3, %d\n  ret i32 %4\n}\n";
    let (built, _) = compiled(&flags(&["-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("[rsp"), "no value goes to the stack\n{listing}");
}

/// A zero live across a call stayed in a callee-saved register under
/// `--isel=dag` (`xor ebx, ebx; mov rdi, rbx; call g; mov rax, rbx`): the
/// allocator did not rematerialize `MOV32r0` where the pin does, a `xor` at
/// each use (`isTriviallyReMaterializable`).
#[test]
fn a_zero_used_around_a_call_is_a_xor_at_each_use_under_the_dag_selector() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i64 @g(i64)\n\
                define i64 @f() {\n  %r = call i64 @g(i64 0)\n  ret i64 0\n}\n";
    let (built, _) = compiled(&flags(&["-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(
        listing.contains("xor edi, edi") && listing.contains("xor eax, eax") && !listing.contains("rbx"),
        "{listing}"
    );
}

/// A quadword zero stayed `mov rax, 0` (7 bytes): the zero idiom took only
/// words and dwords. It is `xor eax, eax`. Old selector: the DAG route keeps
/// one zero in `ebx` across the call where the pin rematerializes it.
#[test]
fn a_quadword_zero_is_a_dword_xor() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i64 @g(i64)\n\
                define i64 @f() {\n  %r = call i64 @g(i64 0)\n  ret i64 0\n}\n";
    let (built, _) = compiled(&flags(&["-O2", "--isel=old"]), text, "f").unwrap();
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
    let held = |value, width| Loc::Held(Held { value, width, offset: 0 });
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @report(i32)\n\
                define i32 @main() {\n  call void @report(i32 7)\n  ret i32 0\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=sysv"]);
    let (built, _) = compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("_main") && listing.contains("_report"), "{listing}");
}

/// A 700-byte constant copy at m32 -Os was a call of `memcpy`; the pin's llc
/// writes `rep movsb` at `minsize`, gcc -Os inlines it too (memory_expansion
/// 23405 B against 22295 and 22749): +1110 B, half of the 88 cells worse than
/// the pin at the same rules. DEVIATE REP-MOVS-OS.
#[test]
fn a_long_constant_copy_at_m32_os_is_rep_movsb() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
                define void @f(ptr align 1 %d, ptr align 1 %s) optsize {\n\
                \x20 call void @llvm.memcpy.p0.p0.i32(ptr align 1 %d, ptr align 1 %s, i32 700, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-mabi=sysv", "-Os"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rep movsb") && !listing.contains("call"), "{listing}");
}

/// `push imm8; pop r` is 5 clocks on the 486 against the `mov`'s 1, and the pin writes it at `minsize` only: at m32 -Os it
/// was also written in once-per-call blocks (`push 0x4d; pop edx` in x_histo2), +3..+9 clk in nine -Os cells and 244k pops
/// in jpeg_fixed, each of them over by that much with bytes under the bar. -Os and -O2 keep the `mov`; `minsize` pushes.
#[test]
fn a_small_immediate_at_m32_os_is_a_mov_and_at_minsize_a_push_pop() {
    let text = |attrs: &str, value: i32| {
        format!(
            "target datalayout = \"@m32-datalayout@\"\n\
             target triple = \"i486-unknown-linux-gnu\"\n\
             define i32 @f() {attrs} {{\n  ret i32 {value}\n}}\n"
        )
    };
    let listing = |flags_: &[&str], attrs: &str, value: i32| {
        let (built, _) = compiled(&flags(flags_), &text(attrs, value), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let at_os = listing(&["-mabi=sysv", "-Os"], "optsize", 40);
    assert!(at_os.contains("mov eax, 40") && !at_os.contains("push"), "{at_os}");
    let marked = listing(&["-mabi=sysv", "-O2"], "minsize optsize", 40);
    assert!(marked.contains("pushd 40") && marked.contains("pop eax"), "{marked}");
    let at_speed = listing(&["-mabi=sysv", "-O2"], "", 40);
    assert!(at_speed.contains("mov eax, 40") && !at_speed.contains("push"), "{at_speed}");
}

/// An unsigned int converted to long double stores a zero high dword (`fild qword`). The zero was made again on every trip
/// (`xor esi,esi` in the loop, and a saved register for it) where the pin holds it from the preheader: x_ld_mix -Os +52 clk.
#[test]
fn a_zero_stored_for_a_conversion_is_made_outside_the_loop_at_m32_os() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @f(i32 %n) optsize {\n\
                entry:\n  %c = icmp sgt i32 %n, 0\n  br i1 %c, label %loop, label %out\n\
                loop:\n  %i = phi i32 [ 0, %entry ], [ %i2, %loop ]\n  %j = phi i32 [ 0, %entry ], [ %j2, %loop ]\n\
                \x20 %s = phi x86_fp80 [ 0xK00000000000000000000, %entry ], [ %s2, %loop ]\n\
                \x20 %u = uitofp i32 %i to x86_fp80\n  %v = add i32 %j, -294967296\n  %w = uitofp i32 %v to x86_fp80\n\
                \x20 %t = fadd x86_fp80 %u, %w\n  %s2 = fadd x86_fp80 %s, %t\n  %i2 = add i32 %i, 4\n  %j2 = add i32 %j, 1\n\
                \x20 %d = icmp eq i32 %j2, %n\n  br i1 %d, label %out, label %loop\n\
                out:\n  %r = phi x86_fp80 [ 0xK00000000000000000000, %entry ], [ %s2, %loop ]\n\
                \x20 %k = fptosi x86_fp80 %r to i32\n  ret i32 %k\n}\n";
    let (built, _) = compiled(&flags(&["-mabi=sysv", "-Os"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let first = lines.iter().position(|line| line.starts_with("fild qword")).expect("a qword fild");
    let top = lines[..first].iter().rposition(|line| line.ends_with(':')).expect("the loop label");
    let back = top + lines[top..].iter().position(|line| line.starts_with("jne")).expect("the loop's branch");
    assert!(!lines[top..back].iter().any(|line| line.starts_with("xor")), "{listing}");
}

/// LEAF-FOLD-OS makes a leaf's constant in the branching block; what that does to the registers after it decides the
/// bytes, and x_bubble -Os grew 3 B (273 against 270), quicksort 2, switch_sparse 1, jpeg_fixed 16, while the sparse switch
/// shrank 2. The module is assembled both ways and the smaller kept. DEVIATE LEAF-FOLD-OS. The sparse switch keeps a join
/// (`xor` of the result): since #9572 its leaves are `mov; ret` (no `push; pop`), so a bare `ret` leaves nothing to fold.
#[test]
fn a_leaf_fold_that_grows_the_object_is_put_back_at_m32_os() {
    let bubble = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @a = internal global [200 x i32] zeroinitializer, align 4\n\
                define i32 @bench_x_bubble(i32 %0)  {\n\
                  %2 = icmp sgt i32 %0, 0\n\
                  br i1 %2, label %6, label %3\n\
                3:\n\
                  %4 = add i32 %0, -1\n\
                  %5 = icmp sgt i32 %0, 1\n\
                  br i1 %5, label %16, label %20\n\
                6:\n\
                  %7 = phi i32 [ %14, %6 ], [ 0, %1 ]\n\
                  %8 = phi i32 [ %10, %6 ], [ 12345, %1 ]\n\
                  %9 = mul i32 %8, 1103515245\n\
                  %10 = add i32 %9, 12345\n\
                  %11 = lshr i32 %10, 16\n\
                  %12 = and i32 %11, 32767\n\
                  %13 = getelementptr inbounds [4 x i8], ptr @a, i32 %7\n\
                  store i32 %12, ptr %13, align 4\n\
                  %14 = add i32 %7, 1\n\
                  %15 = icmp eq i32 %14, %0\n\
                  br i1 %15, label %3, label %6\n\
                16:\n\
                  %17 = phi i32 [ %35, %33 ], [ %4, %3 ]\n\
                  %18 = phi i32 [ %34, %33 ], [ 0, %3 ]\n\
                  %19 = icmp sgt i32 %4, %18\n\
                  br i1 %19, label %22, label %33\n\
                20:\n\
                  %21 = icmp sgt i32 %0, 0\n\
                  br i1 %21, label %37, label %46\n\
                22:\n\
                  %23 = phi i32 [ %26, %31 ], [ 0, %16 ]\n\
                  %24 = getelementptr inbounds [4 x i8], ptr @a, i32 %23\n\
                  %25 = load i32, ptr %24, align 4\n\
                  %26 = add i32 %23, 1\n\
                  %27 = getelementptr inbounds [4 x i8], ptr @a, i32 %26\n\
                  %28 = load i32, ptr %27, align 4\n\
                  %29 = icmp sgt i32 %25, %28\n\
                  br i1 %29, label %30, label %31\n\
                30:\n\
                  store i32 %28, ptr %24, align 4\n\
                  store i32 %25, ptr %27, align 4\n\
                  br label %31\n\
                31:\n\
                  %32 = icmp eq i32 %26, %17\n\
                  br i1 %32, label %33, label %22\n\
                33:\n\
                  %34 = add i32 %18, 1\n\
                  %35 = add i32 %17, -1\n\
                  %36 = icmp eq i32 %34, %4\n\
                  br i1 %36, label %20, label %16\n\
                37:\n\
                  %38 = phi i32 [ %43, %37 ], [ 0, %20 ]\n\
                  %39 = phi i32 [ %44, %37 ], [ 0, %20 ]\n\
                  %40 = mul i32 %38, 31\n\
                  %41 = getelementptr inbounds [4 x i8], ptr @a, i32 %39\n\
                  %42 = load i32, ptr %41, align 4\n\
                  %43 = add i32 %42, %40\n\
                  %44 = add i32 %39, 1\n\
                  %45 = icmp eq i32 %44, %0\n\
                  br i1 %45, label %46, label %37\n\
                46:\n\
                  %47 = phi i32 [ 0, %20 ], [ %43, %37 ]\n\
                  ret i32 %47\n\
                }\n";
    let sparse = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @f(i32 %0)  {\n\
                  switch i32 %0, label %6 [\n\
                    i32 7, label %7\n\
                    i32 99, label %2\n\
                    i32 1234, label %3\n\
                    i32 4001, label %4\n\
                    i32 77, label %5\n\
                  ]\n\
                2:\n\
                  br label %7\n\
                3:\n\
                  br label %7\n\
                4:\n\
                  br label %7\n\
                5:\n\
                  br label %7\n\
                6:\n\
                  br label %7\n\
                7:\n\
                  %8 = phi i32 [ 0, %6 ], [ 61, %5 ], [ 52, %2 ], [ 93, %3 ], [ 17, %4 ], [ 31, %1 ]\n\
                  %9 = xor i32 %8, %0\n\
                  ret i32 %9\n\
                }\n";
    let listing = |text: &str, name: &str, folded: bool| {
        let build = || compiled(&flags(&["-mabi=sysv", "-Os"]), text, name).unwrap();
        let (built, _) = if folded { build() } else { llrm_codegen::backend::leaffold::without(build) };
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let (kept, off) = (listing(bubble, "bench_x_bubble", true), listing(bubble, "bench_x_bubble", false));
    assert_eq!(kept, off, "the fold grew x_bubble and stays: {kept}");
    let (kept, off) = (listing(sparse, "f", true), listing(sparse, "f", false));
    assert_ne!(kept, off, "the fold that shrinks the sparse switch is kept: {kept}");
}

/// IPSCCP's sign-keeping widening (REC5-1) proves the counters that feed a recursive `a % b` non-negative, and the remainder
/// becomes unsigned: `xor edx, edx; div` against `cdq; idiv`, a byte more at each site (recgcd -Os 100 B, frames 110, recmany
/// 139, switch 210: 2..5 B over the pin's shape). -Os is bytes-strict, so it keeps the signed divide; -O2 keeps the proof.
/// DEVIATE REC5-1.
#[test]
fn a_remainder_of_counters_at_m32_os_stays_signed() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @f(i32 %n) {\n\
                entry:\n\
                %n.addr = alloca i32, align 4\n\
                %t = alloca i32, align 4\n\
                %i = alloca i32, align 4\n\
                store i32 %n, ptr %n.addr, align 4\n\
                store i32 0, ptr %t, align 4\n\
                store i32 1, ptr %i, align 4\n\
                br label %for.cond\n\
                for.cond:\n\
                %0 = load i32, ptr %i, align 4\n\
                %1 = load i32, ptr %n.addr, align 4\n\
                %cmp = icmp sle i32 %0, %1\n\
                br i1 %cmp, label %for.body, label %for.end\n\
                for.body:\n\
                %2 = load i32, ptr %i, align 4\n\
                %mul = mul nsw i32 %2, 7\n\
                %add = add nsw i32 %mul, 3\n\
                %3 = load i32, ptr %i, align 4\n\
                %mul1 = mul nsw i32 %3, 5\n\
                %add2 = add nsw i32 %mul1, 2\n\
                %call = call i32 @gcd(i32 %add, i32 %add2)\n\
                %4 = load i32, ptr %i, align 4\n\
                %add3 = add nsw i32 %4, 90\n\
                %call4 = call i32 @gcd(i32 %add3, i32 360)\n\
                %add5 = add nsw i32 %call, %call4\n\
                %5 = load i32, ptr %t, align 4\n\
                %add6 = add nsw i32 %5, %add5\n\
                store i32 %add6, ptr %t, align 4\n\
                br label %for.inc\n\
                for.inc:\n\
                %6 = load i32, ptr %i, align 4\n\
                %inc = add nsw i32 %6, 1\n\
                store i32 %inc, ptr %i, align 4\n\
                br label %for.cond\n\
                for.end:\n\
                %7 = load i32, ptr %t, align 4\n\
                ret i32 %7\n\
                }\n\
                define internal i32 @gcd(i32 %a, i32 %b) {\n\
                entry:\n\
                %retval = alloca i32, align 4\n\
                %a.addr = alloca i32, align 4\n\
                %b.addr = alloca i32, align 4\n\
                store i32 %a, ptr %a.addr, align 4\n\
                store i32 %b, ptr %b.addr, align 4\n\
                %0 = load i32, ptr %b.addr, align 4\n\
                %cmp = icmp eq i32 %0, 0\n\
                br i1 %cmp, label %if.then, label %if.end\n\
                if.then:\n\
                %1 = load i32, ptr %a.addr, align 4\n\
                store i32 %1, ptr %retval, align 4\n\
                br label %return\n\
                if.end:\n\
                %2 = load i32, ptr %b.addr, align 4\n\
                %3 = load i32, ptr %a.addr, align 4\n\
                %4 = load i32, ptr %b.addr, align 4\n\
                %rem = srem i32 %3, %4\n\
                %call = call i32 @gcd(i32 %2, i32 %rem)\n\
                store i32 %call, ptr %retval, align 4\n\
                br label %return\n\
                return:\n\
                %5 = load i32, ptr %retval, align 4\n\
                ret i32 %5\n\
                }\n";
    let listing = |level: &str| {
        let (built, _) = compiled(&flags(&["-mabi=sysv", level]), text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let os = listing("-Os");
    assert!(os.contains("idiv") && !os.contains("\tdiv"), "-Os: {os}");
    let speed = listing("-O2");
    assert!(speed.contains("div") && !speed.contains("idiv"), "-O2: {speed}");
}

/// GVN's load PRE put a load of `*p` into the call path so the join's own load became a phi: a `mov` in the path and
/// an `add reg, reg` where gcc and the unpre'd code fold the load into the `add` (x_jpeg_fixed -Os +28 B, lru +28 B, x_bubble
/// -7.9% clk once off). At m32 -Os the load stays in the join, folded; at -O2 PRE still runs. DEVIATE GVN-PRE-OPTSIZE.
#[test]
fn load_pre_is_off_at_m32_os() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @g()\n\
                define i32 @f(ptr %p, i32 %c) ATTR {\n\
                entry:\n  %t = icmp ne i32 %c, 0\n  br i1 %t, label %then, label %else\n\
                then:\n  %a = load i32, ptr %p\n  br label %join\n\
                else:\n  call void @g()\n  br label %join\n\
                join:\n  %x = phi i32 [ %a, %then ], [ 0, %else ]\n  %b = load i32, ptr %p\n  %s = add i32 %x, %b\n  ret i32 %s\n}\n";
    let listing = |level: &str, attr: &str| {
        let (built, _) = compiled(&flags(&["-mabi=sysv", level]), &text.replace("ATTR", attr), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let folded = |text: &str| text.lines().any(|line| line.trim_start().starts_with("add ") && line.contains("ptr ["));
    let os = listing("-Os", "optsize");
    assert!(folded(&os), "-Os keeps the join's load, folded into the add: {os}");
    let speed = listing("-O2", "");
    assert!(!folded(&speed), "-O2 still PREs the load: {speed}");
}

/// As `load_pre_is_off_at_m32_os`, at m64: GVN's load PRE made the join's own load a phi, a `mov` in the call path and an
/// `add reg, reg` for the `add reg, [p]` (jpeg_fixed -Os +17 B, jpeg_single +15 B). DEVIATE GVN-PRE-OPTSIZE.
#[test]
fn load_pre_is_off_at_m64_os() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @g()\n\
                define i32 @f(ptr %p, i32 %c) ATTR {\n\
                entry:\n  %t = icmp ne i32 %c, 0\n  br i1 %t, label %then, label %else\n\
                then:\n  %a = load i32, ptr %p\n  br label %join\n\
                else:\n  call void @g()\n  br label %join\n\
                join:\n  %x = phi i32 [ %a, %then ], [ 0, %else ]\n  %b = load i32, ptr %p\n  %s = add i32 %x, %b\n  ret i32 %s\n}\n";
    let listing = |level: &str, attr: &str| {
        let (built, _) = compiled(&flags(&["-m64", level, "--isel=dag"]), &text.replace("ATTR", attr), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let folded = |text: &str| text.lines().any(|line| line.trim_start().starts_with("add ") && line.contains("ptr ["));
    let os = listing("-Os", "optsize");
    assert!(folded(&os), "-Os keeps the join's load, folded into the add: {os}");
    let speed = listing("-O2", "");
    assert!(!folded(&speed), "-O2 still PREs the load: {speed}");
}

/// RS1 leaves a loop whose copied guard does not fold and whose trips are not counted unrotated at -Os, and each trip pays the taken
/// `jmp` back to the guard (3 clocks on the 486). A walk of a list is that loop; rotated it is one branch back at 4 B.
/// ROTATION-HOT compiles it rotated too and keeps that where the finished code runs less for a few bytes a loop.
/// DEVIATE ROTATION-HOT.
#[test]
fn a_hot_loop_of_an_unfolded_guard_is_rotated_at_m32_os() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @f(ptr %p, i32 %k) {\n\
                entry:\n  br label %head\n\
                head:\n  %q = phi ptr [ %p, %entry ], [ %n, %body ]\n  %s = phi i32 [ 0, %entry ], [ %s2, %body ]\n\
                \x20 %v = load i32, ptr %q, align 4\n  %z = icmp eq i32 %v, %k\n  br i1 %z, label %out, label %body\n\
                body:\n  %m = mul i32 %v, 3\n  %s2 = add i32 %s, %m\n  %n = getelementptr i32, ptr %q, i32 1\n  br label %head\n\
                out:\n  ret i32 %s\n}\n";
    let (built, _) = compiled(&flags(&["-mabi=sysv", "-Os"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("jmp"), "the loop is rotated, no jmp back to its guard: {listing}");
}

/// The pin leaves the 3x3 window of x_life a loop nest at -Os (`OptSizeThreshold` 0); gcc peels it where the code does not grow. The loops
/// the threshold refused are unrolled in a second compile, and that is kept when the object is smaller (x_life -Os 360 -> 347 B, -55% clk).
/// DEVIATE UNROLL-OS-TRIAL.
#[test]
fn a_small_window_loop_is_unrolled_at_m32_os_when_the_object_shrinks() {
    let text = include_str!("testdata/x_life.ll");
    let labels = |built: &llrm_codegen::backend::masm::Module| {
        llrm_codegen::backend::masm::text(built).unwrap().lines().filter(|line| line.ends_with(':')).count()
    };
    let (built, _) = compiled(&flags(&["-mabi=sysv", "-Os"]), text, "bench_x_life").unwrap();
    let (rolled, _) = llrm_codegen::driver::trial::without_size_unroll(|| compiled(&flags(&["-mabi=sysv", "-Os"]), text, "bench_x_life")).unwrap();
    assert!(labels(&built) < labels(&rolled), "{} labels against {}", labels(&built), labels(&rolled));
}

/// A signed divide by 1024 is `idiv` at m32 -Oz and in a `minsize` function only (the pin's `isIntDivCheap`); at -Os it is the
/// `sar; shr; add; sar` sequence. The tip kept `idiv` at -Os: stft_fixed -Os ran 188.9M clk against main's 96.8M (+95%).
#[test]
fn a_signed_divide_by_a_power_of_two_is_idiv_at_m32_minsize_only() {
    let text = |attrs: &str| {
        format!(
            "target datalayout = \"@m32-datalayout@\"\n\
             target triple = \"i486-unknown-linux-gnu\"\n\
             define i32 @f(i32 %x) {attrs} {{\n  %q = sdiv i32 %x, 1024\n  ret i32 %q\n}}\n"
        )
    };
    let listing = |flags_: &[&str], attrs: &str| {
        let (built, _) = compiled(&flags(flags_), &text(attrs), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let at_os = listing(&["-mabi=sysv", "-Os"], "optsize");
    assert!(!at_os.contains("idiv") && at_os.contains("sar"), "-Os: {at_os}");
    let marked = listing(&["-mabi=sysv", "-O2"], "minsize optsize");
    assert!(marked.contains("idiv"), "minsize function: {marked}");
    let at_speed = listing(&["-mabi=sysv", "-O2"], "");
    assert!(!at_speed.contains("idiv") && at_speed.contains("sar"), "-O2: {at_speed}");
}

/// An unsigned 64-bit integer to `f32` or `f64` at m64 was `mov [rsp-16], rdi; fild; fadd [fudge + 4*sign]; fstp; movsd`: through the
/// x87 and the stack, 38 B and a store-forward stall for `f64`, where the pin has an SSE convert of the halved value. DEVIATE
/// UITOFP-F64: `cvtsi2sd` of the value, or of the halved one doubled.
#[test]
fn an_unsigned_64_bit_integer_to_float_is_an_sse_convert_at_m64() {
    for (ty, convert) in [("double", "cvtsi2sd"), ("float", "cvtsi2ss")] {
        let text = format!(
            "target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\
             define {ty} @f(i64 %x) {{\n  %r = uitofp i64 %x to {ty}\n  ret {ty} %r\n}}\n"
        );
        let (built, _) = compiled(&flags(&["-m64", "-O1"]), &text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(!listing.contains("fild") && listing.contains(convert), "{ty}: {listing}");
    }
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
    compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
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
    compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
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
    let (built, _) = compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
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
    let (built, _) = compiled(&flags(&["-mabi=cdecl"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @main() {\n  ret i32 0\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=watcom"]);
    let (built, _) = compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("public main_") && !listing.contains("_main"), "{listing}");
}

/// `-mabi=watcom` renamed clang's functions but still passed their arguments on
/// the stack, so a callback handed to Open Watcom's `qsort` (which calls it in
/// registers) was never sorted by.
#[test]
fn the_chosen_abi_passes_arguments_of_functions_without_a_convention() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare i32 @ext(i32, i32)\n\
                define i32 @main() {\n  %1 = call i32 @ext(i32 11, i32 22)\n  ret i32 %1\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=watcom"]);
    let (built, _) = compiled(&flags, text, "main").unwrap();
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @counter = global i32 5\n\
                @__iob = external global i32\n\
                define i32 @main() {\n  %1 = load i32, ptr @__iob\n  ret i32 %1\n}\n";
    let flags = flags(&["-fobject-format=omf", "-mabi=watcom"]);
    let (built, _) = compiled(&flags, text, "main").unwrap();
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @\"\\01B$SINCOS\"()\n\
                define void @main() {\n  call void @\"\\01B$SINCOS\"()\n  ret void\n}\n";
    let flags = flags(&["-fobject-format=omf"]);
    let (built, _) = compiled(&flags, text, "main").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("B$SINCOS") && !listing.contains("_B$SINCOS"), "{listing}");
}

/// `section "NAME"` (`#pragma data_seg`, `#pragma code_seg`) stopped the import
/// ("sections are outside MIR's subset"), then would have put the object in
/// `_DATA` and the module's own code segment.
#[test]
fn a_section_names_the_segment_of_a_variable_and_a_function() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @in_xib = global i32 3, section \"XIB\", align 4\n\
                define i32 @hot() section \"HOT_TEXT\" {\n  %1 = load i32, ptr @in_xib\n  ret i32 %1\n}\n\
                define i32 @main() {\n  %1 = call i32 @hot()\n  ret i32 %1\n}\n";
    let flags = flags(&["-fobject-format=omf"]);
    let (built, _) = compiled(&flags, text, "main").unwrap();
    assert!(built.data.iter().any(|(name, items)| name == "XIB" && !items.is_empty()), "{:?}", built.data);
    assert_eq!(built.code_segments.values().collect::<Vec<_>>(), ["HOT_TEXT"], "{:?}", built.code_segments);
}

/// `@llvm.global_ctors` panicked the x86 backends (`no entry found for key`: an
/// intrinsic-named global has no symbol): each target now lays out the function
/// pointers, lowest priority first, in an init-function segment.
#[test]
fn global_ctors_become_init_function_pointers_at_x86_64_and_m32() {
    for (layout, triple) in
        [("@m64-datalayout@", "x86_64-unknown-linux-gnu"), ("@m32-datalayout@", "i486-unknown-linux-gnu")]
    {
        let text = format!(
            "target datalayout = \"{layout}\"\ntarget triple = \"{triple}\"\n\
             @llvm.global_ctors = appending global [3 x {{ i32, ptr, ptr }}] [{{ i32, ptr, ptr }} {{ i32 200, ptr @late, ptr null }}, \
             {{ i32, ptr, ptr }} {{ i32 100, ptr @early, ptr null }}, {{ i32, ptr, ptr }} {{ i32 150, ptr null, ptr null }}]\n\
             define void @early() {{\n  ret void\n}}\ndefine internal void @late() {{\n  ret void\n}}\n"
        );
        let (built, _) = compiled(&flags(&[]), &text, "ctors").unwrap_or_else(|error| panic!("{triple}: {error}"));
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
    let text = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                @llvm.global_ctors = appending global [1 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 1, ptr @f, ptr null }]\n\
                define void @f() {\n  ret void\n}\n";
    let error = compiled(&flags(&["-m16"]), text, "ctors").expect_err("refused");
    assert!(error.contains("llvm.global_ctors") && error.contains("start-up"), "{error}");
}

/// `__builtin_constant_p(x)` of a parameter reached the backend as
/// `llvm.is.constant.i32` and was refused (`@f_: @llvm.is.constant.i16`): it is
/// false, and the branch on it is gone.
#[test]
fn is_constant_of_a_parameter_is_lowered_before_selection() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                declare i1 @llvm.is.constant.i32(i32)\n\
                define i32 @f(i32 %x) {\n  %c = call i1 @llvm.is.constant.i32(i32 %x)\n  %r = select i1 %c, i32 1, i32 2\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-O0"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("is.constant"), "{listing}");
}

/// `floor(x)` of a C program reached the backend as `llvm.floor.f64` and was
/// refused (`@f_: @llvm.floor.f64`): x87 has no instruction for it, the pin
/// calls `floor`.
#[test]
fn a_floor_intrinsic_is_a_call_of_floor() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                declare double @llvm.floor.f64(double)\n\
                define double @f(double %x) {\n  %a = call double @llvm.floor.f64(double %x)\n  ret double %a\n}\n";
    let (built, _) = compiled(&flags(&["-m32"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @llvm.prefetch.p0(ptr, i32, i32, i32)\n\
                define void @f(ptr %p) {\n  call void @llvm.prefetch.p0(ptr %p, i32 0, i32 3, i32 1)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m32"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("prefetch"), "{listing}");
}

/// `__builtin_trap` is `llvm.trap`: `llrm-llc` panicked on it ("no entry found
/// for key", assemble.rs) at every x86 target, for the call was of a name with
/// no declaration behind it. It is `ud2` (`TRAP`), and `llvm.debugtrap` is
/// `int3`. `llvm.ubsantrap` (`UBSAN_UD1`) is not translated: a named refusal,
/// not a panic.
#[test]
fn the_trap_intrinsics_select_or_are_refused_by_name() {
    let one = |name: &str, call: &str| {
        let text = format!(
            "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n\
             declare void @{name}\ndefine void @f() {{\n  call void @{call}\n  unreachable\n}}\n"
        );
        compiled(&flags(&["-m32"]), &text, "f")
    };
    for (name, call, wanted) in
        [("llvm.trap()", "llvm.trap()", "ud2"), ("llvm.debugtrap()", "llvm.debugtrap()", "int3")]
    {
        let (built, _) = one(name, call).unwrap_or_else(|why| panic!("{name}: {why}"));
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(listing.contains(wanted), "{name}: {listing}");
    }
    let why = one("llvm.ubsantrap(i8)", "llvm.ubsantrap(i8 1)").map(|_| ()).unwrap_err();
    assert!(why.contains("llvm.ubsantrap"), "{why}");
}

/// `-Og` makes clang write `llvm.fake.use` (a debugger keeps a value live);
/// `llrm-llc` refused the module, "@f_: @llvm.fake.use", and `-Og` compiled
/// nothing through `llrm-cc`.
#[test]
fn a_fake_use_compiles_to_no_code() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                declare void @llvm.fake.use(...)\n\
                define i32 @f(i32 %a) {\n  %b = add i32 %a, 1\n  call void (...) @llvm.fake.use(i32 %b)\n  ret i32 %a\n}\n";
    let (built, _) = compiled(&flags(&["-m32"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @zero = internal global i32 0, align 4\n\
                @three = internal global i32 3, align 4\n\
                @ro = internal constant i32 0, align 4\n\
                define i32 @f() {\n  store volatile i32 1, ptr @zero\n  %a = load volatile i32, ptr @zero\n  store volatile i32 1, ptr @three\n  %b = load volatile i32, ptr @three\n  %c = load volatile i32, ptr @ro\n  %s = add i32 %a, %b\n  %t = add i32 %s, %c\n  ret i32 %t\n}\n";
    let (built, _) = compiled(&flags(&["-m32"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define weak i32 @hook() {\n  ret i32 1\n}\n\
                declare extern_weak i32 @maybe()\n";
    let refused = compiled(&flags(&["-m32"]), text, "w").map(|_| ()).unwrap_err();
    assert!(refused.contains("@hook") && refused.contains("weak"), "{refused}");
    let strong = text.replace("define weak", "define").replace("declare extern_weak", "declare");
    assert!(compiled(&flags(&["-m32"]), &strong, "w").is_ok());
}

/// A store into the video buffer (`*(char *)0xB8000 = 1`) was taken for a
/// write that may reach the program's own `g`, so `g` was read back after it
/// instead of forwarded: the target's foreign ranges reached only the other
/// frontends, not LLVM IR.
#[test]
fn a_store_to_video_memory_leaves_a_global_forwarded_in_llvm_ir() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @g = global i32 0, align 4\n\
                define i32 @f() {\n  store i32 5, ptr @g, align 4\n  store i8 1, ptr inttoptr (i32 753664 to ptr), align 1\n  %a = load i32, ptr @g, align 4\n  ret i32 %a\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("mov eax, dword ptr _g"), "{listing}");
    let ordinary = text.replace("753664", "4096");
    let (built, _) = compiled(&flags(&["-m32", "-O2"]), &ordinary, "f").unwrap();
    assert!(llrm_codegen::backend::masm::text(&built).unwrap().contains("mov eax, dword ptr _g"));
}

const REGCALL_DATALAYOUT: &str = "target datalayout = \"@m32-datalayout@\"\n\
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
    let (built, _) = compiled(&flags(&["-m32", "-mabi=watcom", "-mregcall-default"]), &text, "c").unwrap();
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
    let (built, _) = compiled(&flags(&["-m32", "-mabi=watcom", "-mregcall-default"]), &text, "c").unwrap();
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
    let (built, _) = compiled(&flags(&["-m32", "-mabi=watcom", "-mregcall-default"]), &text, "c").unwrap();
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
    let refused = |arguments: &[&str]| compiled(&flags(arguments), &text, "c").map(|_| ()).unwrap_err();
    assert!(refused(&["-m32", "-gcodeview", "-fobject-format=elf"]).contains("cannot carry CodeView"));
    assert!(refused(&["-m32", "-gtd", "-fobject-format=elf"]).contains("Turbo Debugger"));
    assert!(refused(&["-m32", "-gtd"]).contains("16-bit"));
    assert!(compiled(&flags(&["-m32", "-g", "-fobject-format=elf"]), &text, "c").is_ok());
}

/// `__stdcall` and `__fastcall` reached the backend as "calling convention
/// 64": no target here has them, and the message did not say which function or
/// what to write instead.
#[test]
fn stdcall_is_refused_by_name() {
    let text = format!("{REGCALL_DATALAYOUT}define x86_stdcallcc i32 @c(i32 %x) {{\n  ret i32 %x\n}}\n");
    let refused = compiled(&flags(&["-m32", "-mabi=watcom"]), &text, "c").map(|_| ()).unwrap_err();
    assert!(refused.contains("@c") && refused.contains("stdcall") && refused.contains("__cdecl"), "{refused}");
}

/// At m16 an unmarked function takes the register-size convention (`_f@3`,
/// callee clobbers CX and DX), as llrm-cc's did, and `main` is Watcom's `main_`.
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
    let (built, _) = compiled(&flags(&["-m16", "-Os"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                target triple = \"i386-unknown-unknown-code16\"\n\
                declare double @atan2(double, double)\n\
                declare i16 @strlen(ptr)\n\
                declare i16 @printf(ptr, ...)\n\
                declare x86_regcallcc i16 @__regcall3__getch()\n\
                declare i16 @mine(i16)\n\
                declare double @llvm.ceil.f64(double)\n\
                define i16 @f(ptr %s) {\n  %n = call i16 @strlen(ptr %s)\n  %a = call double @atan2(double 1.0, double 2.0)\n  %c = call double @llvm.ceil.f64(double %a)\n  store double %c, ptr %s\n  \
                %p = call i16 (ptr, ...) @printf(ptr %s, i16 %n)\n  %g = call i16 @__regcall3__getch()\n  %m = call i16 @mine(i16 %g)\n  ret i16 %m\n}\n";
    let (built, _) = compiled(&flags(&["-m16"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    for name in ["atan2_", "strlen_", "printf_", "getch_", "ceil_"] {
        assert!(listing.contains(&format!("call {name}")), "{name}: {listing}");
    }
    // `mine` is in return position: the DAG default jumps to it, as the pin
    // does
    assert!(listing.contains("call _mine@3") || listing.contains("jmp _mine@3"), "{listing}");
}

/// A C `static` function at m16 stayed a far procedure (`call far ptr`,
/// `retf`) where llrm-cc entered it by a near call: 3 bytes a call site, 1 a
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
    let (built, _) = compiled(&flags(&["-m16", "-O0"]), text, "f").unwrap();
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
                "target datalayout = \"@m32-datalayout@\"".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, _) = compiled(&flags(&["-m32", "-g", "-fobject-format=coff"]), &text, "dbgfix").unwrap();
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
                "target datalayout = \"@m32-datalayout@\"".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let asked = |flag: &str| {
        let (built, _) = compiled(&flags(&["-m32", flag, "-fobject-format=omf"]), &text, "dbgfix").unwrap();
        built.debug.expect("debug info").format
    };
    assert_eq!(asked("-gdwarf-2"), llrm_debug::Format::Dwarf { version: 2 });
    assert_eq!(asked("-gcodeview"), llrm_debug::Format::CodeView);
}

/// Clang's file MD5 reached a `-gdwarf-2` object and the writer refused it
/// ("a file checksum needs DWARF 5"): no m16 DWARF object came out at all.
#[test]
fn a_dwarf_2_object_from_clang_leaves_out_the_files_md5() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/dbg.ll")
        .lines()
        .map(|line| {
            if line.starts_with("target datalayout") {
                "target datalayout = \"@m32-datalayout@\"".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, format) = compiled(&flags(&["-m32", "-gdwarf-2", "-fobject-format=elf"]), &text, "dbgfix").unwrap();
    assert_eq!(built.debug.as_ref().expect("debug info").source.as_ref().map(|(_, md5)| md5.is_none()), Some(true));
    llrm_codegen::backend::objbuild::written_in(
        &built,
        "dbgfix.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
}

/// A 16-bit object with debug information asked of the ELF writer said "DWARF:
/// a segmented address needs the target's segment fixup", because the debug
/// information named no target kinds unless an OMF writer added them. It
/// carries the target's now, so the ELF writer refuses the object itself
/// ("no elf writer for a 16-bit object yet"); the OMF object is written.
#[test]
fn a_16_bit_object_with_debug_info_is_refused_by_the_elf_writer_not_by_dwarf() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/dbg.ll")
        .lines()
        .map(|line| {
            if line.starts_with("target datalayout") { "target datalayout = \"@m16-datalayout@-ni:2:3\"" } else { line }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, _) = compiled(&flags(&["-m16", "-g"]), &text, "dbgfix").unwrap();
    assert!(built.debug.is_some(), "premise: debug information");
    let write = |format| {
        llrm_codegen::backend::objbuild::written_in(
            &built,
            "dbgfix.ll",
            llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
            format,
            llrm_target::Target::objects(&llrm_x86_m16::M16),
        )
    };
    assert!(write(llrm_target::object::Format::Omf).is_ok());
    for format in [llrm_target::object::Format::Elf, llrm_target::object::Format::MachO] {
        let why = write(format).unwrap_err().to_string();
        assert!(why.contains("a 16-bit object yet"), "{why}");
    }
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
                "target datalayout = \"@m32-datalayout@\"".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, format) = compiled(&flags(&["-m32", "-O2", "-g", "-fobject-format=coff"]), &text, "dbgpieces").unwrap();
    llrm_codegen::backend::objbuild::written_in(
        &built,
        "dbgpieces.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
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
                "target datalayout = \"@m32-datalayout@\"".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (built, _) = compiled(&flags(&["-m32", "-g"]), &text, "dbgfix").unwrap();
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
    let (built, _) = compiled(&flags(&["-m32"]), &text, "t").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let data: usize = listing
        .lines()
        .filter(|line| line.starts_with("db "))
        .map(|line| line.trim_start_matches("db ").split(',').count())
        .sum();
    assert_eq!(data, 24, "{listing}");
}

const SMALL_STRUCT_BY_VALUE: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
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
    let (built, _) = compiled(&flags(&["-mabi=watcom", "-m16"]), SMALL_STRUCT_BY_VALUE, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("mov al, 5") && listing.contains("mov dx, 6"), "{listing}");
    assert!(!listing.contains("sub sp") && !listing.contains("push 6") && !listing.contains("pushw 6"), "{listing}");
    assert!(listing.contains("movzx ax, al\n    add ax, dx"), "the callee reads AL and DX\n{listing}");
}

const SMALL_STRUCT_RESULT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
    target triple = \"i386-unknown-unknown-code16\"\n\
    %struct.T = type { i8 }\n\
    define void @make(ptr sret(%struct.T) align 1 %out) addrspace(1) {\n  store i8 7, ptr %out, align 1\n  ret void\n}\n\
    define i16 @f() addrspace(1) {\n  %t = alloca %struct.T, align 1\n  \
    call addrspace(1) void @make(ptr sret(%struct.T) align 1 %t)\n  %x = load i8, ptr %t, align 1\n  \
    %w = zext i8 %x to i16\n  ret i16 %w\n}\n";

/// clang returns `struct { char x; }` through an `sret` pointer; Borland's
/// cdecl returns it in AL. The call pushed the address of a temporary and the
/// callee wrote through it, so a BCC callee (which returns AL) left the
/// caller's copy unwritten: cfaggee's `r_s1` read 2F, not 11 (callconv CFLB).
#[test]
fn a_one_byte_struct_result_comes_back_in_al_under_borland_cdecl() {
    let (built, _) = compiled(&flags(&["-mabi=cdecl", "-m16"]), SMALL_STRUCT_RESULT, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("mov al, 7"), "the callee returns the byte in AL\n{listing}");
    assert!(!listing.contains("lea ") && !listing.contains("add sp, 2"), "no address is pushed for the result\n{listing}");
}

/// The call through a function pointer carries its convention, so a call that
/// passed the `sret` address to a callee returning AL left the result unwritten
/// with no diagnostic.
#[test]
fn a_one_byte_struct_result_through_a_function_pointer_comes_back_in_al_too() {
    let text = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
        target triple = \"i386-unknown-unknown-code16\"\n\
        %struct.T = type { i8 }\n\
        @fp = external global ptr addrspace(1)\n\
        define i16 @f() addrspace(1) {\n  %t = alloca %struct.T, align 1\n  %p = load ptr addrspace(1), ptr @fp, align 4\n  \
        call addrspace(1) void %p(ptr sret(%struct.T) align 1 %t)\n  %x = load i8, ptr %t, align 1\n  \
        %w = zext i8 %x to i16\n  ret i16 %w\n}\n";
    let (built, _) = compiled(&flags(&["-mabi=cdecl", "-m16"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("lea ") && !listing.contains("add sp, 2"), "no address is pushed for the result\n{listing}");
}

const THREE_BYTE_RESULT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
    target triple = \"i386-unknown-unknown-code16\"\n\
    %struct.T = type { [3 x i8] }\n\
    define void @make(ptr sret(%struct.T) align 1 %out, i16 %a) addrspace(1) noinline {\n  \
    %b = trunc i16 %a to i8\n  store i8 %b, ptr %out, align 1\n  ret void\n}\n\
    define i16 @f(i16 %a) addrspace(1) {\n  %t = alloca %struct.T, align 1\n  \
    call addrspace(1) void @make(ptr sret(%struct.T) align 1 %t, i16 %a)\n  %x = load i8, ptr %t, align 1\n  \
    %w = zext i8 %x to i16\n  ret i16 %w\n}\n";

/// A struct of 3 bytes comes back through Borland's hidden far pointer, `ss`
/// and then the offset pushed after the arguments, and DX:AX returns it. clang
/// writes a near `sret`: the call pushed only an offset, so a BCC callee read
/// the argument it found as the segment and wrote through a wild pointer
/// (callconv CFLB/PFLB `r_s3`, `r_s5`, `r_s7`, `r_s8`, `r_s9`, `r_sci`).
#[test]
fn a_three_byte_struct_result_goes_through_a_far_pointer_under_borland_cdecl() {
    let (built, _) = compiled(&flags(&["-mabi=cdecl", "-m16"]), THREE_BYTE_RESULT, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("push ss"), "the caller pushes the segment of the result\n{listing}");
    assert!(listing.contains("add sp, 6"), "offset, segment and the argument leave the stack\n{listing}");
    assert!(listing.contains("[bp+8]"), "the callee finds its argument above the pointer\n{listing}");
}

/// The call through a function pointer carries its convention too.
#[test]
fn a_three_byte_struct_result_through_a_function_pointer_goes_far_too() {
    let text = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
        target triple = \"i386-unknown-unknown-code16\"\n\
        %struct.T = type { [3 x i8] }\n\
        @fp = external global ptr addrspace(1)\n\
        define i16 @f() addrspace(1) {\n  %t = alloca %struct.T, align 1\n  %p = load ptr addrspace(1), ptr @fp, align 4\n  \
        call addrspace(1) void %p(ptr sret(%struct.T) align 1 %t)\n  %x = load i8, ptr %t, align 1\n  \
        %w = zext i8 %x to i16\n  ret i16 %w\n}\n";
    let (built, _) = compiled(&flags(&["-mabi=cdecl", "-m16"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("push ss"), "{listing}");
}

/// Open Watcom's 16-bit convention passes the result address in SI, near: the
/// far pointer is Borland's.
#[test]
fn a_three_byte_struct_result_stays_near_under_the_watcom_convention() {
    let (built, _) = compiled(&flags(&["-mabi=watcom", "-m16"]), THREE_BYTE_RESULT, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("push ss"), "{listing}");
}

/// `-fno-inline=sum5` named the function as the source does, but the module
/// is decorated (`_sum5` under sysv, `sum5_` under watcom) before the flag is
/// looked up: "the program defines no function of that name", so
/// tail_calls_edges.c failed to compile under llrm-cc at every ABI.
#[test]
fn no_inline_takes_the_source_name_under_every_abi() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @sum5(i32 %a) {\n  %r = add i32 %a, 5\n  ret i32 %r\n}\n";
    for abi in ["sysv", "watcom"] {
        let flags = flags(&["-fobject-format=omf", &format!("-mabi={abi}"), "-fno-inline=sum5"]);
        compiled(&flags, text, "sum5").unwrap_or_else(|why| panic!("{abi}: {why}"));
    }
}

/// `-fsanitize=stack` reached llrm-llc from llrm-cc and did nothing for IR:
/// llrm-cc marked the functions it compiled, an IR module's were never marked,
/// so `stackover.c` ran off the stack instead of ending in "Stack Overflow!".
#[test]
fn fsanitize_stack_makes_every_function_compare_sp_on_entry() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i32 @deep(i32 %a) {\n  %r = add i32 %a, 5\n  ret i32 %r\n}\n";
    let listing = |extra: &[&str]| {
        let mut arguments = vec!["-fobject-format=omf", "-mabi=sysv"];
        arguments.extend(extra);
        let (built, _) = compiled(&flags(&arguments), text, "deep").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @mv(ptr %d, ptr %s, i64 %n) {\n  \
                call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "--isel=dag"]), text, "mv").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("memmove"), "{listing}");
}

/// `text` with `@m16-datalayout@`, `@m32-datalayout@` and `@m64-datalayout@`
/// replaced by that target's datalayout string (its `datalayout.toml`, stated
/// once), kept for the life of the test run.
fn m16_ir(text: &str) -> &'static str {
    let stated = |toml: &str| {
        toml.lines()
            .find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"'))
            .expect("datalayout.toml states a datalayout")
            .to_owned()
    };
    Box::leak(
        text.replace("@m16-datalayout@", &stated(llrm_x86_m16::DATALAYOUT_TOML))
            .replace("@m32-datalayout@", &stated(llrm_x86_m32::DATALAYOUT_TOML))
            .replace("@m64-datalayout@", &stated(llrm_x86_m64::DATALAYOUT_TOML))
            .into_boxed_str(),
    )
}

/// `llc::compiled` of `m16_ir(text)`: the placeholders are filled in.
fn compiled(
    flags: &Flags,
    text: &str,
    name: &str,
) -> Result<(llrm_codegen::backend::masm::Module, llrm_target::object::Format), String> {
    crate::llc::compiled(flags, m16_ir(text), name)
}

/// A 64-bit function whose frame is one slot (the alignment of its call)
/// adjusted the stack with `sub rsp, 8` / `add rsp, 8`, six bytes where the pin
/// has `push rax` / `pop rcx`.
#[test]
fn a_one_slot_frame_at_m64_is_push_and_pop() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @g()\n\
                define i32 @f() {\n  call void @g()\n  ret i32 0\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
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
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f(i32 %o) {\n  %h = alloca [7 x i8], align 1\n  %lo = trunc i32 %o to i8\n  %p3 = getelementptr i8, ptr %h, i64 3\n  store i8 %lo, ptr %p3\n  %s = lshr i32 %o, 8\n  %hi = trunc i32 %s to i8\n  %p4 = getelementptr i8, ptr %h, i64 4\n  store i8 %hi, ptr %p4\n  call void @use(ptr %h)\n  ret void\n}\n";
    compiled(&flags(&["-O2", "--isel=dag"]), text, "f").unwrap();
}

/// Clearing a bit of an `i64` by a variable index was `mov rax,-1; btc rax,n;
/// and`: the pin folds `xor (shl 1, n), -1` to `rotl -2, n` (DAGCombiner),
/// which the table selects with the `and` as `btr`.
#[test]
fn a_variable_bit_clear_of_a_quadword_is_btr() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @clear(i64 %x, i64 %n) {\n  %b = shl i64 1, %n\n  %m = xor i64 %b, -1\n  %r = and i64 %x, %m\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-O2", "--isel=dag"]), text, "clear").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("btr")), "{listing}");
}

/// The `.eh_frame` of `text` (an x86-64 module) written as ELF.
fn eh_frame_of(text: &str) -> Vec<u8> {
    eh_frame_in(&["-m64", "-O2"], text)
}

/// The `.eh_frame` of `text` compiled with `arguments`, written as ELF (32 or
/// 64 bit, as its class byte says).
fn eh_frame_in(
    arguments: &[&str],
    text: &str,
) -> Vec<u8> {
    let (built, format) = compiled(&flags(arguments), text, "f").unwrap();
    let bytes = llrm_codegen::backend::objbuild::written_in(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    let wide = bytes[4] == 2;
    let (u16_at, u32_at, u64_at) = (
        |at: usize| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize,
        |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize,
        |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize,
    );
    // `e_shoff`, `e_shentsize`, `e_shnum` and `e_shstrndx`; `sh_offset` and
    // `sh_size` of a header.
    let (headers, size, count, names) = if wide {
        (u64_at(0x28), u16_at(0x3A), u16_at(0x3C), u16_at(0x3E))
    } else {
        (u32_at(0x20), u16_at(0x2E), u16_at(0x30), u16_at(0x32))
    };
    let field = |at: usize, (wide_at, narrow_at): (usize, usize)| {
        if wide { u64_at(at + wide_at) } else { u32_at(at + narrow_at) }
    };
    let header = |index: usize| headers + index * size;
    let strings = field(header(names), (0x18, 0x10));
    (0..count)
        .map(header)
        .find(|&at| {
            let from = strings + u32_at(at);
            bytes[from..].starts_with(b".eh_frame\0")
        })
        .map(|at| bytes[field(at, (0x18, 0x10))..field(at, (0x18, 0x10)) + field(at, (0x20, 0x14))].to_vec())
        .unwrap_or_default()
}

const SYSV: &str = "target datalayout = \"@m64-datalayout@\"\n\
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
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                declare cc 1000 i32 @r(ptr)\n\
                define cc 1000 i32 @f(ptr %p) {\n\
                b1:\n  %slot = alloca i32\n  %n = load i16, ptr %p\n  %g = icmp slt i16 %n, 1\n  br i1 %g, label %b6, label %b5\n\
                b5:\n  %cnt = phi i16 [ %n, %b1 ], [ %cnt1, %b5 ]\n  %st = phi i32 [ %st1, %b5 ], [ 1, %b1 ]\n  %tot = phi i32 [ %tot1, %b5 ], [ 0, %b1 ]\n  %d = and i32 %st, 10\n  store i32 %d, ptr %slot\n  %c = call cc 1000 i32 @r(ptr nonnull %slot)\n  %tot1 = add i32 %c, %tot\n  %cnt1 = add i16 %cnt, -1\n  %st1 = add i32 %st, 77\n  %e = icmp eq i16 %cnt1, 0\n  br i1 %e, label %b6, label %b5\n\
                b6:\n  %res = phi i32 [ 0, %b1 ], [ %tot1, %b5 ]\n  ret i32 %res\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-march=i486", "-Os", "--isel=dag"]), text, "f").unwrap();
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
        "target datalayout = \"@m64-datalayout@\"\n\
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
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.lines().any(|line| line.trim().starts_with("mov qword ptr [rsp")), "{listing}");
}

/// A 20 KB frame written as COFF allocated with a plain `sub rsp, N`: Windows
/// grows the stack a page at a time, so `__chkstk` must touch it first (the
/// pin: `mov eax, N; call __chkstk; sub rsp, rax; .seh_stackalloc N`). The
/// program faulted past the guard page.
#[test]
fn a_big_coff_frame_is_probed_before_it_is_allocated() {
    let text = format!(
        "{SYSV}declare i32 @g(ptr)\ndefine i32 @f() \"disable-tail-calls\"=\"true\" {{\n  %a = alloca [20040 x i8]\n  %r = call i32 @g(ptr %a)\n  ret i32 %r\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-O2"]), &text, "f").unwrap();
    let frame_of = |format| {
        let object = llrm_codegen::backend::objbuild::built_in(
            &built,
            "f.ll",
            llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
            format,
            llrm_target::Target::objects(&llrm_x86_m32::M32),
        )
        .unwrap();
        let frame = object.frames.first().expect("a declared frame").clone();
        let image = object.sections[frame.section].image.clone();
        (image[frame.offset..frame.offset + frame.length].to_vec(), frame.seh)
    };
    let (body, seh) = frame_of(llrm_target::object::Format::Coff);
    let Some((_, llrm_object::Seh::StackAlloc(size))) = seh.first().copied() else { panic!("{seh:?}") };
    assert!(size >= 20040, "{seh:?}");
    // `mov eax, N; call __chkstk; sub rsp, rax` (the form of the `sub` is the
    // encoder's `2B`, the pin's `29`: every register-register ALU op differs).
    assert_eq!(body[..5], [&[0xB8][..], &(size as u32).to_le_bytes()].concat()[..], "{body:02x?}");
    assert_eq!(body[5], 0xE8, "call __chkstk: {body:02x?}");
    assert!(body[10..13] == [0x48, 0x29, 0xC4] || body[10..13] == [0x48, 0x2B, 0xE0], "{body:02x?}");
    let (elf, _) = frame_of(llrm_target::object::Format::Elf);
    assert!(!elf.windows(3).any(|bytes| bytes == [0x48, 0x29, 0xC4]), "ELF is not probed: {elf:02x?}");
}

/// A probed frame that ends in a sibling call gives its stack back before the
/// `jmp`: the probe and the omitted-frame exit must agree on the size.
#[test]
fn a_probed_frame_ending_in_a_sibling_call_gives_its_stack_back_first() {
    let text = format!(
        "{SYSV}declare i32 @g(i32)\ndefine i32 @f(i32 %x) {{\n  %a = alloca [20040 x i8]\n  %q = getelementptr i8, ptr %a, i64 20000\n  store volatile i8 1, ptr %q\n  %r = tail call i32 @g(i32 %x)\n  ret i32 %r\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-O0"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let jmp = lines.iter().position(|line| line.starts_with("jmp ")).unwrap_or_else(|| panic!("{listing}"));
    // The red zone may take some of the frame's bytes; what the prologue
    // takes the exit gives back.
    let taken = lines.iter().find_map(|line| line.strip_prefix("sub rsp, ")).unwrap_or_else(|| panic!("{listing}"));
    assert!(lines[jmp - 1] == format!("add rsp, {taken}") || lines[jmp - 1].starts_with("mov rsp, rbp"), "{listing}");
}

/// The COFF object of `text` (a Windows x86-64 module) built with the pin's
/// section model.
fn windows_object(text: &str) -> llrm_debug::Described {
    let text = format!("target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-pc-windows-msvc\"\n{text}");
    let (built, format) = compiled(&flags(&["-m64", "-O2"]), &text, "f").unwrap();
    assert_eq!(format, llrm_target::object::Format::Coff);
    llrm_codegen::backend::objbuild::built_in(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap()
}

/// A `linkonce_odr` function in a comdat was written into the one `.text`
/// with every other function, so the linker kept every copy and could not
/// drop the duplicates other objects define. The pin (llc d1106deb71) writes
/// it as a section of its own, flagged COMDAT, selection `any`.
#[test]
fn a_comdat_function_and_global_are_comdat_sections() {
    let object = windows_object(
        "$f = comdat any\n$v = comdat largest\n\
         @v = linkonce_odr dso_local global i32 7, comdat, align 4\n\
         define linkonce_odr dso_local i32 @f(i32 %a) comdat {\n  ret i32 %a\n}\n\
         define dso_local i32 @h(i32 %a) {\n  ret i32 %a\n}\n",
    );
    let at = |name: &str| {
        object
            .symbols
            .iter()
            .find(|one| one.name == name)
            .and_then(
                |one| match one.definition {
                    llrm_object::Definition::Defined { section, .. } => Some(section),
                    llrm_object::Definition::Undefined => None,
                },
            )
    };
    let selection = |section: usize| object.comdats.iter().find(|one| one.section == section).map(|one| one.selection);
    assert_eq!(selection(at("f").unwrap()), Some(llrm_object::Selection::Any), "{:?}", object.comdats);
    assert_eq!(selection(at("v").unwrap()), Some(llrm_object::Selection::Largest), "{:?}", object.comdats);
    // controls: a function with no comdat stays in the plain code section,
    // apart from f's
    assert_eq!(selection(at("h").unwrap()), None);
    assert_ne!(at("h"), at("f"));
    let bytes = llrm_x86::mc::coff64::write(&object).unwrap();
    // IMAGE_SCN_LNK_COMDAT in a section header: 2 headers carry it
    let sections = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    let flagged = (0..sections)
        .filter(|n| u32::from_le_bytes(bytes[20 + 40 * n + 36..20 + 40 * n + 40].try_into().unwrap()) & 0x1000 != 0)
        .count();
    assert_eq!(flagged, 2);
}

/// A COMDAT global of zeros moved to `_BSS` after the COMDAT recorded its range
/// in `_DATA`: `llrm-llc` panicked ("range end index 6 out of range"). The pin
/// writes it as a `.bss` section flagged COMDAT, selection `any`, and the
/// global after it keeps its own `.data` section.
#[test]
fn a_comdat_global_of_zeros_is_a_bss_comdat_section() {
    let object = windows_object(
        "$z = comdat any\n$w = comdat any\n\
         @z = linkonce_odr dso_local global i32 0, comdat, align 4\n\
         @w = linkonce_odr dso_local global [2 x i32] [i32 1, i32 2], comdat, align 4\n\
         define i32 @f() {\n  %a = load i32, ptr @z\n  %b = load i32, ptr @w\n  %c = add i32 %a, %b\n  ret i32 %c\n}\n",
    );
    let section = |name: &str| match object.symbols.iter().find(|one| one.name == name).unwrap().definition {
        llrm_object::Definition::Defined { section, .. } => section,
        llrm_object::Definition::Undefined => unreachable!(),
    };
    let selection = |section: usize| object.comdats.iter().find(|one| one.section == section).map(|one| one.selection);
    let (z, w) = (section("z"), section("w"));
    // the writer drops the `$name` suffix: `.bss$z` is `.bss` in the file
    assert_eq!((object.sections[z].name.as_str(), object.sections[z].role), (".bss$z", llrm_object::Role::Bss));
    assert_eq!(selection(z), Some(llrm_object::Selection::Any), "{:?}", object.comdats);
    assert_eq!(object.sections[w].name, ".data$w");
    assert_eq!(selection(w), Some(llrm_object::Selection::Any), "{:?}", object.comdats);
}

/// A comdat of several members folded its globals into one section and gave
/// its function a comdat of its own: the linker could keep one object's `k` and
/// another's `g`. The pin writes the member named like the comdat as the comdat
/// section, selection `any`, and every other member as its own section,
/// associative with it.
#[test]
fn the_members_of_a_comdat_are_associative_with_its_named_member() {
    let object = windows_object(
        "$k = comdat any\n\
         @k = linkonce_odr dso_local global i32 7, comdat, align 4\n\
         @m = linkonce_odr dso_local global i32 9, comdat($k), align 4\n\
         define linkonce_odr dso_local i32 @g(i32 %a) comdat($k) {\n  ret i32 %a\n}\n",
    );
    let section = |name: &str| match object.symbols.iter().find(|one| one.name == name).unwrap().definition {
        llrm_object::Definition::Defined { section, .. } => section,
        llrm_object::Definition::Undefined => unreachable!(),
    };
    let record = |section: usize| {
        object.comdats.iter().find(|one| one.section == section).map(|one| (one.selection, one.associated))
    };
    let key = section("k");
    assert_eq!(record(key), Some((llrm_object::Selection::Any, None)), "{:?}", object.comdats);
    for member in ["m", "g"] {
        assert_ne!(section(member), key, "{member} shares the key's section");
        assert_eq!(
            record(section(member)),
            Some((llrm_object::Selection::Associative, Some(key))),
            "{member}: {:?}",
            object.comdats
        );
    }
}

/// The symbol of a COMDAT function or global was written as a static
/// (storage class 3): the linker saw a private name in each object and kept
/// every copy. The pin writes it external (class 2).
#[test]
fn a_comdat_symbol_is_external_and_a_static_stays_static() {
    let object = windows_object(
        "$g = comdat any\n$v = comdat any\n\
         @v = linkonce_odr dso_local global i32 7, comdat, align 4\n\
         define linkonce_odr dso_local i32 @g(i32 %a) comdat {\n  ret i32 %a\n}\n\
         define internal i32 @s(i32 %a) {\n  ret i32 %a\n}\n\
         define dso_local i32 @h(i32 %a) {\n  %r = call i32 @s(i32 %a)\n  ret i32 %r\n}\n",
    );
    let bytes = llrm_x86::mc::coff64::write(&object).unwrap();
    let table = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut classes = std::collections::BTreeMap::new();
    let mut at = 0;
    while at < count {
        let row = &bytes[table + 18 * at..table + 18 * (at + 1)];
        if row[..4] != [0; 4] {
            let name = String::from_utf8_lossy(&row[..8]).trim_end_matches('\0').to_owned();
            classes.insert(name, row[16]);
        }
        at += 1 + row[17] as usize;
    }
    let class = |name: &str| classes.get(name).copied();
    assert_eq!((class("g"), class("v"), class("h")), (Some(2), Some(2), Some(2)), "{classes:?}");
    // control: an internal function stays a static (class 3), if it is kept at
    // all
    assert_ne!(class("s"), Some(2), "{classes:?}");
}

/// The object of `text` for `triple`, built as `windows_object` builds one.
fn object_for(
    triple: &str,
    format: llrm_target::object::Format,
    text: &str,
) -> llrm_debug::Described {
    let text = format!("target datalayout = \"@m64-datalayout@\"\ntarget triple = \"{triple}\"\n{text}");
    let (built, found) = compiled(&flags(&["-m64", "-O2"]), &text, "f").unwrap();
    assert_eq!(found, format);
    llrm_codegen::backend::objbuild::built_in(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap()
}

const WEAK_IR: &str = "@u = weak_odr dso_local global i32 7, align 4\n\
     define weak_odr dso_local i32 @wf(i32 %a) {\n  ret i32 %a\n}\n\
     define dso_local i32 @h() {\n  %v = load i32, ptr @u\n  ret i32 %v\n}\n";

/// A `weak_odr` function and global were written local: `nm` showed `t` and
/// `d` where the pin shows `W` and `V`, and the linker saw a private name.
#[test]
fn a_weak_definition_is_a_weak_symbol_in_an_elf_object() {
    let object = object_for("x86_64-unknown-linux-gnu", llrm_target::object::Format::Elf, WEAK_IR);
    let binding = |name: &str| object.symbols.iter().find(|one| one.name == name).map(|one| one.binding);
    assert_eq!(
        (binding("wf"), binding("u"), binding("h")),
        (Some(llrm_object::Binding::Weak), Some(llrm_object::Binding::Weak), Some(llrm_object::Binding::Public)),
    );
}

/// The pin writes a weak definition outside a COMDAT as a weak external
/// (class 0x69, search-alias aux) that names a default symbol
/// `.weak.NAME.default.UNIQUE` in the definition's section, UNIQUE the first
/// strong external symbol of the object.
#[test]
fn a_weak_definition_is_a_weak_external_with_a_default_in_a_coff_object() {
    let object = windows_object(WEAK_IR);
    let bytes = llrm_x86::mc::coff64::write(&object).unwrap();
    let table = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let strings = table + 18 * count;
    let name = |row: &[u8]| {
        if row[..4] == [0; 4] {
            let at = strings + u32::from_le_bytes(row[4..8].try_into().unwrap()) as usize;
            let end = bytes[at..].iter().position(|b| *b == 0).unwrap();
            String::from_utf8_lossy(&bytes[at..at + end]).into_owned()
        } else {
            String::from_utf8_lossy(&row[..8]).trim_end_matches('\0').to_owned()
        }
    };
    let mut rows = Vec::new();
    let mut at = 0;
    while at < count {
        let row = &bytes[table + 18 * at..table + 18 * (at + 1)];
        rows.push((at, name(row), u16::from_le_bytes([row[12], row[13]]), row[16], row[17]));
        at += 1 + row[17] as usize;
    }
    let index = |wanted: &str| rows.iter().find(|one| one.1 == wanted).map(|one| one.0);
    for weak in ["wf", "u"] {
        let row = rows.iter().find(|one| one.1 == weak).unwrap_or_else(|| panic!("{weak} in {rows:?}"));
        // section 0, class WEAK_EXTERNAL, one aux
        assert_eq!((row.2, row.3, row.4), (0, 105, 1), "{weak}: {rows:?}");
        let aux = &bytes[table + 18 * (row.0 + 1)..table + 18 * (row.0 + 2)];
        let tag = u32::from_le_bytes(aux[..4].try_into().unwrap()) as usize;
        assert_eq!(index(&format!(".weak.{weak}.default.h")), Some(tag), "{rows:?}");
        assert_eq!(u32::from_le_bytes(aux[4..8].try_into().unwrap()), 3, "search alias");
    }
    // control: the strong function is a plain external
    let strong = rows.iter().find(|one| one.1 == "h").unwrap();
    assert_eq!((strong.2 != 0, strong.3), (true, 2));
}

/// A COMDAT function's `.pdata` row and `.xdata` were in the object's one
/// `.pdata` and `.xdata`: where the linker kept another object's copy of the
/// function and dropped this one's code, the row stayed and pointed into
/// the dropped section. The pin writes both as sections associative with the
/// function's code (selection 5, number of the code section).
#[test]
fn a_comdat_functions_unwind_data_is_associative_with_its_code() {
    let object = windows_object(
        "$f = comdat any\n\
         define linkonce_odr dso_local i32 @f(i32 %a) comdat {\n  %p = alloca [16 x i32]\n  store volatile i32 %a, ptr %p\n  %v = load volatile i32, ptr %p\n  ret i32 %v\n}\n\
         define dso_local i32 @h(i32 %a) {\n  %p = alloca [16 x i32]\n  store volatile i32 %a, ptr %p\n  %v = load volatile i32, ptr %p\n  ret i32 %v\n}\n",
    );
    let object = llrm_x86::mc::win64unwind::expanded(&object).unwrap();
    let code = |name: &str| match object.symbols.iter().find(|one| one.name == name).unwrap().definition {
        llrm_object::Definition::Defined { section, .. } => section,
        llrm_object::Definition::Undefined => unreachable!(),
    };
    let follows: Vec<(&str, usize)> = object
        .comdats
        .iter()
        .filter(|one| one.selection == llrm_object::Selection::Associative)
        .map(|one| (object.sections[one.section].name.as_str(), one.associated.unwrap()))
        .collect();
    assert_eq!(follows, [(".xdata", code("f")), (".pdata", code("f"))], "{:?}", object.comdats);
    // control: h is not in a comdat, so its data stays in the shared pair
    let shared = |name: &str| object.sections.iter().enumerate().filter(|(_, one)| one.name == name).count();
    assert_eq!((shared(".xdata"), shared(".pdata")), (2, 2));
}

/// Every function got an `.xdata` block and a `.pdata` row, a frameless leaf
/// too (12 bytes of `.pdata` each); the pin (llc d1106deb71) writes both only
/// for a function with a `.seh_*` prologue, so a leaf cost 20 bytes the pin's
/// object does not carry.
#[test]
fn a_frameless_leaf_gets_no_unwind_data_as_the_pin_writes_none() {
    let object = windows_object(
        "define dso_local i32 @leaf(i32 %a) uwtable {\n  ret i32 %a\n}\n\
         define dso_local i32 @framed(i32 %a) uwtable {\n  %p = alloca [16 x i32]\n  store volatile i32 %a, ptr %p\n  %v = load volatile i32, ptr %p\n  ret i32 %v\n}\n",
    );
    let object = llrm_x86::mc::win64unwind::expanded(&object).unwrap();
    let size = |name: &str| object.sections.iter().find(|one| one.name == name).map_or(0, |one| one.image.len());
    // one row for `framed`; control: that row is there, so the leaf's absence
    // is not an empty table
    assert_eq!(size(".pdata"), 12, "{:?}", object.sections.iter().map(|one| &one.name).collect::<Vec<_>>());
    assert!(size(".xdata") >= 4);
    let rows = object.sections.iter().find(|one| one.name == ".pdata").unwrap().relocs.len();
    assert_eq!(rows, 3);
}

/// An object with only frameless leaves has no unwind sections at all, as the
/// pin's has none.
#[test]
fn an_object_of_leaves_has_no_unwind_sections() {
    let object = windows_object("define dso_local i32 @leaf(i32 %a) uwtable {\n  ret i32 %a\n}\n");
    let object = llrm_x86::mc::win64unwind::expanded(&object).unwrap();
    assert!(object.sections.iter().all(|one| one.name != ".xdata" && one.name != ".pdata"));
}

/// The listing of `tail call @g(...)` then `ret`, built with `abi` at -O2.
fn tail_call_listing(
    abi: &str,
    signature: &str,
    arguments: &str,
) -> String {
    // a variadic call names its callee's type: `i32 (i32, ...) @g`
    let text = format!(
        "{SYSV}declare i32 @g{signature}\ndefine i32 @f(i32 %x) {{\n  %r = tail call i32 {} @g({arguments})\n  ret i32 %r\n}}\n",
        if signature.contains("...") { "(i32, ...)" } else { "(i32, i32)" }
    );
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", abi]), &text, "f").unwrap();
    llrm_codegen::backend::masm::text(&built).unwrap()
}

/// A variadic tail call under Win64 became `jmp g`; the pin keeps `call g`
/// then `ret` (`isEligibleForSiblingCallOpt`: no variadic sibling call when
/// either side is Win64). SysV, and a Win64 call of a fixed signature, still
/// jump.
#[test]
fn a_win64_variadic_tail_call_stays_a_call() {
    let jumps = |listing: &str| listing.lines().any(|line| line.trim().starts_with("jmp g"));
    let variadic = tail_call_listing("-mabi=win64", "(i32, ...)", "i32 %x, i32 %x");
    assert!(!jumps(&variadic) && variadic.contains("call g"), "{variadic}");
    let fixed = tail_call_listing("-mabi=win64", "(i32, i32)", "i32 %x, i32 %x");
    assert!(jumps(&fixed), "{fixed}");
    let sysv = tail_call_listing("-mabi=sysv", "(i32, ...)", "i32 %x, i32 %x");
    assert!(jumps(&sysv), "{sysv}");
}

/// An internal x86-64 function whose callers are all seen, or one marked
/// `fastcc`, popped its stack arguments (`ret 16`) and its callers did not
/// clean up. The pin's `isCalleePop` is false for `fastcc` there (no
/// `GuaranteedTailCallOpt`), so the callee returns with a plain `ret` and the
/// caller removes the area (QCport corpus: `os_locals_between_pushes`).
#[test]
fn an_x86_64_fastcc_function_returns_with_a_plain_ret() {
    let text = format!(
        "{SYSV}define internal fastcc i64 @work(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i64 %f, i64 %g, i64 %h) noinline {{\n  %x = add i64 %a, %h\n  %y = add i64 %x, %g\n  ret i64 %y\n}}\n\
         define i64 @f(i64 %x) {{\n  %p = call fastcc i64 @work(i64 %x, i64 %x, i64 %x, i64 %x, i64 %x, i64 %x, i64 %x, i64 %x)\n  %q = add i64 %p, 1\n  ret i64 %q\n}}\n"
    );
    for abi in ["-mabi=sysv", "-mabi=win64"] {
        let (built, _) = compiled(&flags(&["-m64", "-O0", abi]), &text, "work").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        let returns: Vec<&str> = listing.lines().map(str::trim).filter(|line| line.starts_with("ret")).collect();
        assert!(!returns.is_empty() && returns.iter().all(|line| *line == "ret"), "{abi}: {listing}");
    }
}

/// The `.xdata` of an x86-64 module written as COFF, found by `.pdata`'s
/// layout: the frames the compiler declared (`.seh_*`) against the bytes read
/// back from the same code agree.
#[test]
fn the_declared_win64_prologue_is_what_the_code_bytes_say() {
    let text = format!(
        "{SYSV}declare i32 @g(i32)\ndefine i32 @f(i32 %a, i32 %b) \"disable-tail-calls\"=\"true\" {{\n  %x = call i32 @g(i32 %a)\n  %y = call i32 @g(i32 %b)\n  %s = add i32 %x, %y\n  %t = add i32 %s, %a\n  ret i32 %t\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-O2"]), &text, "f").unwrap();
    let object = llrm_codegen::backend::objbuild::built(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    let frame = object.frames.first().expect("a declared frame");
    assert!(!frame.seh.is_empty(), "no .seh_* directive reached the frame");
    let section = &object.sections[frame.section];
    let body = &section.image[frame.offset..frame.offset + frame.length];
    let declared = llrm_x86::mc::win64unwind::from_seh("f", &frame.seh).unwrap();
    let read = llrm_x86::mc::win64unwind::from_prologue("f", body).unwrap();
    assert_eq!(declared, read, "{:?}", frame.seh);
}

/// A Win64 function with a frame register (`alloca`) pushes RBP and sets it
/// from RSP; its `.seh_pushreg rbp` and `.seh_setframe rbp, 0` were left out
/// (the frame register is not a register of the file the directives read), so
/// the unwind data held the allocation alone and an unwinder lost the frame.
#[test]
fn a_win64_frame_register_has_its_seh_directives() {
    let body = "declare i64 @g(ptr)\ndefine i64 @f(i64 %n) {\n  %p = alloca i8, i64 %n, align 16\n  %r = call i64 @g(ptr %p)\n  ret i64 %r\n}\n";
    let text = format!("{SYSV}{body}");
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, "f").unwrap();
    let object = llrm_codegen::backend::objbuild::built(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    let frame = object.frames.first().expect("a declared frame");
    let section = &object.sections[frame.section];
    let body = &section.image[frame.offset..frame.offset + frame.length];
    let declared = llrm_x86::mc::win64unwind::from_seh("f", &frame.seh).unwrap();
    let read = llrm_x86::mc::win64unwind::from_prologue("f", body).unwrap();
    assert_eq!(declared, read, "{:?}", frame.seh);
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
         target datalayout = \"@m32-datalayout@\"\n\
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
    let (built, _) = compiled(&flags(&["-m32", "-O2"]), text, "six").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let pushed: Vec<&str> = listing
        .lines()
        .map(str::trim)
        .filter(|line| matches!(*line, "push ebp" | "push ebx" | "push edi" | "push esi"))
        .collect();
    assert_eq!(pushed, ["push ebp", "push edi", "push esi"], "{listing}");
}

const ALIGNED_IR: &str = "declare i32 @use(ptr)\n\
    define i32 @aligned(i32 %n) {\n  %v = alloca [4 x i32], align 64\n  store i32 %n, ptr %v\n  \
    %r = call i32 @use(ptr %v)\n  ret i32 %r\n}\n";

/// A 32-bit local wanting 64 bytes of alignment lay wherever the stack was. The
/// frame is the pin's `push ebp; mov ebp, esp; and esp, -64; sub esp, N` (the
/// pin's `clang -target i386-unknown-linux-gnu` on the same function), the
/// stack taken back from the frame register.
#[test]
fn m32_over_aligned_local_realigns_the_stack_pointer() {
    let text = format!("target datalayout = \"@m32-datalayout@\"\n{ALIGNED_IR}");
    let (built, _) = compiled(&flags(&["-m32", "-O1"]), &text, "aligned").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let at = lines.iter().position(|line| *line == "and esp, -64").unwrap_or_else(|| panic!("{listing}"));
    assert_eq!(lines[at - 1], "mov ebp, esp", "{listing}");
    assert!(lines.contains(&"mov esp, ebp"), "{listing}");
}

/// Real mode cannot realign the stack: a local aligned past 32 is refused by name
/// (#8570), not clamped (#7706); the refusal is the only guard against a wrong
/// promised alignment, and no `and sp` is emitted.
#[test]
fn m16_over_aligned_local_is_refused_by_name() {
    let text = format!("target datalayout = \"@m16-datalayout@-ni:2:3\"\n{ALIGNED_IR}");
    let why = compiled(&flags(&["-m16", "-O1"]), &text, "aligned").err().expect("refused");
    assert!(why.to_string().contains("an object aligned to 64 needs stack realignment"), "{why}");
}

/// jwlink turns a far call to the same segment into `push cs; call near` plus
/// a pad byte, a `ds` prefix at one parity and an executed `nop` at the other
/// (0.60-0.85% of stft_fixed's executed instructions, 47 sites). The object
/// writer states the near form itself where the callee is in the call's own
/// segment: four bytes, no pad, nothing to execute.
#[test]
fn a_far_call_inside_its_own_segment_is_push_cs_and_a_near_call() {
    let text = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n\
                define cc 1005 i32 @g(i32 %a) addrspace(1) noinline {\n  %r = mul i32 %a, 3\n  ret i32 %r\n}\n\
                define cc 1005 i32 @f(i32 %a) addrspace(1) {\n  %r = call cc 1005 addrspace(1) i32 @g(i32 %a)\n  %s = add i32 %r, 1\n  ret i32 %s\n}\n";
    let (built, format) = compiled(&flags(&["-m16", "-O1", "--isel=dag"]), text, "f").unwrap();
    let bytes = llrm_codegen::backend::objbuild::written_in(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m16::M16),
    )
    .unwrap();
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|window| window == needle);
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!has(&[0x9A, 0, 0, 0, 0]), "a far call to g is left to the linker: {listing}");
    assert!(has(&[0x0E, 0xE8]), "push cs; call near: {listing} {bytes:02x?}");
}

/// A callee-saved register pushed before the stack `sub` (the pin's order,
/// `saved_first`) had no `.eh_frame`: the frame was left out of the declared
/// ones. Its offset is declared after the `sub` (`def_cfa_offset n; offset
/// rbx`), as the pin writes it.
#[test]
fn a_pushed_register_before_the_stack_adjustment_is_declared_after_it() {
    let text = format!(
        "{SYSV}declare i32 @g(ptr)\ndefine i32 @f(i32 %a) \"disable-tail-calls\"=\"true\" {{\n  %buf = alloca [24 x i8]\n  %r = call i32 @g(ptr %buf)\n  %s = add i32 %r, %a\n  ret i32 %s\n}}\n"
    );
    let frame = eh_frame_of(&text);
    assert!(frame.windows(4).any(|window| window[0] == 0x0E && window[2..] == [0x83, 0x02]), "{frame:02x?}");
}

/// The pin declares the offsets of the saved registers last pushed first
/// (`rbx` at cfa-32, `r14` at -24, `rbp` at -16 after pushes of rbp, r14,
/// rbx). They were listed in push order, and a pushed `rbp` that was no frame
/// register got no offset at all.
#[test]
fn saved_registers_are_declared_last_pushed_first() {
    let text = format!(
        "{SYSV}declare i32 @g(i32)\ndeclare i32 @h(i32)\ndefine i32 @f(i32 %a, i32 %b) \"disable-tail-calls\"=\"true\" {{\n  %x = call i32 @g(i32 %a)\n  %y = call i32 @h(i32 %b)\n  %z = call i32 @g(i32 %x)\n  %s = add i32 %x, %y\n  %t = add i32 %s, %z\n  ret i32 %t\n}}\n"
    );
    let frame = eh_frame_of(&text);
    let offsets = |window: &[u8]| window[0] & 0xC0 == 0x80 && window[2] & 0xC0 == 0x80 && window[1] > window[3];
    assert!(frame.windows(4).any(offsets), "{frame:02x?}");
}

/// An `add eax, 2` was read as a move of the stack pointer: the frame address
/// went 2 lower (`def_cfa_offset 14`, then 6) in the middle of the function.
/// The bytes are the pin's for `push rax; call; add eax,2; pop rcx; ret`.
#[test]
fn an_add_to_a_register_other_than_the_stack_pointer_does_not_move_the_frame_address() {
    let text = format!(
        "{SYSV}declare i32 @g()\ndefine i32 @f() \"disable-tail-calls\"=\"true\" {{\n  %r = call i32 @g()\n  %s = add i32 %r, 2\n  ret i32 %s\n}}\n"
    );
    assert_eq!(
        eh_frame_of(&text),
        hex("1400000000000000017a5200017810011b0c070890010000140000001c000000000000000b00000000410e10490e0800")
    );
}

const SYSV32: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
                      target triple = \"i386-unknown-linux-gnu\"\n";

/// An x86 32-bit ELF object had no `.eh_frame`. The CIE is the pin's
/// (`clang -target i386-linux-gnu`: 4-byte slots, `esp+4`, return column 8),
/// and `push esi` declares `esi` at `cfa-8` after the prologue and the
/// frame address 4 again at the `pop`.
#[test]
fn a_32_bit_function_with_a_pushed_register_has_the_pins_eh_frame() {
    let text = format!(
        "{SYSV32}declare i32 @g()\ndefine i32 @f(i32 %a) \"disable-tail-calls\"=\"true\" {{\n  %r = call i32 @g()\n  %s = add i32 %r, %a\n  %t = call i32 @g()\n  %u = add i32 %s, %t\n  ret i32 %u\n}}\n"
    );
    assert_eq!(
        eh_frame_in(&["-m32", "-mabi=sysv", "-fobject-format=elf", "-O2"], &text),
        hex(
            "1400000000000000017a5200017c08011b0c040488010000180000001c000000000000001500000000410e08860253 0e04000000"
                .replace(' ', "")
                .as_str()
        )
    );
}

/// The frame address stayed 4 higher after a call whose pushed argument was
/// cleaned up with `add esp, 4`: the `add` was matched against the pseudo stack
/// pointer at another width and never seen, so an unwinder in the rest of the
/// function (and at `ret`) read the wrong return address.
#[test]
fn a_cleaned_up_argument_push_gives_the_frame_address_back() {
    let text = format!(
        "{SYSV32}declare i32 @g(i32)\ndefine i32 @f(i32 %a) \"disable-tail-calls\"=\"true\" {{\n  %r = call i32 @g(i32 %a)\n  %s = add i32 %r, 1\n  ret i32 %s\n}}\n"
    );
    let frame = eh_frame_in(&["-m32", "-mabi=sysv", "-fobject-format=elf", "-O2"], &text);
    assert_eq!(frame[frame.len() - 7..], hex("440e08480e0400"));
}

/// A frame whose registers are pushed before the locals' space (`saved_first`,
/// every x86-64 frame with a pushed register) had no `.eh_frame`: the entry
/// was not declared. The bytes are the pin's for `push rbx; sub rsp, 32 ...
/// add rsp, 32; pop rbx; ret`, but for the local's address: the pin's llc puts
/// the local at `[rsp+8]` (`lea rdi, [rsp+8]`, 5 bytes, FDE 34 long); ours at
/// `[rsp]` (FR1, the call pad above it), and the pin's `tryOptimizeLEAtoMOV`
/// makes that `mov rdi, rsp` (3 bytes, #8601), so 32.
#[test]
fn a_frame_with_pushed_registers_before_its_locals_has_the_pins_eh_frame() {
    let text = format!(
        "{SYSV}declare i32 @g(ptr, i32)\ndeclare i32 @h(i32)\n\
         define i32 @f(i32 %a) \"disable-tail-calls\"=\"true\" {{\n  %buf = alloca [6 x i32]\n  \
         %x = call i32 @g(ptr %buf, i32 %a)\n  %y = call i32 @h(i32 %x)\n  %z = add i32 %y, %x\n  ret i32 %z\n}}\n"
    );
    assert_eq!(
        eh_frame_of(&text),
        hex("1400000000000000017a5200017810011b0c07089001000\
             01c0000001c000000000000002000000000410e10440e308302590e10410e0800")
    );
}

/// An `i32` call argument goes in the dword register, as the pin's
/// `CCPromoteToType` leaves it (only a byte or a word is promoted): the DAG
/// extended it to the convention's quadword register, `mov rdi, rbx` (3 bytes)
/// for `mov edi, ebx`.
#[test]
fn an_i32_call_argument_is_passed_in_the_dword_register_at_m64() {
    let text = format!(
        "{SYSV}declare i32 @g(i32)\ndeclare i32 @h(i32)\n\
         define i32 @f(i32 %a) {{\n  %x = call i32 @g(i32 %a)\n  %y = call i32 @h(i32 %x)\n  %z = add i32 %y, %x\n  ret i32 %z\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("mov edi, ") && !listing.contains("mov rdi, r"), "{listing}");
}

/// `fptoui double to i64` on a target with no conditional move
/// (`double_to_uint64.c` at i486 -Os) was two branches before the DAG: the high
/// word was `mov edx, -2147483648 / xor eax, eax` in the arms and the threshold
/// was loaded twice. The pin keeps one diamond for the float select and `shl
/// 31` of the `setbe`.
#[test]
fn an_unsigned_quadword_conversion_without_cmov_has_one_diamond_and_a_shifted_setcc() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i386-pc-linux-gnu\"\n\
                define i64 @f(double %d) {\n  %r = fptoui double %d to i64\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-march=i486", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("setbe") && listing.contains("shl edx, 31"), "{listing}");
    assert!(!listing.contains("-2147483648"), "{listing}");
}

/// A 72-byte zero fill and copy at m64 -Os (`pattern_fill.c`,
/// `huffman_decode.c`, `idioms.c`): `rep stosq` took its value from EAX where
/// its form reads RAX, and the assembler had no `stosq`/`movsq`: "a division
/// operand (RAX) not set by a glued CopyToReg" refused the function. The -Os copy is `rep movsd` since
/// REP-MOVS-OS (the dword block is a byte shorter than REX.W `movsq`), and the
/// zero fill is `rep stosd` for the same reason (REP-STOS-UNIT).
#[test]
fn a_zero_fill_and_copy_at_m64_are_rep_string_instructions() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr, ptr)\n\
                declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @f(ptr %s) optsize {\n  %b = alloca [72 x i8], align 16\n  %c = alloca [72 x i8], align 16\n\
                \x20 call void @llvm.memset.p0.i64(ptr align 16 %c, i8 0, i64 72, i1 false)\n\
                \x20 call void @llvm.memcpy.p0.p0.i64(ptr align 16 %b, ptr align 16 %s, i64 72, i1 false)\n\
                \x20 call void @use(ptr %b, ptr %c)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rep stosd") && listing.contains("rep movsd"), "{listing}");
}

/// A variable `alloca` at m64 under `--isel=dag` named the 16-bit `sp` at
/// quadword width (`isel: 0x0004 names 25 at width 8`, the verifier): the
/// lowering took `RegId::SP` for every pointer width. It is `rsp`.
#[test]
fn a_variable_alloca_at_m64_names_the_quadword_stack_pointer() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f(i64 %n) {\n  %p = alloca i32, i64 %n, align 16\n  call void @use(ptr %p)\n  ret void\n}\n";
    compiled(&flags(&["-O1", "--isel=dag", "-fverify-ir"]), text, "f").unwrap();
}

/// `!DIArgList` records (a variable made of several values) were read by no
/// pass: `sum` in `bench_recgcd` had no location over the code that follows
/// the record, and gdb printed `<optimized out>`.
#[test]
fn a_variable_made_of_several_values_has_an_expression_location() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/arglist_sum.ll")
        .lines()
        .map(
            |line| {
                if line.starts_with("target datalayout") { "target datalayout = \"@m32-datalayout@\"" } else { line }
            },
        )
        .collect::<Vec<_>>()
        .join("\n");
    let (built, format) = compiled(&flags(&["-m32", "-g", "-fobject-format=elf"]), &text, "arglist").unwrap();
    let object = llrm_codegen::backend::objbuild::built_in(
        &built,
        "arglist.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    let debug = object.debug.expect("debug info");
    let made = |location: &llrm_debug::Location| match location {
        llrm_debug::Location::Expression { .. } => true,
        llrm_debug::Location::List(entries) => {
            entries.iter().any(|(_, one)| matches!(one, llrm_debug::Location::Expression { .. }))
        }
        _ => false,
    };
    let variables: Vec<_> = debug.functions.iter().flat_map(|function| function.variables.iter()).collect();
    for name in ["d", "e"] {
        assert!(
            variables.iter().any(|variable| variable.name == name && made(&variable.location)),
            "{name}: {variables:#?}"
        );
    }
    // `d` is `%mul + %xor`: DW_OP_breg0 0, DW_OP_breg2 0, DW_OP_plus,
    // DW_OP_stack_value.
    let bytes = llrm_codegen::backend::objbuild::written_in(
        &built,
        "arglist.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    let sum = [0x70, 0x00, 0x72, 0x00, 0x22, 0x9f];
    assert!(bytes.windows(sum.len()).any(|window| window == sum), "the sum's expression is in the object");
}

/// An x86-64 parameter whose register the body reuses had no location past the
/// copy (`a` after the call in `f`): gdb printed `<optimized out>` where the
/// pin's object says `DW_OP_entry_value(DW_OP_reg5 RDI), DW_OP_stack_value`.
#[test]
fn a_parameter_whose_register_was_reused_is_told_by_its_entry_value() {
    let text = include_str!("../../../ir/llrm-mir/tests/clang/entry_value.ll")
        .lines()
        .map(
            |line| {
                if line.starts_with("target datalayout") { "target datalayout = \"@m64-datalayout@\"" } else { line }
            },
        )
        .collect::<Vec<_>>()
        .join("\n");
    let (built, format) = compiled(&flags(&["-m64", "-g", "-fobject-format=elf"]), &text, "entryvalue").unwrap();
    let bytes = llrm_codegen::backend::objbuild::written_in(
        &built,
        "entry_value.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap();
    // DW_OP_entry_value (DWARF 5) or DW_OP_GNU_entry_value, 1, DW_OP_reg5,
    // DW_OP_stack_value.
    let found = |op: u8| bytes.windows(4).any(|window| window == [op, 0x01, 0x55, 0x9f]);
    assert!(found(0xa3) || found(0xf3), "the entry value of rdi is in the object");
}

/// The listing of `@f` at m32 -O2, a function with a local array it passes on
/// (so its cells are reached through a frame or the stack pointer), with
/// `attrs` and, if `calls` is false, no call besides the leaf's own work.
fn frame_listing(
    attrs: &str,
    calls: bool,
    extra: &[&str],
) -> String {
    let (group, definition) =
        if attrs.is_empty() { ("", String::new()) } else { ("#0", format!("attributes #0 = {{ {attrs} }}\n")) };
    let body = if calls {
        "%a = alloca [16 x i32]\n  call void @g(ptr %a)\n  ret void"
    } else {
        "%a = alloca [16 x i32]\n  store i32 1, ptr %a\n  %v = load volatile i32, ptr %a\n  ret void"
    };
    let text = format!(
        "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i386-unknown-linux-gnu\"\n\
         declare void @g(ptr)\n\
         define void @f() {group} {{\n  {body}\n}}\n{definition}"
    );
    let mut arguments = vec!["-m32", "-O2"];
    arguments.extend_from_slice(extra);
    let (built, _) = compiled(&flags(&arguments), &text, "f").unwrap();
    llrm_codegen::backend::masm::text(&built).unwrap()
}

/// `"frame-pointer"="all"` was ignored: the function's cells were reached
/// through `esp`, `ebp` was a value register, and llc's bar for a
/// six-register comparison could not be made.
#[test]
fn the_frame_pointer_all_attribute_keeps_the_frame_register() {
    let framed = frame_listing("\"frame-pointer\"=\"all\"", true, &[]);
    assert!(framed.contains("mov ebp, esp"), "{framed}");
    let plain = frame_listing("", true, &[]);
    assert!(!plain.contains("mov ebp, esp"), "{plain}");
}

/// `non-leaf` keeps it where the function calls, and only there.
#[test]
fn the_non_leaf_frame_pointer_policy_frames_a_caller_and_not_a_leaf() {
    assert!(frame_listing("\"frame-pointer\"=\"non-leaf\"", true, &[]).contains("mov ebp, esp"));
    assert!(!frame_listing("\"frame-pointer\"=\"non-leaf\"", false, &[]).contains("mov ebp, esp"));
}

/// The pin's `-frame-pointer=all` gives the attribute to each function that
/// has none; a function's own attribute wins.
#[test]
fn the_frame_pointer_option_gives_the_attribute_to_functions_without_one() {
    assert!(frame_listing("", true, &["-frame-pointer=all"]).contains("mov ebp, esp"));
    assert!(!frame_listing("\"frame-pointer\"=\"none\"", true, &["-frame-pointer=all"]).contains("mov ebp, esp"));
}

const M64_SUM: &str = "target datalayout = \"@m64-datalayout@\"\n\
    target triple = \"x86_64-unknown-linux-gnu\"\n\
    @a = global [100 x i64] zeroinitializer\n\
    define i64 @f(i64 %i) {\n  %p = getelementptr inbounds [100 x i64], ptr @a, i64 0, i64 %i\n  \
    %v = load i64, ptr %p\n  ret i64 %v\n}\n";

/// `-fno-pic` was ignored at m64: a global array indexed was
/// `lea reg, offset a` and a register-indexed access, where the pin's static
/// code is one absolute `a[rdx*8]`.
#[test]
fn a_global_array_index_at_m64_without_the_pic_level_flag_is_one_absolute_access() {
    let text = format!("{M64_SUM}!llvm.module.flags = !{{!0}}\n!0 = !{{i32 7, !\"uwtable\", i32 2}}\n");
    let (built, _) = compiled(&flags(&["-O2"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("offset a"), "{listing}");
}

/// The default stays position independent: with the "PIC Level" flag (or none
/// at all) the symbol's address is made with `lea` from `%rip`.
#[test]
fn a_global_array_index_at_m64_with_the_pic_level_flag_is_an_lea() {
    let flagged = format!(
        "{M64_SUM}!llvm.module.flags = !{{!0, !1}}\n!0 = !{{i32 8, !\"PIC Level\", i32 2}}\n!1 = !{{i32 7, !\"uwtable\", i32 2}}\n"
    );
    for text in [flagged.as_str(), M64_SUM] {
        let (built, _) = compiled(&flags(&["-O2"]), text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(listing.contains("offset a"), "{listing}");
    }
}

/// A loop with a `srem` and a 64-bit sum, called with a constant, asked SCEV
/// to udiv a 128-bit value: the zero-extension to 132 bits made an `Ap::new`
/// of width 132 (debug builds aborted in simplifyindvar).
#[test]
fn an_srem_loop_with_a_constant_trip_count_compiles_in_a_debug_build() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
        target triple = \"x86_64-unknown-linux-gnu\"\n\
        define i64 @f(i32 %n) {\n\
        entry:\n  br label %loop\n\
        loop:\n  %i = phi i32 [ %n, %entry ], [ %next, %loop ]\n  %acc = phi i64 [ 0, %entry ], [ %sum, %loop ]\n  \
        %m = mul i32 %i, 3\n  %r = srem i32 %m, 11\n  %w = sext i32 %r to i64\n  %sum = add i64 %acc, %w\n  \
        %next = sub i32 %i, 1\n  %done = icmp eq i32 %next, 0\n  br i1 %done, label %out, label %loop\n\
        out:\n  ret i64 %sum\n}\n\
        declare void @report(i64)\n\
        define i32 @main() {\n  %v = call i64 @f(i32 2000)\n  call void @report(i64 %v)\n  ret i32 0\n}\n";
    let _ = compiled(&flags(&["-O2"]), text, "main");
}

/// A dense switch at m64 is a table of 4-byte offsets in the read-only data.
/// It was a `.data` section (writable) of `R_X86_64_PLT32` entries, the call
/// kind: a PIE linked with `-z text` refused the one and the linker put a PLT
/// stub behind the other. The pin's are `R_X86_64_PC32` in `.rodata`, each
/// resolving to its slot's `block - table`.
#[test]
fn a_dense_switch_at_m64_is_a_read_only_table_of_pc_relative_entries() {
    let mut text =
        String::from("target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n");
    for i in 0..8 {
        text += &format!("declare void @g{i}()\n");
    }
    text += "define void @f(i32 %x) {\n  switch i32 %x, label %d [\n";
    for i in 0..8 {
        text += &format!("    i32 {i}, label %l{i}\n");
    }
    text += "  ]\n";
    for i in 0..8 {
        text += &format!("l{i}:\n  call void @g{i}()\n  ret void\n");
    }
    text += "d:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
    let object = llrm_codegen::backend::objbuild::built(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap()
    .object;
    let table = object.sections.iter().find(|one| one.role == llrm_object::Role::ROData).expect("a read-only section");
    assert_eq!(table.image.len(), 8 * 4);
    assert_eq!(table.relocs.len(), 8);
    assert!(
        table.relocs.iter().all(|one| one.pc_rel && one.kind == llrm_object::FixupKind::DATA_4),
        "{:?}",
        table.relocs
    );
    // `S + A - P` is `block - table`: the field holds `slot - table`.
    for (k, reloc) in table.relocs.iter().enumerate() {
        assert_eq!(reloc.at, 4 * k);
        assert_eq!(reloc.addend, 4 * k as i64);
    }
}

/// A dense switch at m64 without the "PIC Level" flag (`-fno-pic`) is the pin's
/// static table: a pointer per case, `R_X86_64_64` against the block, jumped
/// through as `jmp *table(,%rax,8)`. Ours was always the PC-relative table of
/// 4-byte offsets, which only a PIE needs.
#[test]
fn a_dense_switch_at_m64_without_the_pic_level_flag_is_a_table_of_pointers() {
    let mut text =
        String::from("target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n");
    for i in 0..8 {
        text += &format!("declare void @g{i}()\n");
    }
    text += "define void @f(i32 %x) {\n  switch i32 %x, label %d [\n";
    for i in 0..8 {
        text += &format!("    i32 {i}, label %l{i}\n");
    }
    text += "  ]\n";
    for i in 0..8 {
        text += &format!("l{i}:\n  call void @g{i}()\n  ret void\n");
    }
    text += "d:\n  ret void\n}\n!llvm.module.flags = !{!0}\n!0 = !{i32 7, !\"uwtable\", i32 2}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
    let object = llrm_codegen::backend::objbuild::built(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap()
    .object;
    let table = object.sections.iter().find(|one| one.role == llrm_object::Role::ROData).expect("a read-only section");
    assert_eq!(table.image.len(), 8 * 8);
    assert!(
        table.relocs.len() == 8
            && table.relocs.iter().all(|one| !one.pc_rel && one.kind == llrm_object::FixupKind::DATA_8),
        "{:?}",
        table.relocs
    );
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("*8]"), "{listing}");
}

/// The same table through the Mach-O and COFF writers: each takes it (the
/// entries are `S + A - P` against the code, as in ELF), and COFF's fields are
/// the `slot + 4 - table` its `REL32` adds the code's address to.
#[test]
fn a_dense_switch_table_is_written_in_mach_o_and_coff_too() {
    let mut text =
        String::from("target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n");
    for i in 0..8 {
        text += &format!("declare void @g{i}()\n");
    }
    text += "define void @f(i32 %x) {\n  switch i32 %x, label %d [\n";
    for i in 0..8 {
        text += &format!("    i32 {i}, label %l{i}\n");
    }
    text += "  ]\n";
    for i in 0..8 {
        text += &format!("l{i}:\n  call void @g{i}()\n  ret void\n");
    }
    text += "d:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
    let write = |format| {
        llrm_codegen::backend::objbuild::written_in(
            &built,
            "f.ll",
            llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
            format,
            llrm_target::Target::objects(&llrm_x86_m64::M64),
        )
        .unwrap()
    };
    assert!(write(llrm_target::object::Format::MachO).windows(6).any(|window| window == b"__text"));
    let coff = write(llrm_target::object::Format::Coff);
    let fields: Vec<u8> = (1..=8u32).flat_map(|k| (4 * k).to_le_bytes()).collect();
    assert!(coff.windows(fields.len()).any(|window| window == fields.as_slice()), "the .rdata fields");
}

/// The pin keeps an x86-64 Mach-O jump table in `__text`, after the code, its
/// entries `block - table` resolved by the assembler; ours was a `__const`
/// section with a relocation per entry.
#[test]
fn a_dense_switch_table_at_m64_is_in_text_without_relocations_in_mach_o() {
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &dense_switch_ir(), "f").unwrap();
    let object = llrm_codegen::backend::objbuild::built_for(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        llrm_target::object::Format::MachO,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap()
    .object;
    assert!(object.sections.iter().all(|one| one.role != llrm_object::Role::ROData), "no read-only section");
    let text = &object.sections[0];
    // The code's own relocations are the calls; the table adds none.
    assert!(text.relocs.iter().all(|one| one.at < text.image.len() - 32), "{:?}", text.relocs);
    let table = text.image.len() - 32;
    for k in 0..8 {
        let entry = i32::from_le_bytes(text.image[table + 4 * k..table + 4 * k + 4].try_into().unwrap());
        // `block - table`: the block is inside the function, before the table.
        assert!(entry < 0 && table as i64 + i64::from(entry) >= 0, "entry {k}: {entry}");
    }
}

/// Eight cases to eight calls: a jump table at m64.
fn dense_switch_ir() -> String {
    let mut text =
        String::from("target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n");
    for i in 0..8 {
        text += &format!("declare void @g{i}()\n");
    }
    text += "define void @f(i32 %x) {\n  switch i32 %x, label %d [\n";
    for i in 0..8 {
        text += &format!("    i32 {i}, label %l{i}\n");
    }
    text += "  ]\n";
    for i in 0..8 {
        text += &format!("l{i}:\n  call void @g{i}()\n  ret void\n");
    }
    text + "d:\n  ret void\n}\n"
}

/// A switch table at m16 with the DAG selector lost its index (m32 never did;
/// its scaled form is checked too):
/// folding the table into the address mode (for the 64-bit table) left the
/// index register as the address's base, which the jump ignored, so
/// `add ax, ax; jmp word ptr $J26` jumped to the table's first entry whatever
/// the case.
#[test]
fn a_dense_switch_at_m16_and_m32_jumps_through_its_index_register() {
    for (mode, layout, int, jump, tail) in [
        ("-m16", "@m16-datalayout@-ni:2:3", "i16", "jmp word ptr cs:$J", "]"),
        ("-m32", "@m32-datalayout@", "i32", "jmp dword ptr $J", "*4]"),
    ] {
        let mut text = format!("target datalayout = \"{layout}\"\n");
        for i in 0..8 {
            text += &format!("declare void @g{i}()\n");
        }
        text += &format!("define void @f({int} %x) {{\n  switch {int} %x, label %d [\n");
        for i in 0..8 {
            text += &format!("    {int} {i}, label %l{i}\n");
        }
        text += "  ]\n";
        for i in 0..8 {
            text += &format!("l{i}:\n  call void @g{i}()\n  ret void\n");
        }
        text += "d:\n  ret void\n}\n";
        let (built, _) = compiled(&flags(&[mode, "-O2", "--isel=dag"]), &text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        let line = listing.lines().map(str::trim).find(|line| line.starts_with(jump)).expect(&listing);
        assert!(line.ends_with(tail), "{mode}: the jump names no index: {line}\n{listing}");
    }
}

/// A function whose `{` is on the line after its name: the pin's first row is
/// `scopeLine` (2), then the first statement (3) where the body starts. Ours
/// began at the first statement, so the line table had no row for the `{`.
#[test]
fn the_first_row_of_a_function_is_its_scope_line_at_m32() {
    for isel in ["--isel=old", "--isel=dag"] {
        let info = debug_info(
            &["-m32", "-O0", "-g", "-fobject-format=elf", isel],
            include_str!("../tests/data/debug_scope_line_m32.ll"),
        );
        let rows: Vec<(usize, u32)> = info.lines.iter().map(|row| (row.offset, row.line)).collect();
        let body = info.functions[0].body.expect("a body").0;
        assert_eq!(rows[0], (0, 2), "{isel}: {rows:?}");
        assert_eq!(rows[1], (body, 3), "{isel}: {rows:?}");
    }
}

/// bench/jpeg_fixed compiled from unoptimized IR at -O2 panicked in the
/// allocator ("Cannot overlap different values"): `v = implicit_def` before a
/// loop and `v := cmove v, x` in it, where the `implicit_def` was dropped and
/// the read stayed, so `v` was live from the function's entry.
#[test]
fn a_loop_carried_value_the_entry_path_never_set_is_allocated() {
    let text = include_str!("../tests/data/implicit_def_loop_carried_m64_o2.ll");
    compiled(&flags(&["-O2"]), text, "f").expect("compiles");
}

/// An instruction with no place at the top of a block is line 0 in the pin,
/// and not a statement; ours left the place of the block before it on every
/// instruction, so a loop's `-O2` table had no line-0 row at all.
#[test]
fn a_block_that_starts_with_no_place_has_a_line_0_row_that_is_not_a_statement() {
    for isel in ["--isel=old", "--isel=dag"] {
        let info = debug_info(
            &["-m32", "-O2", "-g", "-fobject-format=elf", isel],
            include_str!("../tests/data/debug_phi_m32_o2.ll"),
        );
        let zero: Vec<usize> = info.lines.iter().filter(|row| row.line == 0).map(|row| row.offset).collect();
        assert!(!zero.is_empty(), "{isel}: {:?}", info.lines);
        assert!(zero.iter().all(|&at| info.not_stmt.contains(&(0, at))), "{isel}: {:?}", info.not_stmt);
    }
}

/// The debug information of `text` (function `f`) at m32.
fn debug_info(
    arguments: &[&str],
    text: &str,
) -> llrm_debug::Info {
    let (built, _) = compiled(&flags(arguments), text, "f").unwrap();
    llrm_codegen::backend::objbuild::built_for(
        &built,
        "phi.c",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        llrm_target::object::Format::Elf,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap()
    .debug
    .expect("debug information")
}

/// A row on the line of the row before it is not a statement (the pin: a
/// statement is a line change). Ours marked every row of `a * 2 + a` a
/// statement, so a debugger stopped on it once per column.
#[test]
fn a_row_on_the_same_line_as_the_row_before_is_not_a_statement() {
    for isel in ["--isel=old", "--isel=dag"] {
        let info = debug_info(
            &["-m32", "-O0", "-g", "-fobject-format=elf", isel],
            include_str!("../tests/data/debug_same_line_m32.ll"),
        );
        let rows: Vec<(usize, u32)> = info.lines.iter().map(|row| (row.offset, row.line)).collect();
        assert!(rows.windows(2).any(|pair| pair[0].1 == pair[1].1), "premise: two rows on a line: {rows:?}");
        for pair in rows.windows(2) {
            let stmt = !info.not_stmt.contains(&(0, pair[1].0));
            assert_eq!(stmt, pair[0].1 != pair[1].1, "{isel}: {rows:?} not_stmt {:?}", info.not_stmt);
        }
    }
}

/// Two read-modify-writes of one word with another in between (`counts[12]++;
/// counts[14]++; counts[12]++`, split_loops -O3): the first store is dead once
/// the second has the first's value, so the pin's DAG combiner drops it and
/// the pair is one `add word ptr [counts+24], 2`. Ours kept both stores, the
/// value in a register (`mov eax, dword ptr`, `lea ecx, [rax+1]`, `mov word
/// ptr, cx`, ..., `add eax, 2`, `mov word ptr, ax`). The statement-level DSE
/// is off here: it removes the first store before the DAG sees it.
#[test]
fn a_store_the_next_to_the_same_word_overwrites_is_dropped_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @c = internal global [64 x i16] zeroinitializer, align 16\n\
                define void @f() {\n\
                  %a = load i16, ptr getelementptr inbounds (i8, ptr @c, i64 24), align 8\n\
                  %a1 = add i16 %a, 1\n\
                  store i16 %a1, ptr getelementptr inbounds (i8, ptr @c, i64 24), align 8\n\
                  %b = load i16, ptr getelementptr inbounds (i8, ptr @c, i64 28), align 4\n\
                  %b1 = add i16 %b, 1\n\
                  store i16 %b1, ptr getelementptr inbounds (i8, ptr @c, i64 28), align 4\n\
                  %c = load i16, ptr getelementptr inbounds (i8, ptr @c, i64 24), align 8\n\
                  %c1 = add i16 %c, 1\n\
                  store i16 %c1, ptr getelementptr inbounds (i8, ptr @c, i64 24), align 8\n\
                  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-fno-tree-dse"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let stores = listing.lines().filter(|line| line.trim().starts_with("mov word ptr")).count();
    assert!(
        listing.lines().any(|line| line.trim().starts_with("add word ptr") && line.trim().ends_with(", 2")),
        "{listing}"
    );
    assert_eq!(stores, 0, "{listing}");
}

/// `select a, true, freeze b` of two compares is `(x^c)|(y^c) != 0` at x86: the
/// freeze is pushed through the compare (`visitFREEZE`), where kept it hid the
/// `or` of compares from `foldLogicOfSetCCs` and left two `setne`, an `or`, a
/// `movzx` and an `and 1` (memcpy_runtime -O3 m32, 1084 -> 1101 B).
#[test]
fn an_or_of_two_compares_with_one_constant_is_one_setne_at_m32() {
    let ir = "target datalayout = \"@m32-datalayout@\"\n\
              target triple = \"i686-pc-linux-gnu\"\n\
              @to = global [8 x i8] zeroinitializer\n\
              define i32 @f(i32 %acc) {\n  %p = load i8, ptr @to\n  \
              %q = load i8, ptr getelementptr (i8, ptr @to, i32 2)\n  \
              %a = icmp ne i8 %p, -86\n  %b = icmp ne i8 %q, -86\n  \
              %c = select i1 %a, i1 true, i1 %b\n  %z = zext i1 %c to i32\n  \
              %r = add i32 %z, %acc\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-O2", "-march=i486", "--isel=dag"]), ir, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert_eq!(listing.matches("setne").count(), 1, "{listing}");
    assert!(!listing.contains("and ecx, 1"), "{listing}");
}

/// `br (select i1 %u, i1 %bit, i1 false)` is combined as the pin does: the
/// `brcond` is visited first and `rebuildSetCC` makes `setne(select, -1)`
/// before the select becomes an `and`. A select made an `and` ahead of the
/// DAG left `xor(and, -1)`, which De Morgan turned into `setae` + `or`: a
/// second flag in a spill slot (switch_table_holes -O2 m32, 122 -> 132 B).
/// Runs at m64: the hook is off at m32, where it costs sieve -O3 3 B and 172
/// clk.
#[test]
fn a_branch_on_a_bool_select_is_an_and_and_a_compare_not_an_or_of_inverses_at_m64() {
    let ir = "target datalayout = \"@m64-datalayout@\"\n\
              target triple = \"x86_64-unknown-linux-gnu\"\n\
              declare void @a()\ndeclare void @b()\n\
              define void @f(i32 %x) {\n  %u = icmp ult i32 %x, 14\n  %t = trunc i32 %x to i16\n  \
              %s = lshr i16 8707, %t\n  %bit = trunc i16 %s to i1\n  \
              %c = select i1 %u, i1 %bit, i1 false\n  br i1 %c, label %t1, label %e\nt1:\n  \
              call void @a()\n  ret void\ne:\n  call void @b()\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), ir, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("setae"), "the compare was inverted:\n{listing}");
    assert!(!listing.lines().any(|l| l.trim().starts_with("or ")), "the and became an or:\n{listing}");
}

/// `and` of an `icmp ne` and an `icmp ult`, branched on, at m32 -O2: since #8912 the condition is two branches
/// (`cmp; je; add; cmp; ja`), taken in the `and`'s operand order whichever compare the IR defines first. The
/// merged form this test pinned (De Morgan with each compare's IR order, jpeg -O2 +59 B of spill) no longer exists.
#[test]
fn the_compares_of_an_and_are_two_branches_in_operand_order_at_m32() {
    let ir = |first: &str, second: &str| {
        format!(
            "target datalayout = \"@m32-datalayout@\"\n\
             target triple = \"i686-pc-linux-gnu\"\n\
             declare void @a()\ndeclare void @b()\n\
             define void @f(i32 %x) {{\n  %s = add nsw i32 %x, -201\n  {first}\n  {second}\n  \
             %c = and i1 %n, %u\n  br i1 %c, label %t, label %e\nt:\n  call void @a()\n  ret void\ne:\n  \
             call void @b()\n  ret void\n}}\n"
        )
    };
    let ult = "%u = icmp ult i32 %s, 7";
    let ne = "%n = icmp ne i32 %x, 204";
    let listing = |text: String| {
        let (built, _) = compiled(&flags(&["-m32", "-O2", "-march=i486", "--isel=dag"]), &text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    for text in [ir(ult, ne), ir(ne, ult)] {
        let listing = listing(text);
        let at = |needle: &str| listing.lines().position(|l| l.trim().starts_with(needle));
        assert!(!listing.contains("setb") && !listing.contains("seta"), "the compares were joined:\n{listing}");
        let (ne, ult) = (at("cmp eax, 204"), at("cmp eax, 6"));
        assert!(ne.is_some() && ne < ult, "the compares are not in operand order:\n{listing}");
    }
}

/// The copy that puts a call's argument in its register is of the call's line
/// (the pin's `COPY` has the call's `DebugLoc`). Ours had none, so every case
/// block of a switch began with a line-0 row: 5 of them in 8 cases.
#[test]
fn the_argument_move_of_a_switch_case_is_of_the_calls_line() {
    let info = debug_info(
        &["-m32", "-O2", "-g", "-fobject-format=elf", "--isel=dag"],
        include_str!("../tests/data/debug_switch_args_m32_o2.ll"),
    );
    let zero = info.lines.iter().filter(|row| row.line == 0).count();
    assert!(zero <= 2, "{zero} line-0 rows: {:?}", info.lines);
}

/// The record types of an OMF object, in order (type byte, 2-byte length,
/// body).
fn omf_record_types(bytes: &[u8]) -> Vec<u8> {
    let mut types = Vec::new();
    let mut at = 0;
    while at + 3 <= bytes.len() {
        types.push(bytes[at]);
        at += 3 + usize::from(u16::from_le_bytes([bytes[at + 1], bytes[at + 2]]));
    }
    types
}

fn omf_of(arguments: &[&str]) -> Vec<u8> {
    let directory = unique_directory("fsec");
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("two.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
         define i16 @callee(i16 %a) {\n  %b = add i16 %a, 1\n  ret i16 %b\n}\n\
         define i16 @caller(i16 %a) {\n  %b = call i16 @callee(i16 %a)\n  ret i16 %b\n}\n\
         define i16 @unused(i16 %a) {\n  %b = mul i16 %a, 3\n  ret i16 %b\n}\n",
    )
    .unwrap();
    let output = directory.join("two.obj");
    let mut all = vec!["-m16", "-fobject-format=omf", "-o", output.to_str().unwrap()];
    all.extend_from_slice(arguments);
    crate::llc::compile(&flags(&all), &source, None).unwrap();
    let bytes = std::fs::read(output).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    bytes
}

/// A directory name no other call in this process shares. `omf_of` named its
/// directory after its arguments, so the three tests calling it with `&[]`
/// rewrote one `two.ll`/`two.obj` under each other: 4 to 12 of 40 whole-lib
/// runs failed, always a function_sections test, each passing alone.
fn unique_directory(tag: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("{tag}-{}-{call}", std::process::id()))
}

/// Borland's `interrupt` takes the registers it saved as parameters
/// (`bp, di, si, ds, es, dx, cx, bx, ax`). They were read as the frame's words
/// in address order, so `ax` was the high half of EDI and the harness cell
/// CFLL saw `bp = 0, di = 0, si = ES, ds = DS` for a handler that was given
/// BP = 0xB0B0. The description names each parameter's slot; `ax` is the low
/// word of EAX.
#[test]
fn an_interrupt_handler_parameter_is_the_low_word_of_the_register_slot_it_names() {
    let directory = unique_directory("intr");
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("isr.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
         @g = global i16 0\n\
         define x86_intrcc void @isr(i16 %bp, i16 %di, i16 %si, i16 %ds, i16 %es, i16 %dx, i16 %cx, i16 %bx, i16 %ax) {\n  store i16 %ax, ptr @g\n  ret void\n}\n",
    )
    .unwrap();
    let output = directory.join("isr.s");
    crate::llc::compile(&flags(&["-m16", "-S", "-O0", "-o", output.to_str().unwrap()]), &source, None).unwrap();
    let text = std::fs::read_to_string(output).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    // gs fs es ds (2 each), edi esi ebp esp ebx edx ecx (4 each): EAX is 36 bytes up the frame, past BP.
    assert!(text.contains("word ptr [bp+38]"), "{text}");
}

#[test]
fn concurrent_omf_of_calls_with_the_same_arguments_do_not_share_files() {
    assert_ne!(unique_directory("t"), unique_directory("t"));
    let expected = omf_of(&[]);
    let handles: Vec<_> =
        (0..8).map(|_| std::thread::spawn(|| (0..4).map(|_| omf_of(&[])).collect::<Vec<_>>())).collect();
    for handle in handles {
        assert!(handle.join().unwrap().iter().all(|bytes| *bytes == expected));
    }
}

/// The C path wrote one segment for the module's code whatever the program,
/// so the linker could not drop an uncalled function (sfm16_mul: 2687 B of
/// text, 662 B reached). `-ffunction-sections` writes a COMDAT per function.
#[test]
fn function_sections_write_a_comdat_per_function_on_omf() {
    let comdats = |bytes: &[u8]| omf_record_types(bytes).into_iter().filter(|kind| matches!(kind, 0xC2 | 0xC3)).count();
    assert_eq!(comdats(&omf_of(&["-ffunction-sections"])), 3);
    assert_eq!(comdats(&omf_of(&[])), 0, "off by default: today's object");
}

/// With the flag off the object is the one written before the flag existed.
#[test]
fn no_function_sections_is_the_default_object() {
    assert_eq!(omf_of(&["-fno-function-sections"]), omf_of(&[]));
}

/// Every function's COMDAT is allocated in the module's code segment
/// (`<MODULE>_TEXT`), the segment the one-segment layout names: function
/// sections change the grouping, not the segment.
#[test]
fn function_sections_allocate_in_the_modules_code_segment() {
    let has = |object: &[u8]| object.windows(8).any(|window| window == b"TWO_TEXT");
    assert!(has(&omf_of(&["-ffunction-sections"])));
    assert!(has(&omf_of(&[])));
}

/// `-mcode-segment=` was read only on the Nib/QB path: a C module from llrm-cc
/// always went to `<MODULE>_TEXT`, so a BCC caller's near call into it left the
/// segment and the harness cell CNBL crashed.
#[test]
fn mcode_segment_names_the_segment_of_an_llvm_ir_module() {
    let has = |object: &[u8], name: &[u8]| object.windows(name.len()).any(|window| window == name);
    let object = omf_of(&["-mcode-segment=HOT_TEXT"]);
    assert!(has(&object, b"HOT_TEXT") && !has(&object, b"TWO_TEXT"));
}

/// `llrm-llc` on a module stated for Windows or macOS x86-64 answered `no
/// target for x86_64-pc-windows-msvc` (the target knew one triple), so no COFF
/// or Mach-O object of x86-64 could be made to compare with the pin's.
#[test]
fn an_x86_64_module_of_windows_or_macos_is_written_in_that_systems_object_format() {
    for (triple, mangling, magic, format) in [
        ("x86_64-pc-windows-msvc", "w", &[0x64u8, 0x86][..], llrm_target::object::Format::Coff),
        ("x86_64-apple-macosx", "o", &[0xcf, 0xfa, 0xed, 0xfe][..], llrm_target::object::Format::MachO),
        ("x86_64-unknown-linux-gnu", "e", &b"\x7fELF"[..], llrm_target::object::Format::Elf),
    ] {
        let text = format!(
            "target datalayout = \"e-m:{mangling}-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
             target triple = \"{triple}\"\ndeclare void @g()\ndefine void @f() {{\n  call void @g()\n  ret void\n}}\n"
        );
        let (built, chosen) =
            crate::llc::compiled(&flags(&[]), &text, "f").unwrap_or_else(|why| panic!("{triple}: {why}"));
        assert_eq!(chosen, format, "{triple}");
        let object = llrm_codegen::backend::objbuild::written_in(
            &built,
            "f.ll",
            llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
            chosen,
            llrm_target::Target::objects(&llrm_x86_m64::M64),
        )
        .unwrap();
        assert!(object.starts_with(magic), "{triple}: {:02x?}", &object[..8]);
    }
}

/// Windows x86-64 IR was refused while the target lacked its ABI ("the Win64
/// calling convention ... not carried"); the rows have landed, so it compiles
/// with no flag, and with Win64's registers.
#[test]
fn a_windows_module_compiles_with_the_win64_abi_and_no_flag() {
    let text = "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-pc-windows-msvc\"\ndefine i64 @f(i64 %a, i64 %b) {\n  %r = add i64 %a, %b\n  ret i64 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&[]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rcx") && listing.contains("rdx") && !listing.contains("rdi"), "{listing}");
}

/// An x86-64 Mach-O object named its symbols `f` and `g`; the system's linker
/// looks for `_f` and `_g`, so no call resolved.
#[test]
fn an_x86_64_macho_object_spells_its_symbols_with_the_leading_underscore() {
    let text = "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-apple-macosx\"\ndeclare void @g()\ndefine void @f() {\n  call void @g()\n  ret void\n}\n";
    let (built, format) = crate::llc::compiled(&flags(&[]), text, "f").unwrap();
    let object = llrm_codegen::backend::objbuild::written_in(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap();
    let has = |name: &[u8]| object.windows(name.len()).any(|window| window == name);
    assert!(has(b"_f\0") && has(b"_g\0"), "the symbol table lacks `_f`/`_g`");
}

/// The pin's clang writes `x86_64-apple-macosx10.4.0`; the lookup wanted the
/// bare triple, so every module clang made was refused (`no target for ...`)
/// and the x86-64 Mach-O unwind comparison read 0 functions of 60 programs.
#[test]
fn a_triple_with_the_os_version_names_the_same_system() {
    assert!(crate::same_system("x86_64-apple-macosx", "x86_64-apple-macosx10.4.0"));
    assert!(!crate::same_system("x86_64-apple-macosx", "x86_64-pc-windows-msvc"));
    assert!(crate::by_triple("x86_64-apple-macosx10.4.0").is_some());
}

/// clang writes `x86_64-pc-windows-msvc19.0.0`: that module is the Windows
/// system too, and takes Win64's registers like the bare triple.
#[test]
fn a_versioned_windows_triple_takes_the_win64_abi() {
    let text = "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-pc-windows-msvc19.0.0\"\ndefine i64 @f(i64 %a, i64 %b) {\n  %r = add i64 %a, %b\n  ret i64 %r\n}\n";
    let (built, _) = crate::llc::compiled(&flags(&[]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rcx") && listing.contains("rdx") && !listing.contains("rdi"), "{listing}");
}

/// The pin's clang writes `x86_64-apple-macos11` for the OS that compact unwind
/// needs (10.6 or later); the lookup knew only `macosx`, so the Mach-O unwind
/// comparison had to run at the default 10.4, where the pin writes no
/// `__compact_unwind`.
#[test]
fn macos_and_macosx_name_the_same_system() {
    assert!(crate::same_system("x86_64-apple-macosx", "x86_64-apple-macos11"));
    assert!(crate::by_triple("x86_64-apple-macos11").is_some());
    assert!(!crate::same_system("x86_64-apple-macosx", "arm64-apple-macos11"));
}

/// clang writes `"frame-pointer"="all"` for macOS; a leaf function of ours had
/// no `push rbp`, so its unwind rows were empty where the pin's state the frame
/// (16 of 130 functions of the x86-64 Mach-O corpus).
#[test]
fn a_function_whose_attribute_keeps_the_frame_pointer_pushes_it_even_as_a_leaf() {
    let ir = |attribute: &str| {
        format!(
            "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
             target triple = \"x86_64-apple-macosx\"\n\
             define i32 @h(i32 %a) #0 {{\n  %r = mul i32 %a, 3\n  ret i32 %r\n}}\n\
             attributes #0 = {{ nounwind {attribute} }}\n"
        )
    };
    let listing = |attribute: &str| {
        let (built, _) = crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), &ir(attribute), "h").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    assert!(listing("\"frame-pointer\"=\"all\"").contains("push rbp"), "{}", listing("\"frame-pointer\"=\"all\""));
    assert!(!listing("").contains("push rbp"), "control: no attribute, no frame");
}

/// The pin writes no frame address after a pop on Darwin (`emitEpilogue`'s
/// `NeedsDwarfCFI` is off); ours put `DW_CFA_def_cfa rsp, 8` after the `pop
/// rbp` of 75 of 130 functions of the x86-64 Mach-O corpus.
#[test]
fn an_x86_64_macho_frame_states_no_address_at_its_exit_but_elf_does() {
    let ir = |triple: &str, mangling: &str| {
        format!(
            "target datalayout = \"e-m:{mangling}-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
             target triple = \"{triple}\"\n\
             define i32 @h(i32 %a) #0 {{\n  %r = mul i32 %a, 3\n  ret i32 %r\n}}\n\
             attributes #0 = {{ nounwind uwtable \"frame-pointer\"=\"all\" }}\n"
        )
    };
    let def_cfa_rsp_8 = |triple: &str, mangling: &str| {
        let (built, format) = crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), &ir(triple, mangling), "h").unwrap();
        let object = llrm_codegen::backend::objbuild::written_in(
            &built,
            "h.ll",
            llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
            format,
            llrm_target::Target::objects(&llrm_x86_m64::M64),
        )
        .unwrap();
        object.windows(3).filter(|window| *window == [0x0c, 0x07, 0x08]).count()
    };
    // The CIE's initial rows state it once; an exit's rows are a second.
    assert_eq!(def_cfa_rsp_8("x86_64-unknown-linux-gnu", "e"), 2, "control: ELF states the address after the pop");
    assert_eq!(def_cfa_rsp_8("x86_64-apple-macosx", "o"), 1, "Mach-O states none");
}

/// A 64-bit function that calls nothing, or has no locals, pushed its
/// callee-saved registers in the wrong order or after the space for its locals:
/// `push rbx; push r14` where the pin has `push r14; push rbx`, and `sub rsp,
/// 8; push rbx` where the pin has the register first. The unwind rows named
/// other offsets for 16 of 130 functions of the x86-64 Mach-O corpus.
#[test]
fn a_64_bit_frame_pushes_its_registers_first_and_the_last_of_the_list_first() {
    let ir = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
              target triple = \"x86_64-unknown-linux-gnu\"\n\
              define i32 @fib(i32 %n) #0 {\n\
              entry:\n  %c = icmp slt i32 %n, 2\n  br i1 %c, label %done, label %loop\n\
              loop:\n  %i = phi i32 [ %i2, %loop ], [ %n, %entry ]\n  %acc = phi i32 [ %acc2, %loop ], [ 0, %entry ]\n\
              \x20 %a = add nsw i32 %i, -1\n  %r = call i32 @fib(i32 %a)\n  %i2 = add nsw i32 %i, -2\n  %acc2 = add i32 %r, %acc\n\
              \x20 %t = icmp ult i32 %i, 4\n  br i1 %t, label %done, label %loop\n\
              done:\n  %x = phi i32 [ %n, %entry ], [ %i2, %loop ]\n  %y = phi i32 [ 0, %entry ], [ %acc2, %loop ]\n\
              \x20 %z = add i32 %x, %y\n  ret i32 %z\n}\n\
              attributes #0 = { nounwind \"frame-pointer\"=\"all\" }\n";
    let (built, _) = crate::llc::compiled(&flags(&["-O2", "--isel=dag"]), ir, "fib").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let at = |text: &str| listing.find(text).unwrap_or_else(|| panic!("no `{text}` in\n{listing}"));
    assert!(at("push r14") < at("push rbx"), "{listing}");
}

/// A one-slot frame after the saved registers was `sub rsp, 8` / `add rsp, 8`
/// (4 bytes each), where the pin has `push rax` / `pop rcx` (1 byte each): the
/// saved-first order lost the one-slot form, +6 bytes on 84 of 595 cells of the
/// x86-64 corpus (fib -Os, bintree, histo, quicksort ...).
#[test]
fn a_one_slot_frame_after_the_saved_registers_is_push_rax_and_pop_rcx() {
    let ir = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
              target triple = \"x86_64-unknown-linux-gnu\"\n\
              define i32 @fib(i32 %n) #0 {\n\
              entry:\n  %c = icmp slt i32 %n, 2\n  br i1 %c, label %done, label %loop\n\
              loop:\n  %i = phi i32 [ %i2, %loop ], [ %n, %entry ]\n  %acc = phi i32 [ %acc2, %loop ], [ 0, %entry ]\n\
              \x20 %a = add nsw i32 %i, -1\n  %r = call i32 @fib(i32 %a)\n  %i2 = add nsw i32 %i, -2\n  %acc2 = add i32 %r, %acc\n\
              \x20 %t = icmp ult i32 %i, 4\n  br i1 %t, label %done, label %loop\n\
              done:\n  %x = phi i32 [ %n, %entry ], [ %i2, %loop ]\n  %y = phi i32 [ 0, %entry ], [ %acc2, %loop ]\n\
              \x20 %z = add i32 %x, %y\n  ret i32 %z\n}\n\
              attributes #0 = { nounwind optsize }\n";
    let (built, _) = crate::llc::compiled(&flags(&["-Os", "--isel=dag"]), ir, "fib").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("push rbx") || listing.contains("push r14"), "premise: saved registers\n{listing}");
    assert!(listing.contains("push rax") && listing.contains("pop rcx"), "{listing}");
    assert!(!listing.contains("sub rsp, 8") && !listing.contains("add rsp, 8"), "{listing}");
}

/// A 64-bit function with saved registers and no locals got a slot only for
/// alignment: a leaf with two pushed registers `push rax` (+2 B: mandel -Os), a
/// caller with one `sub rsp, 16` (+8 B: sbb_wide_sub, memcpy_runtime -Os) where
/// the pin has neither.
#[test]
fn a_frame_with_saved_registers_and_no_locals_has_no_alignment_slot() {
    let head = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n";
    let compiled = |text: &str, name: &str| {
        let (built, _) = crate::llc::compiled(&flags(&["-Os", "--isel=dag"]), text, name).unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    // sbb_wide_sub's `main`: a volatile function pointer called twice, the
    // result kept across the second call in a saved register, no locals.
    let caller = format!(
        "{head}@through = dso_local global ptr null, align 8\ndeclare void @report(i64)\n\
         define dso_local i32 @main() #0 {{\n  %1 = load volatile ptr, ptr @through, align 8\n  %2 = tail call i64 %1(i64 5, i64 7)\n\
         \x20 %3 = ashr i64 %2, 32\n  tail call void @report(i64 %3)\n  tail call void @report(i64 %2)\n  ret i32 0\n}}\n\
         attributes #0 = {{ nounwind optsize uwtable }}\n"
    );
    let listing = compiled(&caller, "main");
    assert!(listing.contains("push rbx"), "premise: one saved register\n{listing}");
    assert!(!listing.contains("sub rsp") && !listing.contains("push rax"), "{listing}");
    // A leaf that keeps twelve values live across a loop needs saved registers.
    let phis: String = (0..12).map(|i| format!("  %v{i} = phi i64 [ {i}, %entry ], [ %n{i}, %loop ]\n")).collect();
    let adds: String = (0..12).map(|i| format!("  %n{i} = add i64 %v{i}, %v{}\n", (i + 1) % 12)).collect();
    let total: String = (1..12)
        .map(|i| format!("  %s{i} = add i64 {}, %n{i}\n", if i == 1 { "%n0".into() } else { format!("%s{}", i - 1) }))
        .collect();
    let leaf = format!(
        "{head}define i64 @h(i64 %a) #0 {{\nentry:\n  br label %loop\nloop:\n{phis}{adds}  %c = icmp ult i64 %n0, %a\n  br i1 %c, label %loop, label %done\ndone:\n{total}  ret i64 %s11\n}}\nattributes #0 = {{ nounwind optsize }}\n"
    );
    let listing = compiled(&leaf, "h");
    assert!(listing.matches("push r").count() >= 2, "premise: two or more saved registers\n{listing}");
    assert!(!listing.contains("push rax") && !listing.contains("sub rsp"), "{listing}");
}

/// `getOptimalMemOpType` gives 16 bytes (`v4i32`) with SSE2: a 16-byte clear
/// is `xorps` and one 16-byte store (5 B), not two `mov qword [..], 0` (18 B);
/// nbody O2 was +31 B over the pin on four of them.
#[test]
fn a_16_byte_clear_is_xorps_and_one_16_byte_store_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @g = global [16 x i8] zeroinitializer, align 16\n\
                declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                define void @f() {\n  call void @llvm.memset.p0.i64(ptr @g, i8 0, i64 16, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let has = |start: &str| listing.lines().any(|line| line.trim().starts_with(start));
    assert!(has("xorps") && has("movaps"), "{listing}");
    assert!(!listing.contains("qword"), "{listing}");
}

/// m64 never stated `pref_loop_alignment`, so its loops were laid out
/// unaligned where the pin pads every loop header to 16 bytes (not in
/// size-tuned code).
#[test]
fn a_loop_header_is_16_aligned_at_m64_unless_tuned_for_size() {
    let ir = || {
        format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             define i64 @f(ptr %p, i64 %n) {{\n\
             entry:\n  br label %l\n\
             l:\n  %i = phi i64 [ 0, %entry ], [ %j, %l ]\n  %s = phi i64 [ 0, %entry ], [ %t, %l ]\n\
             \x20 %q = getelementptr i64, ptr %p, i64 %i\n  %v = load i64, ptr %q\n  %t = add i64 %s, %v\n\
             \x20 %j = add i64 %i, 1\n  %c = icmp ult i64 %j, %n\n  br i1 %c, label %l, label %x\n\
             x:\n  ret i64 %t\n}}\n"
        )
    };
    let listing = |level: &str| {
        let (built, _) = compiled(&flags(&["-m64", level, "--isel=dag"]), &ir(), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let plain = listing("-O2");
    assert!(plain.contains("    align 16\n"), "{plain}");
    let small = listing("-Os");
    assert!(!small.contains("    align 16\n"), "{small}");
}

/// A jump table's index register is copied by the switch lowering itself, not
/// through `getCopyFromRegs`, so no live-out `AssertZext` wraps it. With one,
/// the narrowed shift needed a `mov eax, edi` re-zero-extending the index in
/// the table block (jpeg_fixed +3 B per table).
#[test]
fn a_jump_table_index_is_not_zero_extended_again_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @a()\ndeclare void @b()\ndeclare void @c()\ndeclare void @d()\n\
                define void @f(i32 %x) {\n\
                \x20 switch i32 %x, label %e [ i32 192, label %la\n i32 193, label %lb\n i32 194, label %lc\n i32 195, label %ld ]\n\
                la:\n  call void @a()\n  ret void\nlb:\n  call void @b()\n  ret void\n\
                lc:\n  call void @c()\n  ret void\nld:\n  call void @d()\n  ret void\ne:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("movsx rcx, dword ptr"), "{listing}");
    let moves: Vec<&str> = listing
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("mov e") && line.split(", ").nth(1).is_some_and(|r| r.starts_with('e')))
        .collect();
    assert!(moves.is_empty(), "{listing}");
}

/// `while (h() != 255 && !bad)` on an `i1` global was `mov al, bad; not al;
/// test al, 1; jne` (11 B); the pin's `rebuildSetCC` makes it `cmp bad, 1; jne`
/// (jpeg_fixed `-Os` m64, two sites).
#[test]
fn a_branch_on_a_negated_bool_global_is_one_compare_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @bad = global i1 false\n\
                declare i32 @h()\n\
                define void @f() {\n\
                br label %lp\nlp:\n  %v = call i32 @h()\n  %c = icmp eq i32 %v, 255\n  br i1 %c, label %x, label %t\n\
                t:\n  %b = load i1, ptr @bad\n  br i1 %b, label %x, label %lp\nx:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("not al"), "{listing}");
    assert!(listing.contains("cmp byte ptr bad, 1"), "{listing}");
}

/// The exit flag of a probe loop (`phi i1` from a compare) read by a branch
/// whose true successor follows was `not cl; test cl, 1; je`, and `and cl, 1;
/// cmp cl, 1` once rebuilt (x_hash `-Os` m32, +5 B on the pin): the phi
/// register knew nothing of its bits. The pin's `ComputePHILiveOutRegInfo`
/// tells it `i1`.
#[test]
fn a_branch_on_an_inverted_bool_phi_is_not_negated_at_m32() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @keys = global [256 x i32] zeroinitializer\n\
                @vals = global [256 x i32] zeroinitializer\n\
                define void @f(i32 %n, i32 %k, i32 %s0) {\n\
                e:\n  %p = getelementptr [4 x i8], ptr @keys, i32 %s0\n  %v = load i32, ptr %p\n\
                \x20 %c0 = icmp eq i32 %v, -1\n  %m0 = icmp eq i32 %v, %k\n  %o0 = or i1 %c0, %m0\n\
                \x20 br i1 %o0, label %j, label %h\n\
                h:\n  br label %lp\n\
                lp:\n  %s = phi i32 [ %s1, %lp ], [ %s0, %h ]\n  %a = add i32 %s, 1\n  %s1 = and i32 %a, 255\n\
                \x20 %q = getelementptr [4 x i8], ptr @keys, i32 %s1\n  %w = load i32, ptr %q\n\
                \x20 %c1 = icmp eq i32 %w, -1\n  %m1 = icmp eq i32 %w, %k\n  %o1 = or i1 %c1, %m1\n\
                \x20 br i1 %o1, label %j, label %lp\n\
                j:\n  %i = phi i32 [ %s0, %e ], [ %s1, %lp ]\n  %fl = phi i1 [ %c0, %e ], [ %c1, %lp ]\n\
                \x20 br i1 %fl, label %ins, label %upd\n\
                ins:\n  %pk = getelementptr [4 x i8], ptr @keys, i32 %i\n  store i32 %k, ptr %pk\n\
                \x20 %pv = getelementptr [4 x i8], ptr @vals, i32 %i\n  store i32 %n, ptr %pv\n  br label %x\n\
                upd:\n  %pu = getelementptr [4 x i8], ptr @vals, i32 %i\n  %o = load i32, ptr %pu\n\
                \x20 %t = add i32 %o, %n\n  store i32 %t, ptr %pu\n  br label %x\n\
                x:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("not ") && !listing.contains("and cl, 1"), "{listing}");
    assert!(listing.contains("test cl, cl"), "{listing}");
}

/// `while (env(i, line) >= 0 && len > 8 && line[0]=='C' && ... line[7]=='=')`
/// as an unrotated loop: the eight byte loads of `line` stayed in the block
/// above the chain, `mov r8, [esp+k]` held across the branches (os_execute m32
/// -Os); the pin sinks each into the chain block that compares it.
#[test]
fn byte_loads_of_a_chain_sink_into_their_compare_blocks_at_m32() {
    let mut body = String::new();
    for (at, byte) in [67, 79, 77, 83, 80, 69, 67, 61].iter().enumerate() {
        let prev = if at == 0 { "%big".to_owned() } else { format!("%c{}", at - 1) };
        body += &format!(
            "  %p{at} = getelementptr inbounds i8, ptr %buf, i32 {at}\n  %v{at} = load i8, ptr %p{at}\n\
             \x20 %e{at} = icmp eq i8 %v{at}, {byte}\n  %c{at} = select i1 {prev}, i1 %e{at}, i1 false\n"
        );
    }
    let text = format!(
        "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n\
         declare i32 @env(i32, ptr)\ndeclare void @hit()\n\
         define void @f() optsize {{\n  %buf = alloca [96 x i8]\n  br label %lp\nlp:\n\
         \x20 %i = phi i32 [ 0, %0 ], [ %n, %chain ]\n  %len = call i32 @env(i32 %i, ptr %buf)\n\
         \x20 %ok = icmp sgt i32 %len, -1\n  br i1 %ok, label %chain, label %out\nchain:\n\
         \x20 %big = icmp ugt i32 %len, 8\n{body}  %n = add i32 %i, 1\n  br i1 %c7, label %yes, label %lp\n\
         yes:\n  call void @hit()\n  ret void\nout:\n  ret void\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m32", "-Os", "--isel=dag"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("mov dl, byte ptr"), "{listing}");
    assert!(listing.matches("cmp byte ptr").count() >= 8, "{listing}");
}

/// `n % 5 == 0` was the divide's multiply-high, `shr`, `lea` and a compare of
/// the product with `n` (7 instructions); the pin's `buildUREMEqFold` is one
/// multiply by the inverse and a compare against `floor((2^32-1)/5)`.
#[test]
fn an_unsigned_remainder_compared_to_a_constant_is_a_multiply_and_a_compare_at_m32() {
    let text = "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n\
                define i1 @f(i32 %n) {\n  %r = urem i32 %n, 5\n  %c = icmp eq i32 %r, 0\n  ret i1 %c\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("cmp eax, 858993460"), "{listing}");
    assert!(!listing.contains("shr ") && !listing.contains("div "), "{listing}");
}

/// The locals' base was aligned to the stack's 16 below the pushed registers
/// (the pin aligns each object to its own and only the total to the stack):
/// a function with one int local and two saved registers had `sub rsp, 24`
/// and the local at `[rsp+12]`, the pin `push rax` and `[rsp+4]`.
#[test]
fn a_one_int_local_costs_one_stack_slot_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @g(ptr)\n\
                define i32 @calls(i32 %x) {\n\
                \x20 %p = alloca i32, align 4\n  store i32 %x, ptr %p\n\
                \x20 %a = call i32 @g(ptr %p)\n  %b = call i32 @g(ptr %p)\n  %s = add i32 %a, %b\n  ret i32 %s\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "calls").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("push rax"), "{listing}");
    assert!(!listing.contains("sub rsp, 24"), "{listing}");
}

/// With the frame register omitted, the call-alignment pad lay below the
/// locals (the pin's place): two `i64` locals sat at `[rsp+8]` and `[rsp+16]`,
/// a disp8 on every access to the lowest. FR1 puts the pad above them.
#[test]
fn the_call_pad_lies_above_the_locals_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @g(ptr, ptr)\n\
                define void @f(i64 %x) {\n\
                \x20 %p = alloca i64, align 8\n  %q = alloca i64, align 8\n  store i64 %x, ptr %p\n  store i64 %x, ptr %q\n\
                \x20 call void @g(ptr %p, ptr %q)\n  call void @g(ptr %q, ptr %p)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("sub rsp, 24"), "{listing}");
    assert!(listing.contains("[rsp]") || listing.contains("lea rdi, [rsp]"), "{listing}");
}

/// -O3 unroll-and-jam never fired on x86-64: the outer loop's size (28) was
/// held to `BasicTTIImpl`'s 28-micro-op loop buffer, so the count was 0
/// (particle: 15850 cycles; gcc's jam of two outer trips, 10262). Jammed, the
/// copies of `px += vx` share one load of `vx`.
#[test]
fn a_nest_with_loads_invariant_in_the_outer_loop_is_jammed_at_o3_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @px = global [101 x i16] zeroinitializer\n@vx = global [101 x i16] zeroinitializer\n\
                @py = global [101 x i16] zeroinitializer\n@vy = global [101 x i16] zeroinitializer\n\
                define void @f() {\nentry:\n  br label %outer\n\
                outer:\n  %t = phi i16 [ 1, %entry ], [ %t1, %next ]\n  br label %inner\n\
                inner:\n  %i = phi i64 [ 0, %outer ], [ %i1, %inner ]\n\
                \x20 %a = getelementptr inbounds nuw [2 x i8], ptr @px, i64 %i\n  %x = load i16, ptr %a\n\
                \x20 %b = getelementptr inbounds nuw [2 x i8], ptr @vx, i64 %i\n  %v = load i16, ptr %b\n\
                \x20 %s = add i16 %v, %x\n  store i16 %s, ptr %a\n\
                \x20 %c = getelementptr inbounds nuw [2 x i8], ptr @py, i64 %i\n  %y = load i16, ptr %c\n\
                \x20 %d = getelementptr inbounds nuw [2 x i8], ptr @vy, i64 %i\n  %w = load i16, ptr %d\n\
                \x20 %r = add i16 %w, %y\n  store i16 %r, ptr %c\n\
                \x20 %i1 = add nuw nsw i64 %i, 1\n  %e = icmp eq i64 %i1, 101\n  br i1 %e, label %next, label %inner\n\
                next:\n  %t1 = add nuw nsw i16 %t, 1\n  %f = icmp eq i16 %t1, 101\n  br i1 %f, label %done, label %outer\n\
                done:\n  ret void\n}\n";
    // The symptom, not the factor: the copies of `px += vx` share one load of
    // `vx` and add it to itself (`add r, r`: x2 for two copies, UJ3's divisor
    // of the 100 outer trips), where an unjammed nest loads `vx` and `vy` once
    // per trip and doubles nothing.
    let shape = |level: &str| {
        let (built, _) = compiled(&flags(&["-m64", level, "--isel=dag"]), text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        let lines = listing.lines().map(str::trim);
        let doubled = lines
            .clone()
            .filter(|line| {
                line.strip_prefix("add ")
                    .and_then(|rest| rest.split_once(", "))
                    .is_some_and(|(a, b)| a == b && !a.starts_with("word"))
            })
            .count();
        let loads = lines.filter(|line| line.starts_with("movzx") && line.contains("ptr [")).count();
        (doubled, loads, listing)
    };
    let (doubled, loads, listing) = shape("-O3");
    assert!(doubled >= 2 && loads == 2, "{listing}");
    // Control: without the jam (-O2) nothing is doubled.
    let (doubled, _, listing) = shape("-O2");
    assert_eq!(doubled, 0, "{listing}");
}

/// fib at m64 was copied into itself to a growth cap (-O2: 150 `gcc_insns`, 25941 clocks, 556 B; gcc's 450: 10793
/// clocks, 1028 B). Since REC-7 (#9043) a recursion is copied two deep at -O2 and -O3 alike: 4 calls at both.
#[test]
fn fib_is_copied_two_deep_into_itself_at_o2_and_o3_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @fib(i32 %n) {\n\
                  %c = icmp slt i32 %n, 2\n\
                  br i1 %c, label %base, label %rec\n\
                base:\n  ret i32 %n\n\
                rec:\n\
                  %a = sub i32 %n, 1\n  %x = call i32 @fib(i32 %a)\n\
                  %b = sub i32 %n, 2\n  %y = call i32 @fib(i32 %b)\n\
                  %r = add i32 %x, %y\n  ret i32 %r\n}\n";
    let calls = |level: &str| {
        let (built, _) = compiled(&flags(&["-m64", level, "--isel=dag"]), text, "fib").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap().matches("call fib").count()
    };
    let (o2, o3) = (calls("-O2"), calls("-O3"));
    assert!(o2 == 4 && o3 == 4, "-O2 made {o2} calls of fib, -O3 {o3}");
}

/// An `i32` divided by a constant at m64 was `mov ecx, 0xcccccccd; mul ecx;
/// shr edx, 3` (the one-operand `mul`, 4 clocks, edx:eax); the pin widens it
/// to `imul rax, rcx; shr rax, 35` (switch -O2: 31690 cycles, clang 27667).
#[test]
fn an_i32_division_by_a_constant_is_one_wide_imul_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(i32 %x) {\n  %r = udiv i32 %x, 10\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("imul rax, rcx") && listing.contains("shr rax, 35"), "{listing}");
    assert!(!listing.contains("mul ecx") && !listing.contains("div"), "{listing}");
}

/// The signed form left `shr rax, 32; sar eax, 2` where the pin has one `sar
/// rax, 34` (`visitSRA`: `sra (trunc (srl x, 32)), 2` is `trunc (sra x, 34)`).
#[test]
fn a_signed_i32_division_by_ten_merges_its_shifts_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(i32 %x) {\n  %r = sdiv i32 %x, 10\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("sar rax, 34"), "{listing}");
    assert!(!listing.contains("shr rax, 32"), "{listing}");
}

/// Static code (no "PIC Level" flag) at m64 made the address of a global a
/// `lea reg, [rip+sym]` (7 bytes) where the pin's `MOV32ri64` is `mov r32,
/// sym` (5 bytes, R_X86_64_32): jpeg_fixed -Os had 20 of them.
#[test]
fn the_address_of_a_global_in_static_code_at_m64_is_a_dword_move() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @arr = internal global [100 x i32] zeroinitializer\n\
                declare void @use(ptr)\n\
                define void @f() {\n  call void @use(ptr @arr)\n  ret void\n}\n\
                !llvm.module.flags = !{!0}\n!0 = !{i32 7, !\"uwtable\", i32 2}\n";
    let (built, format) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let bytes = llrm_codegen::backend::objbuild::written_in(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    // `mov edi, imm32` (BF) against `lea rdi, [rip+disp32]` (48 8D 3D).
    assert!(bytes.windows(5).any(|code| code == [0xBF, 0, 0, 0, 0]), "no `mov edi, sym`: {listing}");
    assert!(!bytes.windows(3).any(|code| code == [0x48, 0x8D, 0x3D]), "a `lea rdi, [rip+sym]`: {listing}");
}

/// A local declared `align 16` sat at `[rsp+4]`: the frame clamped every
/// alignment to 4 (bintree's array; a 16-byte `movaps` clear of it would
/// fault).
#[test]
fn a_local_declared_align_16_is_16_aligned_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                define void @f() {\n  %a = alloca [28 x i8], align 16\n  call void @use(ptr %a)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("lea rdi, [rsp]\n"), "{listing}");
}

/// A 28-byte clear of an `align 16` local used `movaps` at `[rsp+4]`, which
/// faults; every `movaps` must now sit at a 16-aligned offset from `rsp`.
#[test]
fn a_clear_of_an_align_16_local_never_movaps_a_misaligned_address_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr)\n\
                declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                define void @f() {\n  %a = alloca [28 x i8], align 16\n  call void @llvm.memset.p0.i64(ptr %a, i8 0, i64 28, i1 false)\n  call void @use(ptr %a)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let stores: Vec<&str> = listing.lines().map(str::trim).filter(|line| line.starts_with("movaps")).collect();
    assert!(!stores.is_empty(), "{listing}");
    for line in stores {
        let disp = line.split("[rsp").nth(1).and_then(|rest| rest.split(']').next()).unwrap_or("");
        let disp: i64 = disp.trim_start_matches('+').parse().unwrap_or(0);
        assert_eq!(disp % 16, 0, "{listing}");
    }
}

/// Tuned for size, a 128-byte clear is `rep stosd` (9 B, a byte under `stosq`),
/// not `xorps` and eight `movaps` (up to 72 B); a 32-byte clear stays two
/// stores. The pin's store count made it eight `movaps`: pattern_fill -Os +50 B,
/// huffman_decode +12 B over the `rep stos` the port had.
#[test]
fn a_128_byte_clear_at_m64_os_is_rep_stosd_and_a_32_byte_one_is_stores() {
    let ir = |size: u32| {
        format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             @g = global [{size} x i8] zeroinitializer, align 16\n\
             declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
             define void @f() optsize {{\n  call void @llvm.memset.p0.i64(ptr @g, i8 0, i64 {size}, i1 false)\n  ret void\n}}\n"
        )
    };
    let listing = |size: u32| {
        let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), &ir(size), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let large = listing(128);
    assert!(large.contains("rep stosd") && !large.contains("movaps"), "{large}");
    let small = listing(32);
    assert!(small.contains("xorps") && !small.contains("rep stos"), "{small}");
}

/// A 16-byte copy of unaligned memory is `movups` load and store.
#[test]
fn an_unaligned_16_byte_copy_is_movups_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @f(ptr %d, ptr %s) {\n  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 16, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let count = listing.lines().filter(|line| line.trim().starts_with("movups")).count();
    assert_eq!(count, 2, "{listing}");
}

/// A leaf's 100-byte local paid `sub rsp, 104` / `add rsp, 104` (about 12 B);
/// the pin keeps it in the 128-byte red zone below `rsp`. A call, `noredzone`
/// and a frame past the zone keep their `sub`.
#[test]
fn a_leaf_frame_within_128_bytes_has_no_sub_at_m64() {
    let ir = |size: u32, attrs: &str, call: &str| {
        format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             @n = global i64 0\n\
             declare void @g()\n\
             define void @f() {attrs} {{\n  %a = alloca [{size} x i8], align 1\n  %i = load volatile i64, ptr @n\n  %p = getelementptr i8, ptr %a, i64 %i\n  store volatile i8 1, ptr %a\n  store volatile i8 2, ptr %p\n{call}  ret void\n}}\nattributes #0 = {{ noredzone }}\n"
        )
    };
    let listing = |size: u32, attrs: &str, call: &str| {
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &ir(size, attrs, call), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let leaf = listing(100, "", "");
    assert!(!leaf.contains("sub rsp") && !leaf.contains("add rsp") && leaf.contains("[rsp-"), "{leaf}");
    let called = listing(100, "", "  call void @g()\n  store volatile i8 3, ptr %a\n");
    assert!(called.contains("sub rsp"), "{called}");
    assert!(listing(100, "#0", "").contains("sub rsp"));
    // 200 B (208 with the frame slot): 80 B stay above the zone.
    let big = listing(200, "", "");
    assert!(big.contains("sub rsp, 80") && big.contains("add rsp, 80"), "{big}");
}

/// A leaf that pushes callee-saved registers and has a red-zone frame: the
/// saved registers' pops `exits_inserted` puts in the body must not take the
/// zone away at the entry (matmul, nbody_fixed, nbody_single: "the frame cannot
/// be addressed through the stack pointer", the exits at depth 128).
#[test]
fn a_leaf_with_saved_registers_keeps_its_red_zone_at_m64() {
    let loads: String =
        (0..14).map(|i| format!("  %v{i} = load volatile i64, ptr getelementptr (i64, ptr @n, i64 {i})\n")).collect();
    let sums: String = (1..14)
        .map(|i| {
            format!(
                "  %s{i} = add i64 %{prev}, %v{i}\n",
                prev = if i == 1 { "v0".to_owned() } else { format!("s{}", i - 1) }
            )
        })
        .collect();
    let text = format!(
        "target datalayout = \"@m64-datalayout@\"\n\
         target triple = \"x86_64-unknown-linux-gnu\"\n\
         @n = global [16 x i64] zeroinitializer\n\
         define i64 @f(i64 %k) {{\n  %a = alloca [64 x i8], align 8\n  %q = getelementptr i8, ptr %a, i64 %k\n{loads}  store volatile i8 1, ptr %q\n  %m = mul i64 %v1, %v2\n  %m2 = mul i64 %m, %v3\n{sums}  %t = add i64 %s13, %m2\n  %u = mul i64 %t, %v5\n  %w = add i64 %u, %v6\n  %x = mul i64 %w, %v7\n  %y = add i64 %x, %v8\n  %z = add i64 %y, %v9\n  %zz = mul i64 %z, %v10\n  %zzz = add i64 %zz, %v11\n  %r = mul i64 %zzz, %v12\n  %r2 = add i64 %r, %v13\n  %r3 = add i64 %r2, %v0\n  %r4 = mul i64 %r3, %v4\n  ret i64 %r4\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("push rbx") || listing.contains("push r12"), "{listing}");
    assert!(!listing.contains("sub rsp") && listing.contains("[rsp+rdi-"), "{listing}");
}

/// A constant non-zero fill of 64 bytes at m64 was eight `mov qword [..], rcx`
/// (fill_nest Os +20 B over the pin): the pin loads the splat from the
/// constant pool and stores it with `movaps`.
#[test]
fn a_constant_fill_at_m64_is_a_pool_splat_and_16_byte_stores() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @g = global [64 x i8] zeroinitializer, align 16\n\
                declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                define void @f() {\n  call void @llvm.memset.p0.i64(ptr align 16 @g, i8 65, i64 64, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert_eq!(listing.matches("movaps").count(), 5, "{listing}");
    assert!(!listing.contains("qword ptr [rsp") && !listing.contains("movabs"), "{listing}");
}

/// Tuned for size, a 128-byte constant fill (not zero) is a `rep stosb` of
/// `mov al, imm` (9 B; `stosq` needs a 7-10 B value) (idioms Os: eight
/// RIP-relative `movaps` and the pool load were +41 B over it); a 32-byte one
/// stays a pool splat and two `movaps`.
#[test]
fn a_128_byte_constant_fill_at_m64_os_is_rep_stosb_and_a_32_byte_one_is_stores() {
    let ir = |size: u32| {
        format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             @g = global [{size} x i8] zeroinitializer, align 16\n\
             declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
             define void @f() optsize {{\n  call void @llvm.memset.p0.i64(ptr @g, i8 255, i64 {size}, i1 false)\n  ret void\n}}\n"
        )
    };
    let listing = |size: u32| {
        let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), &ir(size), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let (large, small) = (listing(128), listing(32));
    assert!(large.contains("rep stosb") && !large.contains("movaps"), "{large}");
    assert!(small.contains("movaps") && !small.contains("rep stos"), "{small}");
}

/// `x + 1` and `x + 2` of a live 64-bit source were `mov rax, rdi; inc rax` /
/// `mov rax, rdi; add rax, 2` (6-7 B each); the pin's `convertToThreeAddress`
/// makes an `ADD64ri` and an `INC64r` of a live source a `lea` (4 B)
/// (os_locals_between_pushes O2 +27 B).
#[test]
fn a_64_bit_add_of_a_live_source_is_a_lea_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(i64, i64, i64)\n\
                define void @f(i64 %x) {\n  %a = add i64 %x, 1\n  %b = add i64 %x, 2\n  call void @use(i64 %x, i64 %a, i64 %b)\n  call void @use(i64 %x, i64 %x, i64 %x)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("lea rsi, [rbx+1]") && listing.contains("lea rdx, [rbx+2]"), "{listing}");
    assert!(!listing.contains("add r") && !listing.contains("inc r"), "{listing}");
}

/// A bit-field read (`shl 24; ashr 24` of a dword load, then `sext`) was a
/// dword load, `movsx eax, al` and `movsxd` (bitfield_overhang_init 137 B,
/// 34 vs the pin's 28 instructions); it is one `movsx rdi, byte ptr`.
#[test]
fn a_signed_bit_field_read_is_one_byte_sextload_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @g = global i32 0, align 4\n\
                define i64 @f() {\n  %1 = load i32, ptr @g, align 4\n  %2 = shl i32 %1, 24\n  %3 = ashr exact i32 %2, 24\n  %4 = sext i32 %3 to i64\n  ret i64 %4\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("movsx rax, byte ptr"), "{listing}");
    assert!(!listing.contains("movsx eax, al") && !listing.contains("mov eax, dword"), "{listing}");
}

/// The same bit-field read at m32: one `movsx eax, byte ptr`, not a dword
/// load and `movsx eax, al` (stft_fixed and bitfield_overhang_init A/B).
#[test]
fn a_signed_bit_field_read_is_one_byte_sextload_at_m32() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @g = global i32 0, align 4\n\
                define i32 @f() {\n  %1 = load i32, ptr @g, align 4\n  %2 = shl i32 %1, 24\n  %3 = ashr exact i32 %2, 24\n  ret i32 %3\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("movsx eax, byte ptr"), "{listing}");
    assert!(!listing.contains("movsx eax, al"), "{listing}");
}

/// A signed word of a global at m32 is `mov eax, [g]; cwde` (5 + 1 B), not
/// `movsx eax, word ptr [g]` (7 B): the sextload fold cost 33 B on jpeg_fixed
/// -O2 and +2 B at -Os; the peephole splits it where the pair is shorter.
#[test]
fn a_signed_word_of_a_global_is_a_load_and_cwde_at_m32() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                @g = global i32 0, align 4\n\
                define i32 @f() {\n  %1 = load i32, ptr @g, align 4\n  %2 = shl i32 %1, 16\n  %3 = ashr exact i32 %2, 16\n  ret i32 %3\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("movsx eax, ax") && !listing.contains("movsx eax, word ptr"), "{listing}");
}

/// A nest whose inner loop the O3 unroll removes: the parent's IndVars sees the
/// unrolled body and writes the sum as a constant, the loop goes (byte_multiply
/// O3 1636 B against the pin's 18 instructions).
#[test]
fn a_parent_of_an_unrolled_loop_is_folded_by_indvars_at_m64_o3() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @report(i64)\n\
                define void @f() {\nentry:\n  br label %outer\n\
                outer:\n  %a = phi i32 [ 0, %entry ], [ %an, %latch ]\n  %s = phi i64 [ 0, %entry ], [ %p2, %latch ]\n  %t = trunc i32 %a to i8\n  br label %inner\n\
                inner:\n  %b = phi i32 [ 0, %outer ], [ %bn, %inner ]\n  %p = phi i64 [ %s, %outer ], [ %p2, %inner ]\n  %tb = trunc i32 %b to i8\n  %m = mul i8 %tb, %t\n  %z = zext i8 %m to i64\n  %p2 = add i64 %p, %z\n  %bn = add i32 %b, 7\n  %c = icmp ult i32 %bn, 256\n  br i1 %c, label %inner, label %latch\n\
                latch:\n  %an = add i32 %a, 5\n  %c2 = icmp ult i32 %an, 256\n  br i1 %c2, label %outer, label %exit\n\
                exit:\n  call void @report(i64 %p2)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O3", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("mov rdi, 229508"), "{listing}");
}

/// A 31-byte struct copy at m64 -Os was a `call memcpy`, then the cheaper of two
/// overlapping 16-byte pieces and the call (DEVIATE MEM-OS, #7892). With REP-MOVS-OS
/// `mov ecx, n; rep movsb` is smaller than both and is taken.
#[test]
fn a_31_byte_copy_at_m64_os_is_rep_movsb() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @f(ptr %d, ptr %s) optsize {\n  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 31, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rep movsb"), "{listing}");
    assert!(!listing.contains("memcpy") && !listing.contains("movups"), "{listing}");
}

/// A 136-byte local read through an index at m64: the red zone put `-0x80` on
/// every access (a disp8 where `[rsp+rdi*2]` had none) to save a `sub`/`add`
/// pair (shellsort O3 +23 B over the frame it replaced).
#[test]
fn a_red_zone_that_costs_displacement_bytes_is_not_taken_at_m64() {
    let accesses: String = (0..12)
        .map(|k| format!("  %p{k} = getelementptr [68 x i16], ptr %a, i64 0, i64 %i\n  store volatile i16 {k}, ptr %p{k}, align 2\n"))
        .collect();
    let text = format!(
        "target datalayout = \"@m64-datalayout@\"\n\
         target triple = \"x86_64-unknown-linux-gnu\"\n\
         define void @f(i64 %i) {{\n  %a = alloca [68 x i16], align 2\n{accesses}  ret void\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("sub rsp") || listing.contains("push"), "{listing}");
    assert!(!listing.contains("rsp+rdi*2-"), "{listing}");
}

/// `(x + y) & 0xffffffff` was a 64-bit `add` and a `movzx` (REX.W): the pin's
/// `ShrinkDemandedOp` runs it as one 32-bit `add`, which zero-extends for free
/// at x86-64 (pattern_fill Os).
#[test]
fn a_zero_extended_operand_is_computed_at_32_bits_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(i64 %x, i64 %y) {\n  %s = add i64 %x, %y\n  %m = and i64 %s, 4294967295\n  ret i64 %m\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("rax") && !listing.contains("movzx"), "{listing}");
}

/// The pin's `ShrinkDemandedOp` made `(and i16 x, 1023)` scaled by 8 a 32-bit
/// `shl` plus `zext`, and the address stopped using `*8` (bench ring, +2 B,
/// 40 cells above release/next): the x86 matcher widens `zext(shl nuw)`.
#[test]
fn a_zero_extended_shifted_index_is_a_scaled_address_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(ptr %p, i16 %i) {\n  %m = and i16 %i, 1023\n  %z = zext nneg i16 %m to i64\n  %g = getelementptr inbounds nuw [8 x i8], ptr %p, i64 %z\n  %v = load i64, ptr %g\n  ret i64 %v\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("*8]") && !listing.contains("shl"), "{listing}");
}

/// `isDef32` answered `false` for every node, so `zext (shl i32)` stayed an
/// explicit `movzx rax, eax` after a shift that already zeroes the upper half
/// (bench bitfield_long_long, +1 B per site).
#[test]
fn a_zero_extended_32_bit_definition_costs_no_instruction_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(i32 %x) {\n  %s = shl i32 %x, 8\n  %z = zext i32 %s to i64\n  ret i64 %z\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("movzx") && !listing.contains("mov eax, eax"), "{listing}");
}

/// A node the address matcher made (the widened shift) was selected before
/// the `add` that uses it, so the `add` saw a machine node and lost its scaled
/// `lea`: `b + 2*(x != 0) + (y != 3)` was `add rcx, rcx` and two `add`s
/// (bench logical_value, +7 B).
#[test]
fn a_node_made_by_the_address_matcher_is_selected_after_its_user_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(i64 %b, i32 %x, i32 %y) optsize {\n  %c = icmp eq i32 %x, 0\n  %s = select i1 %c, i64 0, i64 2\n  %a = add nsw i64 %s, %b\n  %d = icmp ne i32 %y, 3\n  %z = zext i1 %d to i64\n  %r = add nsw i64 %a, %z\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("*2]"), "{listing}");
}

/// The pin turns `x * 10` and `x * 7` into `lea; lea` and `lea; sub`: one more
/// instruction and 3 B for 1 clock, a loss at one clock per two bytes
/// (frames/histo -O2: +4.6 % / +5.7 % instructions, +3 B). The `imul` stays;
/// `x * 5` is one `lea` and stays that.
#[test]
fn a_multiply_by_10_or_7_stays_an_imul_and_by_5_a_lea_at_m64() {
    let listing = |c: u32| {
        let text = format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             define i32 @f(i32 %x, i32 %y) {{\n  %m = mul i32 %x, {c}\n  %s = sub i32 %y, %m\n  ret i32 %s\n}}\n"
        );
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    for c in [10, 7] {
        let listing = listing(c);
        assert!(listing.contains("imul") && !listing.contains("lea"), "{c}: {listing}");
    }
    let five = listing(5);
    assert!(five.contains("lea") && !five.contains("imul"), "{five}");
}

/// `x * 10 + y` is `lea; lea`: the chain's last step folds into the `add`.
/// The priced `imul` left an `add` behind (struct_odd_sizes -O2: 157 B against
/// 154 B before the pricing and 152 B for the pin).
#[test]
fn a_product_summed_into_an_add_keeps_the_lea_chain_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(i32 %x, i32 %y) {\n  %m = mul i32 %x, 10\n  %s = add i32 %y, %m\n  ret i32 %s\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("lea") && !listing.contains("imul"), "{listing}");
}

/// The Os fill rule flips at a length that depends on the byte: the value the
/// string fill needs in `rax` costs 2 B (`xor`) for 0, 7 for 255 (`mov rax,
/// -1`) and 10 (`movabs`) otherwise, and the splat's load 3, 7 and 7. Each
/// byte is stores one step below the boundary and `rep stos` at it, of the unit
/// with the fewest bytes: `stosd` for zero, `stosb` (`mov al, imm`) otherwise.
#[test]
fn the_os_fill_rule_flips_at_a_length_that_depends_on_the_byte_at_m64() {
    let listing = |byte: u32, size: u32| {
        let text = format!(
            "target datalayout = \"@m64-datalayout@\"\n\
             target triple = \"x86_64-unknown-linux-gnu\"\n\
             @g = global [{size} x i8] zeroinitializer, align 16\n\
             declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
             define void @f() optsize {{\n  call void @llvm.memset.p0.i64(ptr @g, i8 {byte}, i64 {size}, i1 false)\n  ret void\n}}\n"
        );
        let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), &text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    // (byte, the longest fill that stays stores, the shortest that is `rep
    // stos`, its unit)
    for (byte, stores, rep, unit) in [(0, 32, 48, "stosd"), (255, 32, 48, "stosb"), (65, 48, 64, "stosb")] {
        let (inline, string) = (listing(byte, stores), listing(byte, rep));
        assert!(inline.contains("movaps") && !inline.contains("rep stos"), "byte {byte}, {stores} B: {inline}");
        assert!(string.contains(&format!("rep {unit}")) && !string.contains("movaps"), "byte {byte}, {rep} B: {string}");
    }
}

/// A 72-byte fill and copy at m64 -Os (`pattern_fill.c`,
/// `huffman_decode.c`, `idioms.c`): `rep stosq` took its value from EAX where
/// its form reads RAX, and the assembler had no `stosq`/`movsq`: "a division
/// operand (RAX) not set by a glued CopyToReg" refused the function. The fill
/// byte is a register's, so the fill stays `rep stosb`: a constant fill is a
/// pool splat and 16-byte stores (the pin's `getOptimalMemOpType`), and the
/// `rep stosq` of a large clear is the 128-byte test's.
#[test]
fn a_register_fill_and_a_quadword_copy_at_m64_are_rep_string_instructions() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @use(ptr, ptr)\n\
                declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @f(ptr %s, i8 %v) optsize {\n  %b = alloca [72 x i8], align 16\n  %c = alloca [72 x i8], align 16\n\
                \x20 call void @llvm.memset.p0.i64(ptr align 16 %c, i8 %v, i64 72, i1 false)\n\
                \x20 call void @llvm.memcpy.p0.p0.i64(ptr align 16 %b, ptr align 16 %s, i64 72, i1 false)\n\
                \x20 call void @use(ptr %b, ptr %c)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rep stosb") && listing.contains("rep movsd"), "{listing}");
}

/// `mov rdi, rbx` (3 B) of a value a dword write zeroed above bit 31 is `mov
/// edi, ebx` (2 B): the pin emits the `movq`, gcc the dword (bench_io,
/// os_files, os_find +1 B over release/next).
#[test]
fn a_copy_of_a_zero_extended_dword_is_a_dword_copy_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @h()\ndeclare void @g(i64)\n\
                define i64 @f() {\n  %r = call i32 @h()\n  %z = zext i32 %r to i64\n  call void @g(i64 %z)\n  ret i64 %z\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("mov edi, ebx") && !listing.contains("mov rdi, rbx"), "{listing}");
}

/// A copy of a register nothing proved zero above bit 31 stays 64 bits wide.
#[test]
fn a_copy_of_a_full_width_value_stays_a_qword_copy_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @g(i64)\n\
                define i64 @f(i64 %x) {\n  call void @g(i64 %x)\n  ret i64 %x\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("mov rbx, rdi") && !listing.contains("mov ebx, edi"), "{listing}");
}

/// SysV x86-64 passes an `i128` in two consecutive registers and returns it in
/// RAX:RDX; with fewer than two left it takes 16 aligned bytes of the stack.
/// It was refused ("a i128 value"), and with only the size known the second
/// half was read from the first one's bytes.
#[test]
fn a_sysv_i128_argument_takes_two_registers_or_sixteen_aligned_stack_bytes() {
    let listing_of = |body: &str, name: &str| {
        let text = format!("{SYSV}{body}");
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, name).unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let add = listing_of("define i128 @f(i128 %a, i128 %b) {\n  %r = add i128 %a, %b\n  ret i128 %r\n}\n", "f");
    assert!(add.contains("adc") && !add.contains("[rsp"), "registers rdi:rsi and rdx:rcx: {add}");
    // Five integers leave R9 alone: the pair goes to the stack, the next
    // integer takes R9.
    let spilled = listing_of(
        "define i128 @f(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i128 %x, i64 %g) {\n  %p = add i128 %x, 1\n  %q = zext i64 %g to i128\n  %r = add i128 %p, %q\n  ret i128 %r\n}\n",
        "f",
    );
    assert!(spilled.contains("[rsp+8]") && spilled.contains("[rsp+16]") && spilled.contains("r9"), "{spilled}");
    // A seventh integer on the stack leaves the pair 8 bytes off its
    // 16-byte alignment: it skips them.
    let aligned = listing_of(
        "define i128 @f(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i64 %f, i64 %g, i128 %h) {\n  %p = zext i64 %g to i128\n  %r = add i128 %h, %p\n  ret i128 %r\n}\n",
        "f",
    );
    assert!(aligned.contains("[rsp+8]") && aligned.contains("[rsp+24]") && aligned.contains("[rsp+32]"), "{aligned}");
}

/// Win64 puts argument `n` in RCX, RDX, R8, R9 by position, the fifth on the
/// stack above the 32-byte home area the caller reserved.
#[test]
fn a_win64_function_takes_its_arguments_by_position_above_the_home_area() {
    let text = format!(
        "{SYSV}declare i64 @g(i64, i64, i64, i64, i64)\ndefine i64 @f(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e) {{\n  %r = call i64 @g(i64 %e, i64 %d, i64 %c, i64 %b, i64 %a)\n  %s = add i64 %r, 1\n  ret i64 %s\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    // g(e, d, c, b, a): the fifth argument is stack slot 4 over the 32-byte
    // home area, and the callee's own fifth is above it, past the return
    // address.
    assert!(lines.contains(&"mov qword ptr [rsp+32], rcx"), "{listing}");
    assert!(lines.contains(&"sub rsp, 40"), "{listing}");
    assert!(lines.contains(&"mov rcx, qword ptr [rsp+80]"), "{listing}");
    assert!(!listing.contains("rdi") && !listing.contains("rsi"), "SysV registers: {listing}");
}

/// A Win64 function that keeps doubles in xmm6/xmm7 across a call must save
/// and reload them (callee-saved there); with no save it clobbered the
/// caller's xmm6/xmm7. SysV has none callee-saved and gets no `movaps`.
/// Win64 splits a 128-bit integer into two i64 pieces, each taking the next
/// argument position (pin `llc`: `f(i128, i128)` is RCX:RDX and R8:R9, the
/// result RAX:RDX; a pair past the fourth position is stack slots 4 and 5,
/// `[rsp+40]` and `[rsp+48]`). The module was refused (row W64-1).
#[test]
fn a_win64_i128_argument_is_two_consecutive_i64_positions() {
    let listing_of = |body: &str| {
        let text = format!("{SYSV}{body}");
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let add = listing_of("define i128 @f(i128 %a, i128 %b) {\n  %r = add i128 %a, %b\n  ret i128 %r\n}\n");
    assert!(add.contains("adc") && add.contains("r8") && add.contains("r9") && !add.contains("[rsp"), "{add}");
    let stacked = listing_of(
        "define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d, i128 %x) {\n  %t = trunc i128 %x to i64\n  %s = lshr i128 %x, 64\n  %u = trunc i128 %s to i64\n  %r = sub i64 %t, %u\n  ret i64 %r\n}\n",
    );
    assert!(stacked.contains("[rsp+40]") && stacked.contains("[rsp+48]"), "{stacked}");
    // A call passes the pair in R8:R9 after the first two arguments (a tail
    // call here) and the result comes back in RAX:RDX.
    let call = listing_of(
        "declare i128 @g(i64, i64, i128)\ndefine i128 @f(i64 %a, i128 %b) {\n  %r = call i128 @g(i64 %a, i64 %a, i128 %b)\n  ret i128 %r\n}\n",
    );
    assert!(call.contains("jmp g") && call.contains("mov r9, r8") && call.contains("mov r8, rax"), "{call}");
}

#[test]
fn a_win64_function_saves_the_xmm_registers_it_keeps_across_a_call() {
    let body = "declare double @g(double)\ndefine double @f(double %a, double %b) {\n  %x = call double @g(double %a)\n  %y = call double @g(double %b)\n  %s = fadd double %x, %y\n  %t = fadd double %s, %a\n  %u = fadd double %t, %b\n  ret double %u\n}\n";
    let text = format!("{SYSV}{body}");
    let listing_of = |abi: &str| {
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", abi]), &text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let win = listing_of("-mabi=win64");
    let lines: Vec<&str> = win.lines().map(str::trim).collect();
    let offset = |line: &&str| -> Option<i64> {
        let rest = line.strip_prefix("movaps xmmword ptr [rsp+")?;
        rest.split(']').next()?.parse().ok()
    };
    let saves: Vec<i64> = lines.iter().filter_map(offset).collect();
    let reloads =
        lines.iter().filter(|line| line.starts_with("movaps xmm") && line.contains(", xmmword ptr [rsp+")).count();
    assert!(saves.len() >= 2 && reloads == saves.len(), "{win}");
    assert!(saves.iter().all(|at| at % 16 == 0), "movaps faults off a 16-byte boundary: {win}");
    let sysv = listing_of("-mabi=sysv");
    assert!(!sysv.contains("movaps"), "{sysv}");
}

/// With an `alloca` the stack pointer moves after the prologue, so the xmm
/// reloads must address the saves from the frame register, not `rsp`.
#[test]
fn a_win64_function_with_alloca_reloads_its_xmm_saves_through_the_frame_register() {
    let body = "declare double @g(double, ptr)\ndefine double @f(double %a, double %b, i64 %n) {\n  %p = alloca i8, i64 %n, align 16\n  %x = call double @g(double %a, ptr %p)\n  %y = call double @g(double %b, ptr %p)\n  %s = fadd double %x, %y\n  %t = fadd double %s, %a\n  %u = fadd double %t, %b\n  ret double %u\n}\n";
    let text = format!("{SYSV}{body}");
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    // The frame register lies the pushes and the `sub` above the stack pointer.
    let pushes = lines
        .iter()
        .take_while(|l| !l.starts_with("sub rsp"))
        .filter(|l| l.starts_with("push") && **l != "push rbp")
        .count() as i64;
    let sub: i64 = lines.iter().find_map(|l| l.strip_prefix("sub rsp, ")?.parse().ok()).unwrap();
    let distance = 8 * pushes + sub;
    let number = |l: &str, open: &str| -> i64 {
        let rest = &l[l.find(open).unwrap() + open.len()..];
        rest.split(']').next().unwrap().replace('+', "").parse().unwrap()
    };
    let saves: Vec<i64> =
        lines.iter().filter(|l| l.starts_with("movaps xmmword ptr [rsp+")).map(|l| number(l, "[rsp")).collect();
    let reloads: Vec<i64> = lines
        .iter()
        .filter(|l| l.starts_with("movaps xmm") && l.contains(", xmmword ptr [rbp"))
        .map(|l| number(l, "[rbp"))
        .collect();
    assert!(!saves.is_empty(), "{listing}");
    assert_eq!(reloads, saves.iter().map(|at| at - distance).collect::<Vec<_>>(), "{listing}");
}

/// Win64 passes a variadic call's double in both the vector and the integer
/// register of its position (`printf("%f", d)` reads it from RDX).
#[test]
fn a_win64_variadic_call_repeats_a_double_in_the_integer_register() {
    let body = "declare i32 @p(ptr, ...)\ndefine i32 @f(ptr %s, double %d) {\n  %r = call i32 (ptr, ...) @p(ptr %s, double %d)\n  ret i32 %r\n}\n";
    let text = format!("{SYSV}{body}");
    let listing_of = |abi: &str| {
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", abi]), &text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let win = listing_of("-mabi=win64");
    assert!(win.contains("movq rdx, xmm1"), "{win}");
    assert!(!win.contains("mov al"), "Win64 has no vector count: {win}");
    let sysv = listing_of("-mabi=sysv");
    assert!(sysv.contains("mov al, 1") && !sysv.contains("rdx"), "{sysv}");
}

/// A Win64 variadic function finds its unnamed register arguments in the home
/// area: the callee stores RDX, R8 and R9 there, and `va_start` points at the
/// first.
#[test]
fn a_win64_variadic_function_homes_its_register_arguments_for_va_start() {
    let body = "declare void @llvm.va_start.p0(ptr)\ndefine i32 @f(i32 %n, ...) {\n  %ap = alloca ptr\n  call void @llvm.va_start.p0(ptr %ap)\n  %p = load ptr, ptr %ap\n  %v = load i32, ptr %p\n  ret i32 %v\n}\n";
    let text = format!("{SYSV}{body}");
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    // The homes of RDX, R8 and R9: the return address is at [rsp], the home of
    // the first argument above it.
    let below: i64 = lines.iter().find_map(|l| l.strip_prefix("sub rsp, ")?.parse().ok()).unwrap_or(0);
    for (at, register) in [(16, "rdx"), (24, "r8"), (32, "r9")] {
        let home = format!("mov qword ptr [rsp+{}], ", at + below);
        assert!(
            lines.iter().any(|l| l.starts_with(&home)),
            "{register} not homed {at} above the return address: {listing}"
        );
    }
    assert!(!listing.contains("test al"), "SysV register save area: {listing}");
    // Win64 has no red zone: nothing is written below the stack pointer.
    assert!(!listing.contains("[rsp-"), "{listing}");
}

/// Win64 passes the hidden result pointer in RCX, the first position, so the
/// next argument is in RDX; the callee returns the pointer in RAX.
#[test]
fn a_win64_struct_result_shifts_the_arguments_by_one_position() {
    let body = "define void @f(ptr sret([3 x i64]) align 8 %o, i64 %a) {\n  store i64 %a, ptr %o\n  ret void\n}\ndeclare void @h(ptr sret([3 x i64]) align 8, i64)\ndefine void @g(ptr %p) {\n  call void @h(ptr sret([3 x i64]) align 8 %p, i64 5)\n  ret void\n}\n";
    let text = format!("{SYSV}{body}");
    let listing_of = |name: &str| {
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, name).unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let callee = listing_of("f");
    assert!(callee.contains("], rdx"), "{callee}");
    assert!(callee.contains("mov rax, rcx"), "{callee}");
    let caller = listing_of("g");
    assert!(caller.contains("dx, 5"), "{caller}");
}

/// A variadic Win64 function with a variable-sized `alloca` homes its
/// register arguments through the frame register: the stack pointer moves.
#[test]
fn a_win64_variadic_function_with_alloca_homes_through_the_frame_register() {
    let body = "declare void @llvm.va_start.p0(ptr)\ndeclare void @u(ptr, ptr)\ndefine void @f(i64 %n, ...) {\n  %ap = alloca ptr\n  call void @llvm.va_start.p0(ptr %ap)\n  %buf = alloca i8, i64 %n, align 16\n  call void @u(ptr %buf, ptr %ap)\n  ret void\n}\n";
    let text = format!("{SYSV}{body}");
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    for at in [24, 32, 40] {
        let home = format!("mov qword ptr [rbp+{at}], ");
        assert!(listing.contains(&home), "[rbp+{at}] not a home: {listing}");
    }
    assert!(!listing.contains("[rsp-"), "{listing}");
}

/// Row W64-1: Win64 shapes with their own ABI rules (a 128-bit integer, a
/// vector, a by-value struct) are refused, not passed in SysV's way.
#[test]
fn a_win64_function_with_an_unprobed_shape_is_refused() {
    // (function, whether it is the Win64 refusal that stops it: the selector
    // refuses a 128-bit integer and a vector under SysV too)
    let cases = [
        ("define i256 @f(i256 %a) {\n  ret i256 %a\n}\n", false),
        ("define <4 x i32> @f(<4 x i32> %a) {\n  ret <4 x i32> %a\n}\n", false),
        ("define i64 @f(ptr byval([3 x i64]) align 8 %p) {\n  %v = load i64, ptr %p\n  ret i64 %v\n}\n", true),
    ];
    for (body, by_row) in cases {
        let text = format!("{SYSV}{body}");
        let refused = compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=win64"]), &text, "f").err();
        assert!(refused.is_some(), "silently compiled: {body}");
        if by_row {
            assert!(refused.as_ref().is_some_and(|why| why.contains("W64-1")), "{refused:?} for {body}");
            assert!(compiled(&flags(&["-m64", "-O2", "--isel=dag", "-mabi=sysv"]), &text, "f").is_ok(), "{body}");
        }
    }
}

/// `(x & 0xffff0000) == 0` on an `i64` is a shift and a 16-bit test, the pin's
/// shifted-mask case of `X86ISD::CMP`: the mask needs a 64-bit immediate
/// otherwise, and the `and` held a copy of `x` live. Not translated, it was
/// `mov`, `and`, `test rax, rax`.
#[test]
fn a_64_bit_compare_with_a_shifted_mask_is_a_shift_and_a_narrow_test() {
    let listing_of = |mask: u64| {
        let body =
            format!("define i1 @f(i64 %x) {{\n  %a = and i64 %x, {mask}\n  %c = icmp eq i64 %a, 0\n  ret i1 %c\n}}\n");
        let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &format!("{SYSV}{body}"), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let middle = listing_of(0xFFFF_0000);
    assert!(middle.contains("shr rdi, 16") && middle.contains("test di, di"), "{middle}");
    assert!(!middle.contains("and "), "{middle}");
    // A mask reaching the low bit is a `shl` (one reaching the sign bit is
    // `icmp ult` before selection).
    let low = listing_of(0x0000_FFFF_FFFF_FFFF);
    assert!(low.contains("shl rdi, 16"), "{low}");
    assert!(!low.contains("and ") && !low.contains("movabs"), "{low}");
}

/// ZIMM priced `0x9E3779B9` at one instruction in a size build too, so
/// ConstantHoisting kept every `mov r32, imm32` (5 B) where one register
/// copy (3 B) did: `mul_constants` -Os +30 B.
#[test]
fn a_zero_extended_constant_used_many_times_in_a_size_build_is_materialized_once_at_m64() {
    // Eight loops, each seeded with the same constant, as `mul_constants` has.
    let loops: String = (0..8)
        .map(|i| {
            format!(
                "pre{i}:\n  br label %l{i}\n\
                 l{i}:\n  %x{i} = phi i64 [ 2654435769, %pre{i} ], [ %n{i}, %l{i} ]\n\
                 \x20 %c{i} = phi i64 [ 0, %pre{i} ], [ %d{i}, %l{i} ]\n\
                 \x20 %m{i} = mul i64 %x{i}, 1664525\n  %n{i} = add i64 %m{i}, 1013904223\n\
                 \x20 %d{i} = add i64 %c{i}, 1\n  %e{i} = icmp eq i64 %d{i}, 200\n\
                 \x20 br i1 %e{i}, label %o{i}, label %l{i}\n\
                 o{i}:\n  call void @g(i64 %n{i})\n  br label %pre{next}\n",
                next = i + 1
            )
        })
        .collect();
    let text = format!(
        "target datalayout = \"@m64-datalayout@\"\n\
         target triple = \"x86_64-unknown-linux-gnu\"\n\
         declare void @g(i64)\n\
         define void @f() optsize {{\nentry:\n  br label %pre0\n{loops}pre8:\n  ret void\n}}\n"
    );
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), &text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let loads = listing.lines().filter(|line| line.contains("0x9e3779b9") || line.contains("2654435769")).count();
    assert_eq!(loads, 1, "{listing}");
}

/// ZIMM priced `0xFF000000` and `0x80000000` at one instruction each, so
/// ConstantHoisting never made them candidates and lost the pin's base and
/// offset (`div_constants_long` -O3 +16 B, `constant_hoisting_shares_a_wide_
/// compare_constant64.c` +4 B).
#[test]
fn two_neighbouring_zero_extended_constants_share_a_base_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i1 @f(i64 %a, i64 %b) {\n\
                \x20 %x = shl i64 %a, 1\n  %c1 = icmp ugt i64 %x, 4278190080\n\
                \x20 %y = shl i64 %b, 1\n  %c2 = icmp ugt i64 %y, 4278190080\n\
                \x20 %p = sub i64 2147483648, %a\n  %q = sub i64 2147483648, %b\n\
                \x20 %c3 = icmp ult i64 %p, %q\n  %r1 = or i1 %c1, %c2\n  %r = xor i1 %r1, %c3\n  ret i1 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("2130706432"), "{listing}");
}

/// A lone zero-extended constant read in both arms of a branch stays two
/// `mov r32, imm32`: one in the entry would be a register held across the
/// branch (a same-block pair is merged by ISel whatever the pass does).
#[test]
fn a_lone_zero_extended_constant_in_two_blocks_is_not_hoisted_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @h1(i1)\n declare void @h2(i1)\n\
                define void @f(i64 %a, i64 %b, i1 %c) {\n\
                \x20 br i1 %c, label %t, label %e\n\
                t:\n  %x = shl i64 %a, 1\n  %r1 = icmp ugt i64 %x, 4278190080\n  call void @h1(i1 %r1)\n  ret void\n\
                e:\n  %y = shl i64 %b, 1\n  %r2 = icmp ugt i64 %y, 4278190080\n  call void @h2(i1 %r2)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert_eq!(listing.matches("4278190080").count(), 2, "{listing}");
}

/// The mask of a `select c, 0, K` was hoisted beside `0xFFFFFFFF` as a `lea`
/// held in a register through the loop, pushing the loop's temporaries to
/// `r8`/`r9` (`crc` -O1 147 -> 162 B). Kept attached it is `and eax, imm32`.
#[test]
fn a_zero_extended_select_mask_stays_an_immediate_in_the_loop_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(i64 %x, i64 %n) {\n\
                entry:\n  %a = xor i64 %x, 4294967295\n  br label %loop\n\
                loop:\n  %i = phi i64 [ 0, %entry ], [ %i2, %loop ]\n\
                \x20 %v = phi i64 [ %a, %entry ], [ %nv, %loop ]\n\
                \x20 %b = and i64 %v, 1\n  %c = icmp eq i64 %b, 0\n\
                \x20 %m = select i1 %c, i64 0, i64 3988292384\n  %s = lshr i64 %v, 1\n\
                \x20 %nv = xor i64 %s, %m\n  %i2 = add i64 %i, 1\n  %e = icmp eq i64 %i2, %n\n\
                \x20 br i1 %e, label %exit, label %loop\n\
                exit:\n  %r = xor i64 %nv, 4294967295\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O1", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("306674911"), "{listing}");
}

/// Position-independent m64: a symbol defined elsewhere is reached through
/// the GOT, as the pin does (`X86Subtarget::classifyGlobalReference` is
/// `MO_GOTPCREL` for a declaration that is not `dso_local`): the address is
/// one `mov r64, [rip + g@GOTPCREL]` and the value a second load. Every symbol
/// was assumed `dso_local` (`dag_function.rs`), so the read was the single
/// `mov eax, [rip + g]` of a static link and the extern read flattered every
/// cell by one load.
#[test]
fn an_extern_global_read_at_m64_pie_is_loaded_through_the_got() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @g = external global i32\n\
                define i32 @f() {\n  %v = load i32, ptr @g\n  ret i32 %v\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // masm names a symbol operand without brackets: `dword ptr g` is the
    // direct read; the GOT form reads through a register (`ptr [r..]`). The
    // first version counted `[` and read 0 loads for the direct listing.
    assert!(!listing.contains("ptr g"), "{listing}");
    assert!(listing.lines().any(|line| line.trim().starts_with("mov eax, dword ptr [")), "{listing}");
}

/// A call to a function defined elsewhere stays a direct `call` (the linker
/// makes the PLT entry), and a global defined in the module is read directly:
/// only an undefined data symbol goes through the GOT.
#[test]
fn a_call_and_a_defined_global_at_m64_pie_stay_direct() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                @d = global i32 5\n\
                declare i32 @ext()\n\
                define i32 @f() {\n  %a = load i32, ptr @d\n  %b = call i32 @ext()\n  %c = add i32 %a, %b\n  ret i32 %c\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("ptr d") && listing.contains("call ext"), "{listing}");
}

/// `zext(icmp eq (and i64 x, 0x80000000), 0) + y` (the last step of `__clzsi2`)
/// compiled to `and edi, 0x80000000; cmp rdi, 1; adc eax, 0` (13 B); the pin
/// tests the narrow sign bit (`(and X, 2^k) == 0` is `(trunc X) >= 0`,
/// `foldSetCCWithAnd`) and shifts it down: `not; shr 31; add` (7 B, -8 B at
/// sfm16_float_cmp -Os).
#[test]
fn a_single_bit_mask_test_at_m64_is_a_narrow_sign_test_not_cmp_adc() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(i64 %x, i32 %y) optsize {\n\
                \x20 %a = and i64 %x, 2147483648\n  %c = icmp eq i64 %a, 0\n  %z = zext i1 %c to i32\n\
                \x20 %r = add nuw nsw i32 %y, %z\n  ret i32 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("adc") && listing.contains("setns"), "{listing}");
}

/// The pin's `combineMul` covers i64 as well as i32: `x * 40` is `lea; shl`. Ours returned early for i64 and kept
/// `imul r64, r64, imm`; 100+ products in mul_constants -O2 became `movabs` + `imul` (+450 B over the pin llc).
/// `add(mul(add(A, CA), CM), CB)` folds only when `CA*CM + CB` is a legal add immediate (`isInt<32>` on x86). Ours
/// had no check: the unrolled LCG chain of mul_constants became 64-bit `mov` constants and `imul r, r` (+875 B at -O2/-O3).
#[test]
fn a_mul_add_chain_is_not_folded_into_a_64_bit_immediate_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(i64 %x0, i64 %t0, i64 %n) {\nentry:\n  br label %l\nl:\n  \
                %v2 = phi i64 [ %x0, %entry ], [ %v11, %l ]\n  %v3 = phi i64 [ 0, %entry ], [ %v23, %l ]\n  \
                %v4 = phi i64 [ %t0, %entry ], [ %r, %l ]\n  %v5 = mul i64 %v2, 1664525\n  \
                %v6 = add i64 %v5, 1013904223\n  %v7 = mul i64 %v4, 961\n  %v8 = mul i64 %v6, 93\n  \
                %v9 = add i64 %v8, %v7\n  %v10 = mul i64 %v6, 1664525\n  %v11 = add i64 %v10, 1013904223\n  \
                %v12 = mul i64 %v11, 3\n  %r = add i64 %v12, %v9\n  %v23 = add nuw nsw i64 %v3, 2\n  \
                %v24 = icmp eq i64 %v23, %n\n  br i1 %v24, label %out, label %l\nout:\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let wide = listing.split(|c: char| !c.is_ascii_digit()).filter(|n| n.len() >= 11).count();
    assert_eq!(wide, 0, "{listing}");
}

#[test]
fn an_i64_multiply_by_a_lea_constant_is_not_an_imul_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(i64 %x) {\n  %r = mul i64 %x, 40\n  ret i64 %r\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("imul"), "{listing}");
}

/// A call through a pointer pinned the frame register at m64 (`hasOpaqueSPAdjustment`
/// read "the callee may pop"), though no m64 convention pops: rbp was never a value
/// register, so memfuncs_runtime -O2 spent REX-prefixed r12-r15 where the pin has rbp (+111 B).
#[test]
fn a_function_calling_through_a_pointer_uses_rbp_as_a_value_register_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(ptr %p, ptr %fp) {\nentry:\n  \
                %a = load volatile i64, ptr %p\n  %p1 = getelementptr i64, ptr %p, i64 1\n  %b = load volatile i64, ptr %p1\n  \
                %p2 = getelementptr i64, ptr %p, i64 2\n  %c = load volatile i64, ptr %p2\n  %p3 = getelementptr i64, ptr %p, i64 3\n  %d = load volatile i64, ptr %p3\n  \
                %p4 = getelementptr i64, ptr %p, i64 4\n  %e = load volatile i64, ptr %p4\n  %p5 = getelementptr i64, ptr %p, i64 5\n  %g = load volatile i64, ptr %p5\n  \
                call void %fp()\n  \
                store volatile i64 %a, ptr %p\n  store volatile i64 %b, ptr %p\n  store volatile i64 %c, ptr %p\n  \
                store volatile i64 %d, ptr %p\n  store volatile i64 %e, ptr %p\n  store volatile i64 %g, ptr %p\n  ret i64 0\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("rbp"), "{listing}");
}

/// A volatile `memcpy` kept eight-byte pieces at m64 (the 16-byte pieces carried no volatile mark, so the second
/// read of the same source could be forwarded from the first); the pin copies it as `v4i32` pieces, each a volatile
/// access (double_to_uint64 -O2: 80 bytes, ten pairs for five).
#[test]
fn a_volatile_copy_is_made_of_16_byte_pieces_and_each_is_read_and_written_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @f(ptr %src, ptr %d1, ptr %d2) {\n  \
                call void @llvm.memcpy.p0.p0.i64(ptr align 16 %d1, ptr align 16 %src, i64 16, i1 true)\n  \
                call void @llvm.memcpy.p0.p0.i64(ptr align 16 %d2, ptr align 16 %src, i64 16, i1 true)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let pieces = listing.matches("movups").count() + listing.matches("movaps").count();
    assert_eq!(pieces, 4, "{listing}");
}

/// The pin tail-calls the library routine of a `memmove`/`memcpy`/`memset` that ends a function (`jmp memmove`); ours
/// pushed a slot, called, popped and returned: three bytes in each of the two hundred wrappers of memory_expansion
/// (+4201 B at -O2).
#[test]
fn a_memmove_ending_a_function_is_a_tail_call_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @f(ptr %d, ptr %s) {\n  \
                tail call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 300, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("jmp memmove") && !listing.contains("call memmove"), "{listing}");
}

/// `memcpycall` rebuilt a large `memcpy` as a plain call and dropped the `tail` marker, so the call kept its
/// `push; call; pop; ret` where the pin jumps (memory_expansion -O2: ten `cp*` wrappers, +3 B each).
#[test]
fn a_memcpy_ending_a_function_is_a_tail_call_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                define void @f(ptr %d, ptr %s) {\n  \
                tail call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 600, i1 false)\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("jmp memcpy") && !listing.contains("call memcpy"), "{listing}");
}

/// `x_matmul_f` (the m32 vsgcc kernel behind a volatile pointer, clang IR before the passes): its
/// innermost loop indexes `B[k][j]` of `double B[12][12]`, so it makes at most 12 trips. gcc peels it;
/// the pin cannot know the bound and kept 3 `fmul` (392 B, about 7,000 clocks over gcc at -O3).
#[test]
fn a_loop_bounded_by_the_object_it_indexes_is_peeled_at_m32_o3() {
    let text = include_str!("tests_data/matmul_f.ll");
    let (built, _) = compiled(&flags(&["-m32", "-O3", "-march=i486"]), text, "bench_x_matmul_f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let bench = listing.split("bench_x_matmul_f_ proc").nth(1).and_then(|rest| rest.split("endp").next()).expect(&listing);
    assert!(bench.matches("fmul").count() >= 12, "{listing}");
}

/// An `i64` select of two constants was branched in MIR before the DAG saw it
/// (`jne; xor; mov; jmp; xor; mov 2`, compare_to_int64 -Os); the pin gives
/// `setne; add eax, eax` (type legalization splits the select, isel folds it).
#[test]
fn an_i64_select_of_constants_is_no_branch_at_m32() {
    let text = "target datalayout = \"@m32-datalayout@\"\n\
                target triple = \"i486-unknown-linux-gnu\"\n\
                define i64 @f(i32 %a, i32 %b) optsize {\n  %c = icmp eq i32 %a, %b\n\
                  %s = select i1 %c, i64 0, i64 2\n  ret i64 %s\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-Os", "-march=i486"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("setne") && !listing.contains("\n    jne") && !listing.contains("jmp"), "{listing}");
}

/// `cvtsi2ss xmm1, eax` in a loop rewrote only the low lane of `xmm1`, so each
/// iteration waited for the last one's result (the pin: `xorps xmm1, xmm1`
/// first, BreakFalseDeps). sfm16_float_i2f m64 -O2: 9 converts, no `xorps`.
#[test]
fn a_convert_in_a_loop_is_preceded_by_a_xorps_of_its_register() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define float @f(i32 %n) {\nentry:\n  br label %l\n\
                l:\n  %i = phi i32 [ 0, %entry ], [ %i1, %l ]\n  %s = phi float [ 1.0, %entry ], [ %s1, %l ]\n\
                  %c = sitofp i32 %i to float\n  %s1 = fadd float %s, %c\n  %i1 = add i32 %i, 1\n\
                  %d = icmp eq i32 %i1, %n\n  br i1 %d, label %x, label %l\n\
                x:\n  ret float %s1\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let (xor, convert) = (listing.find("xorps"), listing.find("cvtsi2ss"));
    assert!(xor.is_some() && xor < convert, "{listing}");
}

/// `movd eax, xmm0` writes the whole of `rax`, but the zero extension that followed was `mov eax, eax` (the pin: none).
/// The check took every `MOV*rr` for a register copy, `MOVPDI2DIrr` among them: 2 in sfm16_float_i2f's loop, 4 per
/// iteration there.
#[test]
fn a_float_bits_zero_extended_at_m64_is_one_movd() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i64 @f(float %x) {\n  %u = bitcast float %x to i32\n  %z = zext i32 %u to i64\n  ret i64 %z\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("movd"), "{listing}");
    assert!(!listing.contains("movzx"), "{listing}");
}

/// A float `==` used as a value was `ucomiss; setnp; sete; and; movzx` (the pin: `cmpeqss; movd; and 1`, one
/// instruction fewer and no flag reads; the `combineCompareEqual` combine was not translated). int_compared_with_float
/// -Os m64: 274 B against the pin's 268.
#[test]
fn a_float_equality_as_a_value_is_a_cmpss() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define i32 @f(float %a, float %b) {\n  %c = fcmp oeq float %a, %b\n  %z = zext i1 %c to i32\n  ret i32 %z\n}\n";
    let (built, _) = compiled(&flags(&["-m64", "-Os", "--isel=dag"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("cmpss xmm0, xmm1, 0"), "{listing}");
    assert!(!listing.contains("setnp") && !listing.contains("ucomiss"), "{listing}");
}

/// `callee` is `add3`, a default-visibility global the pin's MC never binds
/// at assembly time (`ELFObjectWriter::isSymbolRefDifferenceFullyResolvedImpl`:
/// a PC-relative reference resolves only for `STB_LOCAL`). `-fPIC` code must
/// leave the call and the sibling jump to it to the linker (`R_X86_64_PLT32`):
/// a shared object may be interposed. Ours filled the displacement in at any
/// flag, so an interposed `add3` was never reached from `same` and `twice`.
fn pc_relative_relocs_to_add3(
    module_flags: &str,
    linkage: &str,
) -> Vec<(usize, bool)> {
    let text = format!(
        "target datalayout = \"@m64-datalayout@\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\
         define {linkage}i64 @add3(i64 %a, i64 %b, i64 %c) #0 {{\n  %t = mul i64 %b, 3\n  %u = add i64 %a, %t\n  \
         %v = add i64 %u, %c\n  ret i64 %v\n}}\n\
         define i64 @same(i64 %a, i64 %b, i64 %c) {{\n  %r = tail call i64 @add3(i64 %a, i64 %b, i64 %c)\n  ret i64 %r\n}}\n\
         define i64 @twice(i64 %a) {{\n  %x = call i64 @add3(i64 %a, i64 1, i64 2)\n  %y = call i64 @add3(i64 %x, i64 3, i64 4)\n  ret i64 %y\n}}\n\
         attributes #0 = {{ noinline }}\n{module_flags}"
    );
    let (built, _) = compiled(&flags(&["-m64", "-O2", "--isel=dag"]), &text, "same").unwrap();
    let object = llrm_codegen::backend::objbuild::built(
        &built,
        "f.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap()
    .object;
    let text = object.sections.iter().find(|one| one.role == llrm_object::Role::Text).expect("a text section");
    text.relocs
        .iter()
        .filter(|one| {
            one.pc_rel
                && matches!(
                    one.target,
                    llrm_object::Target::Symbol(at) if object.symbols[at].name == "add3"
                )
        })
        .map(|one| (one.at, one.pc_rel))
        .collect()
}

const PIC_FLAGS: &str = "!llvm.module.flags = !{!0}\n!0 = !{i32 8, !\"PIC Level\", i32 2}\n";
const PIE_FLAGS: &str =
    "!llvm.module.flags = !{!0, !1}\n!0 = !{i32 8, !\"PIC Level\", i32 2}\n!1 = !{i32 2, !\"PIE Level\", i32 2}\n";
const STATIC_FLAGS: &str = "!llvm.module.flags = !{!0}\n!0 = !{i32 7, !\"uwtable\", i32 2}\n";

#[test]
fn a_call_and_sibling_jump_to_a_default_visibility_global_at_pic_keep_their_plt32() {
    assert_eq!(pc_relative_relocs_to_add3(PIC_FLAGS, "").len(), 3, "the tail jump and two calls");
}

#[test]
fn a_call_and_sibling_jump_at_pie_and_static_are_resolved_without_a_relocation() {
    for flags in [PIE_FLAGS, STATIC_FLAGS, ""] {
        assert!(pc_relative_relocs_to_add3(flags, "").is_empty(), "{flags}");
    }
}

#[test]
fn a_hidden_or_internal_callee_at_pic_is_resolved_and_a_weak_one_never_is() {
    assert!(pc_relative_relocs_to_add3(PIC_FLAGS, "hidden ").is_empty());
    assert!(pc_relative_relocs_to_add3(PIC_FLAGS, "internal ").is_empty());
    assert_eq!(
        pc_relative_relocs_to_add3(STATIC_FLAGS, "weak ").len(),
        3,
        "a strong definition elsewhere wins at link time"
    );
}

/// Two 64-bit arguments swapped for a call (`g(b, a)`, the caller keeps neither) were three
/// moves through `rax` at m64 -Os, 9 B where gcc's `xchg rdi, rsi` is 3 B. The rewrite has a unit
/// test on a hand-built body; this is the pipeline's, at 64 bits, where the `rax` the swap
/// borrows is read nowhere after it.
#[test]
fn a_swap_of_two_64_bit_arguments_for_a_call_is_one_xchg_at_m64_os() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i64 @g(i64, i64)\n\
                define i64 @f(i64 %a, i64 %b) {\n  %r = call i64 @g(i64 %b, i64 %a)\n  %s = add i64 %r, 1\n  ret i64 %s\n}\n";
    let xchgs = |level: &str| {
        let (built, _) = compiled(&flags(&["-m64", level, "--isel=dag"]), text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        (listing.lines().filter(|line| line.trim_start().starts_with("xchg")).count(), listing)
    };
    let (size, listing) = xchgs("-Os");
    assert_eq!(size, 1, "{listing}");
    let (speed, listing) = xchgs("-O2");
    assert_eq!(speed, 0, "{listing}");
}

/// A 16-trip loop of calls in an internal function that nothing will inline
/// (`-fno-inline-functions` with called-once off) was unrolled unpriced: the
/// early unroll leaves an internal function's loops to be priced at each
/// inlined site, and no site came. huffman_decode -O3 at m64: `decode` copied
/// 17 calls of `getbit`, 443 B against gcc's 137, 2099 B in all against 1184
/// and 27% over gcc in clocks plus bytes.
#[test]
fn a_loop_of_calls_in_a_function_nothing_inlines_is_priced_not_unrolled_at_m64_o3() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                declare i32 @h()\n\
                define internal i32 @f(i32 %x) {\nentry:\n  br label %loop\n\
                loop:\n  %i = phi i32 [ 1, %entry ], [ %j, %loop ]\n  %c = phi i32 [ %x, %entry ], [ %d, %loop ]\n\
                  %b = call i32 @h()\n  %s = shl i32 %c, 1\n  %d = add i32 %s, %b\n  %j = add i32 %i, 1\n\
                  %t = icmp ult i32 %j, 17\n  br i1 %t, label %loop, label %done\n\
                done:\n  ret i32 %d\n}\n\
                define i32 @g(i32 %x) {\n  %r = call i32 @f(i32 %x)\n  ret i32 %r\n}\n";
    let calls_of_h = |arguments: &[&str]| {
        let (built, _) = compiled(&flags(arguments), text, "g").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap().matches("call h").count()
    };
    let off = ["-m64", "-O3", "--isel=dag", "-fno-inline-functions", "-fno-inline-functions-called-once"];
    assert!(calls_of_h(&off) <= 2, "the loop stays rolled: one call of h, not 16");
    let on = ["-m64", "-O3", "--isel=dag"];
    assert!(calls_of_h(&on) >= 1);
}

/// `p >= 65535.0f ? 65535.0f : p` was branched before selection (`ucomiss`,
/// `jb`, `movss`): fp selects never reached the DAG, so its `pseudo_fmin` was
/// dead code and the clamps of jpeg_fixed and stft_single grew 24-80 B at m64.
/// It is `minss`.
#[test]
fn a_float_clamp_is_minss_at_m64() {
    let text = "target datalayout = \"@m64-datalayout@\"\n\
                target triple = \"x86_64-unknown-linux-gnu\"\n\
                define float @f(float %p) {\n\
                  %c = fcmp oge float %p, 6.553500e+04\n\
                  %q = select i1 %c, float 6.553500e+04, float %p\n\
                  ret float %q\n}\n";
    let (built, _) = compiled(&flags(&["-O2"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("minss") && !listing.contains("ucomiss"), "{listing}");
}

/// `x_life`'s 3x3 neighbour nest (the m32 vsgcc kernel behind a volatile pointer, clang IR before the passes) at -O1
/// with `-funroll-loops`: the -O1 unroll rule counts the 3-trip inner loop one instruction too large and kept both
/// loops, which spill their counters to the stack (416 B, 564,234 clk, gcc 368,623). The pin's threshold unrolls the
/// nest to nine straight loads, and unrolled it is smaller (401 B, 214,872 clk), which only a second compile can see
/// (DEVIATE UNROLL-O1-TRIAL): the nest is kept unrolled.
#[test]
fn a_nest_whose_unroll_is_no_larger_and_no_slower_is_unrolled_at_o1() {
    let text = include_str!("tests_data/life_nest.ll");
    let (built, _) = compiled(&flags(&["-m32", "-O1", "-funroll-loops", "-march=i486"]), text, "bench_x_life").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // The eight neighbours of the centre are loaded in one straight run: no loop is left around them.
    let mut loads = 0;
    let mut most = 0;
    for line in listing.lines().map(str::trim) {
        if line.ends_with(':') {
            loads = 0;
        } else if line.contains(", byte ptr [") && !line.starts_with("cmp") {
            loads += 1;
            most = most.max(loads);
        }
    }
    assert!(most >= 8, "the nest kept a loop: at most {most} neighbour loads in a block: {listing}");
}

/// `matmul` at -O1 with `-funroll-loops`: the pin's threshold unrolls it (303 -> 1904 B), which the trial prices
/// and puts back: the code stays well under the unrolled size (about 100 instructions rolled, 600 unrolled).
#[test]
fn a_nest_whose_unroll_grows_the_code_stays_rolled_at_o1() {
    let text = include_str!("tests_data/matmul_int.ll");
    let (built, _) = compiled(&flags(&["-m32", "-O1", "-funroll-loops", "-march=i486"]), text, "bench_matmul").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let instructions = listing.lines().filter(|line| line.starts_with("    ") && !line.trim_end().ends_with(':')).count();
    assert!(instructions < 250, "matmul unrolled to {instructions} instructions: {listing}");
}

/// x_adler -O2: `movzx esi, bl` of a counter that stays live ran 3 clocks on
/// the i486 where gcc's `mov; and` runs 2 (47,972 clk with the mask, 50,972
/// without; gcc 49,551). MaskByAnd (DEVIATE MA1) skipped every source with
/// another reader; a live source is copied first and the copy is priced.
#[test]
fn a_byte_extension_of_a_live_counter_is_a_mask_in_a_hot_loop_at_o2() {
    let text = include_str!("tests_data/live_byte_mask.ll");
    let (built, _) = compiled(&flags(&["-m32", "-O2", "-march=i486"]), text, "sum_low_bytes").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("movzx"), "the loop extends with movzx (3 clk): {listing}");
    assert!(listing.contains("and ") && listing.contains(", 255"), "no mask: {listing}");
}

/// x_int64_division -O2: the same mask of a live byte, wanted again past two
/// calls, kept the masked value in a register across both and spilled one more
/// value (+700 clk): the copy-and-mask is not made where its result outlives a
/// call.
#[test]
fn a_masked_byte_wanted_past_a_call_stays_a_movzx_at_o2() {
    let text = include_str!("tests_data/live_byte_mask_calls.ll");
    let (built, _) = compiled(&flags(&["-m32", "-O2", "-march=i486"]), text, "low_bytes_across_calls").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("movzx"), "the extension was made a mask: {listing}");
}

/// x_cordic -O2: both arms of the body joined in the loop latch (`inc; cmp; branch back`), one reached by a taken
/// `jmp` that ran 3 clocks a trip (50,829 clk; 49,539 with the latch copied into each arm, gcc 49,031). The pin's
/// tail-duplication bound (2 instructions) kept the latch; the priced copy (DEVIATE TAILDUP-HOT) takes it.
#[test]
fn a_loop_latch_joining_two_arms_is_copied_into_each_at_o2() {
    let text = include_str!("tests_data/latch_arms.ll");
    let (built, _) = compiled(&flags(&["-m32", "-O2", "-march=i486"]), text, "arms").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    // The latch's `xor` of 5 stands once in each arm, not once behind a `jmp` from one of them.
    let copies = listing.lines().filter(|line| line.trim() == "xor esi, 5").count();
    assert_eq!(copies, 2, "the latch is shared by the arms: {listing}");
}

/// x86-m32 x_prefix -Os: five counted loops, each rotated for a guard that does not fold (RS1 rotates a
/// counted loop), 136 B of kernel where gcc's unrotated loops are 110 B. ROTATION-TRIAL compiles the
/// module again with those loops left and keeps the smaller object: 297 B against 326 B rotated.
#[test]
fn counted_loops_that_are_smaller_unrotated_stay_unrotated_at_os() {
    let text = include_str!("tests_data/prefix_sums.ll");
    let (built, format) = compiled(&flags(&["-m32", "-Os", "-march=i486"]), text, "bench_prefix").unwrap();
    let object = llrm_codegen::backend::objbuild::written_in(
        &built,
        "prefix_sums.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    assert!(object.len() < 310, "{} bytes: {}", object.len(), llrm_codegen::backend::masm::text(&built).unwrap());
}

/// x86-m32 x_crc32 -Os: the table-driven loop is counted, its copied guard does not fold, and the unrotated
/// trial was a few bytes smaller, so the loop stayed unrotated and paid a taken `jmp` a trip (+7.5% clocks
/// against clang). ROTATION-HOT keeps the rotated loop when it runs less for at most 4 B a loop.
/// DEVIATE ROTATION-HOT.
#[test]
fn a_counted_loop_that_runs_less_rotated_stays_rotated_at_m32_os() {
    let text = include_str!("tests_data/crc_table_loop.ll");
    let (built, _) = compiled(&flags(&["-m32", "-Os", "-march=i486"]), text, "bench_x_crc32").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    let back = listing.lines().enumerate().any(|(at, line)| {
        let Some(target) = line.trim().strip_prefix("jmp ") else { return false };
        listing.lines().position(|label| label == format!("{target}:")).is_some_and(|label| label < at)
    });
    assert!(!back, "a loop is unrotated, a jmp back to its guard: {listing}");
}

/// COMBINED-TRIAL: the -Os trials were nested wrappers, each re-running the mid-end and the backend over the whole
/// module (2 x 2 x 2): recchop, with two noted dimensions, selected its two functions in four compiles, 8 selections
/// (compile time x4.4 against none). The base compile notes the dimensions, each is compiled against the base alone and priced
/// on the functions that differ, and a final compile is made only where two or more stay.
#[test]
fn the_os_trials_compile_each_dimension_against_the_base_once() {
    let text = include_str!("tests_data/recchop_trials.ll");
    let before = llrm_mir::counts::get(llrm_mir::counts::Event::Selections);
    compiled(&flags(&["-m32", "-Os", "-march=i486"]), text, "bench_recchop").unwrap();
    let selections = llrm_mir::counts::get(llrm_mir::counts::Event::Selections) - before;
    assert!(selections <= 6, "{selections} selections of two functions: more than three compiles");
}

/// x86-m32 x_bubble -Os: the unrotated trial module was 16 B shorter as an object file and 2 B
/// longer in code (the padding between functions moved). The trial compares the bytes without
/// padding, so the rotated loops stay.
#[test]
fn the_rotation_trial_compares_code_without_the_padding_between_functions() {
    let text = include_str!("tests_data/bubble_sort.ll");
    let (built, format) = compiled(&flags(&["-m32", "-Os", "-march=i486"]), text, "bench_x_bubble").unwrap();
    let bytes = llrm_codegen::backend::objbuild::unpadded_bytes(
        &built,
        "bubble_sort.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m32::M32),
    )
    .unwrap();
    assert!(bytes <= 156, "{bytes} bytes: the unrotated module was kept");
}

/// COMPILE-OSROT: the rotation trial assembled the functions that differ twice and the module once
/// more, so a module of two loop functions and a leaf that does not change cost three assemblies
/// where two are the floor (x_jpeg_fixed -Os, 6 of 8 functions differ: 2.0x the compile time). Each
/// of the three functions is selected once per assembly, twice in all.
#[test]
fn the_rotation_trial_assembles_the_module_twice_whatever_part_of_it_differs() {
    let one = include_str!("tests_data/prefix_sums.ll");
    let start = one.find("; Function Attrs").unwrap();
    let end = one[start..].find("\n}\n").unwrap() + start + 3;
    let function = &one[start..end];
    let second = function.replace("@bench_prefix", "@bench_prefix2").replace("mul nsw i32 %11, 5", "mul nsw i32 %11, 7");
    let text = format!(
        "{}\n{second}\ndefine dso_local i32 @leaf(i32 %x) {{\n  ret i32 %x\n}}\n{}",
        &one[..end],
        &one[end..]
    );
    let before = llrm_mir::counts::get(llrm_mir::counts::Event::Selections);
    compiled(&flags(&["-m32", "-Os", "-march=i486"]), &text, "bench_prefix").unwrap();
    let selections = llrm_mir::counts::get(llrm_mir::counts::Event::Selections) - before;
    assert_eq!(selections, 6, "{selections} selections of three functions");
}

/// x86-m32 `--no-midend`: the pin's `llc` takes its IR as given, so the instrument that prices our
/// backend against it must too. `llrm-llc` ran the whole mid-end over any IR it was handed (a hand-made
/// branch form of x_heapsort's child select compiled to the same 37,923 clk as the unedited IR), so
/// "the pin's code on our IR" could not be isolated. With the switch a slot that mem2reg promotes stays
/// a stack slot.
#[test]
fn the_no_midend_switch_leaves_the_module_to_the_backend_as_given() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
        define i32 @f(i32 %x) {\n  %p = alloca i32, align 4\n  store i32 %x, ptr %p, align 4\n  %v = load i32, ptr %p, align 4\n  ret i32 %v\n}\n";
    let listing = |extra: &[&str]| {
        let mut arguments = vec!["-m32", "-O2", "-march=i486"];
        arguments.extend_from_slice(extra);
        let (built, _) = compiled(&flags(&arguments), text, "slot").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    let optimized = listing(&[]);
    let given = listing(&["--no-midend"]);
    assert!(!optimized.contains("[esp"), "the mid-end keeps the slot: {optimized}");
    assert!(given.contains("[esp"), "the slot was promoted though the mid-end was off: {given}");
}

/// x86-m32 queens -O3: `place(q, 0, n)` was never cloned for its first row (ipa-cp clones were off at
/// -O3, as the pin has none; gcc's `-fipa-cp-clone` is on there), so `safe` stayed a rolled loop over a
/// symbolic row: 160,324 clk against gcc's 94,851. With the clones the rows are constants: 79,692 clk.
#[test]
fn a_recursion_called_with_a_constant_row_is_cloned_for_it_at_o3() {
    let text = include_str!("tests_data/queens_place.ll");
    let lines = |extra: &[&str]| {
        let mut arguments = vec!["-m32", "-O3", "-march=i486"];
        arguments.extend_from_slice(extra);
        let (built, _) = compiled(&flags(&arguments), text, "queens").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap().lines().count()
    };
    let (default, without) = (lines(&[]), lines(&["-fno-ipa-cp-clone"]));
    assert!(default > 2 * without, "-O3 made no clones: {default} lines against {without} without clones");
}

/// x86-m32 recchop -O3: `chop(0, 64)` halves its range, so neither `lo = 0` nor `hi = 64` alone makes
/// the loop's trip count known (gcc: "loops with known iterations: 0", evaluation 47.6 against 500, no
/// clone, 1204 B). Pricing both together as one context cloned `chop` eight times: 4,948 B and 6,452 clk
/// against 1,269 B and 5,144.
#[test]
fn a_halving_recursion_called_with_constant_bounds_is_not_cloned_at_o3() {
    let text = include_str!("tests_data/recchop_halving.ll");
    let lines = |extra: &[&str]| {
        let mut arguments = vec!["-m32", "-O3", "-march=i486"];
        arguments.extend_from_slice(extra);
        let (built, _) = compiled(&flags(&arguments), text, "recchop").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap().lines().count()
    };
    let (default, without) = (lines(&[]), lines(&["-fno-ipa-cp-clone"]));
    assert_eq!(default, without, "-O3 cloned chop: {default} lines against {without} without clones");
}

/// x86-m32 x_hanoi2 -O3: `mv(n-1, via, to, from)` hands each parameter to another, which gcc takes for a
/// variable lattice (evaluation 137 against 500, no clone). Following those contexts as a chain priced
/// `mv(n, 0, 2, 1)` at 5,951 and cloned it: 169,494 clk against 160,540 without.
#[test]
fn a_recursion_permuting_its_parameters_is_not_cloned_at_o3() {
    let text = include_str!("tests_data/hanoi_permuting.ll");
    let lines = |extra: &[&str]| {
        let mut arguments = vec!["-m32", "-O3", "-march=i486"];
        arguments.extend_from_slice(extra);
        let (built, _) = compiled(&flags(&arguments), text, "hanoi").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap().lines().count()
    };
    let (default, without) = (lines(&[]), lines(&["-fno-ipa-cp-clone"]));
    assert_eq!(default, without, "-O3 cloned mv: {default} lines against {without} without clones");
}

/// A call of an `interrupt` function was emitted as a plain `call far`, so the
/// handler's `iret` popped flags that were never pushed and left SP 8 bytes
/// off (harness cells CFLB and CFLL `i_plain`: `lost 0008`).
#[test]
fn a_call_of_an_interrupt_function_pushes_the_flags_first() {
    let directory = unique_directory("intrcall");
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("caller.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
         define void @f(ptr %p) addrspace(1) {\n  store i16 1, ptr %p\n  call x86_intrcc addrspace(1) void @isr()\n  ret void\n}\n\
         declare void @isr() addrspace(1)\n",
    )
    .unwrap();
    let output = directory.join("caller.s");
    crate::llc::compile(&flags(&["-m16", "-S", "-O0", "-o", output.to_str().unwrap()]), &source, None).unwrap();
    let text = std::fs::read_to_string(output).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    let at = text.find("call far ptr _isr").expect(&text);
    assert!(text[..at].trim_end().ends_with("pushf"), "{text}");
}

/// x86-m64 sieve -O2: `i <= 31 && i*i < n` stayed one merged test, `setb; setb; and; cmp 1; jne` (262 B, 13734
/// instructions against gcc's 177 B, 13215). The pin's budget does not price the `setcc` pair and the logic op;
/// SD103-M64 takes them off it, as m32's does, and the test is two branches.
#[test]
fn a_merged_pair_of_compares_is_two_branches_at_o2_on_m64() {
    let text = include_str!("tests_data/sieve_merged.ll");
    let (built, _) = compiled(&flags(&["-m64", "-O2"]), text, "bench_sieve").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("setb"), "the pair of compares was merged: {listing}");
}

/// x86-m64 recpow -Os: the square-and-multiply loop RS1 rotates for a guard that does not fold
/// kept its copy, 204 B of code against gcc's 175 B. ROTATION-TRIAL compiles the module again with
/// the loop unrotated and keeps the smaller one: 189 B against 204 B rotated.
#[test]
fn a_counted_loop_that_is_smaller_unrotated_stays_unrotated_at_os_on_m64() {
    let text = include_str!("tests_data/recpow_os.ll");
    let (built, format) = compiled(&flags(&["-m64", "-Os"]), text, "bench_recpow").unwrap();
    let bytes = llrm_codegen::backend::objbuild::unpadded_bytes(
        &built,
        "recpow_os.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap();
    assert!(bytes <= 195, "{bytes} bytes: {}", llrm_codegen::backend::masm::text(&built).unwrap());
}

/// MUL-HOT (m32 -Os): `x * 45` keeps its `imul` in a block that runs once and is the pin's `lea` chain in a loop (the pin
/// decomposes at -Os, `hasMinSize` is -Oz's). The tip kept the `imul` everywhere at -Os: x_mul_constants -Os ran
/// 388,733 clk against clang's 286,797.
#[test]
fn a_multiply_in_a_loop_is_a_chain_at_m32_os_and_an_imul_outside_it() {
    let head = "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n";
    let once = format!("{head}define i32 @f(i32 %x) {{\n  %m = mul i32 %x, 45\n  ret i32 %m\n}}\n");
    let looped = format!(
        "{head}define i32 @f(i32 %n, ptr %p) {{\nentry:\n  br label %body\n\
         body:\n  %i = phi i32 [ 0, %entry ], [ %j, %body ]\n  %a = getelementptr i32, ptr %p, i32 %i\n  \
         %v = load i32, ptr %a\n  %m = mul i32 %v, 45\n  store i32 %m, ptr %a\n  %j = add i32 %i, 1\n  \
         %c = icmp slt i32 %j, %n\n  br i1 %c, label %body, label %done\n\
         done:\n  ret i32 0\n}}\n"
    );
    let listing = |text: &str| {
        let (built, _) = compiled(&flags(&["-m32", "-mabi=sysv", "-Os"]), text, "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    assert!(listing(&once).contains("imul"), "a multiply that runs once keeps its imul:\n{}", listing(&once));
    assert!(!listing(&looped).contains("imul"), "a multiply in a loop is the chain:\n{}", listing(&looped));
}

/// MUL-HOT (m32 -Os): a block runs often when its function does, not only when its loop does. x_dct -Os called `dct8`
/// (`x * 7`, `x * 5`, `x * 3` in straight-line code) from two nested loops and kept every `imul`: 314,722 clk against
/// clang's 211,103. A function called from a loop is hot in all its blocks; one called once keeps its `imul`.
#[test]
fn a_multiply_in_a_function_called_from_a_loop_is_a_chain_at_m32_os() {
    let head = "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n";
    let callee = "define internal i32 @g(i32 %x) noinline {\n  %m = mul i32 %x, 45\n  ret i32 %m\n}\n";
    let once = format!("{head}{callee}define i32 @f(i32 %x) {{\n  %r = call i32 @g(i32 %x)\n  ret i32 %r\n}}\n");
    let looped = format!(
        "{head}{callee}define i32 @f(i32 %n) {{\nentry:\n  br label %body\n\
         body:\n  %i = phi i32 [ 0, %entry ], [ %j, %body ]\n  %s = phi i32 [ 0, %entry ], [ %t, %body ]\n  \
         %r = call i32 @g(i32 %i)\n  %t = add i32 %s, %r\n  %j = add i32 %i, 1\n  \
         %c = icmp slt i32 %j, %n\n  br i1 %c, label %body, label %done\n\
         done:\n  ret i32 %s\n}}\n"
    );
    let callee_listing = |text: &str| {
        let (built, _) = compiled(&flags(&["-m32", "-mabi=sysv", "-Os"]), text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        let start = listing.find("g proc").unwrap_or_else(|| panic!("no g:\n{listing}"));
        listing[start..].split("g endp").next().unwrap().to_owned()
    };
    assert!(callee_listing(&once).contains("imul"), "a function called once keeps its imul:\n{}", callee_listing(&once));
    assert!(!callee_listing(&looped).contains("imul"), "a function called from a loop is the chain:\n{}", callee_listing(&looped));
}

/// `RegisterCoalescer::reMaterializeDef` refuses a constant that is not
/// `isAsCheapAsAMove`: at -Os outside 64-bit mode +1 and -1 are
/// `MOV32r1`/`MOV32r_1` (xor and inc), so the pin makes one once before a loop
/// and copies it into the argument register each trip (`mov edx, edi`); every
/// other constant is made again in the loop. We made +1 again each trip (`xor
/// edx, edx; inc edx`): x_dct -Os 211,298 clk against clang's 211,103.
#[test]
fn a_hoisted_one_passed_to_a_call_in_a_loop_stays_in_a_register_at_m32_os() {
    let head = "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n";
    let loop_of = |level: &str, constant: i32| {
        let text = format!(
            "{head}declare void @g(ptr, i32)\ndefine void @f(i32 %n, ptr %p) {{\nentry:\n  br label %body\n\
             body:\n  %i = phi i32 [ 0, %entry ], [ %j, %body ]\n  %a = getelementptr i32, ptr %p, i32 %i\n  \
             call void @g(ptr %a, i32 {constant})\n  %j = add i32 %i, 1\n  \
             %c = icmp ne i32 %j, 8\n  br i1 %c, label %body, label %done\n\
             done:\n  ret void\n}}\n"
        );
        let (built, _) = compiled(&flags(&["-m32", level]), &text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        let call = listing.find("call g_").unwrap_or_else(|| panic!("no call:\n{listing}"));
        let from = listing[..call].rfind("\nL0_").unwrap_or_else(|| panic!("no loop:\n{listing}"));
        let body = listing[from..call].to_owned();
        (body, listing)
    };
    let (body, listing) = loop_of("-Os", 1);
    assert!(!body.contains("inc ") && !body.contains("xor "), "+1 is made once, outside the loop:\n{listing}");
    assert!(body.contains("mov edx, e"), "the loop copies it into the argument register:\n{listing}");
    // What the pin makes again in the loop, and still does.
    assert!(loop_of("-Os", 7).0.contains("mov edx, 7"), "{}", loop_of("-Os", 7).1);
    assert!(loop_of("-Os", 0).0.contains("xor edx, edx"), "{}", loop_of("-Os", 0).1);
    assert!(loop_of("-O1", 1).0.contains("mov edx, 1"), "{}", loop_of("-O1", 1).1);
}

/// PO1 (m32 -Os): a call's two argument words come off the stack with `add esp, 8`, as the pin does below `minsize`, not
/// `pop ecx` twice (one byte shorter, eight clocks against one a call). x_hanoi2 -Os spent 245,837 clk against clang's
/// 221,263 for 4,096 calls.
#[test]
fn a_calls_arguments_come_off_the_stack_by_add_at_m32_os() {
    let text = "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n\
                declare i32 @g(i32, i32)\ndefine i32 @f(i32 %a, i32 %b) {\n  %x = call i32 @g(i32 %a, i32 %b)\n  \
                %y = call i32 @g(i32 %x, i32 %b)\n  ret i32 %y\n}\n";
    for level in ["-Os", "-O1"] {
        let (built, _) = compiled(&flags(&["-m32", "-mabi=sysv", level]), text, "f").unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(listing.contains("add esp, 8"), "{level}: arguments are added off:\n{listing}");
        assert!(!listing.contains("pop ecx"), "{level}: no pop cleans them:\n{listing}");
    }
}

/// LoopIdiomRecognize forms the `memcpy` of a copy loop whose length is known at run time and the target calls it (m32 has a
/// libc; `Tuning::libc_memcpy`), as the pin does. It stayed a loop at -Os where the inline `rep movs` was more bytes:
/// x_transpose -Os ran 17,984 loop trips for 9,792 and 8,192 `movsd`.
#[test]
fn a_copy_loop_of_run_time_length_is_a_memcpy_call_at_m32_os() {
    let text = "target datalayout = \"@m32-datalayout@\"\ntarget triple = \"i486-unknown-linux-gnu\"\n\
                @d = external global [64 x i32]\n@s = external global [64 x i32]\n                define void @f(i32 %n) {\nentry:\n  %e = icmp eq i32 %n, 0\n  br i1 %e, label %done, label %body\n\
                body:\n  %i = phi i32 [ 0, %entry ], [ %j, %body ]\n  %a = getelementptr i32, ptr @s, i32 %i\n  %v = load i32, ptr %a\n  \
                %b = getelementptr i32, ptr @d, i32 %i\n  store i32 %v, ptr %b\n  %j = add i32 %i, 1\n  %c = icmp ne i32 %j, %n\n  \
                br i1 %c, label %body, label %done\ndone:\n  ret void\n}\n";
    let (built, _) = compiled(&flags(&["-m32", "-mabi=sysv", "-Os"]), text, "f").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("memcpy"), "the loop is a call of memcpy:\n{listing}");
}

/// REP-MOVS-OS (m32 -Os): a constant copy is `rep movs` in the widest block its size and alignment allow, not `movsb`:
/// the same bytes, a third to a quarter of the clocks. scroll -Os copied 4000 bytes of two-byte-aligned shorts a byte at a
/// time: 958,326 clk against clang's 536,067.
#[test]
fn an_aligned_constant_copy_is_rep_movs_in_wide_blocks_at_m32_os() {
    let text = |align: u32, bytes: u32| {
        format!(
            "target datalayout = \"@m32-datalayout@\"\n\
             target triple = \"i486-unknown-linux-gnu\"\n\
             @a = external global [2000 x i16], align {align}\n\
             @b = external global [2000 x i16], align {align}\n\
             declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
             define void @f() {{\n  call void @llvm.memcpy.p0.p0.i32(ptr align {align} @b, ptr align {align} @a, i32 {bytes}, i1 false)\n  ret void\n}}\n"
        )
    };
    let listing = |align, bytes| {
        let (built, _) = compiled(&flags(&["-m32", "-mabi=sysv", "-Os"]), &text(align, bytes), "f").unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    };
    assert!(listing(2, 4000).contains("movsw"), "two-byte aligned:\n{}", listing(2, 4000));
    assert!(listing(4, 4000).contains("movsd"), "four-byte aligned:\n{}", listing(4, 4000));
    assert!(listing(1, 4000).contains("movsb") && !listing(1, 4000).contains("movsw"), "unaligned stays bytes");
    assert!(!listing(2, 30).contains("movsw"), "a short copy would pay the operand-size prefix for nothing:\n{}", listing(2, 30));
    assert!(listing(4, 4002).contains("movsw") && !listing(4, 4002).contains("movsd"), "a size of 2 mod 4 stays whole blocks of 2");
}

/// x86-m64 shellsort -O2: `bench_main_` took a second 255 B copy of `bench_shellsort`, a public function that stays,
/// for one constant argument: 499 B of code against gcc's 235. INL-O2 RATE prices the copy's bytes at the m64 rate
/// as m32's does, and the caller keeps the call (271 B).
#[test]
fn a_speed_inline_pays_its_bytes_at_o2_on_m64() {
    let text = include_str!("tests_data/shellsort_o2.ll");
    let (built, format) = compiled(&flags(&["-m64", "-O2"]), text, "bench_shellsort").unwrap();
    let bytes = llrm_codegen::backend::objbuild::unpadded_bytes(
        &built,
        "shellsort_o2.ll",
        llrm_codegen::backend::objbuild::CodeLayout::OneSegment,
        format,
        llrm_target::Target::objects(&llrm_x86_m64::M64),
    )
    .unwrap();
    assert!(bytes <= 350, "{bytes} bytes: {}", llrm_codegen::backend::masm::text(&built).unwrap());
}

/// x86-m64 crc -O2: the rate kept `bench_main_`'s call of `bench_crc(0)`, whose copy folds to one constant: 21 executed
/// instructions became 561 and the object grew 5 B. A priced trial measures the folded copy (no growth, fewer clocks)
/// and keeps it, as the pin's inliner does.
#[test]
fn a_call_whose_copy_folds_to_a_constant_is_inlined_at_o2_on_m64() {
    let text = include_str!("tests_data/crc_o2.ll");
    let (built, _) = compiled(&flags(&["-m64", "-O2"]), text, "bench_main_").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(!listing.contains("call bench_crc"), "main still calls bench_crc:\n{listing}");
}

/// x86-m64 shellsort -Os: the uncounted `while (at >= gap && ...)` loop stayed unrotated, so each trip tested
/// `at >= gap` through `add; jb` ahead of the body (9276 instructions against clang's 7708 for the same 179 B). The
/// ROTATION-HOT trial rotates it where the object does not grow: `sub di, ax; jae` closes the loop.
#[test]
fn an_uncounted_loop_is_rotated_at_os_on_m64_where_the_object_does_not_grow() {
    let text = include_str!("tests_data/shellsort_os.ll");
    let (built, _) = compiled(&flags(&["-m64", "-Os"]), text, "bench_shellsort").unwrap();
    let listing = llrm_codegen::backend::masm::text(&built).unwrap();
    assert!(listing.contains("sub di, ax"), "the loop is not rotated:\n{listing}");
}
