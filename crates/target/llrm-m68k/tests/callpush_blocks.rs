//! A call sequence belongs to one block. Two paths each called `b(x, n)` and met in a block that
//! cleaned up both; the next cleanup after `b1`'s call was the odd path's own, so `b1`'s pushes
//! were paired with it and the odd path popped 8 bytes nothing pushed
//! (`shift_int32_variable` printed the unshifted value at every odd count, 12 cells).

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::driver::{self, Level};

mod support;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt").join(name)
}

fn run_with(cpu: &str) {
    let Some(root) = support::oracle_root() else { return };
    let Some(qemu) = support::qemu() else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    let ir = std::fs::read_to_string(fixture("diamond_calls.ll")).unwrap();
    let text = driver::assembly(&ir, driver::cpu_of(cpu).expect("a cpu"), Level::O2).expect("compiles");
    let source = support::unique_path("blocks-s");
    let object = support::unique_path("blocks-o");
    let exe = support::unique_path("blocks-exe");
    std::fs::write(&source, format!(".text\n.globl diamonds\n{text}")).unwrap();
    assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{text}");
    let linked = tool("gcc")
        .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
        .arg(fixture("diamond_calls.c"))
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
    assert_eq!(code, Some(0), "{cpu}: the shifted values summed wrong:\n{text}");
}

#[test]
fn a_call_on_each_path_of_a_diamond_keeps_its_own_cleanup_at_68000() {
    run_with("68000");
}

#[test]
fn a_call_on_each_path_of_a_diamond_keeps_its_own_cleanup_at_68020() {
    run_with("68020");
}
