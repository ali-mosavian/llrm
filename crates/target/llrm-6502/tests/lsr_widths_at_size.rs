//! LSR's native 16-bit widths hold at -Os too: the gate `native_widths: if size
//! { &[] }` left strnlen's loop with an index and a counter at -Os (69 B
//! against 50 B with the pointer IV).

use llrm_transforms::lsr_pass::LoopStrengthReduce;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8";

const SUM: &str = "define i16 @sum(ptr %p, i16 %n) {
entry:
  %g = icmp sgt i16 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %s = phi i16 [ 0, %entry ], [ %add, %loop ]
  %a = getelementptr i8, ptr %p, i16 %i
  %x = load i8, ptr %a
  %z = zext i8 %x to i16
  %add = add i16 %s, %z
  %next = add nuw nsw i16 %i, 1
  %c = icmp eq i16 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  %r = phi i16 [ 0, %entry ], [ %add, %loop ]
  ret i16 %r
}
";

fn reduced(size: bool) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n\n{SUM}");
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size, bytes_strict: false });
    manager.run_module(&mut module, llrm_6502::driver::machine()).expect("runs");
    llrm_mir::print::module(&module)
}

#[test]
fn a_counted_byte_loop_walks_a_pointer_at_size_as_at_speed() {
    let speed = reduced(false);
    assert!(speed.contains("phi ptr"), "premise: at speed the loop walks a pointer\n{speed}");
    assert_eq!(reduced(true), speed);
}
