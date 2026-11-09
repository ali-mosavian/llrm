//! `store i8 9, (buf + k + 500)` with `buf` a PC-relative local symbol had no pattern: `select_arii` refused the
//! constant 500 as an index (for `ARID` to take it), but `ARID` takes one base register, and `buf + k` is two.
//! `x_strlen` failed to compile at every level (`CannotSelect(Store)`); `scroll` at -O0 and -Oz.

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const IR: &str = include_str!("data_rt/far_index_store.ll");

#[test]
fn a_store_through_a_symbol_an_index_and_a_far_displacement_runs() {
    let (Some(root), Some(qemu)) = (support::oracle_root(), support::qemu()) else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            let source = support::unique_path("far-s");
            let object = support::unique_path("far-o");
            let exe = support::unique_path("far-exe");
            std::fs::write(&source, format!(".text\n{text}")).unwrap();
            assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
            let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/far_index_store_main.c");
            let mut link = tool("gcc");
            link.args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"]);
            link.arg(main).arg(&object).arg("-o").arg(&exe);
            assert!(link.status().unwrap().success());
            let code = support::run(&qemu, &["-cpu", &format!("m{cpu}")], &exe);
            for path in [source, object, exe] {
                std::fs::remove_file(path).ok();
            }
            assert_eq!(code, Some(0), "{cpu} {level:?}: a store missed buf[500 + k]");
        }
    }
}
