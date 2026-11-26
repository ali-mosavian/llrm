//! A loop over six arrays lost its `(An)+` walks to the walk limit of 5 and
//! indexed them (`(a,d.l)`, 14 clocks for 8): particle `advance` ran 1,486,352
//! clocks at 68000 -O2 against gcc's 922,096. Six address registers are always
//! there for the walks.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SIX: &str = include_str!("../data/six_array_walks.ll");

#[test]
fn a_loop_over_six_arrays_walks_each_by_post_increment() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let asm = assembly(SIX, cpu_of(cpu).unwrap(), level).expect("compiles");
            let indexed: Vec<&str> =
                asm.lines().filter(|line| (0..8).any(|d| line.contains(&format!(",%d{d})")))).collect();
            assert!(indexed.is_empty(), "{cpu} {level:?}: indexed accesses {indexed:?}:\n{asm}");
            let walked = (0..7).filter(|a| asm.contains(&format!("(%a{a})+"))).count();
            assert_eq!(walked, 6, "{cpu} {level:?}:\n{asm}");
        }
    }
}
