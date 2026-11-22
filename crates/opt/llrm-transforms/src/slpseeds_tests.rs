//! `vectorizeOnceUsedSeeds` on the arm64 cost model.

use crate::slpvectorize::SlpVectorize;
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

const HEAD: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// A division by a variable is a group of its own (`generateKeySubkey`: a
/// potentially expensive instruction is never batched).
#[test]
fn divisions_by_a_variable_stay_scalar() {
    let text = run(&format!(
        "{HEAD}define void @f(ptr %a, ptr %b, i64 %x, i64 %y, i64 %n, i64 %m) {{\nentry:\n  \
%q1 = sdiv i64 %x, %n\n  store i64 %q1, ptr %a, align 8\n  %q2 = sdiv i64 %y, %m\n  store i64 %q2, ptr %b, align 8\n  ret void\n}}\n"
    ));
    assert_eq!(text.matches("sdiv i64").count(), 2, "{text}");
}

/// The lanes of a standalone seed cannot read each other: the second sum reads
/// the first, so the two are not one vector.
#[test]
fn a_seed_fed_by_another_is_not_a_lane_of_it() {
    let text = run(&format!(
        "{HEAD}define void @f(ptr %a, ptr %b, i64 %x, i64 %y) {{\nentry:\n  \
%p = mul i64 %x, %y\n  %q1 = sdiv i64 %p, 2\n  store i64 %q1, ptr %a, align 8\n  %r = sdiv i64 %q1, 2\n  store i64 %r, ptr %b, align 8\n  ret void\n}}\n"
    ));
    assert!(!text.contains("<2 x i64>"), "{text}");
}
