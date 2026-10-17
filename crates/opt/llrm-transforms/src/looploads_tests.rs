//! A rotated loop that reads `a[j]` and `a[j + 1]` read the second again as
//! the first: bubble sort's inner loop (`x_bubble`) loaded both on every trip,
//! 3.04 memory operands a trip where clang's carried one, 1.47 times its
//! program's memory operands.

use llrm_analysis::manager::Summaries;
use llrm_analysis::testing::DOS;
use llrm_mir::module::Module;
use llrm_mir::passes::PassManager;

use super::*;
use crate::testing::{parsed, printed, results};

fn carried(mut module: Module) -> Module {
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(LoopLoads);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("verifies");
    module
}

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}@a = global [64 x i16] zeroinitializer\ndeclare void @anything()\n\n{body}"))
}

fn loads(text: &str) -> usize {
    text.matches("= load ").count()
}

/// Bubble sort's pass: a swap stores the second element's old first.
const BUBBLE: &str = "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %j = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %p = getelementptr i16, ptr @a, i16 %j
  %x = load i16, ptr %p
  %next = add i16 %j, 1
  %q = getelementptr i16, ptr @a, i16 %next
  %y = load i16, ptr %q
  %c = icmp sgt i16 %x, %y
  br i1 %c, label %b2, label %b3

b2:
  store i16 %y, ptr %p
  store i16 %x, ptr %q
  br label %b3

b3:
  %more = icmp slt i16 %next, %n
  br i1 %more, label %b1, label %b4

b4:
  %last = load i16, ptr @a
  ret i16 %last
}
";

/// The loop loaded twice a trip; now once, the first from the preheader and
/// the second element carried (a phi of what the swap stored and what was
/// read), and the answers are the same.
#[test]
fn test_bubble_loop_reads_each_element_once() {
    let before = module(BUBBLE);
    let after = carried(before.clone());
    let text = printed(&after);
    assert_eq!(loads(&printed(&before)), 3, "{text}");
    assert_eq!(loads(&text), 3, "one load left in the loop and one in the preheader: {text}");
    let in_loop = text.split("b1:").nth(1).and_then(|rest| rest.split("b4:").next()).expect("loop");
    assert_eq!(loads(in_loop), 1, "{text}");
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[40]];
    assert_eq!(results(&after, inputs), results(&before, inputs), "{text}");
}

/// A call that may write the array between the loads and the latch leaves
/// nothing to carry.
#[test]
fn test_call_in_the_loop_keeps_both_loads() {
    let body = BUBBLE.replace("  %more =", "  call void @anything()\n  %more =");
    assert_eq!(printed(&carried(module(&body))), printed(&module(&body)));
}
