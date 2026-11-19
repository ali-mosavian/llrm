//! A timing lookup compared its label with every row of the table,
//! normalizing each (an allocation per row): `timing::table::key` and its
//! `memcmp` were 25% of the instructions of a corpus compile, in the pricing
//! of every trial build. A lookup normalizes a handful of labels.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::timing::{cost, keys_made};

#[test]
fn pricing_an_instruction_normalizes_a_handful_of_labels() {
    let text = ["add.l (%a0)+,%d1", "move.l (%a0),%d0", "lsl.l #2,%d1", "cmp.w %d0,%d1"];
    for cpu in [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060] {
        for one in text {
            // the tables load on first use
            cost(cpu, one, 2, false).expect("priced");
            let before = keys_made();
            cost(cpu, one, 2, false).unwrap_or_else(|why| panic!("{one}: {why}"));
            let made = keys_made() - before;
            assert!(made <= 12, "{made} labels normalized to price `{one}` on {cpu:?}");
        }
    }
}
