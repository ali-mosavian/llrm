//! Adapted from llrm-core's `optimize/loopsimplify_tests.rs`, the port of
//! `tests/test_loopsimplify.py`, each body now MIR text.
//!
//! Stay behind, reading BC corpora: test_adjacent_angle_loops_reach_a_fixed_point
//! (qrender's MDL_ANGLEMOD) and test_real_timer_loop_has_one_backedge (nbody's
//! and fpbench's PITSNAP).

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_graph::loops;
use llrm_mir::module::Module;

use super::{grouped, simplified};

fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn printed(module: &Module) -> String {
    assert_eq!(llrm_mir::verify::verify(module), Vec::<String>::new());
    llrm_mir::print::module(module)
}

/// The block named `name` of @f.
fn at(module: &Module, name: &str) -> i64 {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let block = function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).expect("a block");
    cfg::id(*block)
}

/// The loop of `loop_with_exit_use`, entered from two blocks with their
/// own seeds.
const TWO_ENTRIES: &str = "define i16 @f(i1 %c, i16 %s, i16 %t) {
b0:
  br i1 %c, label %b5, label %b4

b5:
  br label %b1

b4:
  br label %b1

b1:
  %carried = phi i16 [ %s, %b5 ], [ %t, %b4 ], [ %stepped, %b2 ]
  br i1 %c, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  ret i16 %carried
}
";

#[test]
fn test_grouping_preserves_each_phi_edge_value() {
    let mut module = parsed(TWO_ENTRIES);
    let (header, sources) = (at(&module, "b1"), BTreeSet::from([at(&module, "b5"), at(&module, "b4")]));
    let (_, function) = module.function_mut("f").expect("@f");
    grouped(function, header, &sources).expect("groups");
    // Two distinct entries retain their own source value at the preheader.
    assert_eq!(
        printed(&module),
        "define i16 @f(i1 %c, i16 %s, i16 %t) {
b0:
  br i1 %c, label %b5, label %b4

b5:
  br label %0

b4:
  br label %0

b1:
  %carried = phi i16 [ %stepped, %b2 ], [ %1, %0 ]
  br i1 %c, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  ret i16 %carried

0:
  %1 = phi i16 [ %s, %b5 ], [ %t, %b4 ]
  br label %b1
}
"
    );
}

#[test]
fn test_unsupported_group_is_atomic() {
    for hazard in ["entry", "missing-source", "opaque", "bad-phi"] {
        let text = match hazard {
            // A terminator that is not `br`, as the old opaque operation.
            "opaque" => TWO_ENTRIES.replace("b5:\n  br label %b1", "b5:\n  switch i16 %s, label %b1 []"),
            _ => TWO_ENTRIES.to_owned(),
        };
        let mut module = parsed(&text);
        let (mut target, mut sources) = (at(&module, "b1"), BTreeSet::from([at(&module, "b5"), at(&module, "b4")]));
        let (_, function) = module.function_mut("f").expect("@f");
        match hazard {
            "entry" => target = cfg::id(function.entry().expect("an entry")),
            "missing-source" => sources = BTreeSet::from([999]),
            "bad-phi" => {
                let phi = function.block(cfg::block(target)).instructions()[0];
                let operands = function.instruction(phi).operands[4..].to_vec();
                function.set_operands(phi, operands);
            }
            _ => {}
        }
        let before = function.clone();
        assert_eq!(grouped(function, target, &sources), None, "{hazard}");
        assert_eq!(*function, before, "{hazard}");
    }
}

#[test]
fn test_conditional_entry_and_shared_exit_become_dedicated() {
    for unsupported_exit in [false, true] {
        let header = match unsupported_exit {
            false => "br i1 %d, label %b2, label %b3",
            true => "switch i16 %s, label %b2 [ i16 0, label %b3 ]",
        };
        let text = format!(
            "define i16 @f(i1 %c, i1 %d, i16 %s) {{
b0:
  br i1 %c, label %b1, label %b3

b1:
  %carried = phi i16 [ 0, %b0 ], [ %stepped, %b2 ]
  {header}

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  %r = phi i16 [ 5, %b0 ], [ %carried, %b1 ]
  ret i16 %r
}}
"
        );
        let mut module = parsed(&text);
        let (header_at, exit_at) = (at(&module, "b1"), at(&module, "b3"));
        let (_, function) = module.function_mut("f").expect("@f");
        let before = function.clone();
        let changed = simplified(function);
        if unsupported_exit {
            assert!(!changed);
            assert_eq!(*function, before);
            continue;
        }
        let graph = cfg::graph(function);
        let found = loops::loops(&graph, Some(cfg::id(function.entry().unwrap())));
        assert_eq!(found.len(), 1);
        let loop_ = &found[0];
        let predecessors = loops::predecessors(&graph);
        let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
        assert_eq!(outside.len(), 1);
        assert_eq!(function.successors(cfg::block(outside[0])), [cfg::block(header_at)]);
        let exits = graph
            .iter()
            .filter(|block| loop_.body.contains(&block.at))
            .flat_map(|block| block.succ.iter().copied())
            .filter(|at| !loop_.body.contains(at))
            .collect::<BTreeSet<_>>();
        assert_ne!(exits, BTreeSet::from([exit_at]));
        assert!(exits.iter().all(|at| predecessors[at].is_subset(&loop_.body)));
        assert!(!simplified(function));
        printed(&module);
    }
}
