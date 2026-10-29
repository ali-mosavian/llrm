//! The NES image and the Atari XEX file: assembled and linked with ca65 and ld65, their headers,
//! vectors and entry bytes checked. Nothing runs them (no NES or Atari emulator is installed).

use std::path::PathBuf;
use std::process::Command;

use llrm_6502::RUNTIME;
use llrm_6502::emit::equates;
use llrm_6502::platform::{ATARI8_LD65, NES_LD65, Platform, atari8_startup, nes_startup};

const MAIN: &str = "  .export _main\n_main:\n  lda #7\n  rts\n";

/// The linked file and ld65's label file for `startup` + a `_main` returning 7.
fn build(
    name: &str,
    row: &str,
    config: &str,
    startup: fn(&Platform) -> String,
) -> (Vec<u8>, String) {
    let platform = Platform::parse(row).unwrap_or_else(|e| panic!("{name} row: {e}"));
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("p.s"), format!("{}{}{MAIN}{RUNTIME}", equates(&platform), startup(&platform))).unwrap();
    std::fs::write(dir.join("p.cfg"), config).unwrap();
    let run = |program: &str, args: &[&str]| {
        let out = Command::new(program).args(args).current_dir(&dir).output().expect(program);
        assert!(out.status.success(), "{program}: {}", String::from_utf8_lossy(&out.stderr));
    };
    run("ca65", &["p.s", "-o", "p.o"]);
    run("ld65", &["-C", "p.cfg", "-Ln", "p.lbl", "-o", "p.bin", "p.o"]);
    (std::fs::read(dir.join("p.bin")).unwrap(), std::fs::read_to_string(dir.join("p.lbl")).unwrap())
}

fn label(
    labels: &str,
    name: &str,
) -> u16 {
    let line = labels.lines().find(|l| l.ends_with(&format!(" .{name}"))).unwrap_or_else(|| panic!("no .{name} in {labels}"));
    u16::from_str_radix(line.split_whitespace().nth(1).unwrap(), 16).unwrap()
}

/// 16-byte iNES header (2 x 16K PRG, 1 x 8K CHR), PRG at $8000 starting with `sei`, the reset vector
/// at $FFFC pointing at `start`.
#[test]
fn the_nes_image_has_its_header_vectors_and_entry() {
    let (image, labels) = build("nes", include_str!("../src/machines/nes.toml"), NES_LD65, nes_startup);
    assert_eq!(image.len(), 16 + 0x8000 + 0x2000);
    assert_eq!(image[..8], *b"NES\x1A\x02\x01\x00\x00");
    let start = label(&labels, "start");
    assert_eq!(start, 0x8000);
    assert_eq!(image[16], 0x78, "sei");
    let vectors = &image[16 + 0x7FFA..16 + 0x8000];
    assert_eq!(u16::from_le_bytes([vectors[2], vectors[3]]), start, "reset vector");
    // sei; cld; ldx #$FF; txs; lda #$00; sta $02; lda #$07; sta $03: rc0:rc1 at $02:$03
    assert_eq!(image[16..30], [0x78, 0xD8, 0xA2, 0xFF, 0x9A, 0xA9, 0x00, 0x85, 0x02, 0xA9, 0x07, 0x85, 0x03, 0x20]);
}

/// $FFFF, load chunk $2000..end (end inclusive), RUNAD chunk $02E0..$02E1 = `start`.
#[test]
fn the_atari_file_has_its_chunks_and_run_address() {
    let (file, labels) = build("atari8", include_str!("../src/machines/atari8.toml"), ATARI8_LD65, atari8_startup);
    assert_eq!(file[..4], [0xFF, 0xFF, 0x00, 0x20]);
    let end = usize::from(u16::from_le_bytes([file[4], file[5]]));
    let body = file.len() - 6 - 6;
    assert_eq!(end, 0x2000 + body - 1, "the end address is inclusive");
    let start = label(&labels, "start");
    assert_eq!(start, 0x2000);
    assert_eq!(file[file.len() - 6..], [0xE0, 0x02, 0xE1, 0x02, 0x00, 0x20]);
    // lda #$00; sta $82; lda #$80; sta $83: the soft stack pointer at rc0:rc1 = $82:$83
    assert_eq!(file[6..14], [0xA9, 0x00, 0x85, 0x82, 0xA9, 0x80, 0x85, 0x83]);
}
