//! `bar` stores a sum only `main` read; `main` folds it, the store goes, `bar`
//! is a lone `rts`, and `foo` still called it (byvalue_struct_address -O1:
//! 56 B, clang 32 B). The second walk at -O1 prices the emptied callee.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@sum = internal global i32 0
declare void @report(i32)
define void @bar(ptr %x, i32 %f, i32 %g, i32 %h, i32 %i, i32 %j) {
  %a = load i32, ptr %x
  %p1 = getelementptr i8, ptr %x, i32 4
  %b0 = load i32, ptr %p1
  %b = mul i32 %b0, 10
  %p2 = getelementptr i8, ptr %x, i32 8
  %c0 = load i32, ptr %p2
  %c = mul i32 %c0, 100
  %p3 = getelementptr i8, ptr %x, i32 12
  %d0 = load i32, ptr %p3
  %d = mul i32 %d0, 1000
  %p4 = getelementptr i8, ptr %x, i32 16
  %e0 = load i32, ptr %p4
  %e = mul i32 %e0, 10000
  %s1 = add i32 %a, %b
  %s2 = add i32 %s1, %c
  %s3 = add i32 %s2, %d
  %s4 = add i32 %s3, %e
  %s5 = add i32 %s4, %f
  %s6 = add i32 %s5, %g
  %s7 = add i32 %s6, %h
  %s8 = add i32 %s7, %i
  %s9 = add i32 %s8, %j
  store i32 %s9, ptr @sum
  ret void
}
define void @foo(ptr %x) {
  call void @bar(ptr %x, i32 6, i32 7, i32 8, i32 9, i32 10)
  ret void
}
define i32 @main() {
  %x = alloca [5 x i32]
  store i32 1, ptr %x
  %p1 = getelementptr i8, ptr %x, i32 4
  store i32 2, ptr %p1
  %p2 = getelementptr i8, ptr %x, i32 8
  store i32 3, ptr %p2
  %p3 = getelementptr i8, ptr %x, i32 12
  store i32 4, ptr %p3
  %p4 = getelementptr i8, ptr %x, i32 16
  store i32 5, ptr %p4
  call void @foo(ptr %x)
  %v = load i32, ptr @sum
  call void @report(i32 %v)
  ret i32 0
}
";

#[test]
fn a_callee_the_folds_emptied_is_not_called_at_o1() {
    for cpu in ["68000", "68020"] {
        let out = assembly(IR, cpu_of(cpu).unwrap(), Level::O1).expect("compiles");
        let foo = out.split("foo:").nth(1).and_then(|rest| rest.split("main:").next()).unwrap_or("");
        assert!(!foo.contains("bar"), "{cpu}: `foo` still calls the empty `bar`:\n{out}");
    }
}
