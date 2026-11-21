//! `splitLargeGEPOffsets` on the arm64 addressing modes.

use crate::cgpaddr::CgpAddr;
use crate::testing::parsed;

fn run(source: &str) -> String {
    let mut module = parsed(source);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager
        .run_module(
            &mut module,
            std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux }),
        )
        .unwrap();
    crate::testing::printed(&module)
}

const FIELDS: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"

; s->a = v; s->b = v + 1; s->c = v + 2 with a at 11936, b at 12938 and c at 21000
define void @f(ptr %s, i16 %v) {
  %a = getelementptr i8, ptr %s, i64 11936
  store i16 %v, ptr %a, align 2
  %b = getelementptr i8, ptr %s, i64 12938
  store i16 %v, ptr %b, align 2
  %c = getelementptr i8, ptr %s, i64 21000
  store i16 %v, ptr %c, align 2
  ret void
}
";

/// QuickrBasic `dbug_eval` -Os: struct fields at 0x2ea0 and beyond, each a
/// `mov` of its own offset and a register-offset access, where the pin makes
/// one `add` of the first offset and reaches the rest from it. The field 9064
/// farther is out of reach of the first base (an `ldrh` reaches 8190) and gets
/// a base of its own.
#[test]
fn geps_of_one_base_at_large_offsets_share_a_new_base_and_a_far_one_starts_another() {
    let text = run(FIELDS);
    assert_eq!(text.matches("getelementptr i8, ptr %s, i64 11936").count(), 1, "{text}");
    assert_eq!(text.matches("i64 1002").count(), 1, "{text}");
    assert_eq!(text.matches("getelementptr i8, ptr %s, i64 21000").count(), 1, "{text}");
    assert!(!text.contains("i64 12938"), "{text}");
}
