//! `instruction_cost` against the pinned `opt -passes='print<cost-model>'`: the
//! price of every instruction of `instructioncost_corpus.ll`, per cpu and kind
//! (rows of `instructioncost_oracle.txt`, made by `instructioncost_oracle.py`).

use std::collections::BTreeMap;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::tti::{CostKind, Tti};

use crate::tti::X86;

const CORPUS: &str = include_str!("instructioncost_corpus.ll");
const ORACLE: &str = include_str!("instructioncost_oracle.txt");

fn kind_of(word: &str) -> CostKind {
    match word {
        "throughput" => CostKind::RecipThroughput,
        "latency" => CostKind::Latency,
        "code-size" => CostKind::CodeSize,
        "size-latency" => CostKind::SizeAndLatency,
        other => panic!("{other}"),
    }
}

/// Every row of the oracle is the translation's price, instruction by
/// instruction. The failure names the first rows that differ.
#[test]
fn test_every_instruction_costs_what_the_pinned_opt_says() {
    // MIR has no `target triple`; the oracle's opt needs it.
    let text: String = CORPUS.lines().filter(|line| !line.starts_with("target triple")).collect::<Vec<_>>().join("\n");
    let module = llrm_mir::parse::module(&text).expect("the corpus parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a layout");
    let mut expected = BTreeMap::<(String, String, String, usize), String>::new();
    for line in ORACLE.lines().filter(|line| !line.starts_with('#')) {
        let w: Vec<&str> = line.split(' ').collect();
        expected.insert((w[0].into(), w[1].into(), w[2].into(), w[3].parse().unwrap()), w[4].into());
    }
    let (mut checked, mut wrong) = (0, Vec::<String>::new());
    for ((cpu, kind, name, index), want) in &expected {
        let x86 = X86::of_cpu(cpu).expect("a cpu");
        let (_, _, function) = module
            .functions()
            .find(|(_, global, _)| global.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no @{name}"));
        let inst = function
            .layout()
            .iter()
            .flat_map(|&block| function.iter_block(block))
            .nth(*index)
            .unwrap_or_else(|| panic!("@{name} has no instruction {index}"));
        let cost = instruction_cost(&x86, &module.context, &layout, &module.globals, function, inst, kind_of(kind));
        let got = if cost.valid { cost.value.to_string() } else { "Invalid".to_owned() };
        checked += 1;
        if &got != want {
            wrong.push(format!("{cpu} {kind} @{name}#{index}: {got}, opt says {want}"));
        }
    }
    assert!(checked > 3000, "the oracle holds {checked} rows");
    assert!(
        wrong.is_empty(),
        "{} of {checked} differ:\n{}",
        wrong.len(),
        wrong.iter().take(25).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// The cost model prints no price for lifetime markers, so the oracle cannot
/// hold them: `getIntrinsicInstrCost` returns `TCC_Free` for them.
#[test]
fn test_lifetime_and_assume_are_free() {
    for name in ["llvm.assume", "llvm.lifetime.start.p0", "llvm.lifetime.end.p0"] {
        let cost = X86::of_cpu("i486").unwrap().intrinsic_cost(name, None, &[], CostKind::CodeSize);
        assert_eq!(cost, llrm_mir::tti::Cost::FREE, "{name}");
    }
}
