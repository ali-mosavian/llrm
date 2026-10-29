//! `indvars` over MIR text the interpreter runs before and after. Choosing and
//! counting a loop's counters is `lsr`'s, tested there.

use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_analysis::testing::layout;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Module};
use llrm_mir::passes::{Analyses, Outer};

use super::IndVars;
use crate::profit::OperationCosts;
use crate::testing::{f, managed, parsed, printed, results};

/// @f of `text` through `change`: whether it changed, the module, and that
/// it computes what it did on `inputs`.
fn through(
    text: &str,
    inputs: &[&[i128]],
    change: impl FnOnce(&mut Context, &DataLayout, &mut Function, &Analyses) -> bool,
) -> (bool, Module) {
    let mut module = parsed(text);
    let before = (printed(&module), results(&module, inputs));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let analyses = Analyses::new(Rc::new(outer));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = change(context, &layout, function, &analyses);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before.1, "{after}");
    assert_eq!(changed, after != before.0, "{after}");
    (changed, module)
}

// ------------------------------------------------------------------ widened

/// `for (unsigned short i = 0; i < n; ++i) a[i] = 0` on a 32-bit address:
/// the counter's `zext` to the index width is a conversion a trip and its
/// counted-loop proof was 16 bits against the pointer's 32, so `loopidioms` and
/// `lsr` found no matching counter (m32 sieve, 19277 executed).
fn clearing(layout: &str) -> String {
    format!(
        "target datalayout = \"{layout}\"

define i32 @f(i16 %n) {{
b0:
  %buf = alloca [64 x i8]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp ult i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %w = zext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i32 0
}}
"
    )
}

fn widening(text: &str) -> (bool, String) {
    let (changed, module) = through(text, &[&[0], &[1], &[7], &[64]], |context, layout, function, analyses| {
        // As the pin's loop passes run: in LoopSimplify form (dedicated exits).
        let formed = crate::loopsimplify::simplified(function);
        formed
            | crate::simplifyindvar::simplified_and_extended(
                context,
                layout,
                function,
                &mut llrm_mir::passes::Declared::default(),
                analyses.outer(),
            )
    });
    (changed, printed(&module))
}

#[test]
fn a_counter_extended_to_the_index_width_is_that_wide() {
    let (changed, after) = widening(&clearing("e-p:32:32-n8:16:32"));
    assert!(changed, "{after}");
    assert!(after.contains("ult i32"), "{after}");
}

/// shellsort's gap loops once the gaps are unrolled: the inner counter steps
/// down by a gap from `i`, its test is `narrow < gap`, and it is read through
/// `zext`. The extension of the counter's recurrence is distributed when
/// `isKnownOnEveryIteration` holds; that was a stub that said no, so the 16-bit
/// counter stayed (`movzx; cmp dx,15` a trip in shellsort -O3 m32).
const STEPPING_DOWN: &str = "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 %n) {
b0:
  %buf = alloca [128 x i8]
  br label %outer

outer:
  %iv = phi i32 [ 16, %b0 ], [ %iv.next, %outer.latch ]
  %acc = phi i32 [ 0, %b0 ], [ %acc.out, %outer.latch ]
  %start = trunc nuw nsw i32 %iv to i16
  br label %inner

inner:
  %m = phi i16 [ %start, %outer ], [ %narrow, %inner ]
  %sum = phi i32 [ %acc, %outer ], [ %sum.next, %inner ]
  %narrow = sub nuw i16 %m, 16
  %idx = zext i16 %narrow to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %idx
  store i8 1, ptr %p
  %sum.next = add i32 %sum, %idx
  %done = icmp ult i16 %narrow, 16
  br i1 %done, label %outer.latch, label %inner

outer.latch:
  %acc.out = phi i32 [ %sum.next, %inner ]
  %iv.next = add nuw nsw i32 %iv, 1
  %ec = icmp ne i32 %iv.next, 64
  br i1 %ec, label %outer, label %exit

exit:
  ret i32 %acc.out
}
";

#[test]
fn a_counter_stepping_down_by_a_gap_and_read_through_zext_is_widened() {
    let (changed, after) = widening(STEPPING_DOWN);
    assert!(changed, "{after}");
    assert!(after.contains("icmp ult i32") && !after.contains("icmp ult i16"), "{after}");
}

#[test]
fn a_counter_extended_past_the_index_width_stays() {
    // 16-bit addresses (a segment's offset): `zext` to 32 is no index.
    let text = clearing("e-p:16:16-n8:16:32")
        .replace("i32 %w", "i16 %w16")
        .replace("zext i16 %i to i32", "zext i16 %i to i32\n  %w16 = trunc i32 %w to i16");
    let (_, after) = widening(&text);
    assert!(after.contains("icmp ult i16") && !after.contains("phi i32"), "{after}");
}

/// `clearing` for `short i`: signed, tested by `slt`, read through `sext`.
fn clearing_signed(layout: &str) -> String {
    clearing(layout).replace("icmp ult i16", "icmp slt i16").replace("zext i16 %i to i32", "sext i16 %i to i32")
}

/// d_faces: `short k; for (k = 0; k < n; k++) a[k]` is `inc cx; movsx esi,cx;
/// cmp cx,bx; jl` a trip (12,700 of the 18,142 `movsx` the file executes
/// against gcc's 5,459). The counter is below `n` when it is stepped, so
/// the step never passes the width's largest value, and its sign extension is
/// the counter of the index's width (LLVM's `WidenIV` with `sext` and `nsw`;
/// gcc's ivopts on the loop bound).
#[test]
fn a_signed_counter_extended_to_the_index_width_is_that_wide() {
    let (changed, after) = widening(&clearing_signed("e-p:32:32-n8:16:32"));
    assert!(changed, "{after}");
    assert!(after.contains("slt i32"), "{after}");
    assert!(after.contains("sext i16 %n to i32"), "{after}");
}

/// A start below zero is a negative number of the wide width, not the narrow
/// pattern.
#[test]
fn a_signed_counter_from_a_negative_start_is_widened_from_the_negative_number() {
    // The index is the counter plus 8, so that the interpreter's buffer holds
    // it.
    let text = clearing_signed("e-p:32:32-n8:16:32")
        .replace("[ 0, %b0 ]", "[ -5, %b0 ]")
        .replace(
            "%p = getelementptr inbounds i8, ptr %buf, i32 %w",
            "%v = add i32 %w, 8\n  %p = getelementptr inbounds i8, ptr %buf, i32 %v",
        );
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("phi i32") && after.contains("[ -5, %b0 ]") && !after.contains("sext i16 -5"), "{after}");
}

/// An unsigned counter read through `sext` is not a signed one: its sign
/// extension differs past 32767.
#[test]
fn an_unsigned_counter_read_through_sext_is_not_widened() {
    let text = clearing("e-p:32:32-n8:16:32").replace("zext i16 %i to i32", "sext i16 %i to i32");
    // The pin stamps `nuw` on the step (its context inference) and widens
    // nothing.
    let (_, after) = widening(&text);
    assert!(after.contains("icmp ult i16") && !after.contains("phi i32"), "{after}");
}

/// bench/nbody: `for (unsigned short j = i + 1; j < 4; ++j) x[j]`: a start the
/// loop does not define. The narrow counter was `inc bx; movzx esi,bx; cmp
/// bx,4` each trip (38 instructions an iteration against gcc's 27); its zero
/// extension is the counter of the index's width from the extended start.
#[test]
fn a_counter_from_a_start_the_loop_does_not_define_is_widened() {
    let text = clearing("e-p:32:32-n8:16:32")
        .replace("define i32 @f(i16 %n) {\nb0:", "define i32 @f(i16 %n) {\nb0:\n  %s = add i16 %n, 1")
        .replace("[ 0, %b0 ]", "[ %s, %b0 ]")
        .replace("icmp ult i16 %i, %n", "icmp ult i16 %i, 4");
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("ult i32") && after.contains("zext i16 %s to i32"), "{after}");
}

/// The same loop entered behind a copy of its test (`-ftree-ch`): the counter
/// is tested stepped, after its trip, and the phi lists its latch first. It
/// stayed 16 bits (a `movzx` and a 5-instruction trip where the pre-tested loop
/// had a pointer and a negative count: m32 sieve -O1 +11.7% clocks with the
/// header copy).
#[test]
fn a_counter_tested_after_its_trip_behind_a_copy_of_its_test_is_widened() {
    let text = "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 %n) {
b0:
  %buf = alloca [64 x i8]
  %some = icmp ne i16 %n, 0
  br i1 %some, label %pre, label %b3

pre:
  br label %b1

b1:
  %i = phi i16 [ %next, %b1 ], [ 0, %pre ]
  %w = zext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add i16 %i, 1
  %go = icmp ult i16 %next, %n
  br i1 %go, label %b1, label %b3

b3:
  ret i32 0
}
";
    let (changed, after) = widening(text);
    assert!(changed, "{after}");
    assert!(after.contains("ult i32"), "{after}");
}

fn with_exit_values(text: &str) -> String {
    with_exit_values_sized(text, false)
}

fn with_exit_values_sized(
    text: &str,
    size: bool,
) -> String {
    let mut module = crate::testing::parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(super::IndVars { lftr: true, size });
    manager.run_module(&mut module, Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    crate::testing::printed(&module)
}

/// The counter's step leaves a loop that also breaks, on `i + 1 < 32` false.
/// The old step read the exit's own count and wrote 32 (mandel: `lea; or; je;
/// mov` where `inc; je` does). `rewriteLoopExitValues` asks `getSCEVAtExit`,
/// which takes the value at the loop's one count, `umin(31, n + 1)`, once
/// `simplifyAndExtend` has given the step `nuw nsw`: invariant, so not
/// evaluated at the exit's count (the per-exit retry is for a variant one), and
/// too dear to write under the budget. The exit keeps reading the counter.
#[test]
fn test_an_exit_phi_of_a_stepped_counter_in_a_loop_with_a_second_exit_keeps_its_value() {
    let text = with_exit_values(
        "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp sgt i32 %i, %n
  br i1 %c, label %b3, label %b2

b2:
  %next = add i32 %i, 1
  %again = icmp slt i32 %next, 32
  br i1 %again, label %b1, label %b4

b3:
  ret i32 -1

b4:
  %last = phi i32 [ %next, %b2 ]
  ret i32 %last
}
",
    );
    assert!(text.contains("phi i32 [ %next, %b2 ]"), "{text}");
}

/// lcexit.bas (`FOR`/`EXIT` over an `INTEGER` counter read as `LONG`): the
/// counter is widened, and the loop's exit count, worked out before that, is
/// still the one in `i16`. LLVM tells ScalarEvolution nothing of the compares
/// the widening rewrote in place, so LFTR tests the wide counter against
/// `zext` of a limit built in `i16`. The port forgot the count and built the
/// limit in `i32`: two copies of `umin(zext n - 1, 9) + 1`.
#[test]
fn test_a_widened_counters_limit_is_built_in_the_narrow_type_and_extended() {
    let text = with_exit_values(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 %n) {
b0:
  %none = icmp eq i16 %n, 0
  br i1 %none, label %b4, label %b2

b2:
  br label %b7

b7:
  %i = phi i16 [ 0, %b2 ], [ %next, %b7 ]
  %acc = phi i32 [ 0, %b2 ], [ %sum, %b7 ]
  %w = zext nneg i16 %i to i32
  %sum = add i32 %acc, %w
  %next = add nuw nsw i16 %i, 1
  %big = icmp ugt i16 %i, 8
  %hit = icmp eq i16 %next, %n
  %stop = select i1 %big, i1 true, i1 %hit
  br i1 %stop, label %x, label %b7

x:
  %sum.out = phi i32 [ %sum, %b7 ]
  %i.out = phi i16 [ %next, %b7 ]
  br label %b4

b4:
  %total = phi i32 [ 0, %b0 ], [ %sum.out, %x ]
  %last = phi i16 [ 0, %b0 ], [ %i.out, %x ]
  %wide = zext i16 %last to i32
  %r = add i32 %total, %wide
  ret i32 %r
}
",
    );
    assert!(text.contains("%wide.trip.count = zext i16"), "{text}");
    assert!(text.contains("llvm.umin.i16") && !text.contains("llvm.umin.i32"), "{text}");
}

/// An exit whose count is not a number is written from the count, as
/// `rewriteLoopExitValues` writes it: the loop runs `smax(n, 1)` times, two
/// instructions, under the budget.
#[test]
fn test_an_exit_phi_of_a_loop_with_an_unknown_count_is_written_from_the_count() {
    let text = with_exit_values(
        "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  br label %b2

b2:
  %next = add i32 %i, 1
  %again = icmp slt i32 %next, %n
  br i1 %again, label %b1, label %b4

b4:
  %last = phi i32 [ %next, %b2 ]
  ret i32 %last
}
",
    );
    assert!(text.contains("llvm.smax.i32(i32 %n, i32 1)") && text.contains("ret i32 %0"), "{text}");
}

/// bench/sieve: `if (i <= 31)` on the narrow counter of a widened loop read
/// `trunc i32 %widen.iv to i16` and compared that, which `split` could not see
/// as a test of the counter: the compare is of the wide counter with the number
/// extended, as the counter never passes the narrow width.
#[test]
fn an_unsigned_compare_of_a_widened_counter_reads_the_wide_one() {
    let text = clearing("e-p:32:32-n8:16:32").replace(
        "  store i8 1, ptr %p",
        "  %low = icmp ule i16 %i, 31\n  %m = zext i1 %low to i8\n  store i8 %m, ptr %p",
    );
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("ule i32") && after.contains(", 31"), "{after}");
    assert!(!after.contains("trunc"), "{after}");
}

/// A signed compare reads the narrow value's sign, which the wide one does not
/// have, so it stays a compare of the low part.
#[test]
fn a_signed_compare_of_a_widened_counter_reads_its_low_part() {
    let text = clearing("e-p:32:32-n8:16:32").replace(
        "  store i8 1, ptr %p",
        "  %low = icmp slt i16 %i, 31\n  %m = zext i1 %low to i8\n  store i8 %m, ptr %p",
    );
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("trunc nuw i32"), "{after}");
}

// ---------------------------------------------------------------- congruent

/// `a[i] = i` and `b[j] = j` in one loop, `j` starting at `start` and
/// stepping by `step` where `i` starts at 0 and steps by 1.
fn two_counters(
    start: i16,
    step: i16,
) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %j = phi i16 [ {start}, %b0 ], [ %j.next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %io = mul i16 %i, 2
  %ia = getelementptr i8, ptr %a, i16 %io
  store i16 %i, ptr %ia
  %jo = mul i16 %j, 2
  %jb = getelementptr i8, ptr %b, i16 %jo
  store i16 %j, ptr %jb
  %i.next = add i16 %i, 1
  %j.next = add i16 %j, {step}
  br label %b1

b3:
  %x = load i16, ptr %a
  %y = load i16, ptr %b
  %z = add i16 %x, %y
  ret i16 %z
}}
"
    )
}

fn header_phis(module: &mut Module) -> usize {
    let function = f(module);
    let [one] = &loops::loops(&cfg::graph(function), function.entry().map(cfg::id))[..] else { panic!("one loop") };
    function
        .iter_block(cfg::block(one.header))
        .filter(|&inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi)
        .count()
}

/// Two counters that start and step alike are one: the loop kept both and
/// addressed `b` by a counter that always equalled `i`.
#[test]
fn counters_with_one_start_and_one_step_are_one_counter() {
    let mut module = parsed(&two_counters(0, 1));
    managed(&mut module, IndVars { lftr: true, size: false });
    assert_eq!(header_phis(&mut module), 1, "{}", printed(&module));
    let mut other = parsed(&two_counters(0, 1));
    assert_eq!(results(&module, &[&[0], &[3], &[9]]), results(&other, &[&[0], &[3], &[9]]));
    assert_eq!(header_phis(&mut other), 2);
}

/// LoopRotate leaves a counter twice: `j` steps from `i`'s value by an `add`
/// that repeats `i`'s own. The loop kept both, and in bench's tile the inner
/// loop took two registers and an `xchg` (+50% clocks at -O1).
#[test]
fn counters_whose_steps_are_the_same_add_are_one_counter() {
    let text = two_counters(0, 1).replace("%j.next = add i16 %j, 1", "%j.next = add i16 %i, 1");
    let mut module = parsed(&text);
    managed(&mut module, IndVars { lftr: true, size: false });
    assert_eq!(header_phis(&mut module), 1, "{}", printed(&module));
    let other = parsed(&text);
    assert_eq!(results(&module, &[&[0], &[3], &[9]]), results(&other, &[&[0], &[3], &[9]]));
}

/// A counter that starts or steps otherwise is another counter.
#[test]
fn counters_that_start_or_step_otherwise_stay_two() {
    for (start, step) in [(1, 1), (0, 2)] {
        let mut module = parsed(&two_counters(start, step));
        managed(&mut module, IndVars { lftr: true, size: false });
        assert_eq!(header_phis(&mut module), 2, "{start} {step}: {}", printed(&module));
    }
}

/// A loop `indvars` has nothing to do for.
const IDLE_LOOP: &str = "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %s = phi i16 [ 0, %b0 ], [ %t, %b1 ]
  %t = add i16 %s, %i
  %next = add i16 %i, 1
  %c = icmp slt i16 %next, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %t
}
";

/// Asks the manager for what `indvars` starts from, and nothing else.
struct AsksWhatIndVarsDoes;

impl llrm_mir::passes::FunctionPass for AsksWhatIndVarsDoes {
    fn name(&self) -> &'static str {
        "asks-what-indvars-does"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
        analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// Each step of a run found the loops of the body for itself (`Shape::of`: the
/// dominators and the loop forest again): the exit values, the exits, the
/// widening, the counters merged: 25 of 119 Minstr of `indvars` at d_faces
/// -O2. A run that changes no block finds them no more often than the manager
/// does for the facts the run starts from.
#[test]
fn a_run_that_changes_no_block_finds_the_loops_once() {
    let derived = |pass: Box<dyn FnOnce(&mut Module)>| {
        let mut module = parsed(IDLE_LOOP);
        let before = cfg::shapes_derived();
        pass(&mut module);
        cfg::shapes_derived() - before
    };
    let manager = derived(Box::new(|module| {
        managed(module, AsksWhatIndVarsDoes);
    }));
    let indvars = derived(Box::new(|module| {
        managed(module, IndVars { lftr: true, size: false });
    }));
    assert!(indvars <= manager, "{indvars} shapes against the manager's {manager}");
}

/// A rotated loop leaves from its latch. The step that took the exit values
/// before asked only the header for its exit (`_exit`), so it left a rotated
/// loop's sum computed by the loop; `rewriteLoopExitValues` asks every exit.
#[test]
fn test_a_rotated_loops_exit_value_is_rewritten_from_its_latch() {
    let text = with_exit_values(
        "define i16 @f(i16 %n) {
entry:
  %any = icmp ne i16 %n, 0
  br i1 %any, label %pre, label %done
pre:
  br label %loop
loop:
  %i = phi i16 [ 0, %pre ], [ %inc, %loop ]
  %s = phi i16 [ 0, %pre ], [ %s2, %loop ]
  %s2 = add i16 %s, 2
  %inc = add i16 %i, 1
  %c = icmp ne i16 %inc, %n
  br i1 %c, label %loop, label %exit
exit:
  %out = phi i16 [ %s2, %loop ]
  br label %done
done:
  %r = phi i16 [ 0, %entry ], [ %out, %exit ]
  ret i16 %r
}
",
    );
    assert!(text.contains("shl i16 %n, 1") || text.contains("mul i16 %n, 2"), "{text}");
    assert!(!text.contains("%s2 = add"), "the loop's sum is dead: {text}");
}

/// The flat 32-bit target as the driver builds it: widening past 16 bits is
/// priced by its X86 cost model.
pub(crate) fn m32() -> Rc<dyn llrm_mir::target::Machine> {
    use llrm_target::Target;
    let prices = llrm_target::CpuPrices {
        costs: Vec::new(),
        prefix: 1,
        address_stall: 0,
        registers: 6,
        call_registers: 3,
        float_registers: 0,
        address_forms: Vec::new(),
        operations: OperationCosts::default(),
        spaces: llrm_x86_m32::layout().spaces.roles,
        private: None,
        calling: None,
        foreign: llrm_target::Foreign::none(),
        ports: llrm_x86_m32::M32.flat_ports(),
    };
    (llrm_x86_m32::M32.cost_model())(&prices)
}

/// A value a loop defines that a phi outside it reads, other than a phi in an
/// exit block: the loop is not in closed SSA form. (A read by another
/// instruction is left to the pass that closes loops again.)
fn leaks_a_loop_value(module: &Module) -> Option<String> {
    let (_, _, function) = module.functions().next()?;
    let tree = llrm_mir::domtree::DomTree::of(function);
    let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
    let (values, blocks) = (llrm_mir::print::value_labels(function), llrm_mir::print::block_labels(function));
    for l in loops.get_loops_in_preorder() {
        let exits = l.get_exit_blocks(function);
        for &block in l.get_blocks() {
            for inst in function.iter_block(block) {
                let Some(value) = function.instruction(inst).result else { continue };
                for one in function.users(value) {
                    let home = function.parent(one.user).expect("placed");
                    let phi = function.instruction(one.user).opcode == llrm_mir::opcode::Opcode::Phi;
                    if phi && !l.contains(home) && !exits.contains(&home) {
                        return Some(format!("%{} read in %{}", values[&value], blocks[&home]));
                    }
                }
            }
        }
    }
    None
}

/// Planets (-m32, #2685): the second loop's counter starts at the first
/// loop's widened counter. Expanding that start read the first loop's value
/// from outside the loop, with no exit phi, and the unroll after refused the
/// body as not in closed SSA form.
#[test]
fn test_a_start_expanded_from_a_loop_before_it_is_read_through_that_loops_exit_phi() {
    let mut module = parsed(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i16 @f(ptr %buf, i16 %s, i16 %k, i16 %n) {
b0:
  br label %a

a:
  %i = phi i16 [ 0, %b0 ], [ %in, %a2 ]
  %x = phi i16 [ %s, %b0 ], [ %xn, %a2 ]
  %c = icmp ult i16 %i, %k
  br i1 %c, label %a2, label %m

a2:
  %in = add nuw nsw i16 %i, 1
  %xn = add nuw nsw i16 %x, 1
  br label %a

m:
  %xl = phi i16 [ %x, %a ]
  br label %b

b:
  %j = phi i16 [ 0, %m ], [ %jn, %b2 ]
  %y = phi i16 [ %xl, %m ], [ %yn, %b2 ]
  %d = icmp slt i16 %j, %n
  br i1 %d, label %b2, label %e

b2:
  %w = zext i16 %y to i32
  %p = getelementptr i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %jn = add nuw nsw i16 %j, 1
  %yn = add nuw nsw i16 %y, 1
  br label %b

e:
  %yl = phi i16 [ %y, %b ]
  ret i16 %yl
}
",
    );
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IndVars { lftr: true, size: false });
    manager.run_module(&mut module, m32()).unwrap();
    let after = printed(&module);
    assert_eq!(leaks_a_loop_value(&module), None, "{after}");
}

/// `for (i = 2; i < limit; i++)` tested at the loop's top (not rotated).
const TOP_TESTED: &str = "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 %limit) {
b0:
  %buf = alloca [64 x i8]
  br label %h

h:
  %i = phi i16 [ 2, %b0 ], [ %inc, %b1 ]
  %c = icmp ult i16 %i, %limit
  br i1 %c, label %b1, label %b2

b1:
  %z = zext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %z
  store i8 1, ptr %p
  %inc = add i16 %i, 1
  br label %h

b2:
  ret i32 0
}
";

/// A loop tested at its top got `icmp ne %i, umax(limit, 2)` and a `umax` call
/// before it (+16 B at sieve.c -O1): LFTR is for rotated loops (D22), where
/// loop-rotate's guard makes the limit plain. `lftr: false` leaves the test.
#[test]
fn test_lftr_off_leaves_the_test_of_a_loop_not_rotated() {
    let mut off = parsed(TOP_TESTED);
    managed(&mut off, IndVars { lftr: false, size: false });
    let off = printed(&off);
    assert!(!off.contains("umax") && !off.contains("exitcond"), "{off}");
    let mut on = parsed(TOP_TESTED);
    managed(&mut on, IndVars { lftr: true, size: false });
    let on = printed(&on);
    assert!(on.contains("exitcond"), "{on}");
}

/// bintree-O2 / lru-O2 `pipeline.body`: a loop that only counts and writes
/// nothing. The pin's `predicateLoopExits` makes its test `br i1 false`: it
/// leaves on its first trip. llrm kept the counter and the compare.
#[test]
fn test_a_read_only_counting_loop_leaves_on_its_first_trip() {
    let mut module = parsed(
        "target datalayout = \"e-p:32:32-n8:16:32\"

@a = global [3 x i16] zeroinitializer

define void @f() {
b1:
  br label %b4

b4:
  %i = phi i32 [ %n, %b4 ], [ 0, %b1 ]
  %j = phi i16 [ %k, %b4 ], [ 0, %b1 ]
  %p = getelementptr inbounds i16, ptr @a, i32 %i
  %k = add i16 %j, 1
  %n = sext i16 %k to i32
  %c = icmp slt i16 %k, 3
  br i1 %c, label %b4, label %b3

b3:
  ret void
}
",
    );
    managed(&mut module, IndVars { lftr: false, size: false });
    let after = printed(&module);
    assert!(after.contains("br i1 false, label %b4, label %b3"), "{after}");
}

/// LFTR was off in the pipeline until the translated LSR: bench ring's exit
/// test stayed `ult`, LSR made no compare against zero, and its fill loop ran 6
/// instructions where the pin's runs 4 (48154 against 40103 instructions).
#[test]
fn test_lftr_is_on_in_the_pipeline() {
    assert!(super::lftr_default());
}

/// `loops` counted loops one after another, each in LoopSimplify form.
fn counted_loops(loops: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %n) {\nb0:\n  br label %p0\n\n");
    for at in 0..loops {
        let next = if at + 1 == loops { "done".to_owned() } else { format!("p{}", at + 1) };
        text += &format!(
            "p{at}:\n  br label %h{at}\n\nh{at}:\n  %i{at} = phi i32 [ 0, %p{at} ], [ %n{at}, %l{at} ]\n  %g{at} = icmp ult i32 %i{at}, %n\n  br i1 %g{at}, label %l{at}, label %x{at}\n\nl{at}:\n  %n{at} = add nuw nsw i32 %i{at}, 1\n  br label %h{at}\n\nx{at}:\n  br label %{next}\n\n"
        );
    }
    text + "done:\n  ret i32 0\n}\n"
}

/// The trip counts `indvars` works out for `loops` loops that none of its
/// steps changes.
fn computed_counts(loops: usize) -> usize {
    let mut module = parsed(&counted_loops(loops));
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    managed(&mut module, IndVars { lftr: true, size: false });
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// Each step of `indvars` made a `ScalarEvolution` of its own and worked out
/// every loop's trip count again: 6 steps over d_faces -O2 cost 49 M
/// instructions, 21 M of them trip counts. One run holds one, as LLVM's pass
/// does: a loop that nothing changes is counted once, and the work grows with
/// the loops, not with loops times steps.
#[test]
fn a_loop_nothing_changes_has_its_trip_count_worked_out_once_in_a_run() {
    assert_eq!(computed_counts(4), 4);
    // Four times the loops, four times the counts (the 1x/4x slope).
    assert_eq!(computed_counts(16), 16);
}

/// The scans for a loop's simplify form made while `indvars` runs over `loops`
/// loops that none of its steps changes.
fn simplify_form_scans(loops: usize) -> usize {
    let mut module = parsed(&counted_loops(loops));
    let before = llrm_mir::loopinfo::simplify_form_scans();
    managed(&mut module, IndVars { lftr: true, size: false });
    llrm_mir::loopinfo::simplify_form_scans() - before
}

/// Each of the seven steps looked at each loop's preheader, latch and exits
/// again: the walk of the loop's blocks for the exits was 426 K of d_faces
/// -O2's 10.3 M for `indvars`, 126 walks for 18 loops where the pin's 54 cost
/// 69 K. The form is worked out once for a loop now, whatever the steps.
#[test]
fn a_loops_simplify_form_is_worked_out_once_in_a_run() {
    assert_eq!(simplify_form_scans(4), 4);
    assert_eq!(simplify_form_scans(16), 16);
}

/// Two loops, each with the same counter twice, one after the other.
fn two_loops_of_two_counters() -> String {
    "define i16 @f(i16 %n) {
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %j = phi i16 [ 0, %b0 ], [ %j.next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %c0

b2:
  %io = mul i16 %i, 2
  %ia = getelementptr i8, ptr %a, i16 %io
  store i16 %i, ptr %ia
  %jo = mul i16 %j, 2
  %jb = getelementptr i8, ptr %b, i16 %jo
  store i16 %j, ptr %jb
  %i.next = add i16 %i, 1
  %j.next = add i16 %j, 1
  br label %b1

c0:
  br label %c1

c1:
  %k = phi i16 [ 0, %c0 ], [ %k.next, %c2 ]
  %l = phi i16 [ 0, %c0 ], [ %l.next, %c2 ]
  %cgo = icmp slt i16 %k, %n
  br i1 %cgo, label %c2, label %c3

c2:
  %ko = mul i16 %k, 2
  %ka = getelementptr i8, ptr %a, i16 %ko
  store i16 %k, ptr %ka
  %lo = mul i16 %l, 2
  %lb = getelementptr i8, ptr %b, i16 %lo
  store i16 %l, ptr %lb
  %k.next = add i16 %k, 1
  %l.next = add i16 %l, 1
  br label %c1

c3:
  %x = load i16, ptr %a
  %y = load i16, ptr %b
  %z = add i16 %x, %y
  ret i16 %z
}
"
    .to_owned()
}

/// Phis in the header of each of the two loops, in program order.
fn both_header_phis(module: &mut Module) -> Vec<usize> {
    let function = f(module);
    let mut all = loops::loops(&cfg::graph(function), function.entry().map(cfg::id));
    all.sort_by_key(|one| one.header);
    all.iter()
        .map(|one| {
            function
                .iter_block(cfg::block(one.header))
                .filter(|&inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi)
                .count()
        })
        .collect()
}

/// The loop pass manager hands the loops a full unroll made to the passes after
/// it: `IndVarsOnCreated` runs on those loops and no others, and on none when
/// the unroll made none (the whole-function slot it replaces would run on
/// both).
#[test]
fn indvars_on_the_created_loops_runs_on_those_loops_alone() {
    let mut module = parsed(&two_loops_of_two_counters());
    let second = {
        let function = f(&mut module);
        let mut all = loops::loops(&cfg::graph(function), function.entry().map(cfg::id));
        all.sort_by_key(|one| one.header);
        cfg::block(all[1].header)
    };
    let created = Rc::new(std::cell::RefCell::new(Vec::new()));
    managed(&mut module, super::IndVarsOnCreated { created: Rc::clone(&created) });
    assert_eq!(both_header_phis(&mut module), vec![2, 2], "nothing made: nothing runs");
    *created.borrow_mut() = vec![second];
    managed(&mut module, super::IndVarsOnCreated { created: Rc::clone(&created) });
    assert_eq!(both_header_phis(&mut module), vec![2, 1], "{}", printed(&module));
    assert!(created.borrow().is_empty(), "the run took the list");
}

/// `i < -5` inside `for i = 0 to 9`: the counted loop's range says it never
/// holds. `decide` answered this from ScalarEvolution; the pin's IndVars does,
/// in `eliminateIVComparison`, on every loop including the ones an unroll made.
#[test]
fn a_compare_the_counted_loops_range_rules_out_is_folded_by_indvars() {
    let text = "define i16 @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %s = phi i16 [ 0, %b0 ], [ %t, %b4 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b2, label %b5

b2:
  %neg = icmp slt i16 %i, -5
  br i1 %neg, label %b3, label %b4

b3:
  %u = add i16 %s, 100
  br label %b4

b4:
  %t = phi i16 [ %s, %b2 ], [ %u, %b3 ]
  %next = add i16 %i, 1
  br label %b1

b5:
  ret i16 %s
}
";
    let mut module = parsed(text);
    let before = results(&module, &[&[]]);
    managed(&mut module, IndVars { lftr: true, size: false });
    assert!(!printed(&module).contains("icmp slt i16 %i, -5"), "{}", printed(&module));
    assert_eq!(results(&module, &[&[]]), before);
}

/// After its edits `indvars` swept the whole function for dead code: 0.8 M Ir
/// on d_faces' largest body, where the pin deletes what an edit stopped
/// reading. The edits' own seeds are enough.
#[test]
fn a_run_that_edits_a_loop_does_not_sweep_the_function_for_dead_code() {
    let mut module = parsed(&two_counters(0, 1));
    crate::dead::SCANNED.with(|scanned| scanned.set(0));
    managed(&mut module, IndVars { lftr: true, size: false });
    assert_eq!(header_phis(&mut module), 1, "{}", printed(&module));
    assert_eq!(crate::dead::SCANNED.with(std::cell::Cell::get), 0);
}

/// pressx at m16 -Os (+15 B in the early-GlobalOpt sweep): ten additions of a
/// loop invariant stayed a loop, because the formula's price (an add and a
/// multiply, 5) was set against the loop's four bodies alone (DEVIATE 15). The
/// loop is its two counters and its branch too; the formula is smaller.
#[test]
fn test_under_size_a_ten_trip_sum_of_an_invariant_is_its_product() {
    let text = with_exit_values_sized(
        "declare void @sink(i16)
define i16 @f(i16 %a, i16 %b, i16 %c, i16 %d) {
entry:
  %m = mul i16 %a, %b
  %k = mul i16 %c, %d
  %p = add i16 %m, %k
  br label %loop
loop:
  %i = phi i16 [ 1, %entry ], [ %inc, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s2, %loop ]
  %s1 = add i16 %s, %p
  %s2 = add i16 %s1, %k
  %inc = add nuw nsw i16 %i, 1
  %c2 = icmp ne i16 %inc, 11
  br i1 %c2, label %loop, label %exit
exit:
  %out = phi i16 [ %s2, %loop ]
  call void @sink(i16 %out)
  ret i16 %out
}
",
        true,
    );
    assert!(!text.contains("%s2 = add"), "the loop is gone: {text}");
}

/// A counted loop with an add only the exit reads, which `indvars` sinks into
/// the exit, then `loopdeletion`: the trip counts worked out.
fn counts_over_a_sink_and_deletion() -> usize {
    let mut module = parsed(
        "define i32 @f(i32 %b, i32 %c, ptr %d) {
entry:
  %m = and i32 %b, 3
  %g = icmp ult i32 0, %m
  br i1 %g, label %ph, label %done

ph:
  br label %body

body:
  %i = phi i32 [ 0, %ph ], [ %n, %body ]
  %v = load volatile i32, ptr %d
  %t = add i32 %v, 7
  %n = add nuw nsw i32 %i, 1
  %e = icmp samesign ult i32 %n, %m
  br i1 %e, label %body, label %exit

exit:
  %split = phi i32 [ %t, %body ]
  br label %done

done:
  %r = phi i32 [ %split, %exit ], [ %c, %entry ]
  ret i32 %r
}
",
    );
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IndVars { lftr: true, size: false });
    manager.add(crate::loopdeletion::LoopDeletion);
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// The edits `indvars` makes after its last step (the sink here, LFTR) made it
/// forget every loop they touched, and `loopdeletion` after it counted all 128
/// loops of branches1024 again (0.15 G). The pin forgets a loop only after
/// `optimizeLoopExits` and `predicateLoopExits`.
#[test]
fn the_edits_after_the_last_step_leave_the_trip_counts_standing() {
    assert_eq!(counts_over_a_sink_and_deletion(), 1);
}

/// memcpy's `while (n-- > 0)` after the guard: `ugt %i, 1` on a counter that
/// starts at `n >= 1`.
const DOWN_COUNTER: &str = "define i32 @f(i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i32 [ %n, %entry ], [ %dec, %loop ]
  %dec = add i32 %i, -1
  %c = icmp ugt i32 %i, 1
  br i1 %c, label %loop, label %exit
exit:
  ret i32 0
}
";

/// `exitne` turned the compare to `!=` on x86 and moved nine `bq32` rows the
/// wrong way (jpeg +75 insns). A target that does not say it prefers the
/// rewrite keeps the pin's IR.
#[test]
fn test_a_target_that_does_not_prefer_exit_ne_keeps_the_strict_compare() {
    let mut module = parsed(DOWN_COUNTER);
    let text = crate::testing::managed_on(&mut module, IndVars { lftr: false, size: false }, Default::default());
    assert!(text.contains("icmp ugt i32 %i, 1"), "{text}");
}

#[test]
fn test_a_target_that_prefers_exit_ne_gets_the_not_equal_compare() {
    let mut module = parsed(DOWN_COUNTER);
    let machine = crate::testing::Tuned { exit_ne: true, ..Default::default() };
    let text = crate::testing::managed_on(&mut module, IndVars { lftr: false, size: false }, machine);
    assert!(text.contains("icmp ne i32 %i, 1"), "{text}");
}
