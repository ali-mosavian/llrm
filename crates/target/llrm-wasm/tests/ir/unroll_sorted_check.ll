target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@a = global [1024 x i32] zeroinitializer, align 16

; bench/quicksort's "is it sorted" loop: a[i - 1] > a[i] fails, 1023 trips, so
; the unroller makes three copies; the copies load each element twice.
define i32 @sorted() {
entry:
  br label %loop

loop:
  %i = phi i32 [ 1, %entry ], [ %next, %cont ]
  %p = getelementptr [4 x i8], ptr @a, i32 %i
  %q = getelementptr i8, ptr %p, i32 -4
  %x = load i32, ptr %q
  %y = load i32, ptr %p
  %bad = icmp sgt i32 %x, %y
  br i1 %bad, label %fail, label %cont

cont:
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 1024
  br i1 %done, label %ok, label %loop

fail:
  ret i32 -1

ok:
  ret i32 0
}
