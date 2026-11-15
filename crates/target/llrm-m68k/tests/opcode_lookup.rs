//! Each module that needed a pin opcode's code scanned the whole machine
//! opcode table for it: `short::narrow` did it 204 000 times compiling tile
//! at -O2 (4% of the compile's instructions, the table's whole length each).

use llrm_m68k::dagisel::opcodes;

#[test]
fn a_lookup_of_an_opcode_does_not_walk_the_table() {
    let before = opcodes::probes();
    for _ in 0..100 {
        let code = opcodes::code_of("ADD32dd").expect("a pin row");
        assert_eq!(opcodes::name_of(u32::from(code)), Some("ADD32dd"));
    }
    let probes = opcodes::probes() - before;
    assert!(probes <= 400, "{probes} table entries looked at for 200 lookups");
}
