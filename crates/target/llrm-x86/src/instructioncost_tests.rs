//! `instruction_cost` against the pinned `opt -passes='print<cost-model>'`: the
//! price of every instruction of `instructioncost_corpus.ll`, per cpu and kind
//! (rows of `instructioncost_oracle.txt`, made by `instructioncost_oracle.py`).

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::datalayout::DataLayout;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::tti::{CostKind, Tti};

use crate::tti::X86;

const CORPUS: &str = include_str!("instructioncost_corpus.ll");
const ORACLE: &str = include_str!("instructioncost_oracle.txt");
const MATRIX: &str = include_str!("instructioncost_matrix.ll");
const MATRIX_ORACLE: &str = include_str!("instructioncost_matrix.txt");

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
    let checked = priced_as_the_oracle_says(CORPUS, ORACLE);
    assert!(checked > 3000, "the oracle holds {checked} rows");
}

/// The matrix: every opcode at every type with variable and constant operands,
/// loads as cast sources, truncs stored. `sext (load)` was priced 1 where the
/// pin says 0 because no row held it.
#[test]
fn test_the_matrix_of_opcodes_and_types_costs_what_the_pinned_opt_says() {
    let checked = priced_as_the_oracle_says(MATRIX, MATRIX_ORACLE);
    assert!(checked > 6000, "the matrix holds {checked} rows");
}

/// The prices of `corpus` against `oracle`: the rows checked. Every
/// instruction of the corpus must have a row for each cpu and kind the oracle
/// names, so that an instruction the oracle skipped cannot hide.
fn priced_as_the_oracle_says(
    corpus: &str,
    oracle: &str,
) -> usize {
    // MIR has no `target triple`; the oracle's opt needs it.
    let text: String = corpus.lines().filter(|line| !line.starts_with("target triple")).collect::<Vec<_>>().join("\n");
    let module = llrm_mir::parse::module(&text).expect("the corpus parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a layout");
    let mut expected = BTreeMap::<(String, String, String, usize), String>::new();
    for line in oracle.lines().filter(|line| !line.starts_with('#')) {
        let w: Vec<&str> = line.split(' ').collect();
        expected.insert((w[0].into(), w[1].into(), w[2].into(), w[3].parse().unwrap()), w[4].into());
    }
    let combinations =
        expected.keys().map(|(cpu, kind, ..)| (cpu.clone(), kind.clone())).collect::<BTreeSet<_>>().len();
    let instructions: usize = module
        .functions()
        .filter(|(_, _, function)| !function.layout().is_empty())
        .map(|(_, _, function)| {
            function.layout().iter().map(|&block| function.iter_block(block).count()).sum::<usize>()
        })
        .sum();
    assert_eq!(expected.len(), instructions * combinations, "an instruction of the corpus has no oracle row");
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
    assert!(
        wrong.is_empty(),
        "{} of {checked} differ:\n{}",
        wrong.len(),
        wrong.iter().take(25).cloned().collect::<Vec<_>>().join("\n")
    );
    checked
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

/// `LoopUnrollPass` asks the cost of an instruction as if its operands were
/// the constants it simplified them to: a `getelementptr` with two variable
/// indices costs one (no addressing mode takes two scale registers), and
/// nothing once both are constants. llrm asked the instruction itself, so a
/// simulated address that folds was priced as one that does not, and the nest
/// of a stepped fill (8x8 longs) was priced out of its unroll.
#[test]
fn test_a_gep_asked_with_constant_operands_costs_what_they_make_it() {
    let module = llrm_mir::parse::module(
        "define void @f(ptr %p, i16 %k, i16 %j) {\nb0:\n  %q = getelementptr inbounds [4 x i32], ptr %p, i16 %k, i16 %j\n  store i32 0, ptr %q\n  ret void\n}\n",
    )
    .expect("parses");
    let layout = DataLayout::parse("e-p:16:16-i32:16-n8:16:32").expect("a layout");
    let (_, _, function) = module.functions().next().expect("@f");
    let gep = function.iter_block(function.entry().expect("entry")).next().expect("the gep");
    let x86 = X86::of_cpu("i486").expect("a cpu");
    let cost = |operands: &[llrm_mir::module::Operand]| {
        llrm_mir::instructioncost::instruction_cost_with(
            &x86,
            &module.context,
            &layout,
            &module.globals,
            function,
            gep,
            operands,
            CostKind::SizeAndLatency,
        )
        .value
    };
    let own = function.instruction(gep).operands.clone();
    let constant = |value| llrm_mir::module::Operand::Constant(module.context.int(module.context.types.int(16), value));
    assert_eq!((cost(&own), cost(&[own[0], constant(3), constant(1)])), (1, 0));
}
