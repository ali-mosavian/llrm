//! Adapted from llrm-core's `optimize/profit_tests.rs`: the `profit`-only
//! cases of `tests/test_unroll_budget.py`, each body now MIR text. A body
//! of one block has nothing live out.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;

use super::{OperationCosts, spill_risk};

fn risk(text: &str, capacity: i64) -> Option<i64> {
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let live_out = BTreeMap::from([(cfg::id(function.entry().unwrap()), BTreeSet::new())]);
    let costs = OperationCosts { load: 10, store: 10, ..OperationCosts::default() };
    spill_risk(&module.context, function, &costs, capacity, None, &live_out)
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
