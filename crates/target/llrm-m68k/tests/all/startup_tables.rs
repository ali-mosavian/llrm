//! A per-CPU answer read from the timing tables was computed for all six
//! CPUs on the first ask, parsing five tables the compile never reads: 9.3 M
//! of tile -O2's 128 M compile instructions (3609 labels normalized beyond
//! the lookups themselves).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::timing::{cost, keys_made};

#[test]
fn a_cpu_answer_from_the_timing_tables_reads_only_that_cpus_table() {
    let lookups = [
        ("move.l (0,%a1,%d1), %d0", 4),
        ("move.l (%a1), %d0", 2),
        ("btst #23, %d0", 4),
        ("and.l #8388608, %d0", 6),
        ("btst #15, %d0", 4),
        ("tst.w %d0", 2),
        ("tst.b %d0", 2),
    ];
    // builds the 68020's table
    cost(Cpu::M68020, "moveq #1, %d0", 2, false).expect("priced");
    let start = keys_made();
    for (text, bytes) in lookups {
        cost(Cpu::M68020, text, bytes, false).expect("priced");
    }
    let direct = keys_made() - start; // these lookups alone
    let start = keys_made();
    llrm_m68k::machine::index_is_dearer_than_plain(Cpu::M68020);
    llrm_m68k::dagisel::lowering::btst_beats_and_immediate(Cpu::M68020);
    let made = keys_made() - start;
    assert!(made <= direct, "{made} labels normalized, the same lookups alone {direct}");
}
