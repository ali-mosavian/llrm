//! The tools build.rs puts beside llrm's binaries.

use std::path::Path;
use std::process::Command;

/// A fresh checkout had no jwasm or jwlink; then cargo's DEBUG made their
/// makefiles build into GccUnixD and the build script failed.
#[test]
fn test_jwasm_and_jwlink_are_built_beside_llrm() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let start = Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/modern/start.asm");
    let assembled = Command::new(bin.join("jwasm"))
        .args(["-q", "-c", "-Cp", "-Zg", "-omf"])
        .arg(format!("-Fo{}", scratch.path().join("START.OBJ").display()))
        .arg(start)
        .status()
        .expect("jwasm is in target/<profile>");
    assert!(assembled.success() && scratch.path().join("START.OBJ").exists());
    let linker = Command::new(bin.join("jwlink")).stdin(std::process::Stdio::null()).output().expect("jwlink is in target/<profile>");
    assert!(String::from_utf8_lossy(&linker.stdout).contains("JWlink"));
}

/// The dosrun DOSBox-X only built on macOS (pthread_threadid_np, a missing
/// headless SDL_SetWindowIcon, archive order GNU ld rejects), so no e2e test
/// could run elsewhere.
#[test]
fn test_a_jwlink_exe_runs_under_the_built_dosbox() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    std::fs::write(
        dir.join("HELLO.ASM"),
        ".model small\n.stack 256\n.data\nmsg db 'dosrun', 13, 10, '$'\n.code\nstart: mov ax, @data\n\
         mov ds, ax\nmov dx, offset msg\nmov ah, 9\nint 21h\nmov ax, 4C00h\nint 21h\nend start\n",
    )
    .unwrap();
    let run = |program: &str, args: &[&str]| {
        assert!(Command::new(bin.join(program)).args(args).current_dir(dir).status().unwrap().success(), "{program}");
    };
    run("jwasm", &["-q", "-omf", "-FoHELLO.OBJ", "HELLO.ASM"]);
    run("jwlink", &["format", "dos", "file", "HELLO.OBJ", "name", "HELLO.EXE", "op", "quiet"]);
    let conf = format!("[autoexec]\nmount c {}\nc:\nHELLO > OUT.TXT\nexit\n", dir.display());
    std::fs::write(dir.join("dosbox.conf"), conf).unwrap();
    let conf = dir.join("dosbox.conf");
    let conf = conf.to_str().unwrap();
    run("dosbox-x", &["-nolog", "-exit", "-conf", conf]);
    assert_eq!(std::fs::read_to_string(dir.join("OUT.TXT")).unwrap(), "dosrun\r\n");
}
