//! Adapted from llrm-core's `optimize/profit_tests.rs`: the `profit`-only
//! cases of `tests/test_unroll_budget.py`, each body now MIR text. A body
//! of one block has nothing live out.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;

use llrm_support::hash::IndexMap;

use super::{OperationCosts, UNKNOWN_TRIPS, _frequencies, _loop_products, operation, proven_trips, r#static, spill_risk, weighted};

fn risk(text: &str, capacity: i64) -> Option<i64> {
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let costs = OperationCosts { load: 10, store: 10, ..OperationCosts::default() };
    let room = crate::spill::Room { registers: capacity, across_call: capacity, ..Default::default() };
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).expect("a layout");
    spill_risk(&module.context, &layout, function, &costs, room, &|_| capacity, &_frequencies(&module.context, &module.metadata, &module.globals, function, None).expect("frequencies"), &llrm_analysis::liveness::live(function)).map(|price| price / super::UNIT)
}

#[test]
fn test_pressure_prices_independent_spill_waves() {
    let waves = "declare void @use(i16, i16, i16)

define void @f() {
b0:
  %a0 = add i16 0, 0
  %a1 = add i16 1, 0
  %a2 = add i16 2, 0
  call void @use(i16 %a0, i16 %a1, i16 %a2)
  %b0 = add i16 0, 1
  %b1 = add i16 1, 1
  %b2 = add i16 2, 1
  call void @use(i16 %b0, i16 %b1, i16 %b2)
  ret void
}
";
    assert_eq!(risk(waves, 2), Some(40));
}

#[test]
fn test_integer_pressure_does_not_consume_x87_values() {
    let floating = "@g = global double 0.000000e+00

declare void @use(double, double, double)

define void @f() {
b0:
  %f1 = load double, ptr @g
  %f2 = load double, ptr @g
  %f3 = load double, ptr @g
  call void @use(double %f1, double %f2, double %f3)
  ret void
}
";
    assert_eq!(risk(floating, 1), Some(0));
}

fn module(text: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn function<'m>(module: &'m llrm_mir::module::Module) -> &'m llrm_mir::module::Function {
    module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f").2
}

/// `weighted` of @f with every price 1, and each latch named in `trips`.
fn work(text: &str, trips: &[(&str, i64)]) -> Option<i64> {
    let module = module(text);
    let f = function(&module);
    let latch = |name: &str| cfg::id(*f.layout().iter().find(|&&one| f.block(one).name.as_deref() == Some(name)).expect("a block"));
    let trips = trips.iter().map(|&(name, count)| (latch(name), count)).collect::<IndexMap<_, _>>();
    let callees = llrm_mir::memory::callees(&module);
    weighted(&module.context, &llrm_mir::datalayout::DataLayout::default(), f, &callees, &OperationCosts::default(), &_frequencies(&module.context, &module.metadata, &module.globals, f, Some(&trips))?).map(|total| total / super::UNIT)
}

/// Three priced instructions in a loop body of one block, one before and one after.
const COUNTED: &str = "define void @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ]
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  ret void
}
";

#[test]
fn a_loop_body_counts_32_times_unless_its_trips_are_known() {
    // The loop's heuristic odds, 31 in 32, as branchprob has them.
    assert_eq!(work(COUNTED, &[]), Some(1 + 4 * 32 + 1));
    assert_eq!(work(COUNTED, &[("head", 3)]), Some(1 + 4 * 3 + 1));
}

/// Profit's fallback named induction as the owner of trip counts once
/// ported: a loop it counts is weighted by that count, not 32.
#[test]
fn a_loop_induction_counts_is_weighted_by_its_count() {
    let layout = llrm_mir::datalayout::DataLayout::default();
    let trips = |text: &str| {
        let module = module(text);
        let unit = llrm_analysis::memory::Unit::of(&module, &layout, function(&module));
        let trips = proven_trips(&unit, &llrm_analysis::consts::known(&unit, None, None, None));
        let callees = llrm_mir::memory::callees(&module);
        (trips.len(), weighted(&module.context, &layout, function(&module), &callees, &OperationCosts::default(), &_frequencies(&module.context, &module.metadata, &module.globals, function(&module), Some(&trips)).unwrap()).map(|total| total / super::UNIT))
    };
    assert_eq!(trips(&COUNTED.replace("icmp ult i16 %next, %n", "icmp ult i16 %next, 3")), (1, Some(1 + 4 * 3 + 1)));
    assert_eq!(trips(COUNTED), (0, Some(1 + 4 * 32 + 1)));
}

#[test]
fn a_nested_loop_counts_for_every_enclosing_loop() {
    let text = "define void @f(i1 %c) {
b0:
  br label %outer

outer:
  br label %inner

inner:
  br i1 %c, label %inner, label %latch

latch:
  br i1 %c, label %outer, label %out

out:
  ret void
}
";
    assert_eq!(work(text, &[]), Some(1 + 32 + 32 * 32 + 32 + 1));
    assert_eq!(work(text, &[("inner", 2), ("latch", 3)]), Some(1 + 3 + 6 + 3 + 1));
}

#[test]
fn latches_of_one_loop_with_different_trip_counts_price_nothing() {
    let text = "define void @f(i1 %c) {
b0:
  br label %head

head:
  br i1 %c, label %left, label %right

left:
  br i1 %c, label %head, label %out

right:
  br i1 %c, label %head, label %out

out:
  ret void
}
";
    assert_eq!(work(text, &[("left", 2), ("right", 3)]), None);
    // The header twice; each arm the half of that.
    assert_eq!(work(text, &[("left", 2), ("right", 2)]), Some(1 + 2 + 1 + 1 + 1));
}

#[test]
fn an_unpriced_instruction_makes_the_body_unpriced() {
    let text = "define double @f(double %x) {
b0:
  %y = frem double %x, 3.000000e+00
  ret double %y
}
";
    let module = module(text);
    let callees = llrm_mir::memory::callees(&module);
    assert_eq!(r#static(&module.context, &llrm_mir::datalayout::DataLayout::default(), function(&module), &callees, &OperationCosts::default()), None);
    assert_eq!(work(text, &[]), None);
}

#[test]
fn a_fill_is_priced_by_its_cells_and_floating_accesses_by_their_own_price() {
    let text = "@d = global double 0.000000e+00

declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define void @f(ptr %p, i16 %n) {
b0:
  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %n, i1 false)
  %v = load double, ptr @d
  store double %v, ptr @d
  ret void
}
";
    let module = module(text);
    let f = function(&module);
    let callees = llrm_mir::memory::callees(&module);
    let costs = OperationCosts { fill: 5, fill_cell: 2, float_load: 7, float_store: 11, ..OperationCosts::default() };
    let prices = f.walk().map(|(_, one)| operation(&module.context, &llrm_mir::datalayout::DataLayout::default(), f, &callees, one, &costs)).collect::<Vec<_>>();
    assert_eq!(prices, [Some(5 + 6 * 2), Some(5 + UNKNOWN_TRIPS * 2), Some(7), Some(11), Some(1)]);
}

#[test]
fn values_that_fit_the_registers_risk_no_spill() {
    let fits = "declare void @use(i16, i16)

define void @f() {
b0:
  %a = add i16 0, 0
  %b = add i16 1, 0
  call void @use(i16 %a, i16 %b)
  ret void
}
";
    assert_eq!(risk(fits, 2), Some(0));
    assert_eq!(risk(fits, 1), Some(20), "one of them spills: a store and a load");
}

#[test]
fn a_frame_address_spills_as_cheaply_as_rebuilding_it() {
    let text = "declare void @use(ptr, i16)

define void @f() {
b0:
  %slot = alloca i16
  %a = add i16 0, 0
  call void @use(ptr %slot, i16 %a)
  ret void
}
";
    // The address is rebuilt at its one use for 1, not stored and reloaded for 20.
    assert_eq!(risk(text, 1), Some(1));
}

/// A GEP in a space whose index outruns its offset steps the selector: it is
/// priced as `carry`, not as the address it is for a far pointer. Free, a loop
/// over a huge array would look as cheap as one over a far one.
#[test]
fn test_a_displacement_that_carries_into_the_selector_costs_the_carry() {
    let layout = llrm_mir::datalayout::DataLayout::parse("e-p:16:16-p1:32:16:16:16-p3:32:16:16:32").expect("a layout");
    let price = |space: u32| {
        let text = format!("define void @f(ptr addrspace({space}) %p, i16 %i) {{\n  %q = getelementptr i16, ptr addrspace({space}) %p, i16 %i\n  ret void\n}}\n");
        let module = module(&text);
        let f = function(&module);
        let costs = OperationCosts { address: 2, carry: 9, ..OperationCosts::default() };
        let callees = llrm_mir::memory::callees(&module);
        f.walk().map(|(_, one)| operation(&module.context, &layout, f, &callees, one, &costs)).collect::<Vec<_>>()
    };
    assert_eq!(price(1)[0], Some(2));
    assert_eq!(price(3)[0], Some(9));
}

/// A branch inside a loop splits its trips: the arms run as the header's
/// odds share them, where the product model gave every block of the loop its
/// factor (the old `_loop_products`, which gvn and lsr still price on, #202, #203).
#[test]
fn test_a_branch_in_a_loop_splits_the_frequency_the_products_do_not() {
    let text = "define void @f(i16 %n, i1 %c) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %join ]
  br i1 %c, label %left, label %right

left:
  br label %join

right:
  br label %join

join:
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  ret void
}
";
    let module = module(text);
    let f = function(&module);
    let named = |name: &str| cfg::id(*f.layout().iter().find(|&&one| f.block(one).name.as_deref() == Some(name)).expect("a block"));
    let products = _loop_products(f, None).expect("products");
    assert_eq!((products[&named("head")], products[&named("left")], products[&named("right")]), (10, 10, 10));
    let odds = _frequencies(&module.context, &module.metadata, &module.globals, f, None).expect("frequencies");
    assert_eq!(odds[&named("head")], 32 * super::UNIT, "{odds:?}");
    assert!(odds[&named("left")] + odds[&named("right")] <= odds[&named("head")] + 2 && odds[&named("left")] < odds[&named("head")], "{odds:?}");
}

/// A cold arm priced as whole executions, floor 1, weighed what the code
/// before it did; and a branch's arms each weighed the whole loop's 32 rather
/// than their share. Now a block's weight is its share of the entry's, in
/// `UNIT`ths: the arm to a block ending in `unreachable` weighs one.
#[test]
fn a_block_that_is_all_but_never_reached_weighs_next_to_nothing() {
    let text = "@g = global i16 0

declare void @abort()

define void @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 7
  br i1 %c, label %cold, label %b2

cold:
  call void @abort()
  unreachable

b2:
  ret void
}
";
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    let (_, _, f) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let found = _frequencies(&module.context, &module.metadata, &module.globals, f, None).expect("frequencies");
    let at = |name: &str| *found.get(&cfg::id(f.layout().iter().copied().find(|&one| f.block(one).name.as_deref() == Some(name)).expect("a block"))).expect("a frequency");
    assert_eq!(at("b0"), super::UNIT);
    assert!(at("cold") < super::UNIT / 100, "the cold arm: {}", at("cold"));
    assert!(at("cold") >= 1, "never below one unit");
}

/// A switch was priced as one branch whatever its cases, so inlining a 10-case `pl_view_of` into
/// QCport's viewmdl.c looked free and grew it by 192 bytes.
#[test]
fn test_a_switch_costs_a_compare_and_a_jump_for_each_case() {
    let price = |cases: usize| {
        let arms = (0..cases).map(|case| format!("i16 {case}, label %b2 ")).collect::<String>();
        let text = format!("define void @f(i16 %x) {{\nb1:\n  switch i16 %x, label %b2 [{arms}]\nb2:\n  ret void\n}}\n");
        let module = module(&text);
        let f = function(&module);
        let costs = OperationCosts { branch: 3, add: 2, ..OperationCosts::default() };
        let callees = llrm_mir::memory::callees(&module);
        let layout = llrm_mir::datalayout::DataLayout::default();
        f.walk().map(|(_, one)| operation(&module.context, &layout, f, &callees, one, &costs)).next().flatten()
    };
    assert_eq!((price(0), price(1), price(10)), (Some(3), Some(8), Some(53)));
}

/// A select had no price, so a body holding one was unpriced and never a candidate to inline:
/// MID$'s length (a min and a max) kept its far call.
#[test]
fn a_select_is_priced() {
    let module = crate::testing::parsed("define i16 @f(i16 %a, i16 %b) {\nb1:\n  %c = icmp slt i16 %a, %b\n  %s = select i1 %c, i16 %a, i16 %b\n  ret i16 %s\n}\n");
    let (_, _, function) = module.functions().next().unwrap();
    let costs = OperationCosts { branch: 3, r#move: 2, ..OperationCosts::default() };
    let layout = llrm_mir::datalayout::DataLayout::default();
    let select = function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Select).unwrap();
    assert_eq!(operation(&module.context, &layout, function, &Default::default(), select, &costs), Some(5));
}
