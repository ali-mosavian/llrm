//! Float code run against gcc's: `softfloat_rt/ops.ll` compiled by us, called
//! from a C `main` that gcc builds with `-msoft-float`, so that both sides call
//! the same runtime routines (`softfloat_rt/fpu_rt.s`, the libgcc names over
//! the FPU, run on a 68040 model). The doubles and floats cross the call in the
//! ABI's words, the high word first; an operation mapped to the wrong routine,
//! a comparison read with the wrong sign, or a swapped word is a MISMATCH.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::driver::{self, Level};

mod support;

fn binutils(tool: &str) -> Option<Command> {
    let root = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty());
    let Some(root) = root else {
        assert!(std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1"), "GCC_M68K is not set");
        return None;
    };
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/softfloat_rt").join(name)
}

/// The text with every function label global and the directives gas needs
/// nothing of dropped.
fn globalised(text: &str) -> String {
    let mut out = String::from(".text\n");
    for line in text.lines() {
        let label = line
            .strip_suffix(':')
            .filter(|name| !name.starts_with(".L") && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        if let Some(name) = label {
            out.push_str(&format!(".globl {name}\n"));
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn run_with(cpu: &str) {
    let (Some(mut assemble), Some(mut assemble_rt), Some(mut link)) = (binutils("as"), binutils("as"), binutils("gcc"))
    else {
        return;
    };
    let Some(qemu) = support::qemu() else { return };
    let ir = std::fs::read_to_string(fixture("ops.ll")).unwrap();
    let text = driver::assembly(&ir, driver::cpu_of(cpu).expect("a cpu"), Level::O2).expect("compiles");
    let source = support::unique_path("softfloat-s");
    let ops = support::unique_path("softfloat-ops");
    let rt = support::unique_path("softfloat-rt");
    let exe = support::unique_path("softfloat-exe");
    std::fs::write(&source, globalised(&text)).unwrap();
    assert!(assemble.arg(format!("-m{cpu}")).arg("-o").arg(&ops).arg(&source).status().unwrap().success());
    assert!(assemble_rt.args(["-m68040", "-o"]).arg(&rt).arg(fixture("fpu_rt.s")).status().unwrap().success());
    let linked = link
        .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
        .arg(fixture("main.c"))
        .arg(&ops)
        .arg(&rt)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap();
    assert!(linked.success());
    let code = support::run(&qemu, &["-cpu", "m68040"], &exe);
    for path in [source, ops, rt, exe] {
        std::fs::remove_file(path).ok();
    }
    assert_eq!(code, Some(0), "{cpu}: the float code disagrees with gcc's (run with -- --nocapture to see which)");
}

#[test]
fn float_code_at_68000_agrees_with_gcc_calling_the_same_runtime() {
    run_with("68000");
}

#[test]
fn float_code_at_68020_agrees_with_gcc_calling_the_same_runtime() {
    run_with("68020");
}
