target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@line = internal global [3 x i8] zeroinitializer, align 1
@planes = internal global [8 x i8] zeroinitializer, align 1
@other = internal global [8 x i8] zeroinitializer, align 1

define void @f(ptr %q, i32 %n, i32 %i) {
entry:
  br label %loop
loop:
  %p = phi ptr [ %q, %entry ], [ %p3, %loop ]
  %k = phi i32 [ 0, %entry ], [ %k1, %loop ]
  %a1 = getelementptr i8, ptr @planes, i32 %i
  %a = load i8, ptr %a1, align 1
  %b1 = getelementptr i8, ptr @other, i32 %i
  %b = load i8, ptr %b1, align 1
  store i8 %a, ptr %p, align 1
  %p1 = getelementptr i8, ptr %p, i32 1
  store i8 %b, ptr %p1, align 1
  %p3 = getelementptr i8, ptr %p, i32 3
  %k1 = add i32 %k, 1
  %c = icmp ult i32 %k1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
