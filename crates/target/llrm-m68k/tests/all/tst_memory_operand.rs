//! `tst.l (%a0)` tests the long at the address, not `%a0`: a `tst` whose
//! operand is an address register indirect was dropped after `move.l
//! (4,%sp),%a0` (which sets no codes), so `seq` read stale codes. `if (*p ==
//! 0)` returned the wrong value on every CPU and level.

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn a_tst_of_memory_is_not_dropped_after_a_move_into_its_address_register() {
    let source = include_str!("../data/tst_memory_operand.ll");
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text = assembly(source, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(text.contains("tst.l\t(%a") || text.contains("cmp"), "{cpu}: the memory is never tested:\n{text}");
        }
    }
}
