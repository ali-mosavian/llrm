//! Peel on loops the interpreter runs, before and after. llrm-core's peel
//! had no tests of its own: unroll's pipeline tests covered it, on BC
//! fixtures that stay behind.

use llrm_analysis::cfg;
use llrm_analysis::peelsize::Limits;
use llrm_graph::loops;
use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::PassManager;

use super::Peel;
use crate::testing::{f, managed, parsed, printed, results};

/// A loop of `trips` trips whose diamond multiplies `%acc` on even trips.
fn diamond(bound: &str) -> String {
    format!(
        "define i16 @f(i16 %x, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %acc = phi i16 [ %x, %b0 ], [ %out, %b4 ]
  %go = icmp slt i16 %i, {bound}
  br i1 %go, label %b2, label %b5

b2:
  %odd = and i16 %i, 1
  %even = icmp eq i16 %odd, 0
  br i1 %even, label %b3, label %b4

b3:
  %m = mul i16 %acc, 3
  br label %b4

b4:
  %out = phi i16 [ %acc, %b2 ], [ %m, %b3 ]
  %next = add i16 %i, 1
  br label %b1

b5:
  ret i16 %acc
}}
"
    )
}

const INPUTS: &[&[i128]] = &[&[0, 3], &[1, 3], &[7, 0], &[-5, 9]];

fn loops_of(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

fn multiplies(module: &mut Module) -> usize {
    let function = f(module);
    function.walk().filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Mul)).count()
}

/// `text` through `pass`: whether it changed, and it computes what it did.
fn through(text: &str, pass: Peel) -> (bool, Module) {
    let mut module = parsed(text);
    let before = (printed(&module), results(&module, INPUTS));
    let after = managed(&mut module, pass);
    assert_eq!(results(&module, INPUTS), before.1, "{after}");
    (after != before.0, module)
}

/// Each trip a copy of the body, the first reading the values from before
/// the loop, and the residual loop kept: four and five trips peel; none,
/// one, and a thousand (past max-completely-peel-times) do not.
#[test]
fn every_trip_is_peeled_ahead_of_the_residual_loop() {
    for (trips, peeled) in [(0, false), (1, false), (4, true), (5, true), (1000, false)] {
        let (changed, mut module) = through(&diamond(&trips.to_string()), Peel::default());
        assert_eq!(changed, peeled, "{trips}");
        assert_eq!(loops_of(&mut module), 1, "{trips}");
        if !peeled {
            continue;
        }
        assert_eq!(multiplies(&mut module), trips + 1, "{trips}");
        let function = f(&mut module);
        let entry = function.entry().unwrap();
        let first = function.successors(entry)[0];
        assert_ne!(function.block(first).name.as_deref(), Some("b1"), "{trips}");
        let phis = function.block(first).instructions().iter().filter(|&&inst| function.instruction(inst).opcode == Opcode::Phi).collect::<Vec<_>>();
        assert_eq!(phis.len(), 2);
        for &&phi in &phis {
            assert_eq!(function.instruction(phi).operands[1..], [Operand::Block(entry)], "{trips}");
        }
    }
}

/// Fold, Decide and Dead after peeling leave no loop: the residual never
/// runs, and each copy's diamond goes one way.
#[test]
fn the_residual_loop_is_proven_dead_by_what_follows() {
    let mut module = parsed(&diamond("5"));
    let before = results(&module, INPUTS);
    let mut manager = PassManager::default();
    manager.add(Peel::default());
    manager.add(crate::fold::Fold);
    manager.add(crate::decide::Decide);
    manager.add(crate::dead::Dead);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let text = printed(&module);
    assert_eq!(loops_of(&mut module), 0, "{text}");
    assert_eq!(multiplies(&mut module), 3, "{text}");
    assert_eq!(results(&module, INPUTS), before);
}

#[test]
fn unknown_trips_are_not_peeled() {
    assert!(!through(&diamond("%n"), Peel::default()).0);
}

/// Two unknown operations a trip, eight trips: sixteen, past a budget of one.
#[test]
fn a_copy_over_budget_is_not_peeled() {
    let growing = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %go = icmp slt i16 %i, 8
  br i1 %go, label %b2, label %b3

b2:
  %m = mul i16 %acc, %n
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}
";
    let tight = Peel { limits: Limits { max_unrolled_operations: 1, ..Limits::default() }, ..Peel::default() };
    assert!(!through(growing, tight).0);
    assert!(through(growing, Peel::default()).0);
}

/// A call stays once a trip: growth past it is refused, and @tick still
/// counts every trip.
#[test]
fn a_loop_with_a_call_is_not_peeled() {
    let text = "@count = global i16 0

define void @tick() {
b0:
  %c = load i16, ptr @count
  %d = add i16 %c, 1
  store i16 %d, ptr @count
  ret void
}

define i16 @f(i16 %x, i16 %n) {
b0:
  store i16 0, ptr @count
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %go = icmp slt i16 %i, 6
  br i1 %go, label %b2, label %b3

b2:
  call void @tick()
  %m = mul i16 %acc, %n
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  %t = load i16, ptr @count
  %r = add i16 %t, %acc
  ret i16 %r
}
";
    assert!(!through(text, Peel::default()).0);
}

/// A `select` has no target price, so nothing in its function is copied.
#[test]
fn an_unpriced_function_is_left_alone() {
    let text = diamond("4").replace("  %m = mul i16 %acc, 3\n", "  %m0 = mul i16 %acc, 3\n  %m = select i1 %even, i16 %m0, i16 %acc\n");
    assert!(!through(&text, Peel::default()).0);
}
