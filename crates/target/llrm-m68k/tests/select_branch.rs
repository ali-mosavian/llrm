//! SimplifyCFG folded gcd's `if (a > b) a -= b; else b -= a;` into two
//! selects, and with no conditional move each lowered to a branch and a
//! move on top of both subtracts: 44 B and 236 clk at -O2 on the 68000
//! against gcc's 28 B (-Os: 44 against 26). Each select is now priced as that
//! pair.
//! `gcd_raw.ll` is clang's output with no passes, what `llrm-cc` hands the
//! driver (the bench's own `gcd.ll` has the selects written in).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const GCD: &str = include_str!("fixtures/gcd_raw.ll");

fn instructions(asm: &str) -> usize {
    asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

#[test]
fn gcds_branches_stay_at_every_level() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(GCD, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(instructions(&out) <= 14, "{cpu}: the selects came back ({} insns):\n{out}", instructions(&out));
        }
    }
}
