//! A loop whose header test was copied into the latch branches back with the
//! likelier edge, `cbnz top; b exit`, not `cbz exit; b top` (`optimizeBranches`
//! lost the copy's odds): recpow -O1 ran 1 extra jump per iteration, +7.7%
//! instructions over the pin. The input is recpow's `power` as clang -O0 writes
//! it (the IR #7275's first test used no longer reaches the copy).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_copied_header_test_branches_back_to_the_loop() {
    let asm = driver::assembly(include_str!("data/latch_branch.ll"), Platform::Linux, Level::O1).expect("lowers");
    let power = asm.split("power:").nth(1).unwrap_or("");
    assert!(power.contains("cbnz w1, .Lpower.2"), "{asm}");
}
