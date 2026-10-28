//! Adapted from llrm-codegen's `optimize/loopsimplify_tests.rs`, the port of
//! `tests/test_loopsimplify.py`, each body now MIR text.
//!
//! The BC corpora's test_adjacent_angle_loops_reach_a_fixed_point
//! (qrender's MDL_ANGLEMOD) and test_real_timer_loop_has_one_backedge (nbody's
//! and fpbench's PITSNAP) are asked of the rich-MIR corpus instead.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
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
fn at(
    module: &Module,
    name: &str,
) -> i64 {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let block =
        function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).expect("a block");
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

0:
  %1 = phi i16 [ %s, %b5 ], [ %t, %b4 ]
  br label %b1

b1:
  %carried = phi i16 [ %stepped, %b2 ], [ %1, %0 ]
  br i1 %c, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  ret i16 %carried
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
                let phi = function.block(cfg::block(target)).first().expect("a block with an instruction");
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
        let outside =
            predecessors[&loop_.header].iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>();
        assert_eq!(outside.len(), 1);
        assert_eq!(function.successors(cfg::block(outside[0])), [cfg::block(header_at)]);
        let exits = graph
            .iter()
            .filter(|block| loop_.body.contains(&block.at))
            .flat_map(|block| block.succ.iter().copied())
            .filter(|at| !loop_.body.contains(at))
            .collect::<BTreeSet<_>>();
        assert_ne!(exits, BTreeSet::from([exit_at]));
        assert!(exits.iter().all(|at| predecessors[at].iter().all(|a| loop_.body.contains(a))));
        assert!(!simplified(function));
        printed(&module);
    }
}

/// `f` run on each of `inputs`, every argument an `i16`.
fn results(
    module: &Module,
    inputs: &[&[u128]],
) -> Vec<llrm_mir::interpret::Val> {
    let int = |bits| llrm_mir::interpret::Val::Int { bits, width: 16 };
    inputs
        .iter()
        .map(|one| {
            llrm_mir::interpret::run(module, "f", one.iter().map(|&bits| int(bits)).collect(), 100_000).expect("runs")
        })
        .collect()
}

/// `text` before and after the pass, run under the pass manager's verifier
/// and preserved-analyses check.
fn simplify(text: &str) -> (Module, Module) {
    let before = parsed(text);
    let mut module = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(super::LoopSimplify);
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    (before, module)
}

/// The loop headed by `header` in @f: how many predecessors enter it from
/// outside, how many latches it has, and whether every exit is dedicated.
fn shape(
    module: &Module,
    header: &str,
) -> (usize, usize, bool) {
    let header = at(module, header);
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let graph = cfg::graph(function);
    let found = loops::loops(&graph, function.entry().map(cfg::id));
    let loop_ = found.iter().find(|one| one.header == header).expect("the loop");
    let predecessors = loops::predecessors(&graph);
    let outside = predecessors[&header].iter().filter(|a| !loop_.body.contains(a)).collect::<Vec<_>>();
    let dedicated = graph
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter())
        .filter(|at| !loop_.body.contains(at))
        .all(|at| predecessors[at].iter().all(|a| loop_.body.contains(a)));
    (outside.len(), loop_.latches.len(), dedicated)
}

#[test]
fn two_latches_become_one_and_the_loop_computes_the_same() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %a, %even ], [ %b, %odd ]
  %s = phi i16 [ 1, %b0 ], [ %sa, %even ], [ %sb, %odd ]
  %done = icmp uge i16 %i, %n
  br i1 %done, label %out, label %body

body:
  %bit = and i16 %i, 1
  %isodd = icmp ne i16 %bit, 0
  br i1 %isodd, label %odd, label %even

even:
  %a = add i16 %i, 1
  %sa = add i16 %s, %i
  br label %head

odd:
  %b = add i16 %i, 1
  %sb = mul i16 %s, 3
  br label %head

out:
  ret i16 %s
}
";
    let (before, after) = simplify(text);
    assert_eq!(shape(&before, "head").1, 2);
    assert_eq!(shape(&after, "head"), (1, 1, true));
    let inputs: &[&[u128]] = &[&[0], &[1], &[2], &[5], &[8]];
    assert_eq!(results(&after, inputs), results(&before, inputs));
}

#[test]
fn a_shared_exit_and_a_conditional_entry_keep_what_the_loop_returns() {
    let text = "define i16 @f(i16 %n, i16 %k) {
b0:
  %empty = icmp eq i16 %n, 0
  br i1 %empty, label %out, label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  %hit = icmp eq i16 %i, %k
  br i1 %hit, label %out, label %latch

latch:
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  %r = phi i16 [ 100, %b0 ], [ %i, %head ], [ %next, %latch ]
  ret i16 %r
}
";
    let (before, after) = simplify(text);
    assert_eq!(shape(&before, "head"), (1, 1, false));
    assert_eq!(shape(&after, "head"), (1, 1, true));
    let inputs: &[&[u128]] = &[&[0, 0], &[3, 0], &[3, 2], &[3, 7], &[5, 4]];
    assert_eq!(results(&after, inputs), results(&before, inputs));
}

#[test]
fn nested_loops_are_both_simplified_and_compute_the_same() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %outer

outer:
  %i = phi i16 [ 0, %b0 ], [ %i1, %inner ]
  %s = phi i16 [ 0, %b0 ], [ %t1, %inner ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %inner, label %out

inner:
  %j = phi i16 [ 0, %outer ], [ %j1, %inner ]
  %t = phi i16 [ %s, %outer ], [ %t1, %inner ]
  %t1 = add i16 %t, %j
  %j1 = add i16 %j, 1
  %i1 = add i16 %i, 1
  %again = icmp ult i16 %j1, %i
  br i1 %again, label %inner, label %outer

out:
  ret i16 %s
}
";
    let (before, after) = simplify(text);
    assert_eq!(shape(&after, "inner"), (1, 1, true));
    assert_eq!(shape(&after, "outer"), (1, 1, true));
    let inputs: &[&[u128]] = &[&[0], &[1], &[3], &[6]];
    assert_eq!(results(&after, inputs), results(&before, inputs));
}

#[test]
fn a_loop_already_in_simplified_form_is_left_alone() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ]
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  ret i16 %next
}
";
    let mut module = parsed(text);
    let (_, function) = module.function_mut("f").expect("@f");
    let before = function.clone();
    assert!(!simplified(function));
    assert_eq!(*function, before);
}

#[test]
fn an_irreducible_cycle_leaves_every_loop_alone() {
    let text = "define void @f(i1 %c, i1 %d) {
b0:
  br i1 %c, label %head, label %a

head:
  br i1 %d, label %head, label %b

a:
  br i1 %d, label %b, label %out

b:
  br i1 %c, label %a, label %out

out:
  ret void
}
";
    let mut module = parsed(text);
    let (_, function) = module.function_mut("f").expect("@f");
    let before = function.clone();
    assert!(!simplified(function), "the self loop at %head would otherwise get a preheader");
    assert_eq!(*function, before);
}

#[test]
fn a_function_without_loops_is_left_alone() {
    let mut module = parsed(
        "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %r = phi i16 [ 1, %b0 ], [ 2, %b1 ]
  ret i16 %r
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    let before = function.clone();
    assert!(!simplified(function));
    assert_eq!(*function, before);
}

#[test]
fn a_header_that_is_one_of_two_latches_gets_one_latch_and_the_same_results() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ], [ %skip, %body ]
  %next = add i16 %i, 1
  %small = icmp ult i16 %next, 4
  br i1 %small, label %head, label %body

body:
  %skip = add i16 %i, 3
  %more = icmp ult i16 %skip, %n
  br i1 %more, label %head, label %out

out:
  ret i16 %skip
}
";
    let (before, after) = simplify(text);
    assert_eq!(shape(&before, "head").1, 2);
    assert_eq!(shape(&after, "head"), (1, 1, true));
    let inputs: &[&[u128]] = &[&[0], &[5], &[9], &[20]];
    assert_eq!(results(&after, inputs), results(&before, inputs));
}

/// Each function of the rich-MIR corpus: the latch count of its loops
/// before and after it is simplified, and whether simplifying again changed
/// it. Each simplified module must verify.
fn corpus_simplified() -> Vec<(String, Vec<usize>, Vec<usize>, bool)> {
    let mut out = Vec::new();
    for (name, mut module) in llrm_analysis::testing::corpus() {
        let names: Vec<String> = module
            .functions()
            .filter(|(_, _, one)| one.entry().is_some())
            .filter_map(|(_, global, _)| global.name.owned())
            .collect();
        for function_name in names {
            let (_, function) = module.function_mut(&function_name).unwrap();
            // A loop whose header is a landing pad (an error handler that
            // retries) keeps its latches: the pin cannot insert a backedge
            // to an EH pad (`insertUniqueBackedgeBlock` asserts).
            let latches = |function: &llrm_mir::module::Function| {
                loops::loops(&cfg::graph(function), None)
                    .iter()
                    .filter(|one| !crate::edges::starts_with_landing_pad(function, cfg::block(one.header)))
                    .map(|one| one.latches.len())
                    .collect::<Vec<_>>()
            };
            let before = latches(function);
            simplified(function);
            let after = latches(function);
            let again = simplified(function);
            out.push((format!("{name}/@{function_name}"), before, after, again));
        }
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{name}");
    }
    out
}

/// Generalizes the BC corpus's two loopsimplify tests: MDL_ANGLEMOD's
/// adjacent loops once never reached a fixed point, and PITSNAP's timer loop
/// had two back edges. Every corpus loop ends with one latch, and a second
/// run changes nothing.
#[test]
fn every_corpus_loop_ends_with_one_latch_at_a_fixed_point() {
    let results = corpus_simplified();
    assert!(
        results.iter().any(|(_, before, _, _)| before.iter().any(|&latches| latches > 1)),
        "the corpus has a loop with two latches"
    );
    for (name, _, after, again) in results {
        assert!(after.iter().all(|&latches| latches == 1), "{name}: {after:?}");
        assert!(!again, "{name}: a second run changed it");
    }
}

/// Every loop was tried on a copy of the whole function and its graph rebuilt,
/// though it needed nothing: 39% of compiling 100 sequential loops, and the
/// cost of `mir lsr` grew with the square of them (#556). A loop already in the
/// form makes no copy.
#[test]
fn loops_already_in_simplified_form_make_no_copy_of_the_function() {
    let loops = 30;
    let mut text = String::from("define i16 @f(i16 %x) {\nentry:\n  br label %p0\n");
    for at in 0..loops {
        let next = if at + 1 == loops { "done".to_owned() } else { format!("p{}", at + 1) };
        let before = if at == 0 { "p0".to_owned() } else { format!("h{}", at - 1) };
        let _ = before;
        text += &format!(
            "p{at}:\n  br label %h{at}\nh{at}:\n  %i{at} = phi i16 [ 0, %p{at} ], [ %n{at}, %b{at} ]\n  %c{at} = icmp slt i16 %i{at}, %x\n  br i1 %c{at}, label %b{at}, label %{next}\nb{at}:\n  %n{at} = add i16 %i{at}, 1\n  br label %h{at}\n"
        );
    }
    text += "done:\n  ret i16 0\n}\n";
    let mut module = parsed(&text);
    let function = module.named("f").expect("@f");
    let changed = {
        let llrm_mir::GlobalKind::Function(function) = &mut module.globals[function.0 as usize].kind else {
            panic!("a function")
        };
        simplified(function)
    };
    assert!(!changed, "a loop in the form was changed");
}

/// The blocks a simplified loop gets stand where the pin lays them out: the
/// preheader right after an outside predecessor, the dedicated exit right
/// before the exit block (`placeSplitBlockCarefully`,
/// `SplitBlockPredecessors`). They stood at the end of the function, which
/// JumpThreading, visiting blocks in layout order, read as another loop shape
/// (x_list's list-sum loop kept two pointer phis).
#[test]
fn test_a_new_preheader_and_exit_stand_next_to_the_blocks_they_serve() {
    let mut module = parsed(
        "define void @f(i1 %c, i1 %d) {
entry:
  br i1 %c, label %head, label %side

side:
  br i1 %d, label %head, label %guard

head:
  br i1 %d, label %head, label %guard

guard:
  ret void
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    assert!(simplified(function));
    let order: Vec<&str> =
        function.layout().iter().map(|&one| function.block(one).name.as_deref().unwrap_or("new")).collect();
    assert_eq!(order, ["entry", "side", "new", "head", "new", "guard"]);
}

/// The first loop's exit is the second loop's header, so one new block is
/// both its dedicated exit and the second's preheader. The pin works the last
/// top-level loop first, so that block is made as a preheader and put right
/// after the first loop's header: `entry h1 new b1 h2 b2 exit`. Taken in
/// program order it was made as an exit, stood before `h2`, and bench
/// x_transpose's late JumpThreading saw another layout than the pin's.
#[test]
fn a_loops_are_simplified_last_top_level_loop_first_so_the_preheader_follows_the_first_header() {
    let mut module = parsed(
        "define i32 @f(i32 %n) {
entry:
  br label %h1

h1:
  %i = phi i32 [ 0, %entry ], [ %i1, %b1 ]
  %c1 = icmp slt i32 %i, %n
  br i1 %c1, label %b1, label %h2

b1:
  %i1 = add i32 %i, 1
  br label %h1

h2:
  %j = phi i32 [ 0, %h1 ], [ %j1, %b2 ]
  %c2 = icmp slt i32 %j, %n
  br i1 %c2, label %b2, label %exit

b2:
  %j1 = add i32 %j, 1
  br label %h2

exit:
  ret i32 %j
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    assert!(simplified(function));
    let order: Vec<&str> =
        function.layout().iter().map(|&one| function.block(one).name.as_deref().unwrap_or("new")).collect();
    assert_eq!(order, ["entry", "h1", "new", "b1", "h2", "b2", "exit"]);
}

/// `n` self-loops in a row, each entered from two blocks.
fn entered_twice(n: usize) -> String {
    let mut text = String::from("define void @f(i1 %c) {\nb0:\n  br label %e0\n\n");
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("e{}", k + 1) };
        text += &format!(
            "e{k}:\n  br i1 %c, label %h{k}, label %m{k}\n\nm{k}:\n  br label %h{k}\n\nh{k}:\n  br i1 %c, label %h{k}, label %x{k}\n\nx{k}:\n  br label %{next}\n\n"
        );
    }
    text + "end:\n  ret void\n}\n"
}

/// `simplifyLoop` found each loop's parent by asking every loop whether it held
/// the header, and a header's place by a walk of the function: 128 loops in a
/// row cost the pass 82 M instructions on the generated nests, the work
/// growing with the square of the loops. A block is looked at once for each
/// loop around it.
#[test]
fn finding_the_parents_of_loops_looks_at_each_block_once_per_loop_around_it() {
    let probes = |n: usize| {
        let before = super::parent_probes();
        let (_, after) = simplify(&entered_twice(n));
        assert_eq!(shape(&after, "h0"), (1, 1, true));
        super::parent_probes() - before
    };
    let (a, b) = (probes(32), probes(128));
    assert!(b * 100 <= a * 450, "32 loops: {a}, 128 loops: {b}");
}
