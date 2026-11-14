target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
declare i16 @llvm.sdiv.fix.i16(i16, i16, i32 immarg)
define void @f(ptr noalias %a, ptr noalias %b, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %s = getelementptr inbounds i16, ptr %a, i64 %i
  %v = load i16, ptr %s, align 2
  %m = call i16 @llvm.sdiv.fix.i16(i16 %v, i16 3, i32 2)
  %d = getelementptr inbounds i16, ptr %b, i64 %i
  store i16 %m, ptr %d, align 2
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1920
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
