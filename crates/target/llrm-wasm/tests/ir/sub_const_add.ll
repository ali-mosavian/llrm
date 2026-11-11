target triple = "wasm32-unknown-unknown"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
define i32 @f(i32 %k) {
  %arr = alloca [48 x i16], align 16
  br label %L
L:
  %iv = phi i32 [ 3, %0 ], [ %ivn, %L ]
  %p = phi ptr [ %arr, %0 ], [ %pn, %L ]
  %a0 = add i32 %iv, -3
  %t0 = trunc i32 %a0 to i16
  %s0 = shl nuw nsw i16 %t0, 1
  %r0 = sub nsw i16 70, %s0
  store i16 %r0, ptr %p, align 8
  %a1 = add i32 %iv, -2
  %q1 = getelementptr i8, ptr %p, i32 2
  %t1 = trunc i32 %a1 to i16
  %s1 = shl nuw nsw i16 %t1, 1
  %r1 = sub nsw i16 70, %s1
  store i16 %r1, ptr %q1, align 2
  %a2 = add i32 %iv, -1
  %q2 = getelementptr i8, ptr %p, i32 4
  %t2 = trunc i32 %a2 to i16
  %s2 = shl nuw nsw i16 %t2, 1
  %r2 = sub nsw i16 70, %s2
  store i16 %r2, ptr %q2, align 4
  %q3 = getelementptr i8, ptr %p, i32 6
  %t3 = trunc i32 %iv to i16
  %s3 = shl nuw nsw i16 %t3, 1
  %r3 = sub nsw i16 70, %s3
  store i16 %r3, ptr %q3, align 2
  %pn = getelementptr i8, ptr %p, i32 8
  %ivn = add nuw nsw i32 %iv, 4
  %c = icmp eq i32 %ivn, 51
  br i1 %c, label %X, label %L
X:
  %g = getelementptr i8, ptr %arr, i32 %k
  %v = load volatile i16, ptr %g
  %z = zext i16 %v to i32
  ret i32 %z
}
