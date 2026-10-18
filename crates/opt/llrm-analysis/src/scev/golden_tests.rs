//! Golden cases: expressions built with the pinned LLVM's `ScalarEvolution`
//! (`tools/scev/oracle.cpp`, `tools/scev/golden.py` writes the file), and the
//! text each printed. The folds here must print the same.

use llrm_mir::dominators::DominatorTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::valuetracking::AssumptionCache;

use super::print::Labels;
use super::{Cx, ScalarEvolution};

const FIXTURE: &str = include_str!("../../../../../tools/scev/fixture.ll");
const GOLDEN: &str = include_str!("../../../../../tools/scev/golden.txt");

/// The cases of `GOLDEN`, each against a fresh `ScalarEvolution`, as
/// (expression, LLVM's text, ours).
fn run() -> Vec<(String, String, String)> {
    let module = llrm_mir::parse::module(FIXTURE).expect("the fixture parses");
    let (_, _, function) =
        module.functions().find(|(_, _, function)| !function.layout().is_empty()).expect("a function");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let tree = DominatorTree::new(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx { module: &module, function, layout: &layout, loops: &loops, tree: &tree, assumptions: &assumptions };
    let labels = Labels::of(function);
    GOLDEN
        .lines()
        .map(|line| {
            let (expression, want) = line.split_once("\t").expect("expression TAB text");
            let got = ScalarEvolution::new().eval_line(&cx, &labels, expression);
            (expression.to_owned(), want.to_owned(), got)
        })
        .collect()
}

/// Every case prints as LLVM prints it. Before the folds matched, an `add`
/// of a `zext` and a constant printed `<nuw>` where LLVM printed `<nuw><nsw>`
/// (the signed range of a `zext` was the full set).
#[test]
fn test_every_golden_case_prints_as_the_pinned_llvm_does() {
    let cases = run();
    let wrong: Vec<_> = cases.iter().filter(|(_, want, got)| want != got).collect();
    assert!(wrong.is_empty(), "{} of {} differ, first: {:?}", wrong.len(), cases.len(), wrong.first());
}

/// The premise: the cases are not all one answer, and they hold the shapes
/// the folds are about.
#[test]
fn test_the_golden_cases_hold_many_answers_and_the_shapes_of_the_folds() {
    let cases = run();
    let answers: std::collections::HashSet<&str> = cases.iter().map(|(_, want, _)| want.as_str()).collect();
    assert!(answers.len() > 100, "{} distinct answers", answers.len());
    for shape in ["<nuw><nsw>", "umin_seq", "{", ",+,", "/u", "zext", "sext", "trunc", "<u nuw>", "smax", "umin"] {
        assert!(cases.iter().any(|(_, want, _)| want.contains(shape)), "no case prints {shape}");
    }
}
