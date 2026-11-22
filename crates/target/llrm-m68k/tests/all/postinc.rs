//! A byte-fill loop whose pointer is also read after the loop became a count-up
//! index from -n: `neg.l`, a biased base and a second saved register, 60 B
//! against 46 B with the pointer walk (bench/library/memset, since LG3).
//! `(An)+` steps the pointer for free, so the walk stays.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const FILL: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(ptr %p, i32 %v, i32 %n) {
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %pre, label %done
pre:
  %b = trunc i32 %v to i8
  br label %loop
loop:
  %i = phi i32 [ 0, %pre ], [ %i1, %loop ]
  %q = getelementptr i8, ptr %p, i32 %i
  store i8 %b, ptr %q
  %i1 = add i32 %i, 1
  %e = icmp eq i32 %i1, %n
  br i1 %e, label %done, label %loop
done:
  %end = getelementptr i8, ptr %p, i32 %n
  %last = getelementptr i8, ptr %end, i32 -1
  %x = load i8, ptr %last
  %y = sext i8 %x to i32
  ret i32 %y
}"#;

#[test]
fn a_fill_loop_steps_its_pointer_with_a_post_increment_not_an_index_from_minus_n() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let module = llrm_mir::parse::module(FILL).expect("parses");
        let out = text_with(&module, "k", cpu, Options { size: true, ..Options::default() }).expect("compiles");
        assert!(out.contains(")+"), "{cpu:?}: no (An)+ store:\n{out}");
        assert!(!out.contains("neg.l"), "{cpu:?}: a count-up index from -n:\n{out}");
    }
}
