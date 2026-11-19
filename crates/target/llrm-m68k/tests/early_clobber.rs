//! A zero-extending byte load whose result register was also its index
//! register read the index after the clearing `moveq` had overwritten it, so
//! the allocator could not share them and the copy loop (bench/kernels/copy)
//! spent `move.b` + `and.l #255` (10 B) where gcc spends `moveq #0` + `move.b`
//! (6 B). The load now early-clobbers its result.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const COPY: &str = include_str!("../bench/kernels/copy.ll");

#[test]
fn a_zero_extending_byte_load_masks_nothing_when_its_result_can_not_alias_the_index() {
    let out = assembly(COPY, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    assert!(!out.contains("and.l\t#255"), "the byte load is masked after the move:\n{out}");
}
