//! The C64 PRG layout: ca65 and ld65 with the platform's config. The bytes are worked out by hand
//! from the C64's BASIC line format; VICE (which would run the PRG) has no ROMs here yet.

mod common;

/// `10 SYS 2061` at $0801 is: next-line pointer $080B, line number 10, the SYS token $9E, "2061",
/// a 0, then the program's end 00 00 -- 12 bytes, so the code is at $080D = 2061. In the file the
/// load address 01 08 comes first and the code (`jsr`, opcode $20) follows at offset 14.
#[test]
fn the_prg_is_a_load_address_a_sys_line_and_code_at_2061() {
    let (prg, labels, _) = common::build();
    assert_eq!(prg[..14], [0x01, 0x08, 0x0B, 0x08, 0x0A, 0x00, 0x9E, b'2', b'0', b'6', b'1', 0x00, 0x00, 0x00]);
    assert_eq!(prg[14], 0x20);
    assert!(labels.contains("al 00080D .start"), "start is at $080D: {labels}");
}
