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
