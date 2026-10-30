target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X%" = internal global [2 x i8] zeroinitializer
@"Y&" = internal global [4 x i8] zeroinitializer
@b$seg = external global [2 x i8]

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"X%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  %promoted = load i32, ptr @"Y&", align 1
  store volatile i8 0, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), !tbaa !2
  store volatile i8 0, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), i16 1), !tbaa !2
  %0 = load i16, ptr @"X%", !tbaa !2
  %1 = sext i16 %0 to i32
  %2 = add i32 %promoted, %1
  store volatile i8 37, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), align 2, !tbaa !2
  store volatile i8 11, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), i16 1), !tbaa !2
  %3 = load i16, ptr @"X%", !tbaa !2
  %4 = sext i16 %3 to i32
  %5 = add i32 %2, %4
  store volatile i8 74, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), align 2, !tbaa !2
  store volatile i8 22, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), i16 1), !tbaa !2
  %6 = load i16, ptr @"X%", !tbaa !2
  %7 = sext i16 %6 to i32
  %8 = add i32 %5, %7
  store volatile i8 111, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), align 2, !tbaa !2
  store volatile i8 33, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), i16 1), !tbaa !2
  %9 = load i16, ptr @"X%", !tbaa !2
  %10 = sext i16 %9 to i32
  %11 = add i32 %8, %10
  store volatile i8 -108, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), align 2, !tbaa !2
  store volatile i8 44, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), i16 1), !tbaa !2
  %12 = load i16, ptr @"X%", !tbaa !2
  %13 = sext i16 %12 to i32
  %14 = add i32 %11, %13
  store volatile i8 -71, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), align 2, !tbaa !2
  store volatile i8 55, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), i16 1), !tbaa !2
  %15 = load i16, ptr @"X%", !tbaa !2
  %16 = sext i16 %15 to i32
  %17 = add i32 %14, %16
  store i32 %17, ptr @"Y&", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$DSG0()
  %18 = load i16, ptr @"X%", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %18)
  %19 = load i32, ptr @"Y&", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %19)
  ret void
}

declare cc 1005 void @llrm.qb.B$DSG0() addrspace(1) nocallback nounwind

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
