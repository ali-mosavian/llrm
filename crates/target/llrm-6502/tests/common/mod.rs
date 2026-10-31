//! Shared by the PRG tests: the C64 program built by ca65 and ld65.
// Each test binary compiles this module and uses some of its helpers.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

use llrm_6502::RUNTIME;
use llrm_6502::dagisel::generated::ids::opcode as op;
use llrm_6502::emit::{MInst, Operand, equates, function};
use llrm_6502::platform::{C64, C64_LD65, Platform, SIM65, c64_startup};

/// A PRG whose `_main` returns 7, and its ld65 label file.
pub fn build() -> (Vec<u8>, String, PathBuf) {
    let body = [
        MInst::new(op::LDI8, vec![Operand::reg("rc2"), Operand::Imm(7)]),
        MInst::new(op::COPY8, vec![Operand::reg("a"), Operand::reg("rc2")]),
        MInst::new(op::RET, vec![]),
    ];
    let body: Vec<_> = body.into_iter().map(Ok).collect();
    link("prg", &function("_main", &body).unwrap())
}

/// The C64 PRG of `code` (ca65 text defining `_main`): startup, `code`, then
/// the runtime.
pub fn link(
    name: &str,
    code: &str,
) -> (Vec<u8>, String, PathBuf) {
    let platform = Platform::parse(C64).expect("c64 row");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let source = format!("{}{}{code}{RUNTIME}", equates(&platform), c64_startup(&platform));
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

/// Assembles the sim65 platform's equates, the runtime and `main` (raw ca65),
/// runs it on sim65 and returns the exit code, which is A when `main` returns.
pub fn run_with_runtime(
    name: &str,
    main: &str,
) -> i32 {
    run_with(name, main, "")
}

/// [`run_with_runtime`] with more ca65 text (compiled functions, hand-written
/// callees) after `main`.
pub fn run_with(
    name: &str,
    main: &str,
    extra: &str,
) -> i32 {
    run_on("sim6502", name, main, extra).0
}

/// [`run_with`], and the cycles sim65 counts for the whole program (start-up
/// and exit included).
pub fn cycles_with(
    name: &str,
    main: &str,
    extra: &str,
) -> i64 {
    cycles_on("sim6502", name, main, extra)
}

/// [`cycles_with`] on the cc65 target `target` (`sim6502` or `sim65c02`).
pub fn cycles_on(
    target: &str,
    name: &str,
    main: &str,
    extra: &str,
) -> i64 {
    let platform = Platform::parse(SIM65).expect("sim65 row");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rt-cycles-{target}-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = format!("{}.export _main\n_main:\n{main}  rts\n{extra}{RUNTIME}", equates(&platform));
    std::fs::write(dir.join("p.s"), source).unwrap();
    let built = Command::new("cl65")
        .args(["-t", target, "-o"])
        .arg(dir.join("p.bin"))
        .arg(dir.join("p.s"))
        .output()
        .expect("cl65");
    assert!(built.status.success(), "cl65: {}", String::from_utf8_lossy(&built.stderr));
    let out = Command::new("sim65").args(["-c", "-x", "50000000"]).arg(dir.join("p.bin")).output().expect("sim65");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.trim().strip_suffix(" cycles").and_then(|n| n.trim().parse::<i64>().ok()))
        .expect("a cycle count")
}

/// [`run_with`] on the cc65 target `target` (`sim6502` or `sim65c02`): the exit
/// code and the size of the linked binary.
pub fn run_on(
    target: &str,
    name: &str,
    main: &str,
    extra: &str,
) -> (i32, u64) {
    let platform = Platform::parse(SIM65).expect("sim65 row");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rt-{target}-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = format!("{}.export _main\n_main:\n{main}  rts\n{extra}{RUNTIME}", equates(&platform));
    std::fs::write(dir.join("p.s"), source).unwrap();
    let built = Command::new("cl65")
        .args(["-t", target, "-o"])
        .arg(dir.join("p.bin"))
        .arg(dir.join("p.s"))
        .output()
        .expect("cl65");
    assert!(built.status.success(), "cl65: {}", String::from_utf8_lossy(&built.stderr));
    let code = Command::new("sim65")
        .args(["-x", "50000000"])
        .arg(dir.join("p.bin"))
        .status()
        .expect("sim65")
        .code()
        .expect("exit code");
    (code, std::fs::metadata(dir.join("p.bin")).unwrap().len())
}

/// Runs `prg` in VICE's `x64sc` without a display and returns the monitor's
/// answer to each of `commands` (`m 0400 0400`) once the program has run. Needs
/// `x64sc`, `xvfb-run` and the C64 ROMs in ~/.local/share/vice/C64.
pub fn vice(
    prg: &std::path::Path,
    commands: &[&str],
) -> Vec<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::process::Stdio;
    use std::time::Duration;

    let mut vice = Command::new("xvfb-run")
        .args(["-a", "x64sc", "-default", "+sound", "-warp", "-autostartprgmode", "1"])
        .args(["-remotemonitor", "-remotemonitoraddress", "ip4://127.0.0.1:6510", "-autostart"])
        .arg(prg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("xvfb-run x64sc");
    let mut monitor = (0..60)
        .find_map(|_| {
            TcpStream::connect("127.0.0.1:6510")
                .ok()
                .or_else(
                    || {
                        std::thread::sleep(Duration::from_millis(500));
                        None
                    },
                )
        })
        .expect("VICE's monitor");
    // boot, the injected RUN, the program
    std::thread::sleep(Duration::from_secs(8));
    let answers = commands
        .iter()
        .map(|command| {
            monitor.write_all(format!("{command}\n").as_bytes()).unwrap();
            std::thread::sleep(Duration::from_millis(1000));
            monitor.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
            let mut out = Vec::new();
            let mut chunk = [0u8; 4096];
            while let Ok(n) = monitor.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&chunk[..n]);
            }
            String::from_utf8_lossy(&out).into_owned()
        })
        .collect();
    let _ = monitor.write_all(b"quit\n");
    std::thread::sleep(Duration::from_millis(500));
    let _ = vice.kill();
    answers
}
pub mod pipeline;
