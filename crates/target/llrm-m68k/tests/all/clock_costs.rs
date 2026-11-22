//! `Machine::costs` was `OperationCosts::default()`: every operation one clock, so a fill whose call costs a
//! hundred clocks looked as cheap as the store it replaced (m32_word_vectors -O1 68000 +4% clocks once built hosted).
//! Each class is now the manuals' clocks of the instructions it becomes.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::opcosts;

#[test]
fn the_clock_costs_are_the_tables_clocks_not_one_per_operation() {
    let m68000 = opcosts::clock_costs(Cpu::M68000).expect("the forms price");
    // add.l d1,d0: 8; move.l (4,a0),d0: 16; move.l d0,(4,a0): 16; move.l #3,(4,a0): 24; lea (4,a1),a0: 8
    assert_eq!((m68000.add, m68000.load, m68000.store, m68000.store_immediate, m68000.address), (8, 16, 16, 24, 8));
    let m68020 = opcosts::clock_costs(Cpu::M68020).expect("the forms price");
    assert!(m68020.add < m68000.add && m68020.load < m68000.load, "{m68020:?}");
}

#[test]
fn a_libc_call_costs_more_than_one_store_and_a_longword_less_than_its_stores() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let costs = opcosts::clock_costs(cpu).unwrap();
        assert!(costs.fill > 4 * costs.store_immediate, "{cpu:?}: the call is its pushes, jsr and rts: {}", costs.fill);
        assert!(costs.fill_cell < 2 * costs.store_immediate, "{cpu:?}: a longword is one move and a dbf: {}", costs.fill_cell);
    }
}
