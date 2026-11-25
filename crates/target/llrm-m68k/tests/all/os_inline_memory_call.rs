//! A static helper of two fills and a move of unknown length counted each as
//! one flat instruction in the -Os size guard, so it was copied into every
//! caller: file.c `justify` (94 B) went into `set` three times, 408 B against
//! gcc's 220 B. The guard counts them as the `jsr` and three pushed words
//! they stay.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn module(callers: usize) -> String {
    let mut text = String::from(
        r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
define internal void @fill(ptr %d, i32 %n, ptr %s) {
  call void @llvm.memset.p0.i32(ptr %d, i8 32, i32 %n, i1 false)
  %p = getelementptr i8, ptr %d, i32 %n
  call void @llvm.memmove.p0.p0.i32(ptr %p, ptr %s, i32 %n, i1 false)
  %q = getelementptr i8, ptr %p, i32 %n
  call void @llvm.memset.p0.i32(ptr %q, i8 32, i32 %n, i1 false)
  ret void
}
"#,
    );
    for index in 0..callers {
        text += &format!(
            "define void @c{index}(ptr %d, i32 %n, ptr %s) #0 {{\n  call void @fill(ptr %d, i32 %n, ptr %s)\n  store i8 {index}, ptr %d\n  ret void\n}}\n"
        );
    }
    text + "attributes #0 = { optsize }"
}

#[test]
fn a_helper_of_variable_fills_and_a_move_stays_a_call_at_os() {
    for cpu in ["68000", "68020"] {
        let out = assembly(&module(4), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let moves = out.lines().filter(|line| line.contains("memmove")).count();
        assert_eq!(moves, 1, "{cpu}: fill was copied into its call sites ({moves} memmove calls):\n{out}");
    }
}
