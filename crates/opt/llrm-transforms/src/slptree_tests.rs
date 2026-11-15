//! `slptree` on the arm64 cost model, against the pin
//! (`opt -mtriple=aarch64 -passes=slp-vectorizer`).

use super::super::slpvectorize::SlpVectorize;
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

/// Four `fmuladd`s of one scalar and four consecutive loads, plus a constant,
/// stored to four consecutive floats: the pin makes one `<4 x float>` load, a
/// splat of the scalar, one `llvm.fmuladd.v4f32`, one `fadd` and one store.
/// jpeg_single -O3's `idct` stayed scalar (8.35M instructions against
/// clang's 7.09M) for want of this tree.
#[test]
fn four_fmuladds_of_a_splat_and_consecutive_loads_become_one_vector_chain() {
    let text = run(include_str!("../tests/slp/fmuladd_splat4.ll"));
    assert!(text.contains("load <4 x float>"), "{text}");
    assert!(text.contains("@llvm.fmuladd.v4f32"), "{text}");
    assert!(text.contains("store <4 x float>"), "{text}");
    assert_eq!(text.matches("store float").count(), 0, "{text}");
    assert_eq!(text.matches("@llvm.fmuladd.f32").count(), 1, "{text}");
}

/// The `idct` row: eight `fmuladd`, `fadd`, `fcmp ugt`, `fptosi`, `trunc` and
/// `select` chains stored as eight bytes; the pin makes one `<8 x float>`
/// chain and one `store <8 x i8>`.
#[test]
fn eight_clamped_bytes_of_fmuladds_become_one_vector_store() {
    let text = run(include_str!("../tests/slp/idct_clamp_row8.ll"));
    assert!(text.contains("load <8 x float>"), "{text}");
    assert!(text.contains("@llvm.fmuladd.v8f32"), "{text}");
    assert!(text.contains("fcmp ugt <8 x float>"), "{text}");
    assert!(text.contains("select <8 x i1>"), "{text}");
    assert!(text.contains("store <8 x i8>"), "{text}");
    assert_eq!(text.matches("store i8").count(), 0, "{text}");
}

/// The coefficient loads stand in the block before the loop (LICM hoisted
/// them); the pin makes their `<8 x float>` load there and the rest in the
/// body. jpeg_single -O3's tree took only loads of its own block: eight
/// gathers at 12 each, +7 for the tree, and the idct stayed scalar.
#[test]
fn loads_in_an_earlier_block_are_one_vector_load_there() {
    let text = run(include_str!("../tests/slp/idct_clamp_row8_two_blocks.ll"));
    let (entry, body) = text.split_once("body:").expect("two blocks");
    assert!(entry.contains("load <8 x float>"), "{text}");
    assert!(body.contains("@llvm.fmuladd.v8f32"), "{text}");
    assert!(body.contains("store <8 x i8>"), "{text}");
    assert_eq!(text.matches("load float").count(), 1, "{text}");
}
