target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
@tbl = private unnamed_addr constant [12 x i8] c"\03\0E\0F\5C\41\23\59\4F\20\26\2E\1A", align 8
define i64 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i2, %join]
  %x = phi i64 [1, %entry], [%x2, %join]
  %acc = phi i64 [0, %entry], [%acc2, %join]
  %m = mul nsw i64 %x, 75
  %a = add nsw i64 %m, 74
  %x2 = srem i64 %a, 65537
  %t = trunc nsw i64 %x2 to i32
  %r = srem i32 %t, 10
  %s = sext i32 %r to i64
  %r2 = srem i32 %t, 14
  %cc = icmp ult i32 %r2, 12
  br i1 %cc, label %tb, label %mid
tb:
  %z = zext nneg i32 %r2 to i64
  %gp = getelementptr inbounds nuw i8, ptr @tbl, i64 %z
  %ld = load i8, ptr %gp, align 1
  %q0 = zext i8 %ld to i64
  br label %mid
mid:
  %ph = phi i64 [ %q0, %tb ], [ 1, %loop ]
  %p = mul nsw i64 %s, 37
  %q = mul nsw i64 %p, %s
  switch i64 %q, label %d [ i64 0, label %c0
                            i64 37, label %c1
                            i64 148, label %c2
 i64 333, label %c3
 i64 592, label %c4
 i64 925, label %c5
 i64 1332, label %c6 ]
c0: br label %join
c1: br label %join
c2: br label %join
c3: br label %join
c4: br label %join
c5: br label %join
c6: br label %join
d: br label %join
join:
  %v = phi i64 [1, %d], [11, %c0], [7, %c1], [23, %c2], [5, %c3], [19, %c4], [2, %c5], [13, %c6]
  %ac1 = add i64 %acc, %ph
  %acc2 = add i64 %ac1, %v
  %i2 = add i32 %i, 1
  %e = icmp eq i32 %i2, %n
  br i1 %e, label %out, label %loop
out:
  ret i64 %acc2
}
