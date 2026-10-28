//! Each test is a symptom: the text a promotion must leave, which an
//! unpromoted alloca does not.

use llrm_analysis::testing::DOS;
use llrm_mir::module::Module;

use super::Mem2Reg;
use crate::testing::{managed, parsed};

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}{body}"))
}

fn promoted(body: &str) -> String {
    managed(&mut module(body), Mem2Reg)
}

/// One store that dominates the loads: they take its value. An unpromoted
/// alloca keeps `alloca`, `store` and `load` (the write-through `Promote`
/// keeps the store).
#[test]
fn test_a_single_dominating_store_replaces_its_loads_and_goes_with_the_alloca() {
    let after = promoted(
        "define i32 @f(i32 %a, i1 %c) {
b0:
  %s = alloca i32
  store i32 %a, ptr %s
  br i1 %c, label %b1, label %b2
b1:
  %x = load i32, ptr %s
  ret i32 %x
b2:
  %y = load i32, ptr %s
  ret i32 %y
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert_eq!(after.matches("ret i32 %a").count(), 2, "{after}");
}

/// Stores and loads in one block: each load takes the store above it.
#[test]
fn test_loads_in_one_block_take_the_store_above_them() {
    let after = promoted(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store i32 1, ptr %s
  %x = load i32, ptr %s
  store i32 %a, ptr %s
  %y = load i32, ptr %s
  %z = add i32 %x, %y
  ret i32 %z
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("add i32 1, %a"), "{after}");
}

/// Stores in the arms of a branch: a phi at the join, and nothing in memory.
#[test]
fn test_stores_in_two_arms_make_a_phi_at_the_join() {
    let after = promoted(
        "define i32 @f(i1 %c) {
b0:
  %s = alloca i32
  br i1 %c, label %b1, label %b2
b1:
  store i32 1, ptr %s
  br label %b3
b2:
  store i32 2, ptr %s
  br label %b3
b3:
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(
        after.contains("phi i32 [ 1, %b1 ], [ 2, %b2 ]") || after.contains("phi i32 [ 2, %b2 ], [ 1, %b1 ]"),
        "{after}"
    );
}

/// A counter around a loop: a phi in the header carries it.
#[test]
fn test_a_loop_carried_slot_is_a_phi_in_the_header() {
    let after = promoted(
        "define i32 @f(i32 %n) {
b0:
  %i = alloca i32
  store i32 0, ptr %i
  br label %b1
b1:
  %v = load i32, ptr %i
  %c = icmp slt i32 %v, %n
  br i1 %c, label %b2, label %b3
b2:
  %w = load i32, ptr %i
  %next = add i32 %w, 1
  store i32 %next, ptr %i
  br label %b1
b3:
  %r = load i32, ptr %i
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("phi i32"), "{after}");
}

/// An alloca whose address is passed on is not promotable: it stays, with
/// its store and its load.
#[test]
fn test_an_alloca_whose_address_escapes_stays() {
    let after = promoted(
        "declare void @sink(ptr)

define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store i32 %a, ptr %s
  call void @sink(ptr %s)
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca") && after.contains("store") && after.contains("load i32"), "{after}");
}

/// A volatile load is no promotion.
#[test]
fn test_a_volatile_load_keeps_its_alloca() {
    let after = promoted(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store i32 %a, ptr %s
  %x = load volatile i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca") && after.contains("load volatile"), "{after}");
}

/// An alloca with lifetime markers around its store and load is promoted and
/// the markers go. The markers were a use that is no load or store, and kept
/// the alloca in memory.
#[test]
fn test_an_alloca_with_lifetime_markers_is_promoted_and_the_markers_go() {
    let after = promoted(
        "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  call void @llvm.lifetime.start.p0(i64 4, ptr %s)
  store i32 %a, ptr %s
  %x = load i32, ptr %s
  call void @llvm.lifetime.end.p0(i64 4, ptr %s)
  ret i32 %x
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("call void @llvm.lifetime"), "{after}");
    assert!(after.contains("ret i32 %a"), "{after}");
}

/// A declared variable's alloca is promoted, and the debugger is told the
/// stored value. The declare kept the alloca in memory.
#[test]
fn test_a_declared_alloca_is_promoted_and_its_variable_is_the_stored_value() {
    let after = promoted(
        "define i32 @f(i32 %a) {
b0:
  %x = alloca i32
  #dbg_declare(ptr %x, !0)
  store i32 %a, ptr %x
  %v = load i32, ptr %x
  ret i32 %v
}

!0 = !{!\"f\", !\"x\", !1, i64 0}
!1 = !{!\"int\"}
",
    );
    assert!(!after.contains("alloca") && !after.contains("#dbg_declare"), "{after}");
    assert!(after.contains("#dbg_value(i32 %a, !0)"), "{after}");
}

/// The pipeline the compilers run, with `Options::mem2reg` (what
/// the default sets): a local scalar and a loop-carried local leave no
/// alloca, and the answer is the same as with the write-through `Promote`.
#[test]
fn test_the_pipeline_with_mem2reg_on_leaves_no_alloca_and_the_same_answer() {
    use llrm_mir::program::Program;

    use crate::pipeline::{self, Applied};
    let text = "define i32 @f(i32 %n) {
b0:
  %s = alloca i32
  %i = alloca i32
  store i32 0, ptr %s
  store i32 0, ptr %i
  br label %h
h:
  %iv = load i32, ptr %i
  %c = icmp slt i32 %iv, %n
  br i1 %c, label %body, label %done
body:
  %sv = load i32, ptr %s
  %iv2 = load i32, ptr %i
  %t = add i32 %sv, %iv2
  store i32 %t, ptr %s
  %nx = add i32 %iv2, 1
  store i32 %nx, ptr %i
  br label %h
done:
  %r = load i32, ptr %s
  ret i32 %r
}
";
    let run = |on: bool| {
        let mut module = module(text);
        let applied = Applied {
            options: pipeline::Options { mem2reg: on, copy_headers: false, ..Default::default() },
            ..Applied::default()
        };
        Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
            pipeline::applied(program, &applied)
        })
        .and_then(|done| done)
        .expect("the pipeline runs");
        module
    };
    let names = |on: bool| {
        let applied =
            Applied { options: pipeline::Options { mem2reg: on, ..Default::default() }, ..Applied::default() };
        pipeline::pipeline(&applied).iter().map(|one| one.name()).collect::<Vec<_>>()
    };
    assert!(names(true).contains(&"mem2reg") && !names(true).contains(&"promote"), "{:?}", names(true));
    assert!(!names(false).contains(&"mem2reg"), "{:?}", names(false));
    let (on, off) = (run(true), run(false));
    let printed_on = crate::testing::printed(&on);
    assert!(!printed_on.contains("alloca"), "{printed_on}");
    let inputs: [&[i128]; 3] = [&[0], &[1], &[10]];
    assert_eq!(crate::testing::results(&on, &inputs), crate::testing::results(&off, &inputs));
}

/// Old: a load of memory nothing wrote was `poison`, and a read-modify-write of
/// a fresh bitfield poisoned its container (`and poison, -8` is poison), so
/// `v.x = 5; return v.x` folded wrong. Then it was the zero of the type; the
/// pin gives `undef` (PromoteMemoryToRegister.cpp#L914).
#[test]
fn test_a_read_of_unwritten_memory_is_undef_not_poison() {
    let after = promoted(
        "define i8 @f() {
b0:
  %s = alloca i8
  %x = load i8, ptr %s
  %y = and i8 %x, -8
  %z = or i8 %y, 5
  store i8 %z, ptr %s
  %r = load i8, ptr %s
  ret i8 %r
}
",
    );
    assert!(!after.contains("poison") && !after.contains("alloca"), "{after}");
    assert!(after.contains("and i8 undef, -8"), "{after}");
}

/// The value on entry to a phi is undef as well: one arm stores, the other
/// reaches the join with the slot unwritten.
#[test]
fn test_the_entry_value_of_a_slot_at_a_join_is_undef() {
    let after = promoted(
        "define i32 @f(i32 %a, i1 %c) {
b0:
  %s = alloca i32
  br i1 %c, label %b1, label %b2
b1:
  store i32 %a, ptr %s
  br label %b2
b2:
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(!after.contains("poison"), "{after}");
    assert!(after.contains("phi i32") && after.contains("undef, %b0"), "{after}");
    let after = promoted(
        "define ptr @g() {
b0:
  %s = alloca ptr
  %x = load ptr, ptr %s
  ret ptr %x
}
",
    );
    assert!(after.contains("ret ptr undef"), "{after}");
}

/// Old: `struct { int x:3, y:5; } v; v.x = 5; v.y = 3; return v.x * 8 + v.y`
/// read the fresh container as poison and the optimized program returned
/// poison instead of -21 (clang -O2). Undef reads as any value, and the
/// masks keep only the written bits, so the full pipeline returns -21.
#[test]
fn test_a_bitfield_read_modify_write_on_a_fresh_container_returns_what_clang_does() {
    let text = "define i16 @f() {
b0:
  %s = alloca i8
  %c0 = load i8, ptr %s
  %m0 = and i8 %c0, -8
  %w0 = or i8 %m0, 5
  store i8 %w0, ptr %s
  %c1 = load i8, ptr %s
  %m1 = and i8 %c1, 7
  %w1 = or i8 %m1, 24
  store i8 %w1, ptr %s
  %c2 = load i8, ptr %s
  %l = shl i8 %c2, 5
  %x8 = ashr i8 %l, 5
  %x = sext i8 %x8 to i16
  %c3 = load i8, ptr %s
  %y8 = ashr i8 %c3, 3
  %y = sext i8 %y8 to i16
  %xs = mul i16 %x, 8
  %r = add i16 %xs, %y
  ret i16 %r
}
";
    use llrm_mir::program::Program;

    use crate::pipeline::{self, Applied};
    let mut module = module(text);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = crate::testing::printed(&module);
    assert!(!printed.contains("poison") && !printed.contains("alloca"), "{printed}");
    assert_eq!(
        crate::testing::results(&module, &[&[]]),
        vec![llrm_mir::interpret::Val::Int { bits: 0xffeb, width: 16 }]
    );
}

/// A load whose value is stored back and loaded again in one block: the
/// second load took the first's value, which the first load's own
/// replacement then left behind (the `zext` that SROA builds from the
/// loaded byte of 20000801-1 read an erased load, and ADCE panicked on
/// it). Each load takes the store's value as it is when the load is
/// replaced; the first load comes first on the alloca's list of uses once a
/// pass has rebuilt it.
#[test]
fn test_a_load_stored_and_loaded_again_leaves_no_erased_value_behind() {
    let mut module = module(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store i32 %a, ptr %s
  %x = load i32, ptr %s
  store i32 %x, ptr %s
  %y = load i32, ptr %s
  %z = add i32 %y, 1
  ret i32 %z
}
",
    );
    let function = crate::testing::f(&mut module);
    let first = function.iter_block(function.layout()[0]).nth(2).expect("the first load");
    let pointer = function.instruction(first).operands[0];
    function.set_operand(first, 0, pointer);
    let after = managed(&mut module, Mem2Reg);
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("add i32 %a, 1"), "{after}");
}
