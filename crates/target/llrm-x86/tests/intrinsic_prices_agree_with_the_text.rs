//! Every row of the price text is found and priced as written; a name the
//! text lacks is invalid.

use llrm_mir::tti::{Cost, CostKind, Tti};
use llrm_x86::tti::X86;

const PRICES: &str = include_str!("../src/tti_intrinsics.txt");

fn kind_of(name: &str) -> CostKind {
    match name {
        "throughput" => CostKind::RecipThroughput,
        "latency" => CostKind::Latency,
        "code-size" => CostKind::CodeSize,
        "size-latency" => CostKind::SizeAndLatency,
        other => panic!("{other}"),
    }
}

#[test]
fn test_every_price_row_is_found_and_a_missing_name_is_not() {
    let mut checked = 0;
    for line in PRICES.lines().filter(|line| !line.starts_with('#')) {
        let [profile, kind, name, cost] = line.split(' ').collect::<Vec<_>>()[..] else { panic!("{line}") };
        let x86 = X86::of_cpu(if profile == "sse2" { "x86-64" } else { "i486" }).unwrap();
        let want = cost.parse().map_or(Cost::INVALID, Cost::of);
        assert_eq!(x86.intrinsic_cost(name, None, &[], kind_of(kind)), want, "{line}");
        checked += 1;
    }
    assert!(checked > 1000, "{checked} rows");
    let x86 = X86::of_cpu("i486").unwrap();
    assert_eq!(x86.intrinsic_cost("llvm.no.such.thing", None, &[], CostKind::CodeSize), Cost::INVALID);
}
