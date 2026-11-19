//! The constant table was cached by `cpu == M68020`, so the 68000, 68010,
//! 68030, 68040 and 68060 shared the entry of whichever asked first. A
//! process that compiled for several CPUs (`examples/matrix.rs`, a test) gave
//! the 68040 the 68000's table: at speed the 68040 (`move.l #imm` is one
//! clock, nothing beats it) got 767 `moveq` sequences from a fresh process's
//! none, and the 68030 the 68000's 767 instead of the 68020's 1791.

use llrm_m68k::consts::made;
use llrm_m68k::dagisel::hooks::Cpu;

/// Asked in the order that poisoned the cache: the 68000 first.
#[test]
fn each_cpu_gets_its_own_constant_table_whichever_asked_first() {
    let first = made(Cpu::M68000, false);
    assert!(!first.is_empty());
    assert!(made(Cpu::M68010, false) == first, "the 68010 prices as the 68000");
    assert!(made(Cpu::M68020, false) == made(Cpu::M68030, false), "the 68030 prices as the 68020");
    assert!(made(Cpu::M68030, false) != first, "the 68030 got the 68000's table");
    assert!(made(Cpu::M68040, false) != first, "the 68040 got the 68000's table");
    assert!(made(Cpu::M68060, true) != made(Cpu::M68040, true), "the 68060 got the 68040's size table");
}
