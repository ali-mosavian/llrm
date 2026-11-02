//! A struct copy was six `move.l (d,%a0), (d,%a1)` of 6 bytes each and a
//! read of the copy at a displacement; the pointers walk with `(An)+`, 2
//! bytes each, and the later read takes the displacement less what they
//! stepped. The text, and the values under qemu.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// Copies 24 bytes between two pointers, then reads the copy at 8 and 20.
const COPY: &str = "
define i32 @f(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 24, i1 false)
  %p = getelementptr i8, ptr %d, i32 8
  %a = load volatile i32, ptr %p, align 4
  %q = getelementptr i8, ptr %d, i32 20
  %b = load volatile i32, ptr %q, align 4
  %r = add i32 %a, %b
  ret i32 %r
}
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1 immarg)";

/// The copy walks both pointers, and the read of the copy afterwards is
/// relative to where the destination pointer stopped.
#[test]
fn an_inline_copy_walks_both_pointers_and_later_reads_follow() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(COPY), "f", cpu, Options::default()).expect("compiles");
        let walked = out.lines().filter(|line| line.starts_with("\tmove.l\t(%a") && line.contains(")+, (%a")).count();
        assert!(walked >= 4, "{cpu:?}: {walked} copies walk both pointers:\n{out}");
        assert!(!out.contains("(20,%a") && out.contains("(-4,%a"), "{cpu:?}: the later read is not relative:\n{out}");
    }
}

/// `f()` copies a filled buffer and returns two of its longs summed, low byte.
fn buffers() -> String {
    let mut stores = String::new();
    for i in 0..6 {
        stores.push_str(&format!(
            "  %s{i} = getelementptr i8, ptr %src, i32 {}\n  store volatile i32 {}, ptr %s{i}, align 4\n",
            i * 4,
            i * 37 + 5
        ));
    }
    format!(
        "define i32 @f(i32 %n) {{
  %src = alloca [24 x i8], align 4
  %dst = alloca [24 x i8], align 4
{stores}  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %dst, ptr align 4 %src, i32 24, i1 false)
  %p = getelementptr i8, ptr %dst, i32 8
  %a = load volatile i32, ptr %p, align 4
  %q = getelementptr i8, ptr %dst, i32 20
  %b = load volatile i32, ptr %q, align 4
  %r = add i32 %a, %b
  %m = and i32 %r, 255
  ret i32 %m
}}
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1 immarg)"
    )
}

/// The destination holds what the source did, and the reads after the copy
/// find it: a displacement left unadjusted reads the wrong long.
#[test]
fn the_copy_and_the_reads_after_it_give_the_same_values() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[0]);
    all.extend(items_with(&module(&buffers()), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("chain");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(((2 * 37 + 5) + (5 * 37 + 5)) & 255));
}
