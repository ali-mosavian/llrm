//! Shared by the PRG tests: the C64 program built by ca65 and ld65.

use std::path::PathBuf;
use std::process::Command;

use llrm_6502::dagisel::generated::ids::opcode as op;
use llrm_6502::emit::{MInst, Operand, equates, function};
use llrm_6502::platform::{C64, C64_LD65, Platform, c64_startup};

/// A PRG whose `_main` returns 7, and its ld65 label file.
pub fn build() -> (Vec<u8>, String, PathBuf) {
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
    (std::fs::read(dir.join("p.prg")).unwrap(), std::fs::read_to_string(dir.join("p.lbl")).unwrap(), dir.join("p.prg"))
}


use llrm_6502::platform::SIM65;
use llrm_6502::RUNTIME;

/// Assembles the sim65 platform's equates, the runtime and `main` (raw ca65), runs it on sim65 and
/// returns the exit code, which is A when `main` returns.
pub fn run_with_runtime(
    name: &str,
    main: &str,
) -> i32 {
    run_with(name, main, "")
}

/// [`run_with_runtime`] with more ca65 text (compiled functions, hand-written callees) after `main`.
pub fn run_with(
    name: &str,
    main: &str,
    extra: &str,
) -> i32 {
    let platform = Platform::parse(SIM65).expect("sim65 row");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rt-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = format!("{}.export _main\n_main:\n{main}  rts\n{extra}{RUNTIME}", equates(&platform));
    std::fs::write(dir.join("p.s"), source).unwrap();
    let built = Command::new("cl65")
        .args(["-t", "sim6502", "-o"])
        .arg(dir.join("p.bin"))
        .arg(dir.join("p.s"))
        .output()
        .expect("cl65");
    assert!(built.status.success(), "cl65: {}", String::from_utf8_lossy(&built.stderr));
    Command::new("sim65").arg(dir.join("p.bin")).status().expect("sim65").code().expect("exit code")
}
