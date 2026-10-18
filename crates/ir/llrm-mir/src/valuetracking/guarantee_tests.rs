//! `gp` against the answers of LLVM's `isGuaranteedNotToBePoison` and
//! `isGuaranteedNotToBeUndef` on `tests/vt/guarantee/guarantee.ll`.

use crate::datalayout::DataLayout;
use crate::dominators::DominatorTree;
use crate::module::{GlobalKind, Operand};
use crate::parse;
use crate::valuetracking::{AssumptionCache, Query, column};

/// The lines `vtdump gp` prints for `text`.
fn dump(text: &str) -> String {
    let module = parse::module(text).expect("parses");
    let layout = DataLayout::default();
    let mut out = String::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let name = global.name.clone().unwrap_or_default();
        let dt = DominatorTree::new(function);
        let assumptions = AssumptionCache::new(&module, function);
        let entry_first = function.block(function.entry().expect("an entry")).instructions().first().copied();
        let mut values: Vec<(Operand, _)> =
            function.parameters().iter().map(|p| (Operand::Value(*p), entry_first)).collect();
        for (_, inst) in function.walk() {
            if let Some(result) = function.instruction(inst).result {
                values.push((Operand::Value(result), Some(inst)));
            }
        }
        for (n, (value, cxt)) in values.into_iter().enumerate() {
            let mut query = Query::new(&module, function, &layout).with_cxt(cxt);
            query.dt = Some(&dt);
            query.assumptions = Some(&assumptions);
            out.push_str(&format!(
                "{name} {n}{}
",
                column(&query, value, "gp")
            ));
        }
    }
    out
}

/// A `noundef` argument, `freeze`, a branch condition or divisor used on the
/// way, a use that must be defined before a call that may not return, and
/// `phi`s over them: each as LLVM answers it. Before `gp`, every one of these
/// was 00.
#[test]
fn test_guaranteed_not_to_be_poison_or_undef_matches_llvm() {
    let text = include_str!("../../tests/vt/guarantee/guarantee.ll");
    assert_eq!(dump(text), EXPECTED);
}

const EXPECTED: &str = "args 0 gp=11
args 1 gp=00
args 2 gp=11
args 3 gp=00
flags 0 gp=00
flags 1 gp=00
flags 2 gp=00
flags 3 gp=00
flags 4 gp=11
flags 5 gp=01
constants 0 gp=00
constants 1 gp=00
constants 2 gp=00
calls 0 gp=00
calls 1 gp=11
calls 2 gp=00
calls 3 gp=00
next_use 0 gp=11
next_use 1 gp=00
next_use 2 gp=11
blocked 0 gp=00
blocked 1 gp=00
divide 0 gp=00
divide 1 gp=10
divide 2 gp=00
dominated 0 gp=10
dominated 1 gp=00
dominated 2 gp=11
dominated 3 gp=00
dominated 4 gp=00
phis 0 gp=11
phis 1 gp=11
phis 2 gp=00
phis 3 gp=11
phis 4 gp=00
phis 5 gp=11
phis 6 gp=00
pointers 0 gp=10
pointers 1 gp=10
pointers 2 gp=11
pointers 3 gp=11
pointers 4 gp=11
pointers 5 gp=11
selects 0 gp=00
selects 1 gp=00
selects 2 gp=00
selects 3 gp=00
selects 4 gp=00
";

/// `isGuaranteedToExecuteForEveryIteration` read `Loop`'s fields from the
/// deleted `loops` module, so the crate stopped building after #2195
/// (`unresolved import crate::loops`). A header instruction before a call that
/// may not return runs on every iteration; one after it, and one outside the
/// header, do not.
#[test]
fn test_every_iteration_is_the_header_up_to_a_call_that_may_not_return() {
    use crate::loopinfo::LoopInfo;
    let module = parse::module(
        "declare void @f()
define i32 @g(i32 %n) {
entry:
  br label %head
head:
  %i = phi i32 [ 0, %entry ], [ %next, %body ]
  %a = add i32 %i, 1
  call void @f()
  %b = add i32 %i, 2
  %c = icmp slt i32 %b, %n
  br i1 %c, label %body, label %exit
body:
  %next = add i32 %i, 3
  br label %head
exit:
  ret i32 %i
}
",
    )
    .expect("parses");
    let layout = DataLayout::default();
    let GlobalKind::Function(function) = &module.globals.iter().find(|g| g.name.as_deref() == Some("g")).unwrap().kind
    else {
        panic!("a function");
    };
    let info = LoopInfo::analyze(function, || DominatorTree::new(function));
    let one = info.top_level_loops().next().expect("a loop");
    let query = Query::new(&module, function, &layout);
    let adds: Vec<_> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            matches!(
                function.instruction(inst).opcode,
                crate::opcode::Opcode::Binary(crate::opcode::BinaryOp::Add)
            )
        })
        .collect();
    let every =
        |inst| crate::valuetracking::guarantee::is_guaranteed_to_execute_for_every_iteration(&query, inst, &one);
    assert!(every(adds[0]));
    assert!(!every(adds[1]));
    assert!(!every(adds[2]));
}
