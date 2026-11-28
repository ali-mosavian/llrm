//! `llvm.fshl`/`llvm.fshr` had no price on a target that expands them, so a
//! loop holding one was "not computed" to the unroller and never unrolled: 6502
//! `rotate_small` -O2 kept both loops, 5861 B against 912 B once they fold. The
//! pin prices them by `BasicTTIImplBase::getIntrinsicInstrCost`: an `or`, a
//! `shl` and a `lshr`; a variable amount adds a `sub` and a modulo (`and` at a
//! power-of-two width); a funnel (X != Y, not a rotate) adds a compare and a
//! select.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::module::BlockId;
use llrm_mir::parse;
use llrm_mir::tti::{Basic, Cost, CostKind};

/// The price of the `fshl` call in `@f(i8 %x, i8 %y, i8 %n)` with the operands
/// `args`.
fn priced(args: &str) -> Cost {
    let source = format!(
        "declare i8 @llvm.fshl.i8(i8, i8, i8)\ndefine i8 @f(i8 %x, i8 %y, i8 %n) {{\nb:\n  %r = call i8 @llvm.fshl.i8({args})\n  ret i8 %r\n}}\n"
    );
    let module = parse::module(&source).expect("parses");
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("f");
    let call = function.iter_block(BlockId(0)).next().expect("the call");
    instruction_cost(
        &Basic,
        &module.context,
        &DataLayout::default(),
        &module.globals,
        function,
        call,
        CostKind::CodeSize,
    )
}

#[test]
fn a_rotate_by_a_variable_costs_or_shl_lshr_sub_and() {
    assert_eq!(priced("i8 %x, i8 %x, i8 %n"), Cost::of(5));
}

#[test]
fn a_funnel_by_a_variable_adds_the_compare_and_select() {
    assert_eq!(priced("i8 %x, i8 %y, i8 %n"), Cost::of(7));
}

#[test]
fn a_funnel_by_a_constant_costs_or_shl_lshr() {
    assert_eq!(priced("i8 %x, i8 %y, i8 3"), Cost::of(3));
}
