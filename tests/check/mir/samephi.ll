; RUN: llrm-mir %s
; CHECK: define {{.*}} @_f(
; A join whose phi takes the same value from both ways: a trivial phi, which a
; run of the spiller removes, and no pressure to spill anything.

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i16 @_f(i16 %n, i16 %k) addrspace(1) {
b0:
  %c = icmp sgt i16 %n, 0
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  br label %j

j:
  %t = phi i16 [ %k, %a ], [ %k, %b ]
  %r = add i16 %t, %n
  ret i16 %r
}
