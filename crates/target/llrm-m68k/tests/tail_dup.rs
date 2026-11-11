//! `bsearch`'s loop ended both arms in a `bra` to one shared latch compare
//! (`cmp.l; ble`): a taken branch per trip down the low arm, 97 clk against
//! gcc's 84 at the 68040 (gcc copies the compare into both arms). The latch is
//! now copied into the arms that jump to it; a size build keeps one copy.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const BSEARCH: &str = include_str!("../bench/library/bsearch.ll");

fn compares(
    cpu: &str,
    level: Level,
) -> (usize, String) {
    let out = assembly(BSEARCH, cpu_of(cpu).unwrap(), level).expect("compiles");
    (out.matches("cmp.l\t%d1, %d2").count(), out)
}

#[test]
fn the_latch_compare_is_copied_into_both_arms_of_the_loop() {
    for cpu in ["68000", "68020", "68040"] {
        let (copies, out) = compares(cpu, Level::O2);
        assert_eq!(copies, 2, "{cpu}: the latch compare is not in both arms:\n{out}");
    }
}

#[test]
fn a_size_build_keeps_one_latch() {
    let (copies, out) = compares("68020", Level::Os);
    assert_eq!(copies, 1, "{out}");
}
