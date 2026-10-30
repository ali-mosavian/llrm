target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@S$ = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"hi" }>
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"x\00" }>
@$float5 = internal constant [4 x i8] c"\00\00\C0?"
@$arguments$8 = internal global [4 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 3, ptr @"X%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @S$)
  call cc 1000 addrspace(1) void @llrm.qb.B$WRIT()
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %0 = load i16, ptr @"X%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @S$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(i16 0)
  call cc 1000 addrspace(1) void @llrm.qb.B$WRIT()
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 7)
  call cc 1000 addrspace(1) void @llrm.qb.B$WRIT()
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 -2)
  %1 = load float, ptr @$float5, !tbaa !2
  store float %1, ptr @$arguments$8, !tbaa !2
  %2 = load float, ptr @$arguments$8, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %2)
  ret void
}

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$WRIT() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
