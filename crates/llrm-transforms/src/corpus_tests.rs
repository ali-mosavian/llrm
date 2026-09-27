//! Each pass over the rich-MIR corpus: every module still verifies after
//! every run, and running again soon changes nothing.

use llrm_analysis::testing::corpus;
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use crate::interprocedural::function_mut;
use crate::testing::bodies;

/// Runs `pass` on each body of each corpus module until it reports no
/// change, verifying after every run. How many bodies it changed.
fn settles(pass: &str, mut run: impl FnMut(&mut Module, llrm_mir::context::GlobalId) -> bool) -> usize {
    let mut changed = 0;
    for (name, mut module) in corpus() {
        for id in bodies(&module) {
            let mut rounds = 0;
            while run(&mut module, id) {
                assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{pass}: {name}");
                rounds += 1;
                assert!(rounds < 64, "{pass}: {name} does not settle");
            }
            changed += usize::from(rounds > 0);
        }
    }
    changed
}

#[test]
fn dead_keeps_every_corpus_module_verifying_and_settles() {
    let changed = settles("dead", |module, id| {
        let callees = llrm_mir::memory::callees(module);
        let (context, function) = function_mut(module, id);
        crate::dead::dead(context, &callees, function)
    });
    assert!(changed > 0, "the corpus has dead code");
}

#[test]
fn fold_keeps_every_corpus_module_verifying_and_settles() {
    let changed = settles("fold", |module, id| {
        let (layout, outer) = (llrm_analysis::testing::layout(module), Outer::of(module, None));
        let (context, function) = function_mut(module, id);
        crate::fold::folded(context, &layout, function, &outer)
    });
    assert!(changed > 0, "the corpus has something to fold");
}

#[test]
fn decide_keeps_every_corpus_module_verifying_and_settles() {
    let changed = settles("decide", |module, id| {
        let (layout, outer) = (llrm_analysis::testing::layout(module), Outer::of(module, None));
        let (context, function) = function_mut(module, id);
        crate::decide::decided(context, &layout, function, &outer).expect("decides")
    });
    assert!(changed > 0, "the corpus has branches to decide");
}

#[test]
fn algebraic_keeps_every_corpus_module_verifying_and_settles() {
    let changed = settles("algebraic", |module, id| {
        let layout = llrm_analysis::testing::layout(module);
        let mut analyses = llrm_mir::passes::Analyses::new(std::rc::Rc::new(Outer::of(module, None)));
        let (context, function) = function_mut(module, id);
        crate::algebraic::simplified(context, &layout, function, &mut analyses)
    });
    assert!(changed > 0, "the corpus has identities to simplify");
}
