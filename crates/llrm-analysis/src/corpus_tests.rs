//! The rich-MIR corpus, and the corpus tests of llrm-core's
//! `analysis/loops_tests.rs` asked of it through `cfg::graph`.

use std::collections::BTreeSet;

use llrm_graph::loops;

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
