//! An object aligned to 16 was refused ("needs stack realignment, not supported
//! yet"), so the 16 cells of `over_aligned_local_after_even_pushes` (4 CPUs x 4
//! levels) had no `ours`. The frame now realigns the stack pointer as the pin
//! does (`M68kFrameLowering::emitPrologue`, `BuildStackAlignAND`): a frame
//! pointer, the registers saved from it, `and` of `%sp`.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// The address of a 16-aligned local, modulo 16, with a call in the way so the
/// object lives in the frame.
fn module(align: u32) -> llrm_mir::module::Module {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"
define i32 @f() {{
  %a = alloca [16 x i8], align {align}
  call void @use(ptr %a)
  %i = ptrtoint ptr %a to i32
  %m = and i32 %i, {mask}
  ret i32 %m
}}
define void @use(ptr %p) {{ ret void }}",
        mask = align - 1
    );
    llrm_mir::parse::module(&ir).expect("parses")
}

fn options() -> Options {
    Options { mid_end: true, ..Options::default() }
}

#[test]
fn an_object_aligned_to_16_is_accepted_and_the_stack_pointer_is_anded() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(16), "f", cpu, options()).expect("compiles");
        assert!(out.contains("link"), "{cpu:?}: no frame pointer:\n{out}");
        assert!(out.contains("and.l\t#-16, %d0"), "{cpu:?}: the stack pointer is not rounded down:\n{out}");
    }
}

#[test]
fn the_object_is_16_aligned_whatever_the_stack_pointer_was() {
    let Some(qemu) = support::qemu() else { return };
    // each extra argument moves the stack pointer at entry by 4
    for pushed in 0..4 {
        let mut all = start("f", &vec![0_i64; pushed]);
        all.extend(items_with(&module(16), "f", Cpu::M68020, options()).unwrap());
        all.extend(items_with(&module(16), "use", Cpu::M68020, options()).unwrap());
        let path = support::unique_path("realign");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68020"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(0), "{pushed} words pushed before the call: the address mod 16");
    }
}

/// Realigning for an 8-aligned slot (a `double` spill) put a `link`, a rounded
/// `%sp` and register saves below the frame pointer into every function that
/// holds one: nbody +42 B and bitcount_u64 +40 B at -O1 68000. The frame keeps
/// 4 there, as it did.
#[test]
fn an_object_aligned_to_8_does_not_realign_the_shipped_frame() {
    let options = Options { short: true, ..options() };
    let out = text_with(&module(8), "f", Cpu::M68000, options).expect("compiles");
    assert!(!out.contains("link") && !out.contains("and.l\t#-"), "an 8-aligned object realigned:\n{out}");
}
