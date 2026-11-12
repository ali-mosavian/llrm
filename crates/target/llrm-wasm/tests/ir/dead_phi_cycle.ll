target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

declare void @report(i32)

define void @f(i32 %n, ptr %p) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i1, %latch ]
  %live = phi i32 [ 0, %entry ], [ %livenext, %latch ]
  %dead = phi i32 [ 0, %entry ], [ %deadnext, %latch ]
  %c = load volatile i32, ptr %p
  %t = icmp eq i32 %c, 0
  br i1 %t, label %a, label %b

a:
  br label %latch

b:
  %x = add i32 %c, 7
  br label %latch

latch:
  %livenext = phi i32 [ %live, %a ], [ %x, %b ]
  %deadnext = phi i32 [ %dead, %a ], [ %x, %b ]
  %i1 = add i32 %i, 1
  %e = icmp eq i32 %i1, %n
  br i1 %e, label %exit, label %h

exit:
  call void @report(i32 %livenext)
  ret void
}

