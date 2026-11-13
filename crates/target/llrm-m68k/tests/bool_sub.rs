//! `s += p[i] > t` took `slt; and.l #1; add.l`: the compare's zero extension
//! is a mask (6 B) and its sign extension one `extb.l` (2 B) at 68020, `ext.w;
//! ext.l` (4 B) at 68000, with `sub` taking the other sign (countgt -Os 44 B
//! against gcc's 34 at 68020). `countgt_raw.ll` is clang's output with no
//! passes, what `llrm-cc` hands the driver.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const COUNTGT: &str = include_str!("fixtures/countgt_raw.ll");

#[test]
fn a_counted_compare_is_subtracted_sign_extended_not_added_masked() {
    // the 68000's word is subtracted into an address register (`suba.w`, see
    // sext_acc.rs)
    for (cpu, extend, subtract) in [("68020", "extb.l", "sub.l"), ("68000", "ext.w", "suba.w")] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(COUNTGT, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("and.l\t#1"), "{cpu}: the compare is masked:\n{out}");
            assert!(out.contains(extend) && out.contains(subtract), "{cpu}: no {subtract} of {extend}:\n{out}");
        }
    }
}
