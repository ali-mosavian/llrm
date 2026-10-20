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

#[test]
fn a_counter_extended_past_the_index_width_stays() {
    // 16-bit addresses (a segment's offset): `zext` to 32 is no index.
    let text = clearing("e-p:16:16-n8:16:32")
        .replace("i32 %w", "i16 %w16")
        .replace("zext i16 %i to i32", "zext i16 %i to i32\n  %w16 = trunc i32 %w to i16");
    let (changed, after) = widening(&text);
    assert!(!changed, "{after}");
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
    let (changed, after) = widening(&text);
    assert!(!changed, "{after}");
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

/// indvars at its defaults asked the old ranges engine for every function with
/// a `sext` (54.8 M instructions, `ranges::bounded_solved`);
/// `simplifyAndExtend` reads ScalarEvolution's ranges, so nothing asks.
#[test]
fn test_indvars_with_scev_widening_does_not_solve_the_old_bounds() {
    let mut module = parsed(
        "define i32 @f(i32 %n) {
b0:
  br label %h

h:
  %i = phi i16 [ 0, %b0 ], [ %in, %l ]
  %s = phi i32 [ 0, %b0 ], [ %sn, %l ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %l, label %end

l:
  %w = sext i16 %i to i32
  %sn = add i32 %s, %w
  %in = add nsw i16 %i, 1
  br label %h

end:
  ret i32 %s
}
",
    );
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(super::IndVars { lftr: true });
    let before = llrm_analysis::ranges::loops_solved();
    manager.run_module(&mut module, Rc::new(crate::testing::Tuned::default())).unwrap();
    assert_eq!(llrm_analysis::ranges::loops_solved() - before, 0, "indvars solved the old bounds");
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
    let mut module = crate::testing::parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(super::IndVars { lftr: true });
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
    managed(&mut module, IndVars { lftr: true });
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
    managed(&mut module, IndVars { lftr: true });
    assert_eq!(header_phis(&mut module), 1, "{}", printed(&module));
    let other = parsed(&text);
    assert_eq!(results(&module, &[&[0], &[3], &[9]]), results(&other, &[&[0], &[3], &[9]]));
}

/// A counter that starts or steps otherwise is another counter.
#[test]
fn counters_that_start_or_step_otherwise_stay_two() {
    for (start, step) in [(1, 1), (0, 2)] {
        let mut module = parsed(&two_counters(start, step));
        managed(&mut module, IndVars { lftr: true });
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
        managed(module, IndVars { lftr: true });
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
    manager.add(IndVars { lftr: true });
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
    managed(&mut off, IndVars { lftr: false });
    let off = printed(&off);
    assert!(!off.contains("umax") && !off.contains("exitcond"), "{off}");
    let mut on = parsed(TOP_TESTED);
    managed(&mut on, IndVars { lftr: true });
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
    managed(&mut module, IndVars { lftr: false });
    let after = printed(&module);
    assert!(after.contains("br i1 false, label %b4, label %b3"), "{after}");
}

/// LFTR was on in the pipeline (the rotated copy ran it with the rotate slot
/// off): sieve.c -O1 m32 grew 32 B. It is opt-in until the translated LSR.
#[test]
fn test_lftr_is_off_in_the_pipeline_unless_asked() {
    assert_eq!(super::lftr_default(), std::env::var_os("LLRM_LFTR").is_some());
}
