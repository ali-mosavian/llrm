//! The six CPUs of the family through the driver. `-mcpu=68040` was refused
//! (`choose 68000 or 68020`), so nothing could be compiled or priced for the
//! 68010, 68030, 68040 or 68060.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
    define i32 @f(i32 %a, i32 %b) {\n  %r = mul i32 %a, %b\n  %s = add i32 %r, %a\n  ret i32 %s\n}\n";

fn text(name: &str) -> String {
    assembly(IR, cpu_of(name).unwrap_or_else(|why| panic!("{why}")), Level::O2).expect("compiles")
}

/// A 32-bit multiply is one `muls.l` from the 68020 on and a library call
/// before it: the 68030, 68040 and 68060 selected nothing at all.
#[test]
fn the_68030_and_up_select_as_the_68020() {
    let reference = text("68020");
    assert!(reference.contains("muls.l") || reference.contains("mulu.l"), "{reference}");
    for name in ["68030", "68040", "68060"] {
        assert_eq!(text(name), reference, "-mcpu={name}");
    }
}

/// The 68010 selects as the 68000 (the pin's `AtLeastM68010` gates only the
/// move from the codes).
#[test]
fn the_68010_selects_as_the_68000() {
    assert_eq!(text("68010"), text("68000"));
}
