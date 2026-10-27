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

/// `opt -passes='print<branch-prob>'` on `bpi_switch_golden.ll` at d1106deb71cc
/// wrote `bpi_switch_golden.txt`: switches with a cold and a noreturn case, an
/// unreachable case and default, and `branch_weights`. SwitchCG reads these
/// edges by successor index.
#[test]
fn test_a_switchs_edges_are_llvms_with_unreachable_cold_and_weighted_cases() {
    let module =
        llrm_mir::parse::module(include_str!("bpi_switch_golden.ll")).unwrap_or_else(|error| panic!("{error}"));
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
        let labels = llrm_mir::print::block_labels(function);
        ours += &bpi.print(function, &|block| labels.get(&block).cloned().unwrap_or_default());
    }
    let llvm = include_str!("bpi_switch_golden.txt");
    assert_eq!(ours, llvm);
}

const TWO_WAY: &str = "define i32 @f(ptr %p, i32 %x) {
entry:
  %c = icmp eq ptr %p, null
  br i1 %c, label %yes, label %other

other:
  %d = icmp sgt i32 %x, 5
  br i1 %d, label %yes, label %no

yes:
  ret i32 1

no:
  ret i32 2
}
";

/// `setEdgeProbability`, `swapSuccEdgesProbabilities`, `copyEdgeProbabilities`
/// and `eraseBlock` on a two-way block the pointer heuristic gives 3/8 and
/// 5/8 (as the pin's `print<branch-prob>` does for `p == null`): passes that
/// change a branch update the probabilities through these.
#[test]
fn test_the_edges_of_a_block_are_set_swapped_copied_and_erased() {
    use llrm_mir::blockfrequency::BranchProbability;
    let module = llrm_mir::parse::module(TWO_WAY).expect("a module");
    let layout = DataLayout::default();
    let (_, _, function) = module.functions().next().expect("a function");
    let mut bpi = BranchProbabilityInfo::of(
        Module { context: &module.context, declarations: &module.globals, metadata: &module.metadata, layout: &layout },
        function,
    );
    let named =
        |name: &str| *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).unwrap();
    let (entry, other) = (named("entry"), named("other"));
    let (taken, kept) = (bpi.get_edge_probability(function, entry, 0), bpi.get_edge_probability(function, entry, 1));
    assert_eq!((taken, kept), (BranchProbability::new(3, 8), BranchProbability::new(5, 8)));
    // Swapped.
    bpi.swap_succ_edges_probabilities(function, entry);
    assert_eq!(
        (bpi.get_edge_probability(function, entry, 0), bpi.get_edge_probability(function, entry, 1)),
        (kept, taken)
    );
    // Set all at once.
    let (quarter, rest) = (BranchProbability::new(1, 4), BranchProbability::new(3, 4));
    bpi.set_edge_probability(function, other, &[quarter, rest]);
    assert_eq!(bpi.get_edge_probability(function, other, 1), rest);
    // Copied from one block to another.
    bpi.copy_edge_probabilities(function, other, entry);
    assert_eq!(
        (bpi.get_edge_probability(function, entry, 0), bpi.get_edge_probability(function, entry, 1)),
        (quarter, rest)
    );
    // Erased: 1/N again.
    bpi.erase_block(entry);
    assert_eq!(bpi.get_edge_probability(function, entry, 0), BranchProbability::new(1, 2));
}
