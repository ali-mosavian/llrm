//! A dense switch run under `qemu-m68k`: each case calls its own function, and
//! the indices either side of the table take the default. The table jump went
//! to the wrong block or off the table when its entries or its dispatch were
//! wrong (exit code 100 + x + 3 names the index, 50 + x the case that ran).

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

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/jump_table_rt/main.c")
}

fn switch() -> String {
    let mut text =
        String::from("target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n");
    for n in 0..10 {
        text += &format!("declare void @g{n}()\n");
    }
    text += "define void @f(i32 %x) {\nentry:\n  switch i32 %x, label %d [\n";
    for n in 0..10 {
        text += &format!("    i32 {n}, label %c{n}\n");
    }
    text += "  ]\n";
    for n in 0..10 {
        text += &format!("c{n}:\n  call void @g{n}()\n  ret void\n");
    }
    text + "d:\n  ret void\n}\n"
}

fn run_with(cpu: &str) {
    let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
    let Some(qemu) = support::qemu() else { return };
    let text = driver::assembly(&switch(), driver::cpu_of(cpu).expect("a cpu"), Level::O2).expect("compiles");
    assert!(text.contains("jmp\t(.LJTI") && text.matches("\t.word\t.LBB").count() == 10, "premise: a table:\n{text}");
    let source = support::unique_path("jt-s");
    let object = support::unique_path("jt-o");
    let exe = support::unique_path("jt-exe");
    std::fs::write(&source, format!(".text\n.globl f\n{text}")).unwrap();
    assert!(assemble.arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{text}");
    let linked = link
        .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
        .arg(fixture())
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
    assert_eq!(code, Some(0), "{cpu}:\n{text}");
}

#[test]
fn a_jump_table_dispatches_every_index_at_68000() {
    run_with("68000");
}

#[test]
fn a_jump_table_dispatches_every_index_at_68020() {
    run_with("68020");
}
