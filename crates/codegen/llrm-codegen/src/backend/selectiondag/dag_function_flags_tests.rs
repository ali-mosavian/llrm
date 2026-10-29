//! Flags: what may sit between a compare and its branch, carries, selects on
//! one compare, shifts, constant branches and dropped blocks.

use super::dag_function::selected;
use super::dag_function_tests::{names, select_with_contracts, verdicts};

const SUM: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, %b
  ret i32 %s
}
";

/// `--isel=dag` made no function: every compile with it was refused. A sum of
/// two stack arguments comes out as the instructions the pin selects.
#[test]
fn a_function_of_one_block_is_selected_to_lir() {
    assert_eq!(names(SUM).unwrap(), ["mov", "add", ""]);
}

const SETCC_STORE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@flag = global i8 0
define void @f(i32 %a, i32 %b) {
entry:
  %c = icmp slt i32 %a, %b
  %z = zext i1 %c to i8
  store i8 %z, ptr @flag
  ret void
}
";

/// A comparison stored to a byte is `SETCCm`, which the emitter did not know.
#[test]
fn a_comparison_stored_to_a_byte_is_a_setcc_on_memory() {
    let module = llrm_mir::parse::module(SETCC_STORE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let set = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("setl")).expect("a setl");
    assert!(matches!(set.dests[0], crate::model::ir::Loc::Mem(_)), "{set:?}");
}

const VARIABLE_SHIFT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x, i32 %n) {
entry:
  %r = shl i32 %x, %n
  ret i32 %r
}
";

/// The matcher copies the shift amount to `CL` with a glued `CopyToReg`, which
/// the adapter turned into a copy to a virtual register (`CL` had no physical
/// id): the shift read a `CL` nothing defined, and the LIR verifier said
/// "value defined N times" (about 20 m32 programs once #4074 let them past
/// "missing source").
#[test]
fn a_variable_shift_reads_the_amount_the_matcher_copied_to_cl() {
    let module = llrm_mir::parse::module(VARIABLE_SHIFT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let shl = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("shl")).expect("a shl");
    assert!(shl.sources.iter().all(|l| matches!(l, crate::model::ir::Loc::Held(_))), "held values: {shl:?}");
}

const SHIFT_PARTS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1002 i64 @f(i64 noundef %0, i32 noundef %1) {
b1:
  %2 = alloca i64
  %3 = alloca i32
  %4 = alloca i64
  store i64 %0, ptr %2
  store i32 %1, ptr %3
  %5 = load i64, ptr %2
  %6 = load i32, ptr %3
  %7 = zext i32 %6 to i64
  %8 = lshr i64 %5, %7
  store i64 %8, ptr %4
  %9 = load i64, ptr %4
  ret i64 %9
}
";

/// The pin's scheduler keeps a unit that defines a physical register from
/// going between that register's definition and the unit that reads it
/// (`DelayForLiveRegsBottomUp`). The `shrd` of a 64-bit shift went between the
/// `test` and the second `jne`, which read the `shrd`'s flags
/// (shift_int64_variable printed wrong values).
#[test]
fn nothing_that_sets_flags_goes_between_a_test_and_the_branches_that_read_it() {
    let module = llrm_mir::parse::module(SHIFT_PARTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let names: Vec<String> =
        insns.iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    let test = names.iter().position(|n| n == "test").expect("a test");
    let last_branch = names.iter().rposition(|n| n == "jne" || n == "je").expect("a branch");
    for name in &names[test + 1..last_branch] {
        assert!(
            matches!(name.as_str(), "mov" | "jne" | "je" | "jmp"),
            "{name} between the test and the branches: {names:?}"
        );
    }
}

const SHIFTS_ON_ONE_COUNT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %x, i64 %y, i32 %n) {
entry:
  %c = zext i32 %n to i64
  %a = lshr i64 %x, %c
  %b = shl i64 %y, %c
  %m = mul i64 %b, 7
  %r = add i64 %a, %m
  ret i64 %r
}
";

/// Two selects on one compare, the second in the sink block of the first: a
/// flag-writing unit between them (the pin's scheduler allows it, its EFLAGS
/// live-in covers it) left the second branch on the flags of a `sub`
/// (shift_int64_variable -O2). Each branch is preceded by the compare that
/// made its flags.
#[test]
fn a_branch_on_flags_follows_the_compare_that_made_them() {
    let module = llrm_mir::parse::module(SHIFTS_ON_ONE_COUNT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let names: Vec<String> =
        insns.iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    for (at, name) in names.iter().enumerate() {
        if !matches!(name.as_str(), "jne" | "je") {
            continue;
        }
        let before = names[..at].iter().rev().find(|n| !matches!(n.as_str(), "mov" | "jmp" | "jne" | "je" | ""));
        assert!(
            matches!(before.map(String::as_str), Some("test" | "cmp" | "and")),
            "{name} after {before:?}: {names:?}"
        );
    }
}

const SHIFT_OPTIMIZED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1002 i64 @f(i64 noundef %0, i32 noundef %1) {
b1:
  %c = zext i32 %1 to i64
  %r = lshr i64 %0, %c
  ret i64 %r
}
";

/// The two selects of a 64-bit shift read one compare: the pin cascades the
/// second into the first's sink block (`EmitLoweredSelect`), one branch with
/// two phis. Two branches, the second with an empty arm, reached the
/// allocator as a shape that lost the zero of the high word (shr -O2 printed
/// the low word's bits there).
#[test]
fn two_selects_on_one_compare_are_one_branch() {
    let module = llrm_mir::parse::module(SHIFT_OPTIMIZED).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let branches =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Branch).count();
    assert_eq!(branches, 1);
    let phis: Vec<usize> = done.body.blocks.iter().map(|b| b.phis.len()).collect();
    assert!(phis.contains(&2), "one sink block with both phis: {phis:?}");
}

const M16_SELECT_IN_ARGUMENT: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare void @g(i64)
define void @f(i64 %x, i64 %n) {
entry:
  %r = shl i64 %x, %n
  call void @g(i64 %r)
  ret void
}
";

/// A select inside an outgoing argument area splits the block between the
/// area and the call: the stores after the split had no area in their block
/// and were refused 'a store through the stack pointer at m16'
/// (shift_int64_variable, shift_int64_masked_amount).
#[test]
fn an_argument_area_split_by_a_select_is_stored_through_a_copy_of_sp() {
    let module = llrm_mir::parse::module(M16_SELECT_IN_ARGUMENT).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

/// `B$SCMP` answers in the flags under `--runtime qb45`; the DAG path tested
/// `ax` instead, so every string comparison of `strcmp_smoke` printed 0. The
/// call is known to answer in the flags.
#[test]
fn a_runtime_call_answering_in_the_flags_says_so() {
    assert_eq!(verdicts(true), [Ok(super::builder::RuntimeAnswer::Flags)]);
}

const DEAD_DEFAULT: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define i16 @f(i16 %x) {
entry:
  switch i16 %x, label %dead [ i16 1, label %a ]
a:
  ret i16 7
dead:
  unreachable
}
";

const CONSTANT_BRANCH: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define i16 @f(i16 %x) {
entry:
  br i1 false, label %a, label %b
a:
  ret i16 7
b:
  ret i16 %x
}
";

/// A branch on a constant leaves one successor; the other block is dropped with
/// no branch left to it (`logical_value.c -O0` stopped on `block goes to
/// 0x001a, which is not in this body` when the MIR block was never built).
#[test]
fn a_branch_on_a_constant_leaves_no_branch_to_a_dropped_block() {
    let done = select_with_contracts(CONSTANT_BRANCH).unwrap_or_else(|why| panic!("{why}"));
    let present: llrm_support::hash::HashSet<i64> = done.body.blocks.iter().map(|b| b.at).collect();
    for block in &done.body.blocks {
        assert!(block.succ.iter().all(|next| present.contains(next)), "{:?} leaves for a missing block", block.at);
    }
}

/// A switch whose default is `unreachable` leaves that block with no edge;
/// the machine verifier stops at a block nothing reaches. It is dropped, as
/// `UnreachableMachineBlockElim` does.
#[test]
fn a_block_no_edge_reaches_is_dropped() {
    let done = select_with_contracts(DEAD_DEFAULT).unwrap_or_else(|why| panic!("{why}"));
    let by_at: llrm_support::hash::HashMap<i64, &crate::model::lir::Block> =
        done.body.blocks.iter().map(|b| (b.at, b)).collect();
    let mut seen = llrm_support::hash::HashSet::default();
    let mut work = vec![done.body.entry];
    while let Some(at) = work.pop() {
        if seen.insert(at) {
            work.extend(by_at[&at].succ.iter().copied());
        }
    }
    assert_eq!(seen.len(), done.body.blocks.len(), "{} blocks, {} reached", done.body.blocks.len(), seen.len());
}

const I1_FREEZE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define i16 @f(i1 %a, i16 %x) {
entry:
  %f = freeze i1 %a
  %z = zext i1 %f to i16
  %r = add i16 %z, %x
  ret i16 %r
}
";

/// Freezing an `i1` promotes the operand and freezes that
/// (`Freeze` of `Simple(I1)` stopped 2 QCport functions: 'promote result').
#[test]
fn an_i1_freeze_is_selected() {
    let module = llrm_mir::parse::module(I1_FREEZE).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_REMAINDER_TWICE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define i8 @f(ptr %p, i16 %a, i16 %b) {
entry:
  %r = srem i16 %a, %b
  store i16 %r, ptr %p
  %q = getelementptr i8, ptr %p, i16 %r
  %v = load i8, ptr %q
  ret i8 %v
}
";

/// A remainder read by a store and folded into an address: the selector
/// morphed the load without recording it as a user of the SDIVREM's result,
/// so the result's replacement missed it and the emitter met an unselected
/// 'SDivRem' (QCport `ls_animate`).
#[test]
fn a_remainder_used_by_a_store_and_an_address_is_selected() {
    let module = llrm_mir::parse::module(M16_REMAINDER_TWICE).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const CARRY_ACROSS_A_TEST: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1003 range(i32 0, -2147483648) i32 @_bench_x_ll_arith(i32 noundef %0) \"llrm-fp-eval\"=\"x87-extended\" memory(none) nounwind norecurse willreturn {
b1:
  %1 = icmp sgt i32 %0, 0
  br i1 %1, label %22, label %b3

b3:
  %2 = phi i32 [ %21, %20 ], [ 0, %b1 ]
  %3 = and i32 %2, 2147483647
  ret i32 %3

b4:
  %4 = phi i32 [ %19, %b4 ], [ 0, %22 ]
  %5 = phi i64 [ %18, %b4 ], [ 0, %22 ]
  %6 = phi i64 [ %8, %b4 ], [ 81985529216486895, %22 ]
  %7 = mul i64 %6, 6364136223846793005
  %8 = add i64 %7, 1442695040888963407
  %9 = lshr i64 %8, 33
  %10 = or i64 %9, 1
  %11 = mul i64 %10, %8
  %12 = and i32 %4, 31
  %13 = zext nneg i32 %12 to i64
  %14 = lshr i64 %8, %13
  %15 = xor i64 %11, %14
  %16 = shl i64 %8, %13
  %17 = add i64 %16, %5
  %18 = add i64 %17, %15
  %19 = add nuw nsw i32 %4, 1
  %exitcond = icmp ne i32 %0, %19
  br i1 %exitcond, label %b4, label %20

20:
  %21 = trunc i64 %18 to i32
  br label %b3

22:
  br label %b4
}
";

/// A 64-bit add's carry read after a select's `test` came between: the pin
/// glues the `adc` to the `add` and the scheduler here could not keep the
/// pair together ('flags overwritten between their producer and a consumer',
/// vsgcc x_ll_arith). The carries are `adc`s of the `add`s' own flags
/// (`combineCarryThroughADD`), with no byte round trip.
#[test]
fn a_carry_survives_a_flag_writer_scheduled_between_the_add_and_its_reader() {
    let module = llrm_mir::parse::module(CARRY_ACROSS_A_TEST).expect("parses");
    let done = selected(
        &module,
        "_bench_x_ll_arith",
        &llrm_x86_m32::M32,
        crate::backend::cpu::profile("486").expect("a cpu"),
        &[],
    )
    .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter_map(|w| w.name.as_deref().map(str::to_owned))
        .collect();
    assert!(
        names.iter().filter(|n| n.as_str() == "adc").count() >= 3 && !names.iter().any(|n| n.as_str() == "setb"),
        "{names:?}"
    );
}

const AFTER_UNREACHABLE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @abort()
define i32 @f(i32 %x, i1 %c) {
entry:
  br i1 %c, label %a, label %dead
a:
  %v = add i32 %x, 1
  br label %live
dead:
  call void @abort()
  unreachable
live:
  ret i32 %v
}
";

/// A block that ends in `unreachable` leads nowhere. The edge from it into the
/// next block in layout was made, and a value that block's real predecessor
/// defines was "read and not defined on every path" (the nib runtime's
/// `heap.allocate`, `@format.put`: 8 huge programs under `--isel=dag`).
#[test]
fn a_block_ending_in_unreachable_does_not_fall_into_the_next() {
    let module = llrm_mir::parse::module(AFTER_UNREACHABLE).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

const SHIFT_IN_MEMORY: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define void @f(ptr %p, i16 %n) {
entry:
  %v = load i16, ptr %p
  %c = and i16 %n, 15
  %r = shl i16 %v, %c
  store i16 %r, ptr %p
  ret void
}
";

/// A variable shift of a value in memory is `shl [p], cl`: the form pins its
/// amount to `cl`, which the glued copy wrote. Only the register forms read
/// it, and `SHL16mCL` stopped the nib runtime's `N$PR4` with `missing source`.
#[test]
fn a_variable_shift_in_memory_reads_cl() {
    let module = llrm_mir::parse::module(SHIFT_IN_MEMORY).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

const THREE_WAY_COMPARE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i8 @f(i32 %a, i32 %b) {
entry:
  %lt = icmp ult i32 %a, %b
  %min = select i1 %lt, i32 %a, i32 %b
  %any = icmp ne i32 %min, 0
  br i1 %any, label %longer, label %done
longer:
  ret i8 7
done:
  %lt2 = icmp ult i32 %a, %b
  %eq = icmp eq i32 %a, %b
  %neg = select i1 %lt2, i8 -1, i8 1
  %r = select i1 %eq, i8 0, i8 %neg
  ret i8 %r
}
";

/// One compare feeds an unsigned less-than, an equality and a branch, with
/// instructions that rewrite the flags between them: the compare is `sub`
/// whose value nothing reads, made again for the next reader (the nib
/// runtime's `N$VCMP` stopped with `flags overwritten between their producer
/// and a consumer`, 3 of 51 nib programs and 25 examples under `--isel=dag`).
#[test]
fn a_compare_read_after_the_flags_changed_is_made_again() {
    let module = llrm_mir::parse::module(THREE_WAY_COMPARE).expect("parses");
    selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

const FLAGS_OF_A_SUB_AFTER_A_SHIFT: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
declare fastcc void @_far_get(ptr, ptr)
declare fastcc i16 @_con_gfx_trim(ptr)
declare fastcc i16 @_con_back_load(ptr, ptr)
declare fastcc i16 @_con_chars_load(ptr, ptr)
declare fastcc void @_con_text(ptr, i32, i16, ptr)
declare fastcc void @_con_back_draw(ptr, i32, i16)
declare cc 1003 i32 @_strlen(ptr)
declare cc 1003 ptr @_strcpy(ptr, ptr)
declare i32 @llvm.smin.i32(i32, i32)
define cc 1003 void @_con_draw(ptr noundef nocapture %0, ptr noundef %1, i32 noundef %2, i16 noundef %3, float noundef %4) \"llrm-fp-eval\"=\"x87-extended\" {
b1:
  %5 = alloca [82 x i8]
  %6 = alloca [80 x i8]
  %7 = getelementptr inbounds nuw i8, ptr %0, i32 4
  %8 = load float, ptr %7
  %9 = fcmp ole float %8, 0.000000e+00
  br i1 %9, label %b3, label %b2

b2:
  %10 = tail call fastcc i16 @_con_back_load(ptr noundef nonnull %0, ptr noundef %1)
  %11 = icmp eq i16 %10, 0
  br i1 %11, label %b9, label %b18

b3:
  %12 = getelementptr inbounds nuw i8, ptr %0, i32 10
  %13 = load i16, ptr %12
  %14 = sext i16 %13 to i32
  %15 = trunc i32 %14 to i16
  %16 = icmp slt i32 %14, 4
  %17 = select i1 %16, i16 %15, i16 4
  %18 = sext i16 %17 to i32
  %19 = tail call fastcc i16 @_con_gfx_trim(ptr noundef nonnull %0)
  %20 = icmp eq i16 %19, 0
  br i1 %20, label %b9, label %b7

b7:
  %21 = tail call fastcc i16 @_con_chars_load(ptr noundef nonnull %0, ptr noundef %1)
  %22 = icmp ne i16 %21, 0
  %23 = icmp sgt i32 %18, 0
  %or.cond = select i1 %22, i1 %23, i1 false
  br i1 %or.cond, label %135, label %b9

b9:
  ret void

b14:
  %24 = phi i32 [ %139, %135 ], [ %49, %b17 ]
  %25 = phi i32 [ %138, %135 ], [ %48, %b17 ]
  %26 = phi i16 [ 0, %135 ], [ %47, %b17 ]
  %27 = getelementptr i8, ptr %0, i32 8
  %28 = load i16, ptr %27
  %29 = sext i16 %28 to i32
  %30 = add i32 %24, %29
  %31 = icmp slt i32 %30, 0
  %32 = select i1 %31, i32 15, i32 0
  %33 = sext i16 %28 to i32
  %34 = add i32 %32, %24
  %35 = add i32 %34, %33
  %36 = lshr i32 %35, 4
  %37 = shl nuw i32 %36, 4
  %38 = sub i32 %33, %37
  %39 = add i32 %24, %38
  %40 = mul i32 %39, 4
  %41 = getelementptr i8, ptr %0, i32 %40
  %42 = getelementptr i8, ptr %41, i32 1296
  %43 = load float, ptr %42
  %44 = fcmp ole float %43, 0.000000e+00
  br i1 %44, label %b17, label %b15

b15:
  %45 = getelementptr inbounds [80 x i8], ptr %136, i32 %39
  call fastcc void @_far_get(ptr noundef %5, ptr noundef nonnull %45)
  call fastcc void @_con_text(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %26, ptr noundef %5)
  %46 = add i16 %26, 8
  br label %b17

b17:
  %47 = phi i16 [ %26, %b14 ], [ %46, %b15 ]
  %48 = add i32 %25, -1
  %49 = add i32 %24, 1
  %50 = icmp samesign ugt i32 %48, 1
  br i1 %50, label %b14, label %b9

b18:
  %51 = getelementptr i8, ptr %0, i32 4
  %52 = load float, ptr %51
  %53 = fptosi float %52 to i16
  %54 = zext nneg i16 %53 to i32
  %55 = add i16 %53, -200
  tail call fastcc void @_con_back_draw(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %55)
  %56 = icmp slt i16 %53, 16
  br i1 %56, label %b9, label %b20

b20:
  %57 = tail call fastcc i16 @_con_chars_load(ptr noundef nonnull %0, ptr noundef %1)
  %58 = icmp eq i16 %57, 0
  br i1 %58, label %b9, label %b22

b22:
  %59 = add nsw i32 %54, -16
  %60 = lshr i32 %59, 3
  %61 = getelementptr inbounds nuw i8, ptr %0, i32 10
  %62 = load i16, ptr %61
  %63 = sext i16 %62 to i32
  %64 = getelementptr inbounds nuw i8, ptr %0, i32 12
  %65 = load i16, ptr %64
  %66 = sext i16 %65 to i32
  %67 = sext i16 %62 to i32
  %68 = sext i16 %65 to i32
  %69 = sub nsw i32 %67, %68
  %70 = icmp sgt i32 %60, %69
  %71 = select i1 %70, i32 %69, i32 %60
  %72 = shl i32 %71, 16
  %73 = ashr exact i32 %72, 16
  %74 = icmp sgt i32 %73, 0
  br i1 %74, label %140, label %b27

b27:
  %75 = trunc i32 %54 to i16
  store i8 93, ptr %5
  %76 = getelementptr inbounds nuw i8, ptr %0, i32 1360
  call fastcc void @_far_get(ptr noundef %6, ptr noundef nonnull %76)
  %77 = icmp slt i16 %3, 0
  %78 = select i1 %77, i16 7, i16 0
  %79 = add i16 %3, %78
  %80 = ashr i16 %79, 3
  %81 = call cc 1003 i32 @_strlen(ptr noundef nonnull %6)
  %82 = shl i32 %81, 16
  %83 = ashr exact i32 %82, 16
  %84 = sext i16 %80 to i32
  %85 = add nsw i32 %84, -4
  %86 = icmp sgt i32 %83, %85
  %87 = sub nsw i32 %83, %85
  %88 = shl i32 %87, 16
  %89 = ashr exact i32 %88, 16
  %90 = select i1 %86, i32 %89, i32 0
  %91 = getelementptr inbounds i8, ptr %6, i32 %90
  %92 = getelementptr inbounds nuw i8, ptr %5, i32 1
  %93 = call cc 1003 ptr @_strcpy(ptr noundef nonnull %92, ptr noundef nonnull %91)
  %94 = call cc 1003 i32 @_strlen(ptr noundef nonnull %5)
  %95 = fmul float %4, 4.000000e+00
  %96 = fptosi float %95 to i32
  %97 = trunc i32 %96 to i8
  %98 = and i8 %97, 1
  %99 = or i8 %98, 10
  %100 = shl i32 %94, 16
  %101 = ashr exact i32 %100, 16
  %102 = getelementptr inbounds i8, ptr %5, i32 %101
  store i8 %99, ptr %102
  %103 = add i32 %100, 65536
  %104 = ashr exact i32 %103, 16
  %105 = getelementptr inbounds i8, ptr %5, i32 %104
  store i8 0, ptr %105
  %106 = add nsw i16 %75, -16
  call fastcc void @_con_text(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %106, ptr noundef %5)
  br label %b9

b28:
  %107 = phi i32 [ %154, %140 ], [ %133, %b28 ]
  %108 = phi i32 [ %153, %140 ], [ %132, %b28 ]
  %109 = phi i16 [ %144, %140 ], [ %131, %b28 ]
  %110 = getelementptr i8, ptr %0, i32 8
  %111 = load i16, ptr %110
  %112 = getelementptr i8, ptr %0, i32 12
  %113 = load i16, ptr %112
  %114 = sext i16 %111 to i32
  %115 = sext i16 %113 to i32
  %116 = sub i32 %114, %115
  %117 = add i32 %107, %116
  %118 = icmp slt i32 %117, 0
  %119 = select i1 %118, i32 15, i32 0
  %120 = sext i16 %111 to i32
  %121 = sext i16 %113 to i32
  %122 = sub i32 %120, %121
  %123 = add i32 %119, %107
  %124 = add i32 %123, %120
  %125 = sub i32 %124, %121
  %126 = lshr i32 %125, 4
  %127 = shl nuw i32 %126, 4
  %128 = sub i32 %122, %127
  %129 = add i32 %107, %128
  %130 = getelementptr inbounds [80 x i8], ptr %145, i32 %129
  call fastcc void @_far_get(ptr noundef %5, ptr noundef nonnull %130)
  call fastcc void @_con_text(ptr noundef nonnull %0, i32 noundef %2, i16 noundef %109, ptr noundef %5)
  %131 = add i16 %109, 8
  %132 = add i32 %108, -1
  %133 = add i32 %107, 1
  %134 = icmp samesign ugt i32 %132, 1
  br i1 %134, label %b28, label %b27

135:
  %136 = getelementptr inbounds nuw i8, ptr %0, i32 14
  %137 = call i32 @llvm.smin.i32(i32 %14, i32 4)
  %138 = add nsw i32 %137, 1
  %139 = sub i32 16, %137
  br label %b14

140:
  %141 = trunc i32 %54 to i16
  %142 = lshr exact i32 %72, 13
  %143 = sub nsw i32 %59, %142
  %144 = trunc i32 %143 to i16
  %145 = getelementptr inbounds nuw i8, ptr %0, i32 14
  %146 = zext i16 %141 to i32
  %147 = add nsw i32 %146, -16
  %148 = lshr i32 %147, 3
  %149 = sub i32 %63, %66
  %150 = call i32 @llvm.smin.i32(i32 %148, i32 %149)
  %151 = trunc i32 %150 to i16
  %152 = sext i16 %151 to i32
  %153 = add nsw i32 %152, 1
  %154 = sub i32 16, %152
  br label %b28
}
";

/// A `sub`'s flags read by a branch after a shift came between: the flags are
/// made again as a `cmp` of the same operands, as a `cmp`'s own were
/// (QCport console `_con_draw` at m32: 'flags overwritten between their
/// producer and a consumer'). A `MOV32r0` is a `mov` here and writes none.
#[test]
fn the_flags_of_a_sub_are_made_again_after_another_unit_wrote_them() {
    let module = llrm_mir::parse::module(FLAGS_OF_A_SUB_AFTER_A_SHIFT).expect("parses");
    selected(&module, "_con_draw", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const SMUL_FIX: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %r = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 12)
  ret i32 %r
}
declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)
";

/// `smul.fix` is the product's two halves and a funnel shift by the scale
/// (the pin's `IMUL32r` and `SHRD32rri8`); `CannotSelect("SMulFix")` stopped
/// the `mandel` example, which does its arithmetic in 20.12 fixed point.
#[test]
fn a_fixed_point_multiply_is_a_widening_multiply_and_a_funnel_shift() {
    let names = names(SMUL_FIX).unwrap_or_else(|why| panic!("{why}"));
    assert!(names.iter().any(|n| n == "imul" || n == "mul"), "{names:?}");
    assert!(names.iter().any(|n| n == "shrd"), "{names:?}");
}

const ZEXT_PRODUCT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i32 %a, i32 %b) {
entry:
  %x = zext i32 %a to i64
  %y = zext i32 %b to i64
  %r = mul i64 %x, %y
  ret i64 %r
}
";

/// The product of two zero-extended 32-bit values is one `mul` (the pin's
/// `mull`). The old selector added two `imul` by a zeroed register for the
/// high cross products; parity/loop at bq32 -O2 ran 302 clocks against 170
/// (indvars' closed form is this product). The DAG path keeps it one `mul`.
#[test]
fn a_product_of_two_zero_extended_words_is_one_mul() {
    let names = names(ZEXT_PRODUCT).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(names.iter().filter(|n| *n == "mul").count(), 1, "{names:?}");
    assert!(!names.iter().any(|n| n == "imul"), "{names:?}");
}

const ASM_TWO_OUTPUTS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define i32 @f() addrspace(1) {
entry:
  %r = call { i16, i16 } @llrm.ia16.asm.cd1a.-.cx_dx.ax.n()
  %hi = extractvalue { i16, i16 } %r, 0
  %lo = extractvalue { i16, i16 } %r, 1
  %h = zext i16 %hi to i32
  %l = zext i16 %lo to i32
  %s = shl i32 %h, 16
  %v = or i32 %s, %l
  ret i32 %v
}
declare { i16, i16 } @llrm.ia16.asm.cd1a.-.cx_dx.ax.n()
";

/// An inline block with two outputs (`speaker.nib`'s BIOS tick count in
/// `cx:dx`) comes back as the fields of one aggregate; the builder refused
/// every block with more than one output.
#[test]
fn an_inline_block_with_two_outputs_is_selected() {
    let module = llrm_mir::parse::module(ASM_TWO_OUTPUTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    assert!(done.inline.values().any(|bytes| bytes[..] == [0xCD, 0x1A]), "{:?}", done.inline);
}

const COMPARE_WITH_ONE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare i16 @g()
define i16 @f() {
entry:
  %r = call i16 @g()
  %c = icmp ne i16 %r, 1
  br i1 %c, label %no, label %yes
no:
  ret i16 0
yes:
  ret i16 5
}
";

/// A compare with 1 was selected as `X86ISD::SUB` and kept as `sub ax, 1; jne`,
/// a write nothing reads (QCport `weapons.c`); the pin makes it `cmp ax, 1`.
#[test]
fn a_sub_whose_difference_is_unread_is_a_cmp() {
    let module = llrm_mir::parse::module(COMPARE_WITH_ONE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    // The pin's PeepholeOptimizer (`optimizeCompareInstr`) makes the `cmp`.
    let body = crate::backend::peepholeopt::compared(&done.body);
    let names: Vec<String> =
        body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    assert!(names.iter().any(|n| n == "cmp"), "{names:?}");
    assert!(!names.iter().any(|n| n == "sub"), "{names:?}");
}

const QB_THREE_COMPARES: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 i16 @f(ptr %a, ptr %b, ptr %c, ptr %d, ptr %e, ptr %g) addrspace(1) {
entry:
  %r1 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  %c1 = icmp eq i16 %r1, 0
  %r2 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %c, ptr %d)
  %c2 = icmp eq i16 %r2, 0
  %r3 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %e, ptr %g)
  %c3 = icmp eq i16 %r3, 0
  %a1 = and i1 %c1, %c2
  %a2 = and i1 %a1, %c3
  br i1 %a2, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}
declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1)
";

fn select_with_flags_answers(text: &str) -> Result<crate::backend::isel::Selected, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let contracts = super::dag_function::RuntimeContracts {
        abi: &super::dag_function_tests::Stub { flags: true },
        segments: &crate::backend::target::BASIC,
        runtime: true,
        frame_zeroed: false,
        facts: None,
    };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .map_err(|e| e.0)
}

/// Three string compares anded into one branch: the merged condition exported
/// the first compares' flags answers to registers, and `Target(24585)` (the
/// answer's node) reached selection (`CannotSelect`; 4 qb45 conformance
/// programs, e.g. `a$ = "dog " and b$ = "!" and len(c$) = 8`).
#[test]
fn string_compares_anded_into_a_branch_are_selected() {
    let done = select_with_flags_answers(QB_THREE_COMPARES);
    assert!(done.is_ok(), "{:?}", done.err());
}

const QB_COMPARE_READ_LATER: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 i16 @f(ptr %a, ptr %b, ptr %c, ptr %d) addrspace(1) {
entry:
  %r1 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  %c1 = icmp slt i16 %r1, 0
  %r2 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %c, ptr %d)
  %c2 = icmp slt i16 %r2, 0
  br i1 %c2, label %out, label %out
out:
  %v = select i1 %c1, i16 2, i16 3
  ret i16 %v
}
declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1)
";

/// A string compare read in a later block (`IF a$ < b$ THEN k% = 2 ELSE k% = 3`
/// after a loop's own compares) was settled to a byte but never exported: the
/// compare's register was read in the next block and written on no path
/// ("value N is read and not defined on every path", `movzx`; 168 qbrt
/// programs errored on string_compare_shapes.bas).
#[test]
fn a_string_compare_read_in_a_later_block_is_exported() {
    let done = select_with_flags_answers(QB_COMPARE_READ_LATER);
    assert!(done.is_ok(), "{:?}", done.err());
}
