target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call i32 @llvm.smul.fix.i32(i32 2032, i32 4096, i32 8)
  %1 = ashr i32 %0, 31
  %2 = and i32 %1, 255
  %3 = add i32 %0, %2
  %4 = ashr i32 %3, 8
  %5 = trunc i32 %4 to i16
  ret i16 %5
}

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
