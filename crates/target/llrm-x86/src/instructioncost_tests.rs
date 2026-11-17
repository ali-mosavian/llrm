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

/// The `insertelement`, broadcast `shufflevector` and vector `fmuladd` of a
/// loop were `Invalid` at code size: the unroller read a loop vectorized to `<2
/// x double>` as "size not computed" and kept its 6 trips rolled
/// (object_extent_loop -O2, +6% executed instructions). The pin's
/// `print<cost-model>`: 0 for an insert into poison, 1 for the next, 1 for the
/// broadcast, 2 for the `fmuladd` (4 for throughput).
#[test]
fn a_vector_insert_broadcast_and_fmuladd_cost_what_the_pin_says() {
    let source = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
        declare <2 x double> @llvm.fmuladd.v2f64(<2 x double>, <2 x double>, <2 x double>)\n\
        define <2 x double> @f(double %a, double %b, <2 x double> %v) {\n\
        %i = insertelement <2 x double> poison, double %a, i64 0\n\
        %j = insertelement <2 x double> %i, double %b, i64 1\n\
        %s = shufflevector <2 x double> %i, <2 x double> poison, <2 x i32> zeroinitializer\n\
        %m = call <2 x double> @llvm.fmuladd.v2f64(<2 x double> %v, <2 x double> %s, <2 x double> %j)\n\
        ret <2 x double> %m\n}\n";
    let module = llrm_mir::parse::module(source).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a layout");
    let tuning = llrm_mir::target::Tuning { vector_float_costs: true, ..llrm_mir::target::Tuning::PIN };
    let x86 = X86::of_cpu("x86-64").expect("a cpu").with_tuning(tuning);
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    let insts: Vec<_> = function.layout().iter().flat_map(|&block| function.iter_block(block)).collect();
    for (kind, want) in [(CostKind::CodeSize, [0, 1, 1, 2]), (CostKind::RecipThroughput, [0, 1, 1, 4])] {
        let got: Vec<String> = insts[..4]
            .iter()
            .map(|&inst| {
                let cost = instruction_cost(&x86, &module.context, &layout, &module.globals, function, inst, kind);
                if cost.valid { cost.value.to_string() } else { "Invalid".to_owned() }
            })
            .collect();
        assert_eq!(got, want.map(|n| n.to_string()), "{kind:?}");
    }
}
