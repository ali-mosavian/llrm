//! A `zext nneg` of a byte that a signed compare reads.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
@g = dso_local global i8 0
define i32 @f() {
  %v = load i8, ptr @g
  %z = zext nneg i8 %v to i32
  %c = icmp slt i8 %v, 5
  %r = select i1 %c, i32 %z, i32 100
  ret i32 %r
}";

/// The pin's `tryToFoldExtOfLoad` (`NonNegZExt`, DAGCombiner.cpp L15727) makes
/// a `zext nneg` of a load that a signed compare reads the sign-extending load,
/// which the compare shares. Here the byte was zero-extended for the value and
/// sign-extended again for the compare: two extensions of one load.
#[test]
fn a_zext_nneg_of_a_load_a_signed_compare_reads_is_one_sign_extended_load() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(IR, cpu, Level::O2).expect("compiles");
        let loads = text.lines().filter(|line| line.contains("(g") && line.trim_start().starts_with("move")).count();
        assert_eq!(loads, 1, "{cpu:?}: the byte is loaded once:\n{text}");
        assert!(text.contains("ext."), "{cpu:?}: the load is sign-extended:\n{text}");
        assert!(!text.contains("and.l\t#255"), "{cpu:?}: and not masked as well:\n{text}");
    }
}
