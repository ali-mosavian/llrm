//! At -Os a stack object's address is formed (`add xd, sp, #imm`) at each use,
//! as the pin does: `ADDXri` is as cheap as the copy it replaces. It was kept
//! in a register and copied (`mov x0, xN`) per call.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_frame_address_is_formed_at_each_call_at_os() {
    let asm =
        driver::assembly(include_str!("data/frame_address_remat_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let adds = asm.lines().filter(|l| l.contains("add") && l.contains("sp,")).count();
    assert!(adds >= 8, "{adds} frame address adds\n{asm}");
}
