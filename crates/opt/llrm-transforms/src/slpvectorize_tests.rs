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

/// jpeg_fixed/jpeg_single -Os: SLP made `store <2 x i8> <66, 77>` (a bitmap
/// magic) that llc refuses (`vector with no register class`); a vector under
/// 64 bits is not priced, so the stores stay scalar.
#[test]
fn constant_stores_that_make_less_than_a_d_register_stay_scalar() {
    let text = run(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
@bmp = global [4 x i8] zeroinitializer\n\
define void @f() {\nentry:\n  store i8 66, ptr @bmp, align 1\n  store i8 77, ptr getelementptr inbounds (i8, ptr @bmp, i64 1), align 1\n  \
store i16 1, ptr getelementptr inbounds (i8, ptr @bmp, i64 2), align 2\n  ret void\n}\n",
    );
    assert_eq!(text.matches("x i8>").count() + text.matches("x i16>").count(), 0, "{text}");
}

const INTERLEAVED: &str = include_str!("../tests/slp/interleaved_object_stores.ll");

/// matmul -O2 (arm64): the stores of `a` and of `b` alternate, and ours took
/// any store between a window's own as a dependence, so `a` got four-lane
/// windows and `b` stayed scalar (943 executed instructions against clang's
/// 840); the pin's scheduler orders stores to different objects freely and
/// makes one `<8 x i16>` store of each.
#[test]
fn stores_to_another_object_between_a_window_do_not_split_it() {
    let text = run(INTERLEAVED);
    assert_eq!(text.matches("store <8 x i16>").count(), 2, "{text}");
    assert_eq!(text.matches("store i16").count(), 0, "{text}");
}

/// x86 takes the SLP vectorizer's store chains too: the machine of the
/// loop-vectorizer's x86 tests, with an SSE2 target's costs.
struct X86At(std::rc::Rc<llrm_x86::tti::X86>);

impl llrm_mir::target::Machine for X86At {
    fn has_vector_mir(&self) -> bool {
        true
    }

    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        Some(self.0.clone())
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn foreign_span(
        &self,
        a: (i64, i64),
        b: (i64, i64),
        c: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(a, b, c)
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }

    fn registers(&self) -> i64 {
        llrm_mir::target::Neutral.registers()
    }

    fn call_registers(&self) -> i64 {
        llrm_mir::target::Neutral.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }
}

/// fill_nest -O3 (x86_64): the 64 stores of `t[y][x] = 4660` a row holds (the
/// inner loop unrolled) stayed 64 scalar stores, 16 `mov qword` a trip after
/// the DAG's merging, while the pin's SLP makes eight `store <8 x i16> splat
/// (i16 4660)`: `words` ran a scalar loop of 4,096 stores and the nest cost the
/// 450 threshold its bytes (4,193 executed against clang 20's 3,681).
#[test]
fn constant_stores_become_vector_stores_on_x86_64() {
    use llrm_mir::tti::Tti as _;
    let tti = llrm_x86::tti::X86::of_cpu("x86-64").unwrap();
    assert!(tti.vector_values(), "x86-64 holds SSE2 vectors");
    let mut module = parsed(CONST_STORES);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(SlpVectorize);
    manager.run_module(&mut module, std::rc::Rc::new(X86At(std::rc::Rc::new(tti)))).unwrap();
    let text = crate::testing::printed(&module);
    let splat = format!("store <8 x i16> <{}>", vec!["i16 1824"; 8].join(", "));
    assert_eq!(text.matches(&splat).count(), 10, "{text}");
    assert_eq!(text.matches("store i16").count(), 0, "{text}");
}

/// Constant stores and summed loads written with no `align`: the vector store
/// and load carried none either, which reads as the vector's own 16 bytes
/// (`movaps` through an 8-aligned global: a segfault on m64 huffman_decode
/// -O3). The pin writes `StoreInst::getAlign` and `LoadInst::getAlign`, the ABI
/// alignment of the scalar: 2.
#[test]
fn a_vector_access_made_of_scalars_with_no_align_takes_the_scalars_abi_alignment() {
    let stores = run(&CONST_STORES.replace(", align 2", ""));
    assert!(stores.contains("store <8 x i16> <") && !stores.contains("store i16"), "{stores}");
    assert!(
        stores.lines().filter(|line| line.contains("store <8 x i16>")).all(|line| line.ends_with(", align 2")),
        "{stores}"
    );
    let loads = run(&TILE.replace(", align 2", ""));
    assert!(loads.contains("load <32 x i16>, ptr") && !loads.contains("load i16"), "{loads}");
    assert!(loads.lines().filter(|line| line.contains("load <")).all(|line| line.contains(", align 2")), "{loads}");
}

/// huffman_decode m64 -O3 stored `<0, 0, 2, 5>` as `xorps; movups`: a window
/// whose first lane is zero became a zero vector, whatever the other lanes
/// held, and `valptr[1][3..5]` read 0 instead of 2 and 5 (a wrong value).
#[test]
fn a_constant_window_that_starts_with_zero_keeps_its_other_lanes() {
    let text = run("@g = global [8 x i32] zeroinitializer, align 16
define void @f() {
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @g, i64 8), align 4
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @g, i64 12), align 4
  store i32 2, ptr getelementptr inbounds nuw (i8, ptr @g, i64 16), align 4
  store i32 5, ptr getelementptr inbounds nuw (i8, ptr @g, i64 20), align 4
  ret void
}
");
    assert!(text.contains("store <4 x i32> <i32 0, i32 0, i32 2, i32 5>"), "{text}");
}

const NBODY_PHIS: &str = include_str!("../tests/slp/nbody_phis.ll");

/// bench/nbody -O2/-O3 (arm64) kept 16 loop-carried doubles as scalar phis
/// (16,441 instructions; clang 12,845): the pin seeds the SLP vectorizer with
/// the loop's phis and makes `<2 x double>` phis of them.
#[test]
fn loop_carried_doubles_become_vector_phis() {
    let text = run(NBODY_PHIS);
    assert!(text.matches("phi <2 x double>").count() >= 3, "{text}");
    // The fourth, from the `insertelement` chain that builds a vector.
    assert_eq!(text.matches("phi <2 x double>").count(), 4, "{text}");
}

/// `fmul a[i], k` in one lane and `fmul k, a[i+1]` in the next kept two
/// gathers (the operands stood as written): `VLOperands::reorder` puts the
/// loads in one column and the splat `k` in the other.
#[test]
fn commutative_lanes_with_swapped_operands_are_put_in_one_order() {
    let text = run(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define void @f(ptr %d, ptr %a, double %k) {\n\
  %a1 = getelementptr double, ptr %a, i64 1\n\
  %x0 = load double, ptr %a, align 8\n\
  %x1 = load double, ptr %a1, align 8\n\
  %m0 = fmul double %x0, %k\n\
  %m1 = fmul double %k, %x1\n\
  %d1 = getelementptr double, ptr %d, i64 1\n\
  store double %m0, ptr %d, align 8\n\
  store double %m1, ptr %d1, align 8\n\
  ret void\n\
}\n",
    );
    assert!(text.contains("load <2 x double>"), "{text}");
    assert!(text.contains("fmul <2 x double>"), "{text}");
    assert!(!text.contains("fmul double"), "{text}");
}

/// A `<2 x double>` built by two `insertelement`s of `fdiv`s of loads stayed
/// two scalar loads, two `fdiv`s and two inserts: the buildvector is seeded
/// (`vectorizeInsertElementInst`) and becomes one vector `fdiv` of a vector
/// load.
#[test]
fn a_buildvector_of_divisions_becomes_a_vector_division() {
    let text = run(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define <2 x double> @f(ptr %a, ptr %b) {\n\
  %a1 = getelementptr double, ptr %a, i64 1\n\
  %b1 = getelementptr double, ptr %b, i64 1\n\
  %x0 = load double, ptr %a, align 8\n\
  %x1 = load double, ptr %a1, align 8\n\
  %y0 = load double, ptr %b, align 8\n\
  %y1 = load double, ptr %b1, align 8\n\
  %d0 = fdiv double %x0, %y0\n\
  %d1 = fdiv double %x1, %y1\n\
  %v0 = insertelement <2 x double> poison, double %d0, i64 0\n\
  %v1 = insertelement <2 x double> %v0, double %d1, i64 1\n\
  ret <2 x double> %v1\n\
}\n",
    );
    assert!(text.contains("fdiv <2 x double>"), "{text}");
    assert!(!text.contains("insertelement"), "{text}");
    assert!(!text.contains("fdiv double"), "{text}");
}

const FIELD_COPIES: &str = include_str!("../tests/slp/interleaved_field_copies.ll");

/// `y->a = x->a; y->b = x->b; ...` of one struct: each load stands after the
/// store before it, and the pin (`opt -passes=slp-vectorizer`) makes two
/// `<2 x i64>` copies, TBAA saying the fields are apart. arm64 kept 4 scalar
/// `ldr`/`str` pairs, where the pin has `ldr q`/`str q` (QuickrBasic
/// `dbug_eval` -Os: +100 B). A copy under one tag may read what it stored, and
/// stays.
#[test]
fn field_copies_one_load_after_each_store_become_vector_copies_where_tbaa_separates_them() {
    let text = run(FIELD_COPIES);
    let save = text.split("define void @save_same_tag").next().unwrap();
    assert_eq!(save.matches("load <2 x i64>").count(), 2, "{text}");
    assert_eq!(save.matches("load i64").count(), 0, "{text}");
    let same = text.split("define void @save_same_tag").nth(1).unwrap();
    assert_eq!(same.matches("load i64").count(), 2, "{same}");
}

const BUTTERFLY: &str = include_str!("../tests/slp/butterfly_seeds.ll");

/// stft_fixed -Os (arm64): four `sdiv x, 2` stored to four different arrays
/// stayed scalar (+60% modelled clocks against the pin's clang): no store
/// chain exists, the pin's `vectorizeOnceUsedSeeds` makes one `<4 x i64>`
/// `sdiv` of the four sums (`-Rpass`: cost -2, tree size 5).
#[test]
fn four_halvings_stored_to_four_arrays_are_one_vector_sdiv() {
    let text = run(BUTTERFLY);
    assert_eq!(text.matches("sdiv <4 x i64>").count(), 1, "{text}");
    assert_eq!(text.matches("sdiv i64").count(), 0, "{text}");
    assert_eq!(text.matches("extractelement <4 x i64>").count(), 4, "{text}");
}

const RDX_TREE: &str = include_str!("../tests/slp/rdx_tree_sdiv.ll");

/// jpeg_fixed (arm64): `s = 131584 + p[0]*181/512 + p[8]*c1/1024 + ...` stayed
/// a scalar chain of eight `sdiv`s; the pin's `tryToReduce` builds a tree over
/// the reduced values (`buildTree(VL, IgnoreList)`): the eight `sdiv`s become
/// one `sdiv <8 x i64>` by the non-uniform power-of-two divisors, their
/// dividends gathered from the scalar `mul`s, and one
/// `llvm.vector.reduce.add.v8i64` ("Vectorized horizontal reduction with cost
/// -4 and with tree size 5").
#[test]
fn a_reduction_of_sdivs_over_gathered_muls_becomes_a_vector_sdiv_and_one_reduction() {
    let text = run(RDX_TREE);
    assert!(text.contains("sdiv <8 x i64>"), "{text}");
    assert!(text.contains("@llvm.vector.reduce.add.v8i64"), "{text}");
    assert_eq!(text.matches("sdiv i64").count(), 1, "only the divide after the sum stays: {text}");
    assert_eq!(text.matches("load i64").count(), 8, "the loads stay scalar: {text}");
}

/// loop_interchange (arm64 -O2): a `trunc` of one value over four lanes was
/// priced 3, as four bulk inserts of `getScalarizationOverhead`; the pin
/// prices a splat one `getVectorInstrCost` insert into poison (free at lane
/// 0) and a broadcast: 1. The loop's x32 scale turned 2 extra into the
/// tree not made (+16 B). Four distinct values do pay every lane: 4 x 2.
#[test]
fn a_splat_gather_is_a_free_insert_and_a_broadcast_and_distinct_values_pay_each_lane() {
    use llrm_mir::module::{Operand, ValueId};
    use llrm_mir::tti::Ty;
    let tti = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let value = |id| Operand::Value(ValueId(id));
    let cost = |lanes: &[Operand]| {
        crate::slpgather::gather_cost(&tti, llrm_mir::tti::CostKind::RecipThroughput, Ty::Int(16), lanes, &[])
    };
    assert_eq!(cost(&[value(1), value(1), value(1), value(1)]), Some(1));
    assert_eq!(cost(&[value(1), value(2), value(3), value(4)]), Some(8));
}

/// stft_fixed -Os (arm64): four distinct i64 lanes gathered at code size were
/// "not priced" (the target's overhead hook returned none), so no `optsize`
/// seed tree could be costed; the pin pays 1 a lane (`VecInstCost`).
#[test]
fn an_integer_gather_at_code_size_pays_one_a_lane() {
    use llrm_mir::module::{Operand, ValueId};
    use llrm_mir::tti::{CostKind, Ty};
    let tti = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let lanes: Vec<_> = (1..=4).map(|id| Operand::Value(ValueId(id))).collect();
    assert_eq!(crate::slpgather::gather_cost(&tti, CostKind::CodeSize, Ty::Int(64), &lanes, &[]), Some(4));
}

/// jpeg_fixed -O2 (arm64): every seed attempt made its own scalar-evolution
/// session (dominators, loops, expressions of the whole function), +22%
/// compile instructions over 101 attempts. One session serves the attempts
/// until a tree is emitted: the butterfly's four-seed window and its single
/// emit take at most two.
#[test]
fn seed_attempts_share_one_scalar_evolution_session_until_a_tree_is_emitted() {
    let before = llrm_analysis::scev::analysis::sessions_made();
    run(BUTTERFLY);
    let made = llrm_analysis::scev::analysis::sessions_made() - before;
    assert!(made <= 3, "{made} sessions");
}

const TRIM_STORE_TREE: &str = include_str!("../tests/slp/trim_store_tree.ll");

/// object_extent_loop (arm64 -O2): `store (fmul (uitofp (trunc i)), 0.5)` over
/// two lanes of a 12-trip loop stayed scalar (+23). The pin's `getTreeCost`
/// trims the `uitofp` subtree to a gather of its two doubles (2 inserts, lane
/// 0 free): total -12, a `fmul <2 x double>` and one vector store.
#[test]
fn a_store_tree_with_an_unprofitable_subtree_is_trimmed_not_dropped() {
    let text = run(TRIM_STORE_TREE);
    assert!(text.contains("fmul <2 x double>"), "{text}");
    assert!(text.contains("store <2 x double>"), "{text}");
}
