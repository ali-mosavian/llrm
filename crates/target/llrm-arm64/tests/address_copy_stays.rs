//! A copy of a global's address stays a `mov`: the coalescer makes a
//! definition again only if it is `isAsCheapAsAMove`, and the `adrp`/`add`
//! pair (`MOVaddr`) is not. The pre-indexed stores' copies of the base were
//! rebuilt as `adrp` + `add` each (lru -O3: 82 instructions, pin 76).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_copy_of_a_global_address_is_not_rebuilt_from_the_symbol() {
    let asm = driver::assembly(include_str!("data/address_copy_stays.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert_eq!(asm.matches("adrp").count(), 1, "{asm}");
}
