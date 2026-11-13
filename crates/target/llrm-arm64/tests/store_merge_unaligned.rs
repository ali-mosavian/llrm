//! Eight halfword constants stored from byte 4 are two 64-bit stores, as the
//! pin makes them (`allowsMisalignedMemoryAccesses`): the merge kept to
//! multiples of its size made a word, a doubleword and a word (jpeg -O2: 16
//! bytes over what the pin's store merge gives).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn constant_halfwords_off_a_doubleword_are_doubleword_stores() {
    let asm =
        driver::assembly(include_str!("data/store_merge_unaligned.ll"), Platform::Linux, Level::O2).expect("lowers");
    let stores =
        asm.lines().map(str::trim).filter(|line| line.starts_with("str ") || line.starts_with("stur ")).count();
    assert_eq!(stores, 2, "{asm}");
    assert!(!asm.contains("strh"), "{asm}");
}

/// Constants stored between stores of another array were kept apart by the
/// merge pass (a store ended the span): 22 halfword stores where the 16 of the
/// other array are all that is left once the constants are wide (matmul -O2:
/// 1500 bytes, pin 1412).
#[test]
fn constants_between_stores_of_another_object_are_merged() {
    let asm =
        driver::assembly(include_str!("data/store_merge_interleaved.ll"), Platform::Linux, Level::O2).expect("lowers");
    let halfwords = asm.lines().map(str::trim).filter(|line| line.starts_with("strh ")).count();
    assert_eq!(halfwords, 16, "{asm}");
}
