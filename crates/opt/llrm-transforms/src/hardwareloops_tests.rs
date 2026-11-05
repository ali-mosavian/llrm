//! `HardwareLoops`: a counted loop is counted by `llvm.loop.decrement`.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::tti::{ArithOp, Basic, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Ty};
use llrm_mir::tti::{HardwareLoopTarget, Tti};

use crate::testing::{parsed, printed, results};

/// `Basic`'s prices, the ones a `Tti` must state.
macro_rules! basic_prices {
    () => {
        fn cmp_sel_cost(
            &self,
            op: CmpSel,
            ty: Ty,
            kind: CostKind,
        ) -> Cost {
            Basic.cmp_sel_cost(op, ty, kind)
        }

        fn memory_op_cost(
            &self,
            op: MemoryOp,
            ty: Ty,
            kind: CostKind,
            stored_constant: bool,
        ) -> Cost {
            Basic.memory_op_cost(op, ty, kind, stored_constant)
        }

        fn cf_instr_cost(
            &self,
            phi: bool,
            kind: CostKind,
        ) -> Cost {
            Basic.cf_instr_cost(phi, kind)
        }

        fn int_imm_cost(
            &self,
            imm: i128,
            bits: u32,
        ) -> Cost {
            Basic.int_imm_cost(imm, bits)
        }

        fn int_imm_cost_inst(
            &self,
            op: ImmUser,
            index: u32,
            imm: i128,
            bits: u32,
        ) -> Cost {
            Basic.int_imm_cost_inst(op, index, imm, bits)
        }

        fn is_truncate_free(
            &self,
            from: Ty,
            to: Ty,
        ) -> bool {
            Basic.is_truncate_free(from, to)
        }

        fn is_legal_addressing_mode(
            &self,
            gv: bool,
            offset: i64,
            base: bool,
            scale: i64,
        ) -> bool {
            Basic.is_legal_addressing_mode(gv, offset, base, scale)
        }

        fn cast_cost(
            &self,
            op: CastKind,
            dst: Ty,
            src: Ty,
            kind: CostKind,
        ) -> Cost {
            Basic.cast_cost(op, dst, src, kind)
        }

        fn arithmetic_cost(
            &self,
            op: ArithOp,
            ty: Ty,
            kind: CostKind,
            first: OperandInfo,
            second: OperandInfo,
        ) -> Cost {
            Basic.arithmetic_cost(op, ty, kind, first, second)
        }
    };
}

/// A target that counts loops in hardware with a `bits`-wide counter.
struct Counts {
    bits: u32,
    nesting: bool,
    /// The counter is a register value carried by a phi.
    phi: bool,
    /// Only where the loop's own counter dies, and not across a call.
    careful: bool,
}

impl Tti for Counts {
    basic_prices!();

    fn is_hardware_loop_profitable(
        &self,
        info: &mut HardwareLoopTarget,
    ) -> bool {
        info.count_bits = self.bits;
        info.is_nesting_legal = self.nesting;
        info.counter_in_reg = self.phi;
        info.requires_dead_counter = self.careful;
        info.rejects_calls = self.careful;
        true
    }
}

/// A target that counts none.
struct Counts0;

impl Tti for Counts0 {
    basic_prices!();
}

/// `text`'s @f converted under `tti`, printed.
fn converted(
    text: &str,
    tti: &dyn Tti,
) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::converted(context, &layout, function, &mut declared, &outer, tti);
    declared.place(&mut module).expect("declarations");
    (changed, printed(&module))
}

/// A loop that runs `trips` times, its exit test on the incremented counter.
fn counted(trips: u32) -> String {
    format!(
        "define void @f(ptr %p) {{
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %q = getelementptr i8, ptr %p, i16 %i
  store i8 1, ptr %q
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, {trips}
  br i1 %c, label %loop, label %exit
exit:
  ret void
}}
"
    )
}

/// The header comment's example: the count is set before the loop and the
/// branch reads the decrement, so the counter phi and the compare are gone.
#[test]
fn test_a_counted_loop_sets_its_trips_and_branches_on_the_decrement() {
    let (changed, after) = converted(&counted(10), &Counts { bits: 8, nesting: false, phi: false, careful: false });
    assert!(changed, "{after}");
    let (before_loop, in_loop) = after.split_once("loop:").expect("a loop block");
    assert!(before_loop.contains("call void @llvm.set.loop.iterations.i8(i8 10)"), "{after}");
    assert!(in_loop.contains("= call i1 @llvm.loop.decrement.i8(i8 1)"), "{after}");
    assert!(!in_loop.contains("icmp"), "the old compare is deleted: {after}");
    assert!(after.contains("declare i1 @llvm.loop.decrement.i8(i8)"), "{after}");
}

/// A target that does not say yes leaves every loop as it was: this is what
/// keeps the pass's identity on all targets while none lowers the intrinsics.
#[test]
fn test_a_target_that_counts_no_loops_changes_nothing() {
    let (changed, after) = converted(&counted(10), &Counts0);
    assert!(!changed, "{after}");
    assert!(!after.contains("llvm.loop"), "{after}");
}

/// DEVIATE 1: an `int` loop of 54 trips has an `i16` exit count, and a
/// one-byte counter holds it. The pin's width test on the type would refuse.
#[test]
fn test_a_sixteen_bit_count_that_fits_a_byte_counter_is_taken() {
    let (changed, after) = converted(&counted(54), &Counts { bits: 8, nesting: false, phi: false, careful: false });
    assert!(changed, "{after}");
    assert!(after.contains("@llvm.set.loop.iterations.i8(i8 54)"), "{after}");
}

/// 256 trips are an exit count of 255 and a trip count of 256, which is zero
/// in a byte: the counter wraps round to run 256 times, as `djnz` with B = 0.
#[test]
fn test_two_hundred_fifty_six_trips_wrap_to_zero_in_a_byte() {
    let (changed, after) = converted(&counted(256), &Counts { bits: 8, nesting: false, phi: false, careful: false });
    assert!(changed, "{after}");
    assert!(after.contains("@llvm.set.loop.iterations.i8(i8 0)"), "{after}");
}

/// 300 trips do not fit a byte counter: the loop stays.
#[test]
fn test_a_count_wider_than_the_counter_leaves_the_loop() {
    let (changed, after) = converted(&counted(300), &Counts { bits: 8, nesting: false, phi: false, careful: false });
    assert!(!changed, "{after}");
    assert!(!after.contains("llvm.loop"), "{after}");
}

/// A count only known at run time is written before the loop.
#[test]
fn test_a_run_time_count_is_written_in_the_preheader() {
    let text = "define void @f(ptr %p, i8 %n) {
entry:
  %z = icmp eq i8 %n, 0
  br i1 %z, label %exit, label %pre
pre:
  br label %loop
loop:
  %i = phi i8 [ 0, %pre ], [ %i.next, %loop ]
  %q = getelementptr i8, ptr %p, i8 %i
  store i8 1, ptr %q
  %i.next = add nuw i8 %i, 1
  %c = icmp ne i8 %i.next, %n
  br i1 %c, label %loop, label %after
after:
  br label %exit
exit:
  ret void
}
";
    let (changed, after) = converted(text, &Counts { bits: 8, nesting: false, phi: false, careful: false });
    assert!(changed, "{after}");
    let (before_loop, in_loop) = after.split_once("loop:").expect("a loop block");
    assert!(before_loop.contains("@llvm.set.loop.iterations.i8(i8 %n)"), "{after}");
    assert!(in_loop.contains("@llvm.loop.decrement.i8"), "{after}");
}

/// A loop that exits on a loaded value has no count: it stays.
#[test]
fn test_a_loop_without_a_computable_count_stays() {
    let text = "define void @f(ptr %p) {
entry:
  br label %loop
loop:
  %q = phi ptr [ %p, %entry ], [ %q.next, %loop ]
  %v = load i8, ptr %q
  %q.next = getelementptr i8, ptr %q, i16 1
  %c = icmp ne i8 %v, 0
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
";
    let (changed, after) = converted(text, &Counts { bits: 16, nesting: false, phi: false, careful: false });
    assert!(!changed, "{after}");
}

const NEST: &str = "define void @f(ptr %p) {
entry:
  br label %outer
outer:
  %j = phi i16 [ 0, %entry ], [ %j.next, %latch ]
  br label %inner
inner:
  %i = phi i16 [ 0, %outer ], [ %i.next, %inner ]
  %q = getelementptr i8, ptr %p, i16 %i
  store i8 1, ptr %q
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 4
  br i1 %c, label %inner, label %latch
latch:
  %j.next = add nuw i16 %j, 1
  %d = icmp ne i16 %j.next, 6
  br i1 %d, label %outer, label %exit
exit:
  ret void
}
";

/// One counter to a nest: the inner loop converts, and the outer one is not
/// tried where the target does not allow a hardware loop inside another.
#[test]
fn test_only_the_inner_loop_of_a_nest_converts() {
    let (changed, after) = converted(NEST, &Counts { bits: 8, nesting: false, phi: false, careful: false });
    assert!(changed, "{after}");
    assert_eq!(after.matches("call void @llvm.set.loop.iterations").count(), 1, "{after}");
    assert!(after.contains("(i8 4)"), "the inner loop's trips: {after}");
    assert!(after.contains("icmp ne i16 %j.next, 6"), "the outer loop keeps its compare: {after}");
}

/// Where nesting is legal both loops are counted.
#[test]
fn test_both_loops_of_a_nest_convert_where_nesting_is_legal() {
    let (changed, after) = converted(NEST, &Counts { bits: 8, nesting: true, phi: false, careful: false });
    assert!(changed, "{after}");
    assert!(after.contains("(i8 4)") && after.contains("(i8 6)"), "{after}");
}

/// A loop that already holds a counter is not counted twice.
#[test]
fn test_a_converted_loop_is_not_converted_again() {
    let (_, once) = converted(&counted(10), &Counts { bits: 8, nesting: false, phi: false, careful: false });
    let mut module = parsed(&once);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = Counts { bits: 8, nesting: false, phi: false, careful: false };
    assert!(!super::converted(context, &layout, function, &mut declared, &outer, &tti), "{once}");
}

/// The phi form carries the count in a register: it starts at the trips, the
/// branch tests what is left, and the interpreter gets the same answers as
/// the loop it replaced.
#[test]
fn test_a_register_counter_is_a_phi_the_interpreter_agrees_with() {
    let text = "define i16 @f(i16 %x) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s.next, %loop ]
  %s.next = add i16 %s, %x
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 10
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %s.next
}
";
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = Counts { bits: 8, nesting: false, phi: true, careful: false };
    assert!(super::converted(context, &layout, function, &mut declared, &outer, &tti));
    declared.place(&mut module).expect("declarations");
    let after = printed(&module);
    let (before_loop, in_loop) = after.split_once("loop:").expect("a loop block");
    assert!(before_loop.contains("= call i8 @llvm.start.loop.iterations.i8(i8 10)"), "{after}");
    assert!(in_loop.contains("%loop.iterations = phi i8 [ %"), "{after}");
    assert!(in_loop.contains("= call i8 @llvm.loop.decrement.reg.i8(i8 %loop.iterations, i8 1)"), "{after}");
    assert!(in_loop.contains("icmp ne i8 %"), "{after}");
    assert_eq!(results(&module, &[&[3], &[7]]), results(&before, &[&[3], &[7]]), "{after}");
}

/// A target that wants its counter's register back converts only where the
/// loop's own counter dies. `counted` stores through `%i`, so the counter
/// lives on: the loop is left, instead of getting a second counter.
#[test]
fn test_a_loop_whose_counter_lives_on_is_left_where_the_target_wants_it_dead() {
    let (changed, after) = converted(&counted(10), &Counts { bits: 8, nesting: false, phi: true, careful: true });
    assert!(!changed, "{after}");
}

/// The counter is read by its update and the compare alone: it dies.
#[test]
fn test_a_loop_whose_counter_only_counts_is_converted_where_the_target_wants_it_dead() {
    let text = "define i16 @f(i16 %x) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s.next, %loop ]
  %s.next = add i16 %s, %x
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 10
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %s.next
}
";
    let (changed, after) = converted(text, &Counts { bits: 8, nesting: false, phi: true, careful: true });
    assert!(changed, "{after}");
}

/// A call in the loop takes the counter's register: left alone.
#[test]
fn test_a_loop_that_calls_is_left_where_the_target_loses_its_counter_to_calls() {
    let text = "declare void @g()
define void @f(i16 %x) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  call void @g()
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 10
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
";
    let (changed, after) = converted(text, &Counts { bits: 8, nesting: false, phi: true, careful: true });
    assert!(!changed, "{after}");
    let (changed, after) = converted(text, &Counts { bits: 8, nesting: false, phi: true, careful: false });
    assert!(changed, "{after}");
}

/// The old counter was a cycle of a phi and its update, dead once the compare
/// goes: it stayed in the loop, a 16-bit update beside the byte counter
/// (rep_count: +18 B, +6.6% clocks at -O2).
#[test]
fn test_the_counter_the_hardware_replaces_is_deleted() {
    let text = "define i16 @f(i16 %x) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s.next, %loop ]
  %s.next = add i16 %s, %x
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 10
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %s.next
}
";
    let (_, after) = converted(text, &Counts { bits: 8, nesting: false, phi: true, careful: true });
    assert!(!after.contains("%i.next") && !after.contains("%i = phi"), "{after}");
    assert!(after.contains("%s = phi"), "the sum stays: {after}");
}
