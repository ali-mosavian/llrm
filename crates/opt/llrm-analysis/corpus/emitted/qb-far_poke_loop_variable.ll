target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"Y&" = internal global [4 x i8] zeroinitializer
@b$seg = external global [2 x i8]
@"I%" = internal global [2 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 0, ptr @"X%", !tbaa !2
  %0 = addrspacecast ptr @"X%" to ptr addrspace(1)
  %1 = addrspacecast ptr addrspace(1) %0 to ptr addrspace(2)
  %2 = ptrtoint ptr addrspace(2) %1 to i16
  store i16 %2, ptr @b$seg, !tbaa !2
  store i16 0, ptr @"I%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
  %3 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %3, !tbaa !2
  br label %b2

b2:
  %4 = getelementptr i8, ptr @$data, i16 2
  %5 = load i16, ptr %4, !tbaa !2
  %6 = icmp sge i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sle i16 %7, %8
  br i1 %9, label %b5, label %b6

b4:
  %10 = load i16, ptr @"I%", !tbaa !2
  %11 = load i16, ptr @$data, !tbaa !2
  %12 = icmp sge i16 %10, %11
  br i1 %12, label %b5, label %b6

b5:
  %13 = addrspacecast ptr @"X%" to ptr addrspace(1)
  %14 = ptrtoint ptr addrspace(1) %13 to i16
  %15 = load i16, ptr @"I%", !tbaa !2
  %16 = mul i16 %15, 37
  %17 = and i16 %16, 255
  %18 = trunc i16 %17 to i8
  %19 = load i16, ptr @b$seg, !tbaa !2
  %20 = inttoptr i16 %19 to ptr addrspace(2)
  %21 = addrspacecast ptr addrspace(2) %20 to ptr addrspace(1)
  %22 = getelementptr i8, ptr addrspace(1) %21, i16 %14
  store volatile i8 %18, ptr addrspace(1) %13, !tbaa !2
  %23 = addrspacecast ptr @"X%" to ptr addrspace(1)
  %24 = ptrtoint ptr addrspace(1) %23 to i16
  %25 = add i16 %24, 1
  %26 = load i16, ptr @"I%", !tbaa !2
  %27 = mul i16 %26, 11
  %28 = and i16 %27, 127
  %29 = trunc i16 %28 to i8
  %30 = load i16, ptr @b$seg, !tbaa !2
  %31 = inttoptr i16 %30 to ptr addrspace(2)
  %32 = addrspacecast ptr addrspace(2) %31 to ptr addrspace(1)
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 %25
  %34 = getelementptr i8, ptr addrspace(1) %23, i16 1
  store volatile i8 %29, ptr addrspace(1) %34, !tbaa !2
  %35 = load i32, ptr @"Y&", !tbaa !2
  %36 = load i16, ptr @"X%", !tbaa !2
  %37 = sext i16 %36 to i32
  %38 = add i32 %35, %37
  store i32 %38, ptr @"Y&", !tbaa !2
  %39 = load i16, ptr @"I%", !tbaa !2
  %40 = getelementptr i8, ptr @$data, i16 2
  %41 = load i16, ptr %40, !tbaa !2
  %42 = add nsw i16 %39, %41
  store i16 %42, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  %43 = load i16, ptr @"X%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %43)
  %44 = load i32, ptr @"Y&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %44)
  ret void
}

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
