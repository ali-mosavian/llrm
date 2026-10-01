//! Adapted from llrm-core's `optimize/profit_tests.rs`: the `profit`-only
//! cases of `tests/test_unroll_budget.py`, each body now MIR text. A body
//! of one block has nothing live out.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;

use llrm_support::hash::IndexMap;

use super::{OperationCosts, UNKNOWN_TRIPS, operation, proven_trips, r#static, spill_risk, weighted};

fn risk(text: &str, capacity: i64) -> Option<i64> {
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let costs = OperationCosts { load: 10, store: 10, ..OperationCosts::default() };
    let room = crate::spill::Room { registers: capacity, across_call: capacity, ..Default::default() };
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).expect("a layout");
    spill_risk(&module.context, &layout, function, &costs, room, &|_| capacity, None, &llrm_analysis::liveness::live(function))
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
    weighted(&module.context, &llrm_mir::datalayout::DataLayout::default(), f, &callees, &OperationCosts::default(), Some(&trips))
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
fn a_loop_body_counts_ten_times_unless_its_trips_are_known() {
    assert_eq!(work(COUNTED, &[]), Some(1 + 4 * 10 + 1));
    assert_eq!(work(COUNTED, &[("head", 3)]), Some(1 + 4 * 3 + 1));
}

/// Profit's fallback named induction as the owner of trip counts once
/// ported: a loop it counts is weighted by that count, not ten.
#[test]
fn a_loop_induction_counts_is_weighted_by_its_count() {
    let layout = llrm_mir::datalayout::DataLayout::default();
    let trips = |text: &str| {
        let module = module(text);
        let unit = llrm_analysis::memory::Unit::of(&module, &layout, function(&module));
        let trips = proven_trips(&unit, &llrm_analysis::consts::known(&unit, None, None, None));
        let callees = llrm_mir::memory::callees(&module);
        (trips.len(), weighted(&module.context, &layout, function(&module), &callees, &OperationCosts::default(), Some(&trips)))
    };
    assert_eq!(trips(&COUNTED.replace("icmp ult i16 %next, %n", "icmp ult i16 %next, 3")), (1, Some(1 + 4 * 3 + 1)));
    assert_eq!(trips(COUNTED), (0, Some(1 + 4 * 10 + 1)));
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
    assert_eq!(work(text, &[]), Some(1 + 10 + 100 + 10 + 1));
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
    assert_eq!(work(text, &[("left", 2), ("right", 2)]), Some(1 + 3 * 2 + 1));
}

#[test]
fn an_unpriced_instruction_makes_the_body_unpriced() {
    let text = "define i16 @f(i1 %c, i16 %x) {
b0:
  %y = select i1 %c, i16 %x, i16 0
  ret i16 %y
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
