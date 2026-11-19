//! The Apple II binary: assembled and linked with ca65 and ld65, its header and
//! first bytes checked. Nothing runs it (no Apple II emulator is installed):
//! the code is the C64 program's, with the platform's zero page and stack,
//! which the checks below read back.

use std::path::PathBuf;
use std::process::Command;

use llrm_6502::RUNTIME;
use llrm_6502::emit::equates;
use llrm_6502::platform::{APPLE2_LD65, Platform, apple2_startup};

#[test]
fn the_apple2_binary_has_its_header_and_entry() {
    let platform = Platform::parse(include_str!("../src/machines/apple2.toml")).expect("apple2 row");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("apple2");
    std::fs::create_dir_all(&dir).unwrap();
    let main = "  .export _main\n_main:\n  lda #7\n  rts\n";
    std::fs::write(dir.join("p.s"), format!("{}{}{main}{RUNTIME}", equates(&platform), apple2_startup(&platform)))
        .unwrap();
    std::fs::write(dir.join("apple2.cfg"), APPLE2_LD65).unwrap();
    let run = |program: &str, args: &[&str]| {
        let out = Command::new(program).args(args).current_dir(&dir).output().expect(program);
        assert!(out.status.success(), "{program}: {}", String::from_utf8_lossy(&out.stderr));
    };
    run("ca65", &["p.s", "-o", "p.o"]);
    run("ld65", &["-C", "apple2.cfg", "-Ln", "p.lbl", "-o", "p.bin", "p.o"]);
    let binary = std::fs::read(dir.join("p.bin")).unwrap();
    let labels = std::fs::read_to_string(dir.join("p.lbl")).unwrap();

    // load address $0803, length = the file without its 4-byte header
    assert_eq!(binary[..2], [0x03, 0x08]);
    assert_eq!(usize::from(u16::from_le_bytes([binary[2], binary[3]])), binary.len() - 4);
    assert!(labels.contains("al 000803 .start"), "start is at $0803: {labels}");
    // lda #$00; sta $80; lda #$96; sta $81: the soft stack pointer, rc0:rc1 at
    // $80:$81, top at HIMEM
    assert_eq!(binary[4..12], [0xA9, 0x00, 0x85, 0x80, 0xA9, 0x96, 0x85, 0x81]);
}
