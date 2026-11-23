//! clang leaves `_Atomic int` access on M68k as the generic `__atomic_load` / `__atomic_store` call (its target info
//! sets no inline atomic width); QuickrBasic `tick.c` Os was 700 B against gcc's 362 B with 15 such calls. A 4-byte
//! access is a plain `move` (M68kInstrAtomics.td).

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn a_generic_atomic_load_or_store_of_a_long_is_a_move_not_a_call() {
    let source = include_str!("../data/atomic_generic_libcall.ll");
    for cpu in ["68000", "68020"] {
        let text = assembly(source, cpu_of(cpu).unwrap(), Level::Os).unwrap_or_else(|why| panic!("{cpu}: {why}"));
        let body = |name: &str| text.split_once(&format!("{name}:")).map_or("", |(_, rest)| rest.split(".globl").next().unwrap_or(""));
        for name in ["started_now", "start"] {
            assert!(!body(name).contains("__atomic"), "{cpu}: {name} stayed a call:\n{text}");
        }
        assert!(body("started_now").contains("started"), "{cpu}: no access to the flag:\n{text}");
        // an address that may be odd (a 68000 address error) or unknown keeps the call
        for name in ["odd_now", "through_pointer"] {
            assert!(body(name).contains("__atomic_load"), "{cpu}: {name} lost its call:\n{text}");
        }
    }
}

/// Linked with no atomic routine, the object runs at 68000 and 68020: the program stores through the generic store,
/// reads back through the generic load and returns the value plus one.
#[test]
fn the_inlined_generic_accesses_run_at_68000_and_68020() {
    use std::process::Command;
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@flag = global i32 0, align 4
declare void @__atomic_load(i32, ptr, ptr, i32)
declare void @__atomic_store(i32, ptr, ptr, i32)
define i32 @stored_back(i32 %x) {
  %in = alloca i32, align 4
  %out = alloca i32, align 4
  store i32 %x, ptr %in, align 4
  call void @__atomic_store(i32 4, ptr @flag, ptr %in, i32 5)
  call void @__atomic_load(i32 4, ptr @flag, ptr %out, i32 5)
  %v = load i32, ptr %out, align 4
  %r = add i32 %v, 1
  ret i32 %r
}";
    let (Some(root), Some(qemu)) = (crate::support::oracle_root(), crate::support::qemu()) else { return };
    for cpu in ["68000", "68020"] {
        let bytes = llrm_m68k::driver::object(ir, cpu_of(cpu).unwrap(), Level::Os, "atomic").expect("compiles");
        let (object, exe) = (crate::support::unique_path("atomic-obj"), crate::support::unique_path("atomic-exe"));
        std::fs::write(&object, bytes).unwrap();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/atomic_rt/main.c");
        let linked = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-gcc"))
            .env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"))
            .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
            .arg(fixture)
            .arg(&object)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(linked.success(), "{cpu}: the object needs an atomic routine");
        assert_eq!(crate::support::run(&qemu, &[], &exe), Some(0), "{cpu}");
    }
}
