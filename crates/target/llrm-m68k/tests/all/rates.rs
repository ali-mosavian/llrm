//! What a byte of code is worth on each CPU, from `timings.times`.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::target::cpu_table;
use llrm_m68k::timing::{byte_milliclocks, order, trade_passes};

/// Thousandths of a clock a byte costs, line fill over line bytes (rows read
/// from the manuals): 4/2, 3/4 and 5/16 clocks per byte.
#[test]
fn each_cpu_prices_a_byte_by_its_bus_fill() {
    let rates: Vec<_> = [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060]
        .into_iter()
        .map(byte_milliclocks)
        .collect();
    assert_eq!(rates, [2000, 2000, 750, 312, 312, 312]);
    assert_eq!(cpu_table("m68040").and_then(|table| table.byte_rate()), Some(312));
}

/// The interim rate (1 clock = 2 bytes) accepted every trade of two bytes per
/// clock on the 68020, which the 68020's own bus fill (1.33 bytes per clock)
/// does not pay for; the 68040 pays up to 3.2.
#[test]
fn two_extra_bytes_for_a_clock_pass_on_the_68040_and_not_on_the_68020() {
    assert!(trade_passes(Cpu::M68040, 2, 1));
    assert!(trade_passes(Cpu::M68060, 2, 1));
    assert!(!trade_passes(Cpu::M68020, 2, 1));
    assert!(!trade_passes(Cpu::M68000, 2, 1));
    assert!(trade_passes(Cpu::M68020, 4, 3));
    assert!(trade_passes(Cpu::M68000, 1, 2));
}

/// `adda.l #imm` (1 clock, 6 bytes) against `lea (d,An)` (2 clocks, 4 bytes):
/// -O1 to -O3 are judged by clocks (user ruling 2026-11-14), so the longer form
/// wins on every CPU; the bytes break a tie.
#[test]
fn a_priced_choice_takes_the_fewer_clocks_on_every_cpu_and_bytes_break_ties() {
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040] {
        assert!(order(cpu, false, 1, 6) < order(cpu, false, 2, 4), "{cpu:?}");
        assert!(order(cpu, false, 2, 4) < order(cpu, false, 2, 6), "{cpu:?}: a tie goes to the smaller");
    }
    // at -Os the bytes decide alone
    assert!(order(Cpu::M68040, true, 2, 4) < order(Cpu::M68040, true, 1, 6));
}
