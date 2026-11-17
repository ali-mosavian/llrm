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
    assert!(stores.lines().filter(|line| line.contains("store <8 x i16>")).all(|line| line.ends_with(", align 2")), "{stores}");
    let loads = run(&TILE.replace(", align 2", ""));
    assert!(loads.contains("load <32 x i16>, ptr") && !loads.contains("load i16"), "{loads}");
    assert!(loads.lines().filter(|line| line.contains("load <")).all(|line| line.contains(", align 2")), "{loads}");
}
