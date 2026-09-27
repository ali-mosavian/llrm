//! Adapted from llrm-core's `optimize/loopclone_tests.rs`, the port of
//! `tests/test_loopclone.py`, each body now MIR text. `is None` becomes
//! `Ok(None)`.
//!
//! What the old tests asserted of the old operation's fields (`inserted`,
//! `absorbed`, a result naming its definition) has no counterpart; the
//! pointer identity, seed and range tables are an instruction's metadata,
//! which a clone keeps.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use crate::graph::loops;
use llrm_mir::interpret::{self, Val};
use llrm_mir::module::{Function, Module, Operand};

use super::peeled;

fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn f(module: &Module) -> &Function {
    module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f").2
}

fn at(function: &Function, name: &str) -> i64 {
    cfg::id(*function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).expect("a block"))
}

/// The loop of `diamond`: an early exit from `b3`, both arms joined in `b5`.
const DIAMOND: &str = "define i16 @f(i16 %seed, ptr %p) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b5 ]
  %go = icmp ult i16 %carried, 100
  br i1 %go, label %b2, label %b6

b2:
  %odd = trunc i16 %carried to i1
  br i1 %odd, label %b3, label %b4

b3:
  %left = add i16 %carried, 3
  %big = icmp ugt i16 %left, 50
  br i1 %big, label %b6, label %b5

b4:
  %right = add i16 %carried, 1
  br label %b5

b5:
  %selected = phi i16 [ %left, %b3 ], [ %right, %b4 ]
  %stepped = add i16 %selected, 1
  br label %b1

b6:
  %answer = phi i16 [ %carried, %b1 ], [ %left, %b3 ]
  ret i16 %answer
}
";

fn only_loop(function: &Function) -> loops::Loop {
    let found = loops::loops(&cfg::graph(function), function.entry().map(cfg::id));
    assert_eq!(found.len(), 1);
    found.into_iter().next().unwrap()
}

/// `text` with @f peeled `count` times, verified.
fn peel(text: &str, count: i64) -> Option<Module> {
    let mut module = parsed(text);
    let function = f(&module);
    let changed = peeled(&module.context, function, &only_loop(function), count).unwrap()?;
    *module.function_mut("f").unwrap().1 = changed;
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
    Some(module)
}

fn runs(module: &Module, seed: u128) -> Val {
    interpret::run(module, "f", vec![Val::Int { bits: seed, width: 16 }, Val::Ptr(0)], 100_000).expect("runs")
}

#[test]
fn test_peeling_clones_diamond_and_early_exit_phis() {
    let original = parsed(DIAMOND);
    let body = f(&original);
    let loop_ = only_loop(body);
    let module = peel(DIAMOND, 2).expect("peeled");
    let changed = f(&module);
    assert_eq!(changed.layout().len(), body.layout().len() + 2 * loop_.body.len());
    let graph = cfg::graph(changed);
    let predecessors = loops::predecessors(&graph);
    let phis = |block: i64| {
        changed.block(cfg::block(block)).instructions().iter().copied().filter(|&one| changed.instruction(one).opcode == llrm_mir::opcode::Opcode::Phi).collect::<Vec<_>>()
    };
    let incoming = |phi| super::incoming(changed, phi);
    for block in &graph {
        for phi in phis(block.at) {
            assert_eq!(incoming(phi).iter().map(|(_, source)| *source).collect::<BTreeSet<_>>(), predecessors[&block.at]);
        }
    }
    let (b0, b1, b5, b6) = (at(changed, "b0"), at(changed, "b1"), at(changed, "b5"), at(changed, "b6"));
    let first = cfg::id(changed.successors(cfg::block(b0))[0]);
    let seed = Operand::Value(changed.parameters()[0]);
    assert_eq!(incoming(phis(first)[0]), vec![(seed, b0)]);
    let residual = incoming(phis(b1)[0]);
    let stepped = body.walk().find_map(|(_, one)| body.instruction(one).result.filter(|&value| body.value(value).name.as_deref() == Some("stepped"))).unwrap();
    assert!(residual.contains(&(Operand::Value(stepped), b5)));
    assert!(!residual.iter().any(|(_, source)| *source == b0));
    assert_eq!(incoming(phis(b6)[0]).len(), 6);
    assert_eq!(loops::loops(&graph, Some(b0)), vec![loop_]);
}

#[test]
fn test_peeled_function_returns_what_the_loop_did() {
    let original = parsed(DIAMOND);
    for count in 1..=3 {
        let module = peel(DIAMOND, count).expect("peeled");
        for seed in [0, 1, 2, 47, 48, 99, 100, 65535] {
            assert_eq!(runs(&module, seed), runs(&original, seed), "count {count}, seed {seed}");
        }
    }
}

#[test]
fn test_clones_read_their_own_values_and_leave_the_originals_alone() {
    let original = parsed(DIAMOND);
    let body = f(&original);
    let module = peel(DIAMOND, 1).expect("peeled");
    let changed = f(&module);
    let fresh = changed.layout()[body.layout().len()..]
        .iter()
        .flat_map(|&block| changed.block(block).instructions())
        .filter_map(|&one| changed.instruction(one).result)
        .collect::<BTreeSet<_>>();
    for &block in &changed.layout()[body.layout().len()..] {
        for &one in changed.block(block).instructions() {
            let instruction = changed.instruction(one);
            if instruction.opcode == llrm_mir::opcode::Opcode::Phi {
                continue;
            }
            assert!(instruction.operands.iter().all(|operand| match operand {
                Operand::Value(value) => fresh.contains(value),
                _ => true,
            }));
        }
    }
    let b3 = cfg::block(at(body, "b3"));
    assert_eq!(changed.block(b3).instructions(), body.block(b3).instructions());
    for &one in body.block(b3).instructions() {
        assert_eq!(changed.instruction(one), body.instruction(one));
    }
}

#[test]
fn test_peeling_refuses_floating_work_behind_an_internal_branch() {
    let text = DIAMOND
        .replace("  %big = icmp", "  %fl = sitofp i16 %left to double\n  %big = icmp")
        .replace("  %stepped = add", "  %fs = phi double [ %fl, %b3 ], [ 0.0, %b4 ]\n  %stepped = add");
    assert!(peel(&text, 2).is_none());
}

#[test]
fn test_peeling_accepts_block_local_floating_values_behind_a_branch() {
    let text = DIAMOND.replace("  %big = icmp", "  %t = load double, ptr %p\n  store double %t, ptr %p\n  %big = icmp");
    assert!(peel(&text, 1).is_some());
}

#[test]
fn test_peeling_clones_keep_their_originals_metadata() {
    let text = format!("{}\n!0 = !{{!\"fact\"}}\n", DIAMOND.replace("%left = add i16 %carried, 3", "%left = add i16 %carried, 3, !fact !0"));
    let module = peel(&text, 1).expect("peeled");
    let changed = f(&module);
    let tagged = changed.walk().filter(|&(_, one)| !changed.instruction(one).metadata.is_empty()).count();
    assert_eq!(tagged, 2);
}

#[test]
fn test_unclosed_loop_value_is_refused() {
    let text = DIAMOND.replace("  ret i16 %answer", "  %escaped = add i16 %carried, 0\n  ret i16 %escaped");
    assert!(peel(&text, 1).is_none());
}

#[test]
fn test_opaque_dispatch_is_not_cloned_as_an_ordinary_branch() {
    let text = format!(
        "declare void @g()\n\ndeclare i32 @personality(...)\n\n{}",
        DIAMOND
            .replace("define i16 @f(i16 %seed, ptr %p) {", "define i16 @f(i16 %seed, ptr %p) personality ptr @personality {")
            .replace("  br i1 %odd, label %b3, label %b4", "  invoke void @g() to label %b3 unwind label %b4")
            .replace("b4:\n", "b4:\n  %pad = landingpad { ptr, i32 } cleanup\n")
    );
    assert!(peel(&text, 1).is_none());
}

#[test]
fn test_nonpositive_peel_count_is_rejected() {
    let module = parsed(DIAMOND);
    let function = f(&module);
    for count in [0, -1] {
        assert!(peeled(&module.context, function, &only_loop(function), count).is_err());
    }
}

#[test]
fn test_peeling_refuses_a_loop_without_one_preheader_or_one_latch() {
    // Two latches.
    let two_latches = DIAMOND.replace("br i1 %big, label %b6, label %b5", "br i1 %big, label %b1, label %b5").replace(
        "%carried = phi i16 [ %seed, %b0 ], [ %stepped, %b5 ]",
        "%carried = phi i16 [ %seed, %b0 ], [ %stepped, %b5 ], [ %left, %b3 ]",
    ).replace("[ %carried, %b1 ], [ %left, %b3 ]", "[ %carried, %b1 ]");
    assert!(peel(&two_latches, 1).is_none());
    // The entry into the loop also branches elsewhere.
    let shared = DIAMOND
        .replace("b0:\n  br label %b1", "b0:\n  %skip = icmp eq i16 %seed, 7\n  br i1 %skip, label %b6, label %b1")
        .replace("[ %carried, %b1 ], [ %left, %b3 ]", "[ %carried, %b1 ], [ %left, %b3 ], [ 0, %b0 ]");
    assert!(peel(&shared, 1).is_none());
    // The header is the entry.
    let header = "define i16 @f(i16 %seed, ptr %p) {
b1:
  %go = icmp ult i16 %seed, 100
  br i1 %go, label %b1, label %b2

b2:
  ret i16 %seed
}
";
    assert!(peel(header, 1).is_none());
}

#[test]
fn test_peeling_clones_a_switch_and_keeps_its_results() {
    let text = "define i16 @f(i16 %seed, ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ %seed, %b0 ], [ %next, %b4 ]
  %sum = phi i16 [ 0, %b0 ], [ %total, %b4 ]
  %more = icmp ult i16 %i, 20
  br i1 %more, label %b6, label %b5

b6:
  %low = and i16 %i, 3
  switch i16 %low, label %b3 [ i16 0, label %b2
                               i16 1, label %b5 ]

b2:
  br label %b4

b3:
  br label %b4

b4:
  %add = phi i16 [ 10, %b2 ], [ %low, %b3 ]
  %total = add i16 %sum, %add
  %next = add i16 %i, 1
  br label %b1

b5:
  %out = phi i16 [ %sum, %b1 ], [ %sum, %b6 ]
  ret i16 %out
}
";
    let original = parsed(text);
    let module = peel(text, 2).expect("peeled");
    let switches = |module: &Module| f(module).walk().filter(|&(_, one)| f(module).instruction(one).opcode == llrm_mir::opcode::Opcode::Switch).count();
    assert_eq!(switches(&module), 3);
    for seed in [0, 2, 3, 17, 19, 20, 400] {
        assert_eq!(runs(&module, seed), runs(&original, seed), "seed {seed}");
    }
}
