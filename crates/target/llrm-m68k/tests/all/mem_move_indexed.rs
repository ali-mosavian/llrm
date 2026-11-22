//! A byte loaded through `(An)+` and stored at `(d,An,Dn)` went through a
//! data register (`move.b (%a0)+, %d2; move.b %d2, (0,%a1,%d3)`, a register
//! and 2 B more); the pin's `move.b (An)+, (d8,An,Xn)` does it in one. The
//! store index `i*i` is not affine, so LSR keeps it as an index.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const COPY: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(ptr %d, ptr %s, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %acc = phi i32 [0, %entry], [%acc1, %loop]
  %p = phi ptr [%s, %entry], [%p1, %loop]
  %v = load i8, ptr %p
  %p1 = getelementptr i8, ptr %p, i32 1
  %sq0 = mul i32 %i, %i
  %q = getelementptr i8, ptr %d, i32 %sq0
  store i8 %v, ptr %q
  %sq = add i32 %sq0, 1
  %acc1 = add i32 %acc, %sq
  %i1 = add i32 %i, 1
  %e = icmp eq i32 %i1, %n
  br i1 %e, label %done, label %loop
done:
  ret i32 %acc1
}";

#[test]
fn a_post_incremented_byte_is_stored_indexed_without_a_temporary() {
    for level in [Level::O2, Level::Os] {
        let out = assembly(COPY, cpu_of("68020").unwrap(), level).expect("compiles");
        let moved = out.lines().any(|line| line.starts_with("\tmove.b\t(%a") && line.contains(")+, (0,%a"));
        assert!(moved, "no move from (An)+ to an indexed destination:\n{out}");
    }
}
