target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@planes = global [3 x [16 x [256 x i8]]] zeroinitializer, align 1
@line = global [771 x i8] zeroinitializer, align 1

; jpeg_fixed's convert, one line of the YCbCr to BGR conversion: the chroma
; planes are read at `x >> hshift` and the three results are stored at
; `3 * x`, `3 * x + 1` and `3 * x + 2`.
define void @f(i64 %width, i32 %hshift, i64 %row) {
entry:
  %luma_row = getelementptr inbounds [256 x i8], ptr @planes, i64 %row
  %cb_row = getelementptr inbounds [256 x i8], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 4096), i64 %row
  %cr_row = getelementptr inbounds [256 x i8], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 8192), i64 %row
  br label %loop
loop:
  %x = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pl = getelementptr inbounds nuw i8, ptr %luma_row, i64 %x
  %l8 = load i8, ptr %pl, align 1
  %l32 = zext i8 %l8 to i32
  %luma = shl nuw nsw i32 %l32, 10
  %x32 = trunc nuw nsw i64 %x to i32
  %xs = lshr i32 %x32, %hshift
  %xi = zext nneg i32 %xs to i64
  %pb = getelementptr inbounds nuw i8, ptr %cb_row, i64 %xi
  %b8 = load i8, ptr %pb, align 1
  %b32 = zext i8 %b8 to i32
  %b10 = shl nuw nsw i32 %b32, 10
  %blue = add nsw i32 %b10, -131072
  %pr = getelementptr inbounds nuw i8, ptr %cr_row, i64 %xi
  %r8 = load i8, ptr %pr, align 1
  %r32 = zext i8 %r8 to i32
  %r10 = shl nuw nsw i32 %r32, 10
  %red = add nsw i32 %r10, -131072
  %t1 = mul nsw i32 %blue, 1815
  %t2 = ashr exact i32 %t1, 10
  %base = or disjoint i32 %luma, 512
  %s1 = add nsw i32 %t2, %base
  %d1 = sdiv i32 %s1, 1024
  %lo1 = tail call i32 @llvm.smax.i32(i32 %d1, i32 0)
  %hi1 = tail call i32 @llvm.umin.i32(i32 %lo1, i32 255)
  %o1 = trunc nuw i32 %hi1 to i8
  %x3 = mul nuw nsw i64 %x, 3
  %q0 = getelementptr inbounds nuw i8, ptr @line, i64 %x3
  store i8 %o1, ptr %q0, align 1
  %u1 = mul nsw i32 %blue, 11
  %u2 = ashr exact i32 %u1, 5
  %u3 = mul nsw i32 %red, 731
  %u4 = ashr exact i32 %u3, 10
  %u5 = add nsw i32 %u2, %u4
  %u6 = sub nsw i32 %base, %u5
  %d2 = sdiv i32 %u6, 1024
  %lo2 = tail call i32 @llvm.smax.i32(i32 %d2, i32 0)
  %hi2 = tail call i32 @llvm.umin.i32(i32 %lo2, i32 255)
  %o2 = trunc nuw i32 %hi2 to i8
  %q1 = getelementptr inbounds nuw i8, ptr %q0, i64 1
  store i8 %o2, ptr %q1, align 1
  %v1 = mul nsw i32 %red, 359
  %v2 = ashr exact i32 %v1, 8
  %v3 = add nsw i32 %v2, %base
  %d3 = sdiv i32 %v3, 1024
  %lo3 = tail call i32 @llvm.smax.i32(i32 %d3, i32 0)
  %hi3 = tail call i32 @llvm.umin.i32(i32 %lo3, i32 255)
  %o3 = trunc nuw i32 %hi3 to i8
  %q2 = getelementptr inbounds nuw i8, ptr %q0, i64 2
  store i8 %o3, ptr %q2, align 1
  %next = add nuw nsw i64 %x, 1
  %done = icmp eq i64 %next, %width
  br i1 %done, label %exit, label %loop
exit:
  ret void
}

declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.umin.i32(i32, i32)
