//! The rich-MIR corpus, and the corpus tests of llrm-core's
//! `analysis/loops_tests.rs` asked of it through `cfg::graph`.

use std::collections::BTreeSet;

use crate::graph::loops;

use crate::cfg;
use crate::testing::corpus;

#[test]
fn every_corpus_module_parses_and_verifies() {
    let modules = corpus();
    for prefix in ["emitted/qb-", "emitted/demo-", "emitted/nib-", "optimized/qb-", "optimized/demo-", "optimized/nib-"] {
        assert!(modules.iter().any(|(name, _)| name.starts_with(prefix)), "the corpus holds no {prefix}");
    }
    for (name, module) in &modules {
        assert_eq!(llrm_mir::verify::verify(module), Vec::<String>::new(), "{name}");
    }
}

/// Each function with a body, named `module/@function`, as its graph.
fn graphs() -> Vec<(String, Vec<cfg::Block>)> {
    let mut out = Vec::new();
    for (name, module) in corpus() {
        for (_, global, function) in module.functions() {
            if function.entry().is_some() {
                out.push((format!("{name}/@{}", global.name.as_deref().unwrap_or("")), cfg::graph(function)));
            }
        }
    }
    out
}

#[test]
fn every_corpus_function_is_reducible() {
    for (name, graph) in graphs() {
        assert_eq!(loops::irreducible(&graph, None), BTreeSet::new(), "{name}");
    }
}

#[test]
fn a_loop_body_always_contains_its_own_header_and_latch() {
    let mut found = 0;
    for (name, graph) in graphs() {
        let known: BTreeSet<i64> = graph.iter().map(|one| one.at).collect();
        for one in loops::loops(&graph, None) {
            assert!(one.body.contains(&one.header), "{name}");
            assert!(one.latches.is_subset(&one.body), "{name}");
            assert!(one.body.is_subset(&known), "{name}: a loop body never names a block outside the graph");
            found += 1;
        }
    }
    assert!(found > 0, "the corpus has loops");
}

/// Loop depth is bounded by the source. Four, where the BC corpus had two:
/// deedlines' INITCROSFADEPICS nests four FORs, and ranked's rank-4 repeat
/// literal fills with four loops.
#[test]
fn nothing_in_the_corpus_nests_past_four_loops() {
    let deepest = graphs().iter().map(|(_, graph)| loops::depth(graph, None).values().copied().max().unwrap_or(0)).max().unwrap();
    assert_eq!(deepest, 4);
}

#[test]
fn the_managed_shape_is_what_the_graph_walks_find() {
    // Dominance and loops are `Dominators` and `Loops`, read as the graph
    // walks read them: the views must answer exactly as those walks did.
    let odd = crate::testing::parsed(
        "define void @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br i1 %c, label %b2, label %b4

b2:
  br i1 %c, label %b1, label %b3

b3:
  br i1 %c, label %b3, label %b0x

b0x:
  br i1 %c, label %b3, label %b4

b4:
  ret void

dead:
  br i1 %c, label %dead, label %b1
}
",
    );
    let mut modules = corpus();
    modules.push(("odd".to_owned(), odd));
    for (name, module) in &modules {
        for (_, _, function) in module.functions().filter(|(_, _, function)| function.entry().is_some()) {
            let graph = cfg::graph(function);
            let shape = cfg::Shape::of(function);
            assert_eq!(shape.loops, loops::loops(&graph, None), "{name}");
            assert_eq!(shape.dominance.dominators(function), loops::dominators(&graph, None), "{name}");
            assert_eq!(shape.dominance.immediate_dominators(function), loops::immediate_dominators(&graph, None), "{name}");
            assert_eq!(shape.dominance.frontiers(function), loops::frontiers(&graph, None), "{name}");
            assert_eq!(shape.dominance.irreducible(function), loops::irreducible(&graph, None), "{name}");
        }
    }
}
