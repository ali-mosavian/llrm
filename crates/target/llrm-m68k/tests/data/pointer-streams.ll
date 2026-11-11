target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @streams(ptr %p0, ptr %p1, ptr %p2, ptr %p3, ptr %p4, i32 %n) {
entry:
  br label %loop
loop:
  %q0 = phi ptr [ %p0, %entry ], [ %n0, %loop ]
  %q1 = phi ptr [ %p1, %entry ], [ %n1, %loop ]
  %q2 = phi ptr [ %p2, %entry ], [ %n2, %loop ]
  %q3 = phi ptr [ %p3, %entry ], [ %n3, %loop ]
  %q4 = phi ptr [ %p4, %entry ], [ %n4, %loop ]
  %i = phi i32 [ 0, %entry ], [ %i2, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s5, %loop ]
  %v0 = load i8, ptr %q0
  %w0 = zext i8 %v0 to i32
  %x0 = xor i32 %s, %w0
  %m0 = shl i32 %x0, 1
  %n0 = getelementptr i8, ptr %q0, i32 300
  %v1 = load i8, ptr %q1
  %w1 = zext i8 %v1 to i32
  %x1 = xor i32 %m0, %w1
  %m1 = shl i32 %x1, 2
  %n1 = getelementptr i8, ptr %q1, i32 500
  %v2 = load i8, ptr %q2
  %w2 = zext i8 %v2 to i32
  %x2 = xor i32 %m1, %w2
  %m2 = shl i32 %x2, 3
  %n2 = getelementptr i8, ptr %q2, i32 700
  %v3 = load i8, ptr %q3
  %w3 = zext i8 %v3 to i32
  %x3 = xor i32 %m2, %w3
  %m3 = shl i32 %x3, 4
  %n3 = getelementptr i8, ptr %q3, i32 900
  %v4 = load i8, ptr %q4
  %w4 = zext i8 %v4 to i32
  %x4 = xor i32 %m3, %w4
  %m4 = shl i32 %x4, 5
  %n4 = getelementptr i8, ptr %q4, i32 1100
  %s5 = add i32 %m4, %i
  %i2 = add i32 %i, 1
  %c = icmp ult i32 %i2, %n
  br i1 %c, label %loop, label %done
done:
  ret i32 %s5
}
