//! A block nothing branches to, on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// The default of a `switch` that is `unreachable` made an empty machine block that falls into the join and has no
/// predecessor. Shrink wrapping read the dominator tree of the blocks the entry reaches and panicked ("B must be in
/// the tree", `domtree.rs:895`): `runtime/qb/dev/host/video.c` (`blend`) did not compile at any level above O0 on
/// arm64. The pin deletes such blocks right after selection (`UnreachableMachineBlockElim`).
#[test]
fn a_switch_with_an_unreachable_default_compiles() {
    // `runtime/qb/dev/host/video.c`'s `blend`, from clang -Oz.
    let ir = format!(
        "{LAYOUT}@width = external global i32\n@pixel_mask = external global i32\n@pixels = external global ptr\n\
define hidden fastcc void @blend(i32 %0, i32 %1, i32 %2, i32 %3) minsize optsize nounwind {{\n  %5 = load ptr, ptr @pixels\n  %6 = zext i32 %1 to i64\n  \
%7 = load i32, ptr @width\n  %8 = zext nneg i32 %7 to i64\n  %9 = mul nuw nsw i64 %8, %6\n  \
%10 = getelementptr inbounds nuw i8, ptr %5, i64 %9\n  %11 = zext i32 %0 to i64\n  \
%12 = getelementptr inbounds nuw i8, ptr %10, i64 %11\n  %13 = load i8, ptr %12\n  %14 = zext i8 %13 to i32\n  \
%15 = load i32, ptr @pixel_mask\n  %16 = and i32 %15, %2\n  %17 = and i32 %3, 3\n  \
switch i32 %17, label %24 [\n    i32 1, label %18\n    i32 2, label %20\n    i32 3, label %22\n    i32 0, label %25\n  ]\n\
18:\n  %19 = and i32 %16, %14\n  br label %25\n\
20:\n  %21 = or i32 %16, %14\n  br label %25\n\
22:\n  %23 = xor i32 %16, %14\n  br label %25\n\
24:\n  unreachable\n\
25:\n  %26 = phi i32 [ %19, %18 ], [ %21, %20 ], [ %23, %22 ], [ %16, %4 ]\n  %27 = and i32 %26, %15\n  \
%28 = trunc nuw i32 %27 to i8\n  store i8 %28, ptr %12\n  ret void\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("strb"), "{asm}");
}
