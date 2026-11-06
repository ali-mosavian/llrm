; RUN: llrm-llc -O2 -march=i486 --isel=dag -S -o /dev/stdout %s
; c's ring at m16 ran `mov si,cx; and si,1023; shl si,2` and `add cx,5` per trip (+12000 clocks over main).
; The counter is the byte offset now: `and bx,4092`, `add cx,20`, no `shl`.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@buf = external global [1024 x i32], align 2

define i32 @ring() {
entry:
  br label %loop

loop:
  %i = phi i16 [ 3, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %m = and i16 %i, 1023
  %p = getelementptr inbounds nuw [4 x i8], ptr @buf, i16 %m
  %v = load i32, ptr %p, align 2
  %s2 = add nsw i32 %v, %s
  %next = add nuw nsw i16 %i, 5
  %c = icmp eq i16 %next, 30003
  br i1 %c, label %done, label %loop

done:
  ret i32 %s2
}

; CHECK-LABEL: _ring@
; CHECK: and bx, 4092
; CHECK: add cx, 20
; CHECK-NOT: shl
