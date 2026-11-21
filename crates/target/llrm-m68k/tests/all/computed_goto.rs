//! `goto *p` (`indirectbr` of a `blockaddress`) was refused at every level:
//! `an indirect jump of no jump table`. The IR pipeline expands it to a
//! `switch` over block numbers; `tests/run/c/computed_goto.c` runs the result
//! under qemu (`tools/m68k_exec_sweep.py`).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = include_str!("../fixtures/computed_goto_raw.ll");

#[test]
fn a_goto_through_a_label_address_compiles() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            assert!(!text.contains("blockaddress"), "{cpu} {level:?}: a block address is left:\n{text}");
        }
    }
}
