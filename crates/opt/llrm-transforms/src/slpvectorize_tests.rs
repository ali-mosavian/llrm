//! `SlpVectorize` on the arm64 cost model, against the pin
//! (`opt -mtriple=aarch64 -passes=slp-vectorizer`).

use super::SlpVectorize;
use crate::testing::parsed;

fn run(source: &str) -> String {
    let mut module = parsed(source);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(SlpVectorize);
    manager
        .run_module(
            &mut module,
            std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux }),
        )
        .unwrap();
    crate::testing::printed(&module)
}

const CONST_STORES: &str = include_str!("../tests/slp/const_stores80.ll");

/// bench/scroll -O3 kept 80 `store i16 1824` scalar (`stp x,x` after DAG
/// merging, +0.18% clocks); the pin makes ten `store <8 x i16> splat (i16
/// 1824)` (the printer here spells the splat out).
#[test]
fn eighty_constant_stores_become_ten_vector_stores() {
    let text = run(CONST_STORES);
    let splat = format!("store <8 x i16> <{}>", vec!["i16 1824"; 8].join(", "));
    assert_eq!(text.matches(&splat).count(), 10, "{text}");
    assert_eq!(text.matches("store i16").count(), 0, "{text}");
}

const TILE: &str = include_str!("../tests/slp/tile_reduction.ll");

/// bench/tile -O3 (arm64) summed 40 `sext i16` loads of a row as 40 scalar
/// loads and adds; the pin makes a `<32 x i16>` and an `<8 x i16>` load, the
/// second added into the low lanes of the first, and one
/// `llvm.vector.reduce.add.v32i64`.
#[test]
fn forty_sign_extended_loads_summed_become_two_vector_loads_and_one_reduction() {
    let text = run(TILE);
    assert!(text.contains("load <32 x i16>"), "{text}");
    assert!(text.contains("load <8 x i16>"), "{text}");
    assert!(text.contains("@llvm.vector.reduce.add.v32i64"), "{text}");
    assert_eq!(text.matches("load i16").count(), 0, "{text}");
    // The pin's `removeInstructionsAndOperands`: the loads' addresses go too.
    assert_eq!(text.matches("getelementptr").count(), 3, "{text}");
    // The 40 values: one `rdx.op` of the eight lanes, and the phi's `op.rdx`.
    assert_eq!(text.matches("add <8 x i64>").count(), 1, "{text}");
    assert_eq!(text.matches("add i64 %rdx, %3").count(), 1, "{text}");
}
