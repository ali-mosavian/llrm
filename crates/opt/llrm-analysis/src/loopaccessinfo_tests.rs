//! `LoopAccessInfo::print` against the pinned LLVM's
//! `opt -passes='print<access-info>'` on `tests/laa/*.ll` (`*.expected` is
//! `tools/llvmport/laa_expected.sh` on the file).

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::valuetracking::AssumptionCache;

use super::LoopAccessInfo;
use crate::basicaa::BasicAA;
use crate::laa_testing::PinText;
use crate::memory::Unit;
use crate::scev::print::Labels;
use crate::scev::{Cx, ScalarEvolution};

/// `print<access-info>` of the one function `text` defines.
fn printed(text: &str) -> String {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, name, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("a function");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("a layout");
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
    let unit = Unit::of(&module, &layout, function);
    let labels = Labels::of(function);
    let text_of = PinText::of(&module, function);
    let mut se = ScalarEvolution::new();
    let mut out = format!("Printing analysis 'Loop Access Analysis' for function '{}':\n", name.name.owned().unwrap_or_default());

    // `appendLoopsToWorklist`, popped from the back: the last loop of the
    // preorder first.
    let mut headers: Vec<_> = loops.get_loops_in_preorder().into_iter().map(|one| one.get_header()).collect();
    headers.reverse();
    for header in headers {
        let label = labels.block_label(header);
        out += &format!("  {label}:\n");
        let mut aa = BasicAA::new(&unit);
        let info = LoopAccessInfo::new(&mut se, &cx, header, &mut aa, false, u32::MAX);
        out += &info.print(&se, &cx, &labels, &text_of, 4);
    }
    out
}

fn check(
    source: &str,
    expected: &str,
) {
    assert_eq!(printed(source), expected);
}

macro_rules! corpus {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check(
                    include_str!(concat!("../tests/laa/", stringify!($name), ".ll")),
                    include_str!(concat!("../tests/laa/", stringify!($name), ".expected")),
                );
            }
        )*
    };
}

corpus!(true_dependence, anti_dependence, unknownk, indirect, invariant);
