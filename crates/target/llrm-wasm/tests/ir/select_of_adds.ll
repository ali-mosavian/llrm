target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

declare i16 @llvm.umax.i16(i16, i16)

define void @f(ptr %base) {
entry:
  br label %loop

loop:
  %x = phi i32 [ 33, %entry ], [ %next, %loop ]
  %p = phi ptr [ %base, %entry ], [ %p2, %loop ]
  %m1 = add i32 %x, -1
  %hi = getelementptr i8, ptr %p, i32 64
  %b = load i16, ptr %hi, align 4
  %m33 = add i32 %x, -33
  %a = load i16, ptr %p, align 4
  %c = icmp ugt i16 %a, %b
  %max = tail call i16 @llvm.umax.i16(i16 %a, i16 %b)
  store i16 %max, ptr %hi, align 4
  %s = select i1 %c, i32 %m33, i32 %m1
  %g = getelementptr inbounds nuw [2 x i8], ptr %base, i32 %s
  store i16 %b, ptr %g, align 4
  %p2 = getelementptr i8, ptr %p, i32 4
  %next = add nuw nsw i32 %x, 2
  %done = icmp eq i32 %next, 65
  br i1 %done, label %exit, label %loop

exit:
  ret void
}
