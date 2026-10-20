use super::*;

/// The frequencies are LLVM's to the integer: `opt -passes='print<block-freq>'`
/// on `blockfreq_golden.ll` at d1106deb71cc.
#[test]
fn test_frequencies_are_llvms_where_branch_weights_fix_the_odds() {
    let text = include_str!("blockfreq_golden.ll");
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let llvm: &[(&str, &[(&str, u64)])] = &[
        (
            "diamond",
            &[
                ("entry", 18014398509481984),
                ("a", 13510798882111488),
                ("b", 4503599627370496),
                ("join", 18014398509481984),
            ],
        ),
        (
            "nest",
            &[
                ("entry", 112589990789120),
                ("outer", 1125899906842624),
                ("inner", 18014398509481983),
                ("latch", 1125899906842624),
                ("exit", 112589990789120),
            ],
        ),
    ];
    for (name, blocks) in llvm {
        let (_, _, function) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some(*name)).expect("the function");
        let layout = llrm_mir::datalayout::DataLayout::default();
        let bpi = crate::bpi::BranchProbabilityInfo::of(
            crate::bpi::Module {
                context: &module.context,
                declarations: &module.globals,
                metadata: &module.metadata,
                layout: &layout,
            },
            function,
        );
        let bfi = BlockFrequencies::of_bpi(function, &bpi);
        for (block, want) in *blocks {
            let id = *function
                .layout()
                .iter()
                .find(|&&one| function.block(one).name.as_deref() == Some(*block))
                .expect("the block");
            assert_eq!(bfi.get_block_freq(id).get_frequency(), *want, "{name}/{block}");
        }
    }
}
