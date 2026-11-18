target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@screen = external global [4000 x i8], align 1

; for (o = 0; o < 4000; o += 2) { screen[o] = 65 + (o & 14); screen[o + 1] = 31; }
define void @f() {
entry:
  br label %loop
loop:
  %o = phi i64 [ 0, %entry ], [ %next, %loop ]
  %t = trunc i64 %o to i8
  %m = and i8 %t, 14
  %v = or disjoint i8 %m, 65
  %a = getelementptr inbounds nuw i8, ptr @screen, i64 %o
  store i8 %v, ptr %a, align 1, !tbaa !1
  %b = getelementptr i8, ptr %a, i64 1
  store i8 31, ptr %b, align 1, !tbaa !1
  %next = add nuw nsw i64 %o, 2
  %done = icmp eq i64 %next, 4000
  br i1 %done, label %exit, label %loop
exit:
  ret void
}

!1 = !{!2, !2, i64 0}
!2 = !{!"omnipotent char", !3, i64 0}
!3 = !{!"Simple C/C++ TBAA"}
