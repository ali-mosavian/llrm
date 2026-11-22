//! `m = max(m, p[i])`: the coalescer joined `m' = v` (the load) before
//! `m' = m`, so `m` and `m'` took two registers and each trip copied on both
//! paths (arrmax -O2 on the 68000: `move.l %d0, %a1` and `move.l %a1, %d0`,
//! 156 clk against gcc's 138). Joining the loop-carried copy first leaves one
//! copy, on the path that takes the new value.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const MAX: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(ptr %p, i32 %n) {
entry:
  %f = load i32, ptr %p
  br label %loop
loop:
  %i = phi i32 [1, %entry], [%i1, %loop]
  %m = phi i32 [%f, %entry], [%m1, %loop]
  %q = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %q
  %c = icmp sgt i32 %v, %m
  %m1 = select i1 %c, i32 %v, i32 %m
  %i1 = add i32 %i, 1
  %d = icmp slt i32 %i1, %n
  br i1 %d, label %loop, label %exit
exit:
  ret i32 %m1
}
";

fn register_copies(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let line = line.trim();
            line.strip_prefix("move.l\t").is_some_and(|rest| {
                let (from, to) = rest.split_once(", ").unwrap_or(("", ""));
                [from, to].iter().all(|one| one.len() == 3 && one.starts_with('%'))
            })
        })
        .count()
}

#[test]
fn a_running_maximum_is_copied_on_one_path_only() {
    let module = llrm_mir::parse::module(MAX).expect("parses");
    let text = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    assert_eq!(register_copies(&text), 1, "{text}");
}
