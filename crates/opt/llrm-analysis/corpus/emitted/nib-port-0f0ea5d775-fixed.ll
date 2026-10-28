target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @scaled(i32 %0, i32 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = call i32 @llvm.smul.fix.i32(i32 %0, i32 %1, i32 16)
  %3 = sext i32 %2 to i64
  %4 = sext i32 %1 to i64
  %5 = shl i64 %3, 16
  %6 = sdiv i64 %5, %4
  %7 = trunc i64 %6 to i32
  ret i32 %7
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) i32 @scaled(i32 98304, i32 147456)
  ret i16 0
}

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
