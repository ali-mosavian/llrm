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
        let x86 = X86::of_cpu(match profile {
            "x64" => "x86-64",
            "sse2" => "core2",
            _ => "i486",
        })
        .unwrap();
        let want = cost.parse().map_or(Cost::INVALID, Cost::of);
        let got = match name.strip_suffix(".zp") {
            Some(call) => x86.zero_poison_intrinsic_cost(call, None, &[], kind_of(kind)),
            None => x86.intrinsic_cost(name, None, &[], kind_of(kind)),
        };
        assert_eq!(got, want, "{line}");
        checked += 1;
    }
    assert!(checked > 1000, "{checked} rows");
    let x86 = X86::of_cpu("i486").unwrap();
    assert_eq!(x86.intrinsic_cost("llvm.no.such.thing", None, &[], CostKind::CodeSize), Cost::INVALID);
}

/// The price text had rows for f32 and f64 only, so every call of an
/// `x86_fp80` intrinsic was invalid: the unroller could not size a loop that
/// held `llvm.fmuladd.f80` (`x * 3.7L - 20`), and x_ld_to_int kept its outer
/// loop and a `fiadd` of the counter in every inner trip, where the pin
/// unrolls the outer loop and adds a constant.
#[test]
fn test_an_extended_intrinsic_has_a_price() {
    let x86 = X86::of_cpu("i486").unwrap();
    for kind in [CostKind::CodeSize, CostKind::SizeAndLatency, CostKind::Latency, CostKind::RecipThroughput] {
        for name in ["llvm.fmuladd.f80", "llvm.sqrt.f80", "llvm.fabs.f80"] {
            assert!(x86.intrinsic_cost(name, None, &[], kind) != Cost::INVALID, "{name} {kind:?}");
        }
    }
}

/// The corpus scan (every target, O1 to Os) met these `Invalid`: a loop that
/// held one was never sized, so it was never unrolled, on any x86 target.
#[test]
fn test_every_intrinsic_the_corpus_meets_has_a_price() {
    let x86 = X86::of_cpu("i486").unwrap();
    for name in [
        "llvm.ctlz.i32",
        "llvm.bitreverse.i32",
        "llvm.sadd.sat.i32",
        "llvm.usub.sat.i64",
        "llvm.prefetch.p0",
        "llvm.va_start.p0",
        "llvm.abs.i128",
    ] {
        assert!(x86.intrinsic_cost(name, None, &[], CostKind::SizeAndLatency) != Cost::INVALID, "{name}");
    }
}

/// Long mode read the i486 rows: `ctlz.i64` cost 10 at x86-64 where the pin says 3, and 2 for the zero-poison call, so
/// SimplifyCFG refused to turn `x ? clzl(x) : 64` into a select (bitcount_u64).
#[test]
fn test_x86_64_prices_a_64_bit_count_from_its_own_rows() {
    let kind = CostKind::SizeAndLatency;
    let m64 = X86::of_cpu("x86-64").unwrap();
    let ty = Some(llrm_mir::tti::Ty::Int(64));
    assert_eq!(m64.intrinsic_cost("llvm.ctlz.i64", ty, &[], kind), Cost::of(3));
    assert_eq!(m64.zero_poison_intrinsic_cost("llvm.ctlz.i64", ty, &[], kind), Cost::of(2));
    let m32 = X86::of_cpu("core2").unwrap();
    assert_eq!(m32.intrinsic_cost("llvm.ctlz.i64", ty, &[], kind), Cost::of(10));
    assert_eq!(m32.zero_poison_intrinsic_cost("llvm.ctlz.i64", ty, &[], kind), Cost::of(4));
}
