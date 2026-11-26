//! An `aligned` global had no `.align` and no linker segment to align it: its
//! address was not aligned. Needs `llrm-cc` built (`LLRM_CC`).

mod common;

use std::process::Command;

use common::pipeline::llrm_cc;

fn compiled(level: &str) -> (std::path::PathBuf, String) {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("aligned-globals{level}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/aligned_globals.c");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-S", "-w", "-fwhole-program", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = std::fs::read_to_string(dir.join("a.s")).unwrap();
    (dir, text)
}

/// Linked with the project's sim65 cfg and run: the exit code has a bit for
/// each global whose address is not aligned.
#[test]
fn an_aligned_global_is_at_an_aligned_address() {
    for level in ["-O0", "-O2", "-Os"] {
        let (dir, text) = compiled(level);
        // the compiled text states its own equates
        let source = format!(
            "{text}.export _main\n.segment \"CODE\"\n_main:\n  lda #0\n  sta rc0\n  lda #$F0\n  sta rc1\n  jsr main\n  rts\n{}",
            llrm_6502::RUNTIME
        );
        std::fs::write(dir.join("p.s"), source).unwrap();
        let built = Command::new("cl65")
            .args(["-t", "sim6502", "-C", concat!(env!("CARGO_MANIFEST_DIR"), "/src/machines/sim6502.cfg"), "-o"])
            .arg(dir.join("p.bin"))
            .arg(dir.join("p.s"))
            .output()
            .expect("cl65");
        let warnings = String::from_utf8_lossy(&built.stderr);
        assert!(built.status.success() && !warnings.contains("aligned properly"), "{level}: {warnings}");
        let code = Command::new("sim65").args(["-x", "1000000"]).arg(dir.join("p.bin")).status().expect("sim65").code();
        assert_eq!(code, Some(0), "{level}: misaligned");
    }
}

/// The ELF writer places and states the same alignment.
#[test]
fn the_elf_object_states_the_alignment() {
    let text = ".segment \"BSS\"\nodd: .res 3\n.align 16\nbuffer: .res 5\n";
    let object = llrm_6502::elf::object("a", text).unwrap();
    let bss = object.sections.iter().find(|one| one.name == ".bss").expect("a bss");
    assert_eq!((bss.align, bss.image.len()), (16, 21));
}
