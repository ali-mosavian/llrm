//! A variadic `double` read by our callee at the address the caller wrote it.
//! The pin's `expandVAArg` rounds the list pointer to the IR alignment of the
//! type (8: the datalayout has no f64 entry), but the m68k ABI packs stack
//! arguments at 4 and gcc callers do. The callee read the wrong word whenever
//! the first double followed an odd number of words: `sumhi` returned 0 on
//! the pin's output and on ours for `call_b` (M68K-VA-DOUBLE).

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const CALLEE: &str = include_str!("data_rt/va_double_sum.ll");
const CALLEE_AND_CALLERS: &str = include_str!("data_rt/va_double_callers.ll");

/// Our code for `ir` linked with the gcc-built `data_rt/va_double_main.c`
/// (callers from gcc unless `ours_callers`); the program's exit code: 0 right,
/// 1 `call_a` wrong, 2 `call_b` wrong.
fn run(
    ir: &str,
    cpu: &str,
    level: Level,
    ours_callers: bool,
) -> Option<i32> {
    let root = support::oracle_root()?;
    let qemu = support::qemu()?;
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    let mut text = assembly(ir, cpu_of(cpu).unwrap(), level).expect("compiles");
    for name in ["sumhi", "call_a", "call_b"] {
        text = text.replace(&format!("{name}:"), &format!(".globl {name}\n{name}:"));
    }
    let source = support::unique_path("va-double-s");
    let object = support::unique_path("va-double-o");
    let exe = support::unique_path("va-double-exe");
    std::fs::write(&source, format!(".text\n{text}")).unwrap();
    assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
    let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/va_double_main.c");
    let mut link = tool("gcc");
    link.args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"]);
    link.args(ours_callers.then_some("-DOURS_CALLERS")).arg(main).arg(&object).arg("-o").arg(&exe);
    assert!(link.status().unwrap().success());
    let code = support::run(&qemu, &["-cpu", &format!("m{cpu}")], &exe);
    for path in [source, object, exe] {
        std::fs::remove_file(path).ok();
    }
    code
}

#[test]
fn a_gcc_caller_and_our_caller_both_reach_our_callees_doubles() {
    if support::oracle_root().is_none() || support::qemu().is_none() {
        return;
    }
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let gcc_caller = run(CALLEE, cpu, level, false);
            assert_eq!(gcc_caller, Some(0), "{cpu} {level:?}: gcc caller -> our callee");
            let our_caller = run(CALLEE_AND_CALLERS, cpu, level, true);
            assert_eq!(our_caller, Some(0), "{cpu} {level:?}: our caller -> our callee");
        }
    }
}
