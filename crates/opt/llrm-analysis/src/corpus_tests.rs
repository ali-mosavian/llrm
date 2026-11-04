//! The rich-MIR corpus, and the corpus tests of llrm-codegen's
//! `analysis/loops_tests.rs` asked of it through `cfg::graph`.

use std::collections::BTreeSet;

use crate::cfg;
use crate::graph::loops;
use crate::testing::corpus;

#[test]
fn every_corpus_module_parses_and_verifies() {
    let modules = corpus();
    for prefix in ["emitted/qb-", "emitted/demo-", "emitted/nib-", "optimized/qb-", "optimized/demo-", "optimized/nib-"]
    {
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

/// Functions the optimizer leaves irreducible although their source is not.
/// Both are the pin's own passes, run as it runs them; the pin does not keep
/// loops reducible:
/// - `entries/@main`: the late SimplifyCFG's `sinkCommonCodeFromPredecessors`
///   after a loop peel gives the loop header a second entry
///   (`-fno-unroll-loops` hides it). The pin's `opt` makes the same split block
///   from the same CFG:
///   `tests/fixtures/pin-simplifycfg/sink_split_irreducible.*.ll`.
/// - `pipeline/@$state3.next`: DFAJumpThreading on a state machine (on in the
///   pin; `LLRM_NO_DFAJUMPTHREADING=1` makes it reducible).
/// A known function that is reducible fails the test, so the entry goes with
/// the change that makes it so.
const KNOWN_IRREDUCIBLE: &[&str] =
    &["optimized/nib-examples-entries/@main", "optimized/nib-examples-pipeline/@$state3.next"];

/// A function with an error handler has a personality: its RESUME switch
/// (`hir/mir/handling.rs`) enters the statement after, or the statement
/// itself, wherever the error was raised, loop bodies included, so such a
/// function is irreducible by construction (qb-bounds, `ON ERROR RESUME
/// NEXT` in a loop: blocks 6, 7, 9, 11 and 13). Every other function is
/// reducible.
#[test]
fn every_corpus_function_without_an_error_handler_is_reducible() {
    let mut handled = 0;
    let mut known = BTreeSet::new();
    let mut unexpected = BTreeSet::new();
    for (name, module) in corpus() {
        for (_, global, function) in module.functions() {
            if function.entry().is_none() {
                continue;
            }
            let name = format!("{name}/@{}", global.name.as_deref().unwrap_or(""));
            let irreducible = loops::irreducible(&cfg::graph(function), None);
            if function.personality.is_some() {
                handled += 1;
            } else if KNOWN_IRREDUCIBLE.contains(&name.as_str()) {
                assert_ne!(irreducible, BTreeSet::new(), "{name} is reducible now: remove it from KNOWN_IRREDUCIBLE");
                known.insert(name);
            } else if irreducible != BTreeSet::new() {
                unexpected.insert(name);
            }
        }
    }
    assert_eq!(unexpected, BTreeSet::new(), "irreducible with no error handler");
    assert!(handled > 0, "the corpus has functions with an error handler");
    assert_eq!(known.len(), KNOWN_IRREDUCIBLE.len(), "{known:?}");
}

#[test]
fn a_loop_body_always_contains_its_own_header_and_latch() {
    let mut found = 0;
    for (name, graph) in graphs() {
        let known: BTreeSet<i64> = graph.iter().map(|one| one.at).collect();
        for one in loops::loops(&graph, None) {
            assert!(one.body.contains(&one.header), "{name}");
            assert!(one.latches.iter().all(|at| one.body.contains(at)), "{name}");
            assert!(
                one.body.iter().all(|at| known.contains(at)),
                "{name}: a loop body never names a block outside the graph"
            );
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
    let deepest =
        graphs().iter().map(|(_, graph)| loops::depth(graph, None).values().copied().max().unwrap_or(0)).max().unwrap();
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
            assert_eq!(
                shape.dominance.immediate_dominators(function),
                loops::immediate_dominators(&graph, None),
                "{name}"
            );
            assert_eq!(shape.dominance.frontiers(function), loops::frontiers(&graph, None), "{name}");
            assert_eq!(shape.dominance.irreducible(function), loops::irreducible(&graph, None), "{name}");
            assert_eq!(
                shape.dominance.is_irreducible(function),
                !loops::irreducible(&graph, None).is_empty(),
                "{name}"
            );
        }
    }
}

/// `Shape` takes its loops from `LoopInfo` (LLVM's `analyzeImpl`), the graph
/// walks from the address forest: two finders of the same loops. They agree
/// on every loop of the corpus, header, latches and blocks.
#[test]
fn the_loops_of_a_shape_are_those_the_graph_walks_find() {
    let mut found = 0;
    for (name, module) in corpus() {
        for (_, global, function) in module.functions() {
            if function.entry().is_none() {
                continue;
            }
            let walks = |loops: Vec<loops::Loop>| {
                loops
                    .into_iter()
                    .map(|one| (one.header, (one.latches.clone(), one.body.iter().copied().collect::<Vec<_>>())))
                    .collect::<std::collections::BTreeMap<_, _>>()
            };
            let name = format!("{name}/@{}", global.name.as_deref().unwrap_or(""));
            let from_info = walks(cfg::Shape::of(function).loops);
            found += from_info.len();
            assert_eq!(from_info, walks(loops::loops(&cfg::graph(function), None)), "{name}");
        }
    }
    assert!(found > 1000, "premise: the corpus holds many loops, {found} compared");
}

/// The claim above, checked on the pin's own output: its SimplifyCFG sink
/// leaves `f` with a cycle entered at two nodes.
#[test]
fn the_pins_simplifycfg_sink_makes_an_irreducible_loop() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/pin-simplifycfg/sink_split_irreducible.pin.ll");
    let module = crate::testing::parse_corpus_entry("pin-sink", &path);
    let irreducible = module
        .functions()
        .filter(|(_, _, function)| {
            function.entry().is_some() && loops::irreducible(&cfg::graph(function), None) != BTreeSet::new()
        })
        .count();
    assert_eq!(irreducible, 1);
}
