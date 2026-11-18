//! `Cpu::has_bit_field_find` asked one question of the opcode table (is
//! `bfffo Dn,{off:wid},Dn` a hardware form?) and parsed all 2569 rows to
//! answer it: 9.8 M of the 20.6 M instructions an empty function cost to
//! compile.

use llrm_m68k::dagisel::hooks::Cpu;

#[test]
fn asking_whether_bfffo_exists_does_not_parse_the_whole_opcode_table() {
    let answers: Vec<bool> = [Cpu::M68000, Cpu::M68020, Cpu::M68040, Cpu::M68060].map(Cpu::has_bit_field_find).into();
    assert_eq!(answers, [false, true, true, true]);
    let parsed = llrm_m68k::isa::rows_parsed();
    assert!(parsed < 50, "{parsed} rows parsed");
}
