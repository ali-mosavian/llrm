target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define i64 @f(i16 %seed) {
entry:
  %c = alloca [64 x i64], align 8
  br label %fill
fill:
  %k = phi i64 [ 0, %entry ], [ %kn, %fill ]
  %kk = trunc i64 %k to i16
  %kv = add i16 %kk, %seed
  %kx = zext i16 %kv to i64
  %kp = getelementptr [64 x i64], ptr %c, i64 0, i64 %k
  store i64 %kx, ptr %kp, align 8
  %kn = add nuw nsw i64 %k, 1
  %kc = icmp ult i64 %kn, 64
  br i1 %kc, label %oi, label %fill
oi:
  %i = phi i16 [ 0, %fill ], [ %in, %ol ]
  %s = phi i64 [ 0, %fill ], [ %tn, %ol ]
  br label %ij
ij:
  %j = phi i16 [ 0, %oi ], [ %jn, %ij ]
  %t = phi i64 [ %s, %oi ], [ %tn, %ij ]
  %iz = zext i16 %i to i64
  %jz = zext i16 %j to i64
  %i8 = shl nuw nsw i64 %iz, 3
  %idx = add nuw nsw i64 %i8, %jz
  %p = getelementptr [64 x i64], ptr %c, i64 0, i64 %idx
  %v = load i64, ptr %p, align 8
  %w = add nuw nsw i64 %idx, 1
  %m = mul i64 %v, %w
  %x1 = xor i64 %m, %w
  %y1 = lshr i64 %x1, 3
  %tn = add i64 %t, %y1
  %jn = add nuw nsw i16 %j, 1
  %cj = icmp ult i16 %jn, 8
  br i1 %cj, label %ij, label %ol
ol:
  %in = add nuw nsw i16 %i, 1
  %ci = icmp ult i16 %in, 8
  br i1 %ci, label %oi, label %out
out:
  ret i64 %tn
}
