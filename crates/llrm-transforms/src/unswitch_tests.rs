//! Adapted from llrm-core's `optimize/unswitch_tests.rs`, the port of
//! `tests/test_unswitch.py`; which tests stay behind is in `unswitch.rs`.

use llrm_graph::loops;
use llrm_analysis::cfg;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_mir::passes::Unit;

use super::{optimized, specialized};
use crate::profit::OperationCosts;
use crate::testing::{f, parsed, printed, results};

/// A counted sum whose body adds only when the invariant `%k` is zero.
const INVARIANT: &str = "define i16 @f(i16 %n, i16 %k) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %sum = phi i16 [ 0, %b0 ], [ %total, %b4 ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %b2, label %b5

b2:
  %flag = icmp eq i16 %k, 0
  br i1 %flag, label %b3, label %b4

b3:
  %bumped = add i16 %sum, %i
  br label %b4

b4:
  %total = phi i16 [ %bumped, %b3 ], [ %sum, %b2 ]
  %next = add i16 %i, 1
  br label %b1

b5:
  ret i16 %sum
}
";

const INPUTS: &[&[i128]] = &[&[0, 0], &[5, 0], &[5, 3], &[9, 1]];

fn loop_count(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

/// `text`'s @f specialized, or `None`.
fn specialize(text: &str) -> Option<Module> {
    let mut module = parsed(text);
    let function = f(&mut module).clone();
    let candidate = specialized(&mut module.context, &function).unwrap()?;
    *f(&mut module) = candidate;
    Some(module)
}

/// Each way the condition goes gets its own loop, the copy the one where it
/// holds, and the function returns what it did.
#[test]
fn test_an_invariant_branch_specializes_the_loop() {
    let mut module = specialize(INVARIANT).expect("specialized");
    let text = printed(&module);
    assert_eq!(loop_count(&mut module), 2, "{text}");
    assert!(text.contains("b0:\n  %0 = icmp eq i16 %k, 0\n  br i1 %0, label %"), "{text}");
    assert!(text.contains("b2:\n  %flag = icmp eq i16 %k, 0\n  br label %b4\n"), "{text}");
    assert!(!text.contains("b3:"), "the arm the original no longer takes is gone: {text}");
    assert_eq!(results(&module, INPUTS), results(&parsed(INVARIANT), INPUTS));
}

#[test]
fn test_condition_must_be_pure_and_loop_invariant() {
    let variant = INVARIANT.replace("%flag = icmp eq i16 %k, 0", "%flag = icmp eq i16 %i, 0");
    let memory = INVARIANT
        .replace("define i16 @f(i16 %n, i16 %k)", "define i16 @f(i16 %n, i16 %k, ptr %p)")
        .replace("%flag = icmp eq i16 %k, 0", "%v = load i16, ptr %p\n  %flag = icmp eq i16 %v, 0");
    for text in [variant, memory] {
        assert!(specialize(&text).is_none(), "{text}");
    }
}

/// Only a branch inside the body is specialized: the header's is the
/// loop's own test.
#[test]
fn test_a_condition_in_the_header_is_not_unswitched() {
    let text = INVARIANT.replace(
        "  %more = icmp ult i16 %i, %n\n  br i1 %more, label %b2, label %b5",
        "  %more = icmp ult i16 %k, %n\n  br i1 %more, label %b2, label %b5",
    );
    let text = text.replace("%flag = icmp eq i16 %k, 0", "%flag = icmp eq i16 %i, 0");
    assert!(specialize(&text).is_none());
}

/// A loop entered from two blocks has no one place for the test.
#[test]
fn test_a_loop_without_one_preheader_is_not_unswitched() {
    let text = INVARIANT
        .replace("b0:\n  br label %b1", "b0:\n  %skip = icmp eq i16 %n, 7\n  br i1 %skip, label %b6, label %b1\n\nb6:\n  br label %b1")
        .replace("[ 0, %b0 ], [ %next, %b4 ]", "[ 0, %b0 ], [ 1, %b6 ], [ %next, %b4 ]")
        .replace("[ 0, %b0 ], [ %total, %b4 ]", "[ 0, %b0 ], [ 0, %b6 ], [ %total, %b4 ]");
    assert!(specialize(&text).is_none());
}

/// @f of `text` through `optimized`, its candidate re-optimized into a copy
/// of `@replacement` when named; whether it was kept, and the module.
fn through(text: &str, replacement: Option<&str>, costs: OperationCosts) -> (bool, String) {
    let mut module = parsed(text);
    let replacement = replacement.map(|name| module.function_mut(name).expect("the replacement").1.clone());
    let layout = DataLayout::default();
    let callees = llrm_mir::memory::callees(&module);
    let sizes = llrm_mir::valuetracking::sizes(&module, &layout);
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").unwrap();
    let mut declared = llrm_mir::passes::Declared::default();
    let mut unit = Unit { context, layout: &layout, function, callees: &callees, metadata: &metadata, sizes: &sizes, declared: &mut declared };
    let kept = optimized(&mut unit, &costs, &mut |trial: &mut Unit| {
        if let Some(one) = &replacement {
            *trial.function = one.clone();
        }
    })
    .unwrap();
    (kept, printed(&module))
}

/// Replacements for the re-optimized candidate, beside @f: loop-free, and
/// dearer or unpriced.
const REPLACEMENTS: &str = "
define i16 @divide(i16 %n, i16 %k) {
b0:
  %q = udiv i16 %n, %k
  ret i16 %q
}

define i16 @choose(i16 %n, i16 %k) {
b0:
  %c = icmp eq i16 %k, 0
  %s = select i1 %c, i16 %n, i16 %k
  ret i16 %s
}
";

/// Duplicating the loop without simplifying it is not a profitable default.
#[test]
fn test_unswitch_rejects_a_candidate_without_loop_removal() {
    assert_eq!(through(INVARIANT, None, OperationCosts::default()), (false, printed(&parsed(INVARIANT))));
}

/// One DIV outside a loop can cost more than one cheap ADD in the loop;
/// at an ordinary price the same candidate is kept.
#[test]
fn test_unswitch_rejects_lower_count_but_higher_target_cost() {
    let text = format!("{INVARIANT}{REPLACEMENTS}");
    let dear = OperationCosts { add: 1, divide: 1000, ..OperationCosts::default() };
    assert_eq!(through(&text, Some("divide"), dear), (false, printed(&parsed(&text))));
    let (kept, after) = through(&text, Some("divide"), OperationCosts::default());
    assert!(kept);
    assert!(after.starts_with("define i16 @f(i16 %n, i16 %k) {\nb0:\n  %q = udiv i16 %n, %k\n  ret i16 %q\n}\n"), "{after}");
}

/// An unknown operation must not become cheap merely because it is unpriced.
#[test]
fn test_unswitch_rejects_semantic_work_without_a_target_price() {
    let text = format!("{INVARIANT}{REPLACEMENTS}");
    assert_eq!(through(&text, Some("choose"), OperationCosts::default()), (false, printed(&parsed(&text))));
}

/// A target where division is dear.
struct DearDivide;

impl llrm_mir::target::Machine for DearDivide {
    fn foreign_span(&self, _: (i64, i64), _: (i64, i64), _: i64) -> Option<(i64, i64)> {
        None
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts { divide: 1000, ..OperationCosts::default() }
    }
}

/// Re-optimizes a candidate into a copy of the function it holds.
struct Replace(llrm_mir::module::Function);

impl llrm_mir::passes::FunctionPass for Replace {
    fn name(&self) -> &'static str {
        "replace"
    }

    fn run(&mut self, unit: &mut Unit, _: &mut llrm_mir::passes::Analyses) -> llrm_mir::passes::PreservedAnalyses {
        *unit.function = self.0.clone();
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// The pass prices at the target's costs: a dear division sinks the
/// candidate the neutral prices keep, where unswitch used to ignore it.
#[test]
fn test_unswitch_prices_at_the_target_costs() {
    let text = format!("{INVARIANT}{REPLACEMENTS}");
    let kept = |target: Option<std::rc::Rc<dyn llrm_mir::target::Machine>>| {
        let mut module = parsed(&text);
        let divide = module.function_mut("divide").unwrap().1.clone();
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.target = target;
        manager.add(super::Unswitch { passes: vec![Box::new(Replace(divide))] });
        manager.run(&mut module).unwrap();
        printed(&module) != printed(&parsed(&text))
    };
    assert!(kept(None));
    assert!(!kept(Some(std::rc::Rc::new(DearDivide))));
}
