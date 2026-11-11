//! Arguments read from volatile globals were pushed as `move.l (g3,%pc),
//! -(%sp)`, `g2`, `g1`: the load-into-push fold (`callpush`) ran the three
//! volatile loads in reverse source order at every level.

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@g1 = global i32 0
@g2 = global i32 0
@g3 = global i32 0
declare void @use(i32, i32, i32)
define void @f() {
  %a = load volatile i32, ptr @g1
  %b = load volatile i32, ptr @g2
  %c = load volatile i32, ptr @g3
  call void @use(i32 %a, i32 %b, i32 %c)
  store volatile i32 1, ptr @g1
  ret void
}
";

#[test]
fn volatile_loads_feeding_call_arguments_run_in_source_order() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            let at = |name: &str| {
                text.find(&format!("({name},")).unwrap_or_else(|| panic!("{cpu} {level:?}: no {name}\n{text}"))
            };
            assert!(at("g1") < at("g2") && at("g2") < at("g3"), "{cpu} {level:?}: loads out of source order\n{text}");
        }
    }
}

/// `@probe`: three volatile loads through the pointers in `g1..g3`, then
/// `sink(a, b, c)`; written in the order g1, g2, g3, or g3, g2, g1 for
/// `reverse`.
fn probe_ir(reverse: bool) -> String {
    let loads =
        ["%a = load volatile i32, ptr %p1", "%b = load volatile i32, ptr %p2", "%c = load volatile i32, ptr %p3"];
    let order: Vec<&str> = if reverse { loads.iter().rev().copied().collect() } else { loads.to_vec() };
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@g1 = external global ptr
@g2 = external global ptr
@g3 = external global ptr
declare i32 @sink(i32, i32, i32)
define i32 @probe() {{
  %p1 = load ptr, ptr @g1
  %p2 = load ptr, ptr @g2
  %p3 = load ptr, ptr @g3
  {}
  %r = call i32 @sink(i32 %a, i32 %b, i32 %c)
  ret i32 %r
}}
",
        order.join("\n  ")
    )
}

/// The loads' page faults (one PROT_NONE page per global) give the order they
/// ran in, executed on qemu-m68k: `probe` must fault g1, g2, g3, and the
/// control (written g3, g2, g1) the reverse; a control that reads the same
/// order as `probe` would mean the instrument cannot tell.
#[test]
fn volatile_loads_fault_in_source_order_when_run() {
    let Some(root) = support::oracle_root() else { return };
    let Some(qemu) = support::qemu() else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            for (name, reverse) in [("probe", false), ("control", true)] {
                let define = reverse.then_some("-DREV");
                let text = assembly(&probe_ir(reverse), cpu_of(cpu).unwrap(), level).expect("compiles");
                let text = text.replace("probe:", ".globl probe\nprobe:");
                let source = support::unique_path("volatile-s");
                let object = support::unique_path("volatile-o");
                let exe = support::unique_path("volatile-exe");
                std::fs::write(&source, format!(".text\n{text}")).unwrap();
                assert!(
                    tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success()
                );
                let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/volatile_probe.c");
                let mut link = tool("gcc");
                link.args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"]);
                link.args(define).arg(main).arg(&object).arg("-o").arg(&exe);
                assert!(link.status().unwrap().success());
                let code = support::run(&qemu, &["-cpu", &format!("m{cpu}")], &exe);
                for path in [source, object, exe] {
                    std::fs::remove_file(path).ok();
                }
                assert_eq!(code, Some(0), "{cpu} {level:?} {name}: the loads ran out of the order the source wrote");
            }
        }
    }
}
