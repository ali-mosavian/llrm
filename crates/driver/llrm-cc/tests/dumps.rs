//! The debugging switches llrm-c had reach the backend through llrm-cc. Needs
//! the pinned clang (`LLRM_PIN_CLANG`) and the Open Watcom headers (`LLRM_OW`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::{Command, Output};

fn compile(
    name: &str,
    env: &[(&str, &str)],
    flags: &[&str],
) -> Option<(Output, std::path::PathBuf)> {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    // On disk: `--dump` refuses a tmpfs.
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), "int f(int a, int b) { return a * b + 1; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .envs(env.iter().copied())
        .args(["-O2", "-w", "-c"])
        .args(flags)
        .arg("-o")
        .arg(dir.join("d.o"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    Some((run, dir))
}

/// `--dump DIR` wrote the stage files, and `LLRM_DUMP_INPUTS` the input of
/// each pass besides, with its `manifest.txt`.
#[test]
fn dump_and_dump_inputs_reach_the_backend() {
    let dump = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("dump-out-{}", std::process::id()));
    let flags = ["--dump", dump.to_str().unwrap()];
    let Some((_, dir)) = compile("dump", &[("LLRM_DUMP_INPUTS", "1")], &flags) else {
        return;
    };
    let written = std::fs::read_dir(&dump).map(|one| one.count()).unwrap_or(0);
    let manifest = dump.join("manifest.txt").exists();
    let _ = std::fs::remove_dir_all(&dump);
    let _ = std::fs::remove_dir_all(dir);
    assert!(written > 1, "no stage written");
    assert!(manifest, "no input dumped under LLRM_DUMP_INPUTS");
}

/// `LLRM_DEBUG=counts` printed nothing from llrm-llc: llrm-c printed the
/// counts at its end, llrm-cc ran with none.
#[test]
fn debug_counts_are_printed() {
    let Some((run, dir)) = compile("counts", &[("LLRM_DEBUG", "counts")], &[]) else {
        return;
    };
    let _ = std::fs::remove_dir_all(dir);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("[counts] pass_runs"), "{stderr}");
}

/// `ISEL_DUMP` printed the selected instructions.
#[test]
fn isel_dump_is_printed() {
    let Some((run, dir)) = compile("isel", &[("ISEL_DUMP", "1")], &[]) else {
        return;
    };
    let _ = std::fs::remove_dir_all(dir);
    let all = format!("{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    assert!(all.contains("=== ISel"), "{all}");
}
