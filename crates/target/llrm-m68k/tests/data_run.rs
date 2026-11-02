//! Global variables in the assembly, run under `qemu-m68k` against a C `main`
//! that gcc builds: initialised arrays, a string, a struct holding pointers
//! (one with an offset), a zero-filled global and a writable counter.

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
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt").join(name)
}

/// Every function label global, which is what `llrm-cc -S` leaves to the
/// caller.
fn globalised(text: &str) -> String {
    let mut out = String::from(".text\n");
    for line in text.lines() {
        let label = line
            .strip_suffix(':')
            .filter(|name| !name.starts_with(".L") && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        if let Some(name) = label.filter(|name| ["sum", "letter", "word_value", "through_table", "bump"].contains(name))
        {
            out.push_str(&format!(".globl {name}\n"));
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn run_with(cpu: &str) {
    let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
    let Some(qemu) = support::qemu() else { return };
    let ir = std::fs::read_to_string(fixture("globals.ll")).unwrap();
    let text = driver::assembly(&ir, driver::cpu_of(cpu).expect("a cpu"), Level::O2).expect("compiles");
    let source = support::unique_path("data-s");
    let object = support::unique_path("data-o");
    let exe = support::unique_path("data-exe");
    std::fs::write(&source, globalised(&text)).unwrap();
    assert!(assemble.arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{text}");
    let linked = link
        .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
        .arg(fixture("main.c"))
        .arg(&object)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap();
    assert!(linked.success(), "{text}");
    let code = support::run(&qemu, &[], &exe);
    for path in [source, object, exe] {
        std::fs::remove_file(path).ok();
    }
    assert_eq!(
        code,
        Some(0),
        "{cpu}: exit code names the first failing check (1 sum, 2 string, 3 i16, 4 pointers, 5 counter):\n{text}"
    );
}

/// A global was not in the assembly at all (`driver::assembly` printed the
/// functions only), so any program with an array, a string or a counter failed
/// to link ("undefined reference to `array`").
#[test]
fn globals_of_every_kind_are_laid_out_and_read_back_at_68000() {
    run_with("68000");
}

#[test]
fn globals_of_every_kind_are_laid_out_and_read_back_at_68020() {
    run_with("68020");
}
