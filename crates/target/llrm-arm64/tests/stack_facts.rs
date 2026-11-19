//! The datalayout names the stack alignment the frames keep (`S`), and equals
//! the pin's string for the triple
//! (`llrm_target::testing::assert_stack_facts`).

use llrm_arm64::description::Arm64;
use llrm_arm64::machine::Platform;
use llrm_target::Target;

/// What the pin's clang states for each triple (d1106deb71cc, 2026-11-09).
const LINUX: &str = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32";
const DARWIN: &str = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32";

#[test]
fn the_datalayout_states_the_stack_alignment_the_frames_keep() {
    for (name, platform, pin) in [("arm64-linux", Platform::Linux, LINUX), ("arm64-macos", Platform::Darwin, DARWIN)] {
        llrm_target::testing::assert_stack_facts(
            name,
            platform.datalayout(),
            Arm64.stack_alignment() as u64,
            Some(pin),
        );
    }
}
