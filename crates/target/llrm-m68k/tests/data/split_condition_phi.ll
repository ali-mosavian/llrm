; ModuleID = 't.ll'
source_filename = "t.ll"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

define i32 @f(i32 %a, i32 %b) {
entry:
  %c0 = shl i32 %a, 1
  %c1 = icmp ugt i32 %c0, -16777217
  %c2 = shl i32 %b, 1
  %c3 = icmp ugt i32 %c2, -16777217
  %c4 = or i1 %c1, %c3
  br i1 %c4, label %out, label %body

body:                                             ; preds = %entry
  %e1 = lshr i32 %a, 23
  %e2 = and i32 %e1, 255
  %ea = tail call i32 @llvm.umax.i32(i32 %e2, i32 1)
  %f1 = lshr i32 %b, 23
  %f2 = and i32 %f1, 255
  %eb = tail call i32 @llvm.umax.i32(i32 %f2, i32 1)
  %m1 = and i32 %a, 8388607
  %m2 = or i32 %m1, 8388608
  %n1 = and i32 %b, 8388607
  %n2 = or i32 %n1, 8388608
  %x = lshr i32 %m2, 16
  %y = lshr i32 %n2, 16
  %m = mul nuw i32 %y, %x
  %n = icmp ult i32 %m, 32768
  %v = select i1 %n, i32 -127, i32 -126
  %e = add nsw i32 %ea, %eb
  %r = add nsw i32 %e, %v
  %g = icmp sgt i32 %r, 254
  %s = select i1 %g, i32 1, i32 0
  br label %out

out:                                              ; preds = %body, %entry
  %p = phi i32 [ %s, %body ], [ 0, %entry ]
  ret i32 %p
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.umax.i32(i32, i32) #0

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
