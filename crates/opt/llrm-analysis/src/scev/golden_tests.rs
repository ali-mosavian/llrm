//! Golden cases: expressions built with the pinned LLVM's `ScalarEvolution`
//! (`tools/scev/oracle.cpp`, `tools/scev/golden.py` writes the file), and the
//! text each printed. The folds here must print the same.

use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::valuetracking::AssumptionCache;

use super::print::Labels;
use super::{Cx, ScalarEvolution};

const FIXTURE: &str = include_str!("../../../../../tools/scev/fixture.ll");
const GOLDEN: &str = include_str!("../../../../../tools/scev/golden.txt");
const DIVERGENCES: &str = include_str!("../../../../../tools/scev/divergences.txt");

/// The cases of `file`, each against a fresh `ScalarEvolution`, as
/// (expression, the pinned LLVM's text, ours).
fn run_file(file: &str) -> Vec<(String, String, String)> {
    let module = llrm_mir::parse::module(FIXTURE).expect("the fixture parses");
    let (_, _, function) =
        module.functions().find(|(_, _, function)| !function.layout().is_empty()).expect("a function");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let labels = Labels::of(function);
    file.lines()
        .map(|line| {
            let expression = line.split('\t').next().expect("an expression");
            let want = line.split('\t').nth(1).expect("expression TAB text");
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
    let cases = run_file(GOLDEN);
    let wrong: Vec<_> = cases.iter().filter(|(_, want, got)| want != got).collect();
    assert!(wrong.is_empty(), "{} of {} differ, first: {:?}", wrong.len(), cases.len(), wrong.first());
}

/// The premise: the cases are not all one answer, and they hold the shapes
/// the folds are about.
#[test]
fn test_the_golden_cases_hold_many_answers_and_the_shapes_of_the_folds() {
    let cases = run_file(GOLDEN);
    let answers: llrm_support::hash::HashSet<&str> = cases.iter().map(|(_, want, _)| want.as_str()).collect();
    assert!(answers.len() > 100, "{} distinct answers", answers.len());
    for shape in ["<nuw><nsw>", "umin_seq", "{", ",+,", "/u", "zext", "sext", "trunc", "<u nuw>", "smax", "umin"] {
        assert!(cases.iter().any(|(_, want, _)| want.contains(shape)), "no case prints {shape}");
    }
}

/// DEVIATE 8. The pinned LLVM finds the recurrence it has just rewritten
/// with `Ops[i] == AddRec`, which compares the use bits too: held with a use
/// flag it is never found, and the search runs past the array (a segfault)
/// or, in `urem` of such a recurrence, leaves out the `-1` of the remainder
/// (`100 + udiv*addrec` for `100 - udiv*addrec`). Ours finds it by node.
/// `opt -passes='print<scalar-evolution>'` on the same IR, which has no use
/// flags, prints ours.
#[test]
fn test_where_the_pinned_llvm_is_wrong_the_folds_take_the_right_answer() {
    let cases = run_file(DIVERGENCES);
    for (expression, llvm, ours) in &cases {
        assert_ne!(llvm, ours, "{expression}: no longer a divergence, move it to the golden cases");
        let pinned = DIVERGENCES.lines().find(|line| line.starts_with(expression.as_str())).expect("the line");
        assert_eq!(pinned.split('\t').nth(2), Some(ours.as_str()), "{expression}");
        if expression.starts_with("(urem") {
            // The remainder subtracts the product: the negation is in the
            // answer.
            assert!(ours.contains("(-1 * "), "{expression}: the -1 was dropped: {ours}");
        }
    }
    for head in ["(add ", "(mul "] {
        assert!(cases.iter().any(|(e, _, _)| e.starts_with(head)), "no case reaches the {head}lookup");
    }
    let crashes = cases.iter().filter(|(_, llvm, _)| llvm == "SEGFAULT").count();
    assert!(crashes >= 2 && crashes < cases.len(), "premise: both a crash and a wrong answer, {crashes} crashes");
}
