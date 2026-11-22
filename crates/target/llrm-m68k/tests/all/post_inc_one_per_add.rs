//! `p[i] = p[i] + q[i]` in a loop has one pointer increment, and the store
//! takes it (`move.w %d1, (%a0)+`). The pin deletes the add once a load or
//! store has taken it; this DAG kept the dead add, so the load took it too
//! with an updated base nobody read: `move.l %a0, %a2; add.w (%a2)+, %d1`
//! (particle -O2: +51% clocks over gcc).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

const LOOP: &str = "define void @f(ptr %p, ptr %q, i32 %n) {
entry:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %body, label %done
body:
  %i = phi i32 [ 0, %entry ], [ %j, %body ]
  %pa = getelementptr i16, ptr %p, i32 %i
  %qa = getelementptr i16, ptr %q, i32 %i
  %a = load i16, ptr %pa
  %b = load i16, ptr %qa
  %s = add i16 %a, %b
  store i16 %s, ptr %pa
  %j = add i32 %i, 1
  %e = icmp eq i32 %j, %n
  br i1 %e, label %done, label %body
done:
  ret void
}";

#[test]
fn a_pointer_incremented_once_is_incremented_by_one_access() {
    for cpu in ["68000", "68020"] {
        let out = assembly(&format!("{LAYOUT}{LOOP}"), cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        let increments = out.lines().filter(|line| line.contains(")+") && !line.contains("%sp")).count();
        assert!(increments <= 2, "{cpu}: {increments} post-increments for two pointers:\n{out}");
    }
}
