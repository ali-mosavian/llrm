//! `move.l #5, (4,%a0)` is 8 B and 24 clocks on the 68000; `moveq #5, %d0`
//! then `move.l %d0, (4,%a0)` is 6 B and 20. A store of a small constant to
//! memory kept the immediate where a data register was free (zz_f -Os: the
//! call-area stores of four `report(const)` calls, +4 B against gcc).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define void @f(ptr %p) {
  %q = getelementptr i32, ptr %p, i32 1
  store volatile i32 5, ptr %q
  store volatile i32 7, ptr %p
  ret void
}";

#[test]
fn a_small_constant_stored_to_memory_goes_through_a_free_register() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let out = assembly(IR, cpu_of("68000").unwrap(), level).expect("compiles");
        assert!(out.contains("moveq\t#5"), "{level:?}\n{out}");
        assert!(!out.contains("move.l\t#5") && !out.contains("move.l\t#7"), "{level:?}\n{out}");
    }
}
