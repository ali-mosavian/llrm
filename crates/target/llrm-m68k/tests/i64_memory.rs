//! A big-endian `i64` load put its low dword at the address and its high at
//! +4, and the store the same: a copy through one was right, but a load of
//! the second dword of the copy, forwarded from the stored value, read the
//! source's first. A difftest seed copying two longs gave 201 for 200.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// Copies two longs (5 then 7) as one `i64`, then loads the copy's second long.
const COPY: &str = "
define i32 @f() {
  %src = alloca [8 x i8], align 2
  %dst = alloca [8 x i8], align 2
  store volatile i32 5, ptr %src, align 2
  %s1 = getelementptr i8, ptr %src, i32 4
  store volatile i32 7, ptr %s1, align 2
  %v = load i64, ptr %src, align 2
  store i64 %v, ptr %dst, align 2
  %d1 = getelementptr i8, ptr %dst, i32 4
  %b = load i32, ptr %d1, align 2
  ret i32 %b
}";

#[test]
fn the_second_long_of_an_i64_copy_is_the_sources_second_long() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{COPY}")).expect("parses");
    for options in [Options::PINNED, Options { mid_end: false, ..Options::default() }] {
        let mut all = start("f", &[]);
        all.extend(items_with(&module, "f", Cpu::M68000, options).expect("compiles"));
        let path = support::unique_path("i64-memory");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(7), "{options:?}");
    }
}
