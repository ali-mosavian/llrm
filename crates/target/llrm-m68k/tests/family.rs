//! The pin selects on three gates only: the 68000, the 68010 and later, the
//! 68020 and later. `-mcpu=68040` was an error ("unknown m68k cpu"); the 68010
//! and the 68030, 68040 and 68060 now select as the 68000 and the 68020.

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::kernels::DATALAYOUT;

/// A 32-bit multiply: a library call on the 68000 and 68010, `muls.l` from the
/// 68020 on.
const KERNEL: &str = "define i32 @f(i32 %a, i32 %b) {
entry:
  %m = mul i32 %a, %b
  ret i32 %m
}
";

fn on(cpu: &str) -> String {
    let cpu = cpu_of(cpu).unwrap_or_else(|why| panic!("{cpu}: {why}"));
    assembly(&format!("target datalayout = \"{DATALAYOUT}\"\n{KERNEL}"), cpu, Level::O2).expect("assembly")
}

#[test]
fn the_kernel_tells_the_68000_from_the_68020() {
    assert_ne!(on("68000"), on("68020"));
}

#[test]
fn the_68010_selects_as_the_68000() {
    assert_eq!(on("68010"), on("68000"));
}

#[test]
fn the_68030_68040_and_68060_select_as_the_68020() {
    let reference = on("68020");
    for cpu in ["68030", "68040", "68060"] {
        assert_eq!(on(cpu), reference, "{cpu}");
    }
}
