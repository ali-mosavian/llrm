//! A QB program compiled for 32-bit x86 Linux (`llrm-qb -m32
//! -fobject-format=elf`), linked with the host runtime on the headless device
//! by `cc -m32` and run natively. The runtime is C: the object calls it by the
//! C family, not by Open Watcom's registers (which the DOS flat target's
//! runtime is written for), and every procedure frames itself.
//! A machine without 32-bit libc development files (`cc -m32` cannot include
//! errno.h) prints `SKIPPED: why` and passes, and fails where
//! `LLRM_REQUIRE_LIBC32` is set (tools/gate/tiers.toml, capability `libc32`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Whether `cc -m32` can compile against the host's libc headers; when not,
/// says so on stderr, and fails where `LLRM_REQUIRE_LIBC32` is set.
fn host_links_32_bit() -> bool {
    let probe = Command::new("cc")
        .args(["-m32", "-x", "c", "-", "-o", "/dev/null"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            std::io::Write::write_all(
                child.stdin.as_mut().unwrap(),
                b"#include <errno.h>\nint main(void) { return 0; }\n",
            )?;
            child.wait()
        });
    if probe.is_ok_and(|status| status.success()) {
        return true;
    }
    let reason = "needs 32-bit libc development files (`cc -m32` cannot include errno.h)";
    let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("SKIPPED: {reason}\n").as_bytes());
    assert!(std::env::var_os("LLRM_REQUIRE_LIBC32").is_none(), "LLRM_REQUIRE_LIBC32 is set, and: {reason}");
    false
}

/// Compiles `source`, links it with the runtime, runs it with `environment`;
/// the text its screen shows.
fn run(
    name: &str,
    source: &[u8],
    environment: &[(&str, &str)],
) -> String {
    let dir: PathBuf = std::env::temp_dir().join(format!("{name}_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let bas = dir.join(format!("{name}.bas"));
    fs::write(&bas, source).unwrap();
    let object = dir.join(format!("{name}.o"));
    let compiled = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .args(["-m32", "-O2", "-fobject-format=elf"])
        .arg(&bas)
        .arg("-o")
        .arg(&object)
        .output()
        .expect("llrm-qb runs");
    assert!(compiled.status.success(), "llrm-qb: {}", String::from_utf8_lossy(&compiled.stderr));

    let qb = root().join("runtime/qb");
    let mut sources: Vec<PathBuf> = fs::read_dir(&qb)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .collect();
    for extra in [
        "posix/startup.c",
        "posix/block.c",
        "posix/ad.c",
        "posix/mathops.c",
        "posix/paintq.c",
        "posix/module.c",
        "posix/blockentries.c",
        "posix/onerror_host.c",
        "posix/program.c",
        "posix/real.c",
        "dev/headless/headless.c",
        "dev/headless/main.c",
        "dev/host/video.c",
        "dev/host/vgadata.c",
        "dev/host/tick.c",
        "../shared/posix/os.c",
    ] {
        sources.push(qb.join(extra));
    }
    let program = dir.join(name);
    let mut link = Command::new("cc");
    link.args(["-m32", "-std=gnu11", "-w", "-g", "-pthread", "-no-pie", "-D__cdecl="]);
    for include in ["posix", ".", "dev", "dev/headless", "dev/host", "../shared/posix"] {
        link.arg("-I").arg(qb.join(include));
    }
    let linked = link.args(&sources).arg(&object).arg("-lm").arg("-o").arg(&program).output().expect("cc runs");
    assert!(linked.status.success(), "link: {}", String::from_utf8_lossy(&linked.stderr));

    let done = Command::new(&program).envs(environment.iter().copied()).output().expect("the program runs");
    let screen = String::from_utf8_lossy(&done.stdout).into_owned();
    // 124: the headless driver ended a program that was still waiting, with the
    // screen it had (QB_HEADLESS_SECONDS)
    assert!(
        done.status.success() || done.status.code() == Some(124),
        "the program failed ({:?}):\n{screen}{}",
        done.status,
        String::from_utf8_lossy(&done.stderr)
    );
    fs::remove_dir_all(&dir).ok();
    screen
}

/// `-m32 -fobject-format=elf` passed the runtime's arguments in Open Watcom's
/// registers: the first call (`PRINT`'s string) handed the C runtime whatever
/// the stack held and the program crashed (SIGSEGV).
#[test]
fn print_hello_runs_natively_on_x86_32() {
    if !host_links_32_bit() {
        return;
    }
    assert_eq!(run("hello32", b"PRINT \"HELLO\"\nEND\n", &[]).trim_end(), "HELLO");
}

/// DEF SEG, PEEK and POKE of the PC's low memory (NIBBLES reads the keyboard
/// flags at 0:0417) read address 0x417 directly on a host: SIGSEGV. The runtime
/// places them in its image of that memory.
#[test]
fn peek_of_the_pc_low_memory_runs_on_x86_32() {
    if !host_links_32_bit() {
        return;
    }
    let source = b"DEF SEG = 0\r\nPOKE &H417, 5\r\nPRINT PEEK(&H417)\r\nEND\r\n";
    assert_eq!(run("peek32", source, &[]).trim(), "5");
}

/// NIBBLES (QB 4.5's demo, from `QB45_DEMOS_DIR`) typed through its questions:
/// the stack-block routines (COLOR, LOCATE, SCREEN, DIM) are C varargs the
/// caller cleans up, and a procedure with local strings frames itself (no
/// B$ENRA).
#[test]
fn nibbles_reaches_its_first_level_on_x86_32() {
    if !host_links_32_bit() {
        return;
    }
    let Some(dir) = std::env::var_os("QB45_DEMOS_DIR") else {
        eprintln!("QB45_DEMOS_DIR is not set: NIBBLES is not in the tree");
        return;
    };
    let source = fs::read(Path::new(&dir).join("NIBBLES.BAS")).expect("NIBBLES.BAS");
    let screen = run(
        "nibbles32",
        &source,
        &[("QB_HEADLESS_KEYS", " 1\r50\rY\rC\r"), ("QB_HEADLESS_GAP", "400"), ("QB_HEADLESS_SECONDS", "15")],
    );
    assert!(screen.contains("Level 1,  Push Space"), "{screen}");
}

/// GORILLA typed through its intro and four questions: SINGLE and DOUBLE
/// arithmetic on the x87 and the menu text.
#[test]
fn gorilla_reaches_its_menu_on_x86_32() {
    if !host_links_32_bit() {
        return;
    }
    let Some(dir) = std::env::var_os("QB45_DEMOS_DIR") else {
        eprintln!("QB45_DEMOS_DIR is not set: GORILLA is not in the tree");
        return;
    };
    let source = fs::read(Path::new(&dir).join("GORILLA.BAS")).expect("GORILLA.BAS");
    let screen = run(
        "gorilla32",
        &source,
        &[("QB_HEADLESS_KEYS", "     \r\r\r\r\r\r\r\r"), ("QB_HEADLESS_GAP", "800"), ("QB_HEADLESS_SECONDS", "20")],
    );
    assert!(screen.contains("Your Choice?"), "{screen}");
}
