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

/// Four `fmul`s by `1/ya, 1/ya, 1/yb, 1/yb`: the pin makes the two `fdiv`s one
/// `<2 x float>` bundle and reads it with the reuse mask `<0,0,1,1>` (cost
/// -11). The tree took the repeated `fdiv` as a gather of four inserts.
#[test]
fn a_bundle_with_a_repeated_scalar_is_made_once_and_reused_by_a_shuffle() {
    let text = run(include_str!("../tests/slp/reuse_bundle.ll"));
    assert!(text.contains("fdiv <2 x float>"), "{text}");
    assert!(text.contains("<4 x i32> <i32 0, i32 0, i32 1, i32 1>"), "{text}");
    assert!(text.contains("fmul <4 x float>"), "{text}");
    assert_eq!(text.matches("fdiv float").count(), 0, "{text}");
}

/// Four `fmul`s by `a, a, b, b` (two arguments): the pin builds the two
/// distinct scalars once and shuffles them by `<0,0,1,1>` (cost -9, gather
/// 3). The tree inserted all four lanes.
#[test]
fn a_gather_of_repeated_scalars_inserts_each_once_and_shuffles() {
    let text = run(include_str!("../tests/slp/gather_reuse.ll"));
    assert_eq!(text.matches("insertelement").count(), 2, "{text}");
    assert!(text.contains("<4 x i32> <i32 0, i32 0, i32 1, i32 1>"), "{text}");
    assert!(text.contains("store <4 x float>"), "{text}");
}

/// Four `fneg`s of consecutive loads: the pin makes `fneg <4 x float>` (cost
/// -9). `fneg` was no bundle, so the stores stayed scalar.
#[test]
fn four_fnegs_of_consecutive_loads_become_one_vector_fneg() {
    let text = run(include_str!("../tests/slp/fneg4.ll"));
    assert!(text.contains("fneg <4 x float>"), "{text}");
    assert!(text.contains("store <4 x float>"), "{text}");
    assert_eq!(text.matches("fneg float").count(), 0, "{text}");
}

/// Four `extractelement`s of lanes 0..3 of one vector, multiplied by four
/// loaded constants: the pin uses the source vector as the bundle (cost
/// -18): one `fmul <4 x float>` of it, no extract. The extracts were four
/// gathered scalars, so the tree paid four inserts of them to rebuild the
/// vector it had already.
#[test]
fn extracts_of_each_lane_of_one_vector_use_the_vector() {
    let text = run(include_str!("../tests/slp/extract4.ll"));
    assert!(text.contains("fmul <4 x float> %v, %"), "{text}");
    assert_eq!(text.matches("extractelement").count(), 0, "{text}");
    assert_eq!(text.matches("insertelement").count(), 0, "{text}");
}
