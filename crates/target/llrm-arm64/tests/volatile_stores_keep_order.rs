//! Guard for the m68k release-store fix (chain combine and `isSimple`): two
//! volatile stores keep their program order, the data before the flag.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_volatile_data_store_stays_before_the_volatile_flag_store() {
    let source = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@data = dso_local global [4 x i32] zeroinitializer, align 4
@flag = dso_local global i32 0, align 4
define void @publish(i32 %x) {
  store volatile i32 %x, ptr getelementptr inbounds (i8, ptr @data, i64 4), align 4
  store volatile i32 1, ptr @flag, align 4
  ret void
}
"#;
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let asm = driver::assembly(source, Platform::Linux, level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        let stores: Vec<&str> =
            asm.lines().map(str::trim).filter(|line| line.starts_with("str") && !line.contains("sp")).collect();
        assert_eq!(stores.len(), 2, "{level:?}:\n{asm}");
        assert!(stores[0].contains("w0"), "{level:?}: the flag is stored first:\n{asm}");
    }
}
