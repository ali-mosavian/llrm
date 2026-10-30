target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@$arguments$2 = internal global [16 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1000 addrspace(1) ptr @llrm.qb.B$DTSR(i16 1992, i16 2, i16 29)
  %1 = load double, ptr %0
  store double %1, ptr @"D#", !tbaa !2
  %2 = load double, ptr @"D#", !tbaa !2
  store double %2, ptr @$arguments$2, !tbaa !2
  %3 = load double, ptr @$arguments$2, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %3)
  %4 = call cc 1000 addrspace(1) ptr @llrm.qb.B$DTSR(i16 1899, i16 12, i16 30)
  %5 = load double, ptr %4
  %6 = getelementptr i8, ptr @$arguments$2, i16 8
  store double %5, ptr %6, !tbaa !2
  %7 = getelementptr i8, ptr @$arguments$2, i16 8
  %8 = load double, ptr %7, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %8)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 ptr @llrm.qb.B$DTSR(i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
