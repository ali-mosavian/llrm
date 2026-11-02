//! The machine the arm64 driver hands the mid-end has a cost model (a machine with none leaves the passes that
//! price by it on generic rules without any message).

use llrm_arm64::machine::{Arm64Machine, Platform};
use llrm_mir::target::Machine;

#[test]
fn every_platforms_machine_has_a_tti() {
    for platform in [Platform::Darwin, Platform::Linux] {
        assert!(Arm64Machine { platform }.tti().is_some(), "{platform:?}");
    }
}
