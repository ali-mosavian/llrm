//! The mid-end merges adjacent constant stores to an aligned object into one
//! wider store, the first byte in the high bits on a big-endian target: bytes
//! 1, 2, 3, 4 were joined as 0x04030201 and read back as 67305985.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @main2() {
  %a = alloca [4 x i8], align 4
  store i8 1, ptr %a, align 4
  %b = getelementptr i8, ptr %a, i32 1
  store i8 2, ptr %b
  %c = getelementptr i8, ptr %a, i32 2
  store i8 3, ptr %c
  %d = getelementptr i8, ptr %a, i32 3
  store i8 4, ptr %d
  %w = load volatile i32, ptr %a, align 4
  ret i32 %w
}
";

/// The low byte of 0x01020304 is 4.
#[test]
fn merged_constant_stores_read_back_as_written() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(IR).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let mut all = start("main2", &[]);
        all.extend(items_with(&module, "main2", cpu, Options::default()).expect("compiles"));
        let path = support::unique_path("ir_store_merge");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(4), "{cpu:?}");
    }
}
