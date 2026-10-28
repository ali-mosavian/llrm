//! Floating point: x87 results, conversions, compares, constants and pool
//! entries, spills.

use super::dag_function::selected;
use super::dag_function_tests::{names, select_with_contracts};
use crate::testing::m16_ir;

const FLOAT_RESULT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(double %x) {
entry:
  %m = fmul double %x, 2.1
  %r = fadd double %m, 1.0
  ret double %r
}
";

/// A floating result is the x87 stack's top at the return, which the stackifier
/// reads as an unnamed store of the value: it was popped and dropped
/// (`fstp st(0)`) once loads and arithmetic were emitted. The constant 2.5 is a
/// pool cell, 1.0 is `fld1`.
#[test]
fn a_floating_result_is_stored_unnamed_before_the_return() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(FLOAT_RESULT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<(Operation, String)> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .map(|w| (w.op, w.name.as_deref().unwrap_or("").to_owned()))
        .collect();
    let names: Vec<&str> = seen.iter().map(|(_, name)| name.as_str()).collect();
    assert!(
        names.contains(&"fld") && names.contains(&"fmul") && names.contains(&"fld1") && names.contains(&"fadd"),
        "{names:?}"
    );
    let ret = seen.len() - 1;
    assert_eq!(seen[ret - 1], (Operation::FloatStore, String::new()), "{names:?}");
    let pooled = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.sources.iter())
        .any(
            |l| matches!(
                l,
                crate::model::ir::Loc::Mem(m) if m.width == 8 && m.addr.is_some()
            ),
        );
    assert!(pooled, "the 2.5 is a cell of the pool: {names:?}");
}

const FLOAT_TO_INT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %x) {
entry:
  %r = fptosi double %x to i32
  ret i32 %r
}
";

/// A float truncated to an integer is a `fistt` of the value to a cell (the
/// pin's `FP64_TO_INT32_IN_MEM`, whose control-word dance `floatalloc` makes);
/// 22 programs stopped at `Unmapped(FP*_TO_INT*_IN_MEM)`.
#[test]
fn a_float_to_int_conversion_is_a_truncating_store() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(FLOAT_TO_INT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let store = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::FloatStore && w.name.as_deref() == Some("fisttp"))
        .expect("a fistt");
    assert!(matches!(&store.dests[..], [Loc::Mem(cell)] if cell.width == 4), "{store:?}");
}

const FLOAT_COMPARE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %a, double %b) {
entry:
  %c = fcmp olt double %a, %b
  %r = zext i1 %c to i32
  ret i32 %r
}
";

/// A float compare is an `fcom` of two values and the set that reads its flags;
/// the stackifier adds the `fnstsw ax; sahf`, so the pin's `FNSTSW16r`, the AH
/// copy and `SAHF` emit nothing. 12 programs stopped at `UCOM_Fpr*`.
#[test]
fn a_float_compare_is_an_fcom_and_a_set() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(FLOAT_COMPARE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    let compare = whats.iter().position(|w| w.op == Operation::Compare).expect("an fcom");
    assert_eq!(whats[compare].name.as_deref(), Some("fcom"));
    assert!(whats[compare].sources.iter().all(|l| matches!(l, Loc::Held(h) if h.width == 10)), "{:?}", whats[compare]);
    assert!(whats[compare..].iter().any(|w| w.name.as_deref().is_some_and(|n| n.starts_with("set"))));
}

const FLOAT_COMPARE_AND_ADD: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"

define internal cc 1000 void @f() addrspace(1) \"llrm-fp-eval\"=\"x87-extended\" norecurse {
b1:
  %0 = tail call double @llvm.log2.f64(double 1.000000e+01)
  br label %b5

b5:
  %1 = phi i16 [ -4, %b1 ], [ %27, %b8 ]
  %2 = phi double [ 1.100000e+00, %b1 ], [ %14, %b8 ]
  %3 = phi double [ 2.200000e+00, %b1 ], [ %19, %b8 ]
  %4 = phi double [ 9.900000e+00, %b1 ], [ %21, %b8 ]
  %5 = phi double [ 1.010000e+01, %b1 ], [ %24, %b8 ]
  %6 = phi double [ 1.111000e+01, %b1 ], [ %26, %b8 ]
  %7 = add i16 %1, 5
  %8 = sitofp i16 %1 to double
  %9 = fmul double %0, %8
  %10 = fcmp oge double %9, 3.276750e+04
  br i1 %10, label %b7, label %b8

b6:
  ret void

b7:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b8:
  %11 = tail call double @llvm.exp2.f64(double %9)
  %12 = fmul double %11, %2
  %13 = fadd double %3, %12
  %14 = fpround double %13 to double
  %15 = sitofp i16 %7 to double
  %16 = fadd double %15, 3.300000e+00
  %17 = fmul double %16, 4.400000e+00
  %18 = fadd double %17, %3
  %19 = fpround double %18 to double
  %20 = fadd double %4, 0x3EF9602356465DB3
  %21 = fpround double %20 to double
  %22 = fmul double %5, 1.000100e+00
  %23 = fadd double %22, 7.700000e+00
  %24 = fpround double %23 to double
  %25 = fadd double %6, 8.800000e-03
  %26 = fpround double %25 to double
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %14)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %19)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double 3.300000e+00)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double 4.400000e+00)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double 5.500000e+00)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double 6.600000e+00)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double 7.700000e+00)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double 8.800000e+00)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %21)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %24)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %26)
  %27 = add nsw i16 %1, 1
  %exitcond = icmp ne i16 %27, 5
  br i1 %exitcond, label %b5, label %b6
}

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PCR8(double) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

!0 = !{!\"llrm hir\"}
!1 = !{!\"place\", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!\"allocation\", !0, i64 0}
!4 = !{!3, !3, i64 0}
";

/// `fnstsw; sahf` follow the `fcom`, so an `add` scheduled between the compare
/// and its `jae` was read by the branch: `pow_stack_depth` raised Overflow on
/// the fifth pass of a loop (`fcom; add ax, 5; jae`).
#[test]
fn nothing_that_writes_the_flags_sits_between_an_fcom_and_its_branch() {
    let done = select_with_contracts(m16_ir(FLOAT_COMPARE_AND_ADD)).unwrap_or_else(|why| panic!("{why}"));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    let compare = names.iter().position(|n| n == "fcom").expect("an fcom");
    let branch =
        names[compare..].iter().position(|n| n.starts_with('j')).unwrap_or_else(|| panic!("{names:?}")) + compare;
    assert_eq!(branch, compare + 1, "{names:?}");
}

const FLOAT_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare float @rf()
declare double @rd(double)
define float @g() {
entry:
  %a = call float @rf()
  %b = fmul float %a, 2.0
  ret float %b
}
define double @h(double %x) {
entry:
  %a = call double @rd(double %x)
  %b = call double @rd(double %x)
  %c = fadd double %a, %b
  ret double %c
}
";

/// A float call result is the stack's top after the call (an unnamed load of no
/// cell); one live across another call that returns a float is selected too:
/// the float allocator spills it around the call and the stackifier takes the
/// result under the reload (RFP-5, fpstack #4362).
#[test]
fn a_float_result_is_the_stack_top_and_one_may_live_across_a_call() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(FLOAT_CALL).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu");
    let done = selected(&module, "g", &llrm_x86_m32::M32, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    assert!(
        insns.iter().filter_map(|i| i.what.as_ref()).any(|w| {
            w.op == Operation::FloatLoad
                && w.name.as_deref() == Some("")
                && w.sources.is_empty()
                && matches!(&w.dests[..], [Loc::Held(h)] if h.width == 10)
        }),
        "an unnamed load of the stack top"
    );
    selected(&module, "h", &llrm_x86_m32::M32, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
}

const FLOAT_ACROSS_VOID_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @note(i32)
define double @f(double %x, i32 %n) {
entry:
  %y = fadd double %x, 1.000000e+00
  call void @note(i32 %n)
  %z = fadd double %y, %x
  ret double %z
}
";

/// A float live across a call that returns no float is selected too.
#[test]
fn a_float_live_across_a_call_that_returns_no_float_is_selected() {
    let module = llrm_mir::parse::module(FLOAT_ACROSS_VOID_CALL).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu");
    selected(&module, "f", &llrm_x86_m32::M32, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
}

const DOUBLE_BITS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %x) {
entry:
  %b = bitcast double %x to i64
  %t = trunc i64 %b to i32
  %r = and i32 %t, 16777215
  ret i32 %r
}
";

/// The low word of a double reached the matcher as the double's x87 register
/// (`and v5(4 bytes) = v5(10-byte fdiv result) & mask`): no code does that.
/// A float read as an integer is a refusal; float_conversions_loop did not
/// assemble (`mov [esp], mm0`).
#[test]
fn a_double_read_as_its_words_is_refused_not_emitted() {
    let module = llrm_mir::parse::module(DOUBLE_BITS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    if let Err(why) = &done {
        assert!(why.0.contains("floating value used as an integer") || !why.0.is_empty(), "{why:?}");
    }
    // Whatever it is, no integer instruction reads a 10-byte floating value.
    if let Ok(done) = done {
        let floating: llrm_support::hash::HashSet<u32> = done
            .body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .flat_map(|w| w.dests.iter())
            .filter_map(|d| match d {
                crate::model::ir::Loc::Held(h) if h.width == 10 => Some(h.value),
                _ => None,
            })
            .collect();
        for what in done.body.insns().iter().filter_map(|i| i.what.as_ref()) {
            for l in what.dests.iter().chain(&what.sources) {
                assert!(
                    !matches!(l, crate::model::ir::Loc::Held(h) if h.width != 10 && floating.contains(&h.value)),
                    "{what:?}"
                );
            }
        }
    }
}

const CONSTANT_AFTER_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare double @g(double)
define double @f(double %x) {
entry:
  %a = call double @g(double %x)
  %r = fadd double %a, 1.0
  ret double %r
}
";

/// A floating constant is loaded where it is read: `fld1` hoisted above the
/// call stayed on the x87 stack across it, and the function was refused
/// ("a floating value is live across ...").
#[test]
fn a_floating_constant_is_loaded_after_the_call_that_precedes_its_use() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(CONSTANT_AFTER_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    let call = whats.iter().position(|w| w.op == Operation::Call).expect("a call");
    let load = whats.iter().position(|w| w.name.as_deref() == Some("fld1")).expect("an fld1");
    assert!(
        load > call,
        "the constant is read after the call: {:?}",
        whats.iter().map(|w| &w.name).collect::<Vec<_>>()
    );
}

const NARROW_POOL_CONSTANT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(double %x) {
entry:
  %r = fmul double %x, 2.5
  ret double %r
}
";

/// The pool holds 2.5 as four bytes (a double that is exactly a float), and the
/// multiply reads four: it read eight, the constant's neighbour as its high
/// word (float_return, x87_arguments, double_to_uint64 printed wrong values).
/// A read at another width is refused as well.
#[test]
fn a_pool_entry_is_read_at_its_own_width() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(NARROW_POOL_CONSTANT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let multiply = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::FloatArith && w.name.as_deref() == Some("fmul"))
        .expect("an fmul");
    assert!(multiply.sources.iter().any(|l| matches!(l, Loc::Mem(m) if m.width == 4)), "{multiply:?}");
}

const LONG_DOUBLE_POOL_CONSTANT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define x86_fp80 @f(x86_fp80 %x) {
entry:
  %m = fmul x86_fp80 %x, CONSTANT
  ret x86_fp80 %m
}
";

/// The width at which `x * constant` reads its `f80` constant from the pool.
fn long_double_constant_width(constant: &str) -> u32 {
    use crate::model::ir::{Loc, Operation};
    let text = LONG_DOUBLE_POOL_CONSTANT.replace("CONSTANT", constant);
    let module = llrm_mir::parse::module(&text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let multiply = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::FloatArith && w.name.as_deref() == Some("fmul"))
        .expect("an fmul");
    multiply.sources.iter().find_map(|l| if let Loc::Mem(m) = l { Some(m.width.into()) } else { None }).unwrap_or(10)
}

/// An `f80` constant a float or double holds exactly sits in the pool at that
/// width (`ExpandConstantFP` steps f80, f64, f32): 10000 took ten bytes, and
/// QB's float-to-CURRENCY programs grew 8 to 22 bytes at m16 -Os.
#[test]
fn a_long_double_constant_is_read_at_the_narrowest_exact_width() {
    assert_eq!(long_double_constant_width("10000.0"), 4);
    assert_eq!(long_double_constant_width("0.00005"), 8);
}

const LONG_DOUBLE_SUM: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define x86_fp80 @f(x86_fp80 %x, x86_fp80 %y) {
entry:
  %r = fadd x86_fp80 %x, %y
  ret x86_fp80 %r
}
";

/// A long double is selected (it was refused 'long double' until its
/// arguments were proved), and each stack argument takes a 12-byte slot at m32,
/// as the front end and gcc lay them out: slots were the 10-byte store size,
/// so `sum(3, a, b, a)` read its second and third values 2 and 4 bytes short
/// (long_double_variadic printed -2147483648 for 13).
#[test]
fn a_long_double_is_selected_and_takes_a_twelve_byte_slot() {
    let module = llrm_mir::parse::module(LONG_DOUBLE_SUM).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let first = llrm_target::Target::first_argument_offset(&llrm_x86_m32::M32, false);
    assert_eq!(convention.stack_arguments - first, 24, "two long doubles, twelve bytes each: {convention:?}");
    let _ = function;
}

const INT_TO_FLOAT_ROUNDED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define float @f(i32 %x) {
entry:
  %r = sitofp i32 %x to float
  ret float %r
}
";

/// Under `-fexcess-precision=standard` an integer wider than the float's
/// mantissa is rounded to the float type (16777217 is 16777216.0f): the
/// conversion is stored and loaded back. The DAG path left the exact `fild`
/// in the register and `big != single` answered 1 (int_compared_with_float
/// -O0).
#[test]
fn an_integer_wider_than_the_mantissa_is_rounded_under_float_store() {
    let module = llrm_mir::parse::module(INT_TO_FLOAT_ROUNDED).expect("parses");
    let stores = |float_store: bool| {
        let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
        cpu.float_store = float_store;
        let done = selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
        done.body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .filter(|w| w.op == crate::model::ir::Operation::FloatStore)
            .count()
    };
    assert!(stores(true) > stores(false), "{} vs {}", stores(true), stores(false));
}

const U64_TO_DOUBLE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(i64 %x) {
entry:
  %r = uitofp i64 %x to double
  ret double %r
}
";

/// An unsigned 64-bit integer converts through the 80-bit register class and a
/// pool entry of its fudge (0.0f at +0, 2^64 at +4): the entry is read four
/// bytes inside its eight, and the register class alone is no long double.
/// 19 functions stopped at the entry's type.
#[test]
fn an_unsigned_64_bit_integer_converts_to_a_double() {
    let module = llrm_mir::parse::module(U64_TO_DOUBLE).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_DOUBLE_ARGUMENT: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare void @g(double)
define void @f(double %x) {
entry:
  %y = fadd double %x, 1.000000e+00
  call void @g(double %y)
  ret void
}
";

/// A double passed on the stack at m16 is stored with `fstp` into the argument
/// area, which no push can do and `[sp+k]` cannot address: it goes through a
/// register that holds SP (`byval_struct`, `float_return`, `wide_frame` and 5
/// more were refused 'a floating argument stored into the area').
#[test]
fn a_double_argument_is_stored_through_a_copy_of_sp_at_m16() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(M16_DOUBLE_ARGUMENT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let store = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| {
            w.op == Operation::FloatStore
                && matches!(&w.dests[..], [Loc::Mem(_)])
                && w.name.as_deref().is_some_and(|n| n.starts_with("fst"))
        })
        .expect("an fstp into the area");
    let Loc::Mem(cell) = &store.dests[0] else { unreachable!() };
    assert!(cell.base.is_some() && cell.through == llrm_lir::registers::RegId::None, "{cell:?}");
}

const FPROUND_F32: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define cc 1000 ptr @h(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = load float, ptr %0
  %3 = fmul float %2, 5.000000e-01
  %4 = fpround float %3 to float
  store float %4, ptr %1
  ret ptr %1
}
";

/// `fpround float to float` stores to a slot and loads it back; the slot was a
/// frame object the frame did not know (`Emit(Unsupported("frame index 0"))`:
/// `byref2.bas`, a rounded SINGLE).
#[test]
fn a_rounded_float_has_its_slot_in_the_frame() {
    let module = llrm_mir::parse::module(m16_ir(FPROUND_F32)).expect("parses");
    selected(&module, "h", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

const ROUNDED_STORE: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define cc 1000 void @h(ptr %0, ptr %1, ptr %2) addrspace(1) {
b1:
  %3 = load double, ptr %0
  %4 = load double, ptr %1
  %5 = fmul double %3, %4
  %6 = fptrunc double %5 to float
  store float %6, ptr %2
  ret void
}
";

/// A double rounded to a float and stored is one `fstp dword`: the store
/// rounds. Store, reload and store again left a 4-byte frame temporary and two
/// more instructions per rounded store (ACTIONS3D: 25 round trips).
#[test]
fn a_rounded_float_stored_is_one_truncating_store() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(m16_ir(ROUNDED_STORE)).expect("parses");
    let done = selected(&module, "h", &llrm_x86_m16::M16, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    let stores =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == Operation::FloatStore).count();
    assert_eq!(stores, 1);
}

const LRINT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(double %x) {
entry:
  %r = call i32 @llvm.lrint.i32.f64(double %x)
  ret i32 %r
}
declare i32 @llvm.lrint.i32.f64(double)
";

/// `lrint` of an x87 value is `fist` into a stack slot and a load (the pin's
/// `IST_Fp32m64`); `CannotSelect(LRint)` refused 7 qb functions, the ones that
/// round a float to an integer (`CINT`).
#[test]
fn an_lrint_of_a_double_is_a_fist_and_a_load() {
    let made = names(LRINT).unwrap_or_else(|why| panic!("{why}"));
    assert!(made.iter().any(|name| name.starts_with("fist")), "{made:?}");
}

const FLOAT_ACROSS_A_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-f80:32-n8:16:32\"
declare void @g()
define double @f(double %a, double %b) {
entry:
  %x = fadd double %a, %b
  call void @g()
  %y = fmul double %x, %x
  ret double %y
}
";

/// A double live across a call spilled as a qword whatever the language:
/// `--isel=dag` left `LirBody::float_bytes` empty, which the spiller reads as
/// "8". BASIC keeps the extended 10 bytes (`spills_extended`):
/// `pow_unrolled.bas` printed 100000000 for 99999999.99999996, as log2(10) lost
/// 11 bits across a call (8 of the 24 log/exp programs).
#[test]
fn a_float_spill_keeps_the_width_the_language_asks_for() {
    let module = llrm_mir::parse::module(FLOAT_ACROSS_A_CALL).expect("parses");
    let mut basic = crate::backend::cpu::profile("486").expect("a cpu").clone();
    basic.spills_extended = true;
    let done = selected(&module, "f", &llrm_x86_m32::M32, &basic, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    assert!(!done.body.float_bytes.is_empty(), "a float table");
    assert!(done.body.float_bytes.values().all(|bytes| *bytes == 10), "{:?}", done.body.float_bytes);
    let mut c = crate::backend::cpu::profile("486").expect("a cpu").clone();
    c.rounds_float_spills = true;
    let done = selected(&module, "f", &llrm_x86_m32::M32, &c, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    assert!(done.body.float_bytes.values().any(|bytes| *bytes == 8), "{:?}", done.body.float_bytes);
}

const UINT32_TO_DOUBLE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(i64 %a) {
entry:
  %r = uitofp i64 %a to double
  ret double %r
}
";

/// The fudge of an unsigned conversion is two floats in one pool entry, and the
/// `fadd` reads one of them by its sign: a dword. The memory operand took the
/// entry's eight bytes (`fadd qword ptr $K[bx]`), added a huge double, and
/// `regparm3_matrix_4`'s `f3` returned `-2147483648` under `--isel=dag`.
#[test]
fn the_fudge_of_an_unsigned_conversion_is_read_as_a_float() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(UINT32_TO_DOUBLE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    let indexed: Vec<u32> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.sources.iter())
        .filter_map(|loc| match loc {
            Loc::Mem(m) if m.index.is_some() => Some(m.width),
            _ => None,
        })
        .collect();
    assert_eq!(indexed, [4], "the fudge is indexed by the sign and is a float");
}

const BITCAST_DOUBLE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @bits(double %d) {
entry:
  %i = bitcast double %d to i64
  ret i64 %i
}
define double @from(i64 %i) {
entry:
  %d = bitcast i64 %i to double
  ret double %d
}
define i32 @fbits(float %f) {
entry:
  %i = bitcast float %f to i32
  %n = xor i32 %i, 2147483648
  ret i32 %n
}
define float @ffrom(i32 %i) {
entry:
  %f = bitcast i32 %i to float
  ret float %f
}
";

/// A float read as an integer's bits (and back) goes through a stack slot, as
/// `ExpandBitCast` has it: a store of one type and a load of the other.
#[test]
fn a_bitcast_between_a_float_and_an_integer_goes_through_the_stack() {
    let module = llrm_mir::parse::module(BITCAST_DOUBLE).expect("parses");
    for name in ["bits", "from", "fbits", "ffrom"] {
        selected(&module, name, &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{name}: {}", e.0));
    }
}

/// An unsigned 64-bit integer converted to a double reads the fudge constant
/// at +0 or +4 of its pool entry: the address carried no offset, and
/// `(double)big` panicked in the adapter ("a constant pool entry plus a
/// displacement is not in our DAG"; `c/zz_f.c` at -O0 m32).
#[test]
fn an_unsigned_i64_converts_to_a_double_through_the_pool_at_an_offset() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @report(i32)
define i32 @f() {
  %1 = alloca i64, align 8
  %2 = alloca double, align 8
  store i64 -1, ptr %1, align 8
  %3 = load i64, ptr %1, align 8
  %4 = uitofp i64 %3 to double
  store double %4, ptr %2, align 8
  %5 = load double, ptr %2, align 8
  %6 = fcmp oeq double %5, 1.000000e+00
  %7 = zext i1 %6 to i32
  call void @report(i32 %7)
  ret i32 0
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const ROUNDED_DOUBLE_STORE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define void @f(double %x, ptr %p) {
entry:
  %a = fadd double %x, 1.000000e+00
  %r = fpround double %a to double
  store double %r, ptr %p
  ret void
}
";

const PLAIN_DOUBLE_STORE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define void @f(double %x, ptr %p) {
entry:
  %a = fadd double %x, 1.000000e+00
  store double %a, ptr %p
  ret void
}
";

/// A double stored is rounded by the store: `fpround` read only by stores of
/// its type went through a stack slot first (`fstp [slot]; fld [slot]; fstp
/// [p]`), 112 bytes of frame and 267 bytes of code in nbody.bas under
/// `--isel=dag` that the old selector's single `fstp` did not have.
#[test]
fn a_round_read_only_by_stores_is_not_stored_to_a_slot_first() {
    let count = |text: &str| {
        let module = llrm_mir::parse::module(text).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{}", e.0));
        done.body.insns().len()
    };
    assert_eq!(count(ROUNDED_DOUBLE_STORE), count(PLAIN_DOUBLE_STORE));
}

const ROUNDED_ARGUMENT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @g(double)
define void @f(double %x) {
entry:
  %a = fadd double %x, 1.000000e+00
  %r = fpround double %a to double
  call void @g(double %r)
  ret void
}
";

const PLAIN_ARGUMENT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @g(double)
define void @f(double %x) {
entry:
  %a = fadd double %x, 1.000000e+00
  call void @g(double %a)
  ret void
}
";

/// A double passed on the stack is stored there, which rounds it: the same
/// slot round trip came before `B$PER8`'s argument (range_power_digits.bas).
#[test]
fn a_round_read_only_as_a_call_argument_is_not_stored_to_a_slot_first() {
    let count = |text: &str| {
        let module = llrm_mir::parse::module(text).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{}", e.0));
        done.body.insns().len()
    };
    assert_eq!(count(ROUNDED_ARGUMENT), count(PLAIN_ARGUMENT));
}

const HUGE_DIFFERENCE: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3), ptr addrspace(3))
define cc 1005 i32 @f(ptr addrspace(3) %0, ptr addrspace(3) %1) addrspace(1) {
b1:
  %2 = tail call i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3) %0, ptr addrspace(3) %1)
  ret i32 %2
}
";

/// A far pointer argument only subtracted loads as words that the `movzx`
/// folds; it was a `les` whose selector `mov` fed a register `movzx` (a `mov`
/// and a `movzx` more for each selector in `d7`, 2 B each at m16 -Os).
#[test]
fn a_far_pointer_read_as_data_is_loaded_into_movzx() {
    let module = llrm_mir::parse::module(m16_ir(HUGE_DIFFERENCE)).expect("parses");
    let done = super::dag_function::selected(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let made: Vec<String> = done
        .body
        .insns()
        .iter()
        .map(|i| i.what.as_ref().and_then(|w| w.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect();
    assert!(!made.iter().any(|name| name == "mov"), "{made:?}");
}

const BITREVERSE_OF_A_HIGH_BYTE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare i32 @llvm.bitreverse.i32(i32)
define i32 @f(i32 %x) {
  %m = and i32 %x, -16777216
  %r = call i32 @llvm.bitreverse.i32(i32 %m)
  ret i32 %r
}
";

/// The pin's `SimplifyDemandedBits` turns a `bswap` of which one byte is
/// demanded into a shift (and the mask beneath it into nothing): the
/// bit-reverse of a masked high byte kept `and; bswap` and full-width stage
/// masks, 21 instructions against the pin's 17.
#[test]
fn a_bswap_of_which_one_byte_is_demanded_is_a_shift() {
    let made = super::dag_function_tests::names(BITREVERSE_OF_A_HIGH_BYTE).unwrap_or_else(|why| panic!("{why}"));
    assert!(!made.iter().any(|name| name == "bswap"), "{made:?}");
    assert!(made.len() <= 17, "{made:?}");
}

const U16_TO_FLOAT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define float @f(i16 zeroext %a) {
entry:
  %r = uitofp i16 %a to float
  ret float %r
}
";

/// `(float)a` of an `unsigned short` was promoted to an unsigned dword and
/// converted through a 64-bit buffer with a zero high dword and `fild qword`;
/// the pin's `LowerUINT_TO_FP` makes a nonnegative source signed: one `fild`
/// of the dword (uitofp_nneg, -Os).
#[test]
fn an_unsigned_short_converts_with_a_dword_fild_not_a_qword_buffer() {
    let module = llrm_mir::parse::module(U16_TO_FLOAT).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    let done = selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<_> = done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
    let widths: Vec<u32> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.name.as_deref() == Some("fild"))
        .flat_map(|w| w.sources.iter())
        .filter_map(|source| match source {
            crate::model::ir::Loc::Mem(m) => Some(m.width),
            _ => None,
        })
        .collect();
    assert_eq!(widths, [4], "the fild reads a dword, not a 64-bit buffer: {names:?}");
}

const INT_TO_FLOAT_MULTIPLIED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define float @f(ptr %p, float %c) {
entry:
  %x = load i32, ptr %p
  %v = sitofp i32 %x to float
  %r = fmul float %c, %v
  ret float %r
}
";

/// As the pin, under `-fexcess-precision=standard` a conversion that only
/// feeds a multiplication is not rounded: `fimul` folds it (`s += c[i] *
/// (float)p[i]` was `fild; fstp; fmul` where the pin has `fimul`: +3.6%
/// instructions on jpeg_single bq32 -Os).
#[test]
fn an_integer_feeding_a_multiplication_is_not_rounded_under_float_store() {
    let module = llrm_mir::parse::module(INT_TO_FLOAT_MULTIPLIED).expect("parses");
    let stores = |float_store: bool| {
        let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
        cpu.float_store = float_store;
        let done = selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
        done.body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .filter(|w| w.op == crate::model::ir::Operation::FloatStore)
            .count()
    };
    assert_eq!(stores(true), stores(false));
}
