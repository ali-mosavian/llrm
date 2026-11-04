//! `llvm.smul.fix.sat.i32` (clang's `_Sat` fixed-point multiply, tests/run/c
//! `fixed_point`) stopped the legalizer with `convert node to libcall`: with no
//! `MULHS`, no `SMUL_LOHI` and no legal `i64` multiply `forceExpandWideMUL` was
//! not translated. The pin calls `__muldi3`; so does this, and the results are
//! checked against a C reference under qemu.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::driver::{self, Level};

mod support;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixed_rt").join(name)
}

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

/// The scale-0 saturating multiply is a multiply with overflow, which is
/// `muls.l` there: a 68020 instruction the 68000 output must not use (the pin
/// emits it on the 68000 too; its own row).
const SFS0: &str = "
define i32 @sfs0(i32 %a, i32 %b) {
  %r = call i32 @llvm.smul.fix.sat.i32(i32 %a, i32 %b, i32 0)
  ret i32 %r
}
";

fn text(cpu: &str) -> String {
    let mut ir = std::fs::read_to_string(fixture("fx.ll")).unwrap();
    if cpu == "68020" {
        ir.push_str(SFS0);
    }
    driver::assembly(&ir, driver::cpu_of(cpu).expect("a cpu"), Level::O2).expect("compiles")
}

#[test]
fn a_fixed_point_multiply_calls_muldi3_as_the_pin_does() {
    for cpu in ["68000", "68020"] {
        let out = text(cpu);
        assert!(out.contains("__muldi3"), "no wide multiply routine on {cpu}:\n{out}");
    }
}

fn run_with(cpu: &str) {
    let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
    let Some(qemu) = support::qemu() else { return };
    let out = text(cpu);
    let source = support::unique_path("fixed-s");
    let object = support::unique_path("fixed-o");
    let exe = support::unique_path("fixed-exe");
    let mut globalised = String::from(".text\n");
    for line in out.lines() {
        if let Some(name) = line.strip_suffix(':').filter(|name| ["sf", "sfs", "uf", "ufs", "sfs0"].contains(name)) {
            globalised.push_str(&format!(".globl {name}\n"));
        }
        globalised.push_str(line);
        globalised.push('\n');
    }
    std::fs::write(&source, globalised).unwrap();
    assert!(assemble.arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{out}");
    let linked = link
        .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
        .arg(fixture("fx.c"))
        .arg(&object)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap();
    assert!(linked.success(), "{out}");
    let code = support::run(&qemu, &[], &exe);
    for path in [source, object, exe] {
        std::fs::remove_file(path).ok();
    }
    assert_eq!(
        code,
        Some(0),
        "{cpu}: exit code names the first wrong operation (1 smul.fix, 2 .sat, 3 umul.fix, 4 .sat, 5 sat at scale 0):\n{out}"
    );
}

#[test]
fn fixed_point_multiplies_match_the_c_reference_at_68000() {
    run_with("68000");
}

#[test]
fn fixed_point_multiplies_match_the_c_reference_at_68020() {
    run_with("68020");
}
