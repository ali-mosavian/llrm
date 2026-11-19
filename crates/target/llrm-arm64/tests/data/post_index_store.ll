target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx14.0.0"

define void @step(ptr %p, i64 %n) {
entry:
  br label %loop
loop:
  %k = phi i64 [ %n, %entry ], [ %k1, %loop ]
  %a = phi ptr [ %p, %entry ], [ %a1, %loop ]
  %b = getelementptr i8, ptr %a, i64 160
  %v = load i16, ptr %b, align 2
  %w = add i16 %v, 1
  store i16 %w, ptr %a, align 2
  %a1 = getelementptr i8, ptr %a, i64 2
  %k1 = sub i64 %k, 1
  %c = icmp eq i64 %k1, 0
  br i1 %c, label %done, label %loop
done:
  ret void
}

define void @step_volatile(ptr %p, i64 %n) {
entry:
  br label %loop
loop:
  %k = phi i64 [ %n, %entry ], [ %k1, %loop ]
  %a = phi ptr [ %p, %entry ], [ %a1, %loop ]
  %b = getelementptr i8, ptr %a, i64 160
  %v = load i16, ptr %b, align 2
  %w = add i16 %v, 1
  store volatile i16 %w, ptr %a, align 2
  %a1 = getelementptr i8, ptr %a, i64 2
  %k1 = sub i64 %k, 1
  %c = icmp eq i64 %k1, 0
  br i1 %c, label %done, label %loop
done:
  ret void
}
