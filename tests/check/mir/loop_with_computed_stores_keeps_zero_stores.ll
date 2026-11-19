; RUN: llrm-llc -O2 -march=i486 --isel=dag -S -o /dev/stdout %s
; A loop that stays for two computed stores lost its two zero stores to a 24-byte memset each (`rep stos`), +24..26 B over the
; loop alone for 6 stores saved (nbody_fixed bas/bq16 -O2). A fill must pay for the stores it removes, not for the loop.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@px = global [6 x i32] zeroinitializer
@py = global [6 x i32] zeroinitializer
@vx = global [6 x i32] zeroinitializer
@vy = global [6 x i32] zeroinitializer

define void @init(i32 %s) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %x = mul i32 %i, %s
  %y = add i32 %x, %i
  %a = getelementptr [6 x i32], ptr @px, i32 0, i32 %i
  store i32 %x, ptr %a
  %b = getelementptr [6 x i32], ptr @py, i32 0, i32 %i
  store i32 %y, ptr %b
  %c = getelementptr [6 x i32], ptr @vx, i32 0, i32 %i
  store i32 0, ptr %c
  %d = getelementptr [6 x i32], ptr @vy, i32 0, i32 %i
  store i32 0, ptr %d
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 6
  br i1 %done, label %exit, label %loop

exit:
  ret void
}

; CHECK-LABEL: _init@
; CHECK-NOT: rep stos
