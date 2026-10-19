use llrm_mir::datalayout::DataLayout;

use super::*;

/// Each heuristic (pointer, zero, floating point, metadata against
/// unreachable, noreturn and cold blocks, a loop's exit, an unlikely successor,
/// a switch's weights) gives LLVM's probabilities to the bit:
/// `opt -passes='print<branch-prob>'` on `bpi_golden.ll` at d1106deb71cc
/// wrote `bpi_golden.txt`.
#[test]
fn test_probabilities_are_llvms_on_every_heuristic() {
    let module = llrm_mir::parse::module(include_str!("bpi_golden.ll")).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::default();
    let mut ours = String::new();
    for (_, global, function) in module.functions().filter(|(_, _, one)| !one.is_declaration()) {
        let bpi = BranchProbabilityInfo::of(
            Module {
                context: &module.context,
                declarations: &module.globals,
                metadata: &module.metadata,
                layout: &layout,
            },
            function,
        );
        let name = global.name.as_deref().unwrap_or_default();
        ours += &format!("Printing analysis 'Branch Probability Analysis' for function '{name}':\n");
        ours += &bpi.print(function, &|block| function.block(block).name.clone().unwrap_or_default());
    }
    let llvm = include_str!("bpi_golden.txt");
    for (want, got) in llvm.lines().zip(ours.lines()) {
        assert_eq!(got, want, "\nours:\n{ours}");
    }
    assert_eq!(ours.lines().count(), llvm.lines().count(), "\nours:\n{ours}");
}
