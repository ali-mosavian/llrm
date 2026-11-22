target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-linux-gnu"

@g = global [64 x i8] zeroinitializer, align 8

declare i32 @c(i32)

define void @f(ptr %p0, ptr %p1, ptr %p2, ptr %p3, ptr %p4, i32 %v) minsize optsize nounwind {
entry:
  %a = getelementptr inbounds i8, ptr @g, i64 8
  %b = trunc i32 %v to i8
  %q2 = getelementptr inbounds i8, ptr @g, i64 10
  %q3 = getelementptr inbounds i8, ptr @g, i64 11
  store i8 %b, ptr %q2
  store i8 0, ptr %q3
  store i8 0, ptr %a
  %r1 = call i32 @c(i32 1)
  store i8 1, ptr %p0
  %r2 = call i32 @c(i32 %r1)
  store i8 2, ptr %p1
  %r3 = call i32 @c(i32 %r2)
  store i8 3, ptr %p2
  %r4 = call i32 @c(i32 %r3)
  store i8 4, ptr %p3
  %r5 = call i32 @c(i32 %r4)
  %l = load i8, ptr %q2
  %t = icmp ne i8 %l, 0
  br i1 %t, label %late, label %done

late:
  %r6 = call i32 @c(i32 %r5)
  store i8 5, ptr %p4
  %r7 = call i32 @c(i32 %r6)
  store i8 1, ptr %a
  br label %done

done:
  ret void
}
