target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@px = external global [101 x i16]
@py = external global [101 x i16]
@pz = external global [101 x i16]

define i32 @f() {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s3, %loop ]
  %ax = getelementptr inbounds [101 x i16], ptr @px, i32 0, i32 %i
  %vx = load i16, ptr %ax
  %ex = sext i16 %vx to i32
  %s1 = add nsw i32 %s, %ex
  %ay = getelementptr inbounds [101 x i16], ptr @py, i32 0, i32 %i
  %vy = load i16, ptr %ay
  %ey = sext i16 %vy to i32
  %my = mul nsw i32 %ey, 3
  %s2 = add nsw i32 %s1, %my
  %az = getelementptr inbounds [101 x i16], ptr @pz, i32 0, i32 %i
  %vz = load i16, ptr %az
  %ez = sext i16 %vz to i32
  %mz = mul nsw i32 %ez, 7
  %s3 = add nsw i32 %s2, %mz
  %n = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %n, 101
  br i1 %c, label %done, label %loop

done:
  ret i32 %s3
}
