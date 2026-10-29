//! The C64 PRG layout: ca65 and ld65 with the platform's config. The bytes are worked out by hand
//! from the C64's BASIC line format; VICE (which would run the PRG) has no ROMs here yet.

use std::path::PathBuf;
use std::process::Command;

use llrm_6502::dagisel::generated::ids::opcode as op;
use llrm_6502::emit::{MInst, Operand, equates, function};
use llrm_6502::platform::{C64, C64_LD65, Platform, c64_startup};

fn build() -> (Vec<u8>, String) {
    let platform = Platform::parse(C64).expect("c64 row");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("prg");
    std::fs::create_dir_all(&dir).unwrap();
    let body = [
        MInst::new(op::LDI8, vec![Operand::reg("rc2"), Operand::Imm(7)]),
        MInst::new(op::COPY8, vec![Operand::reg("a"), Operand::reg("rc2")]),
        MInst::new(op::RET, vec![]),
    ];
    let body: Vec<_> = body.into_iter().map(Ok).collect();
    let source = format!("{}{}{}", equates(&platform), c64_startup(), function("_main", &body).unwrap());
    std::fs::write(dir.join("p.s"), source).unwrap();
    std::fs::write(dir.join("c64.cfg"), C64_LD65).unwrap();
    let run = |program: &str, args: &[&str]| {
        let out = Command::new(program).args(args).current_dir(&dir).output().expect(program);
        assert!(out.status.success(), "{program}: {}", String::from_utf8_lossy(&out.stderr));
    };
    run("ca65", &["p.s", "-o", "p.o"]);
    run("ld65", &["-C", "c64.cfg", "-Ln", "p.lbl", "-o", "p.prg", "p.o"]);
    (std::fs::read(dir.join("p.prg")).unwrap(), std::fs::read_to_string(dir.join("p.lbl")).unwrap())
}

/// `10 SYS 2061` at $0801 is: next-line pointer $080B, line number 10, the SYS token $9E, "2061",
/// a 0, then the program's end 00 00 -- 12 bytes, so the code is at $080D = 2061. In the file the
/// load address 01 08 comes first and the code (`jsr`, opcode $20) follows at offset 14.
#[test]
fn the_prg_is_a_load_address_a_sys_line_and_code_at_2061() {
    let (prg, labels) = build();
    assert_eq!(prg[..14], [0x01, 0x08, 0x0B, 0x08, 0x0A, 0x00, 0x9E, b'2', b'0', b'6', b'1', 0x00, 0x00, 0x00]);
    assert_eq!(prg[14], 0x20);
    assert!(labels.contains("al 00080D .start"), "start is at $080D: {labels}");
}
