target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"Z%" = internal global [2 x i8] zeroinitializer
@"W%" = internal global [2 x i8] zeroinitializer
@"ROUND%" = internal global [2 x i8] zeroinitializer
@"HI%" = internal global [2 x i8] zeroinitializer
@"LO%" = internal global [2 x i8] zeroinitializer
@"BSE%" = internal global [2 x i8] zeroinitializer
@"SG%" = internal global [2 x i8] zeroinitializer
@b$seg = external global [2 x i8]
@"FOUND%" = internal global [2 x i8] zeroinitializer
@"O%" = internal global [2 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 1, ptr @"W%", !tbaa !2
  store i16 1, ptr @"ROUND%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"ROUND%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"ROUND%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = load i16, ptr @"ROUND%", !tbaa !2
  %11 = mul i16 257, %10
  %12 = add i16 21000, %11
  store i16 %12, ptr @"Z%", !tbaa !2
  %13 = load i16, ptr @"Z%", !tbaa !2
  %14 = sext i16 %13 to i32
  %15 = sdiv i32 %14, 256
  %16 = trunc i32 %15 to i16
  store i16 %16, ptr @"HI%", !tbaa !2
  %17 = load i16, ptr @"Z%", !tbaa !2
  %18 = and i16 %17, 255
  store i16 %18, ptr @"LO%", !tbaa !2
  %19 = addrspacecast ptr @"W%" to ptr addrspace(1)
  %20 = ptrtoint ptr addrspace(1) %19 to i16
  %21 = sub i16 %20, 1024
  store i16 %21, ptr @"BSE%", !tbaa !2
  %22 = load i16, ptr @"BSE%", !tbaa !2
  %23 = icmp slt i16 %22, 0
  br i1 %23, label %b7, label %b8

b6:
  %24 = load i16, ptr @"Z%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %24)
  ret void

b7:
  store i16 0, ptr @"BSE%", !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  %25 = addrspacecast ptr @"W%" to ptr addrspace(1)
  %26 = addrspacecast ptr addrspace(1) %25 to ptr addrspace(2)
  %27 = ptrtoint ptr addrspace(2) %26 to i16
  %28 = add i16 %27, 0
  store i16 %28, ptr @"SG%", !tbaa !2
  %29 = load i16, ptr @"SG%", !tbaa !2
  store i16 %29, ptr @b$seg, !tbaa !2
  store i16 -1, ptr @"FOUND%", !tbaa !2
  %30 = load i16, ptr @"BSE%", !tbaa !2
  store i16 %30, ptr @"O%", !tbaa !2
  %31 = load i16, ptr @"BSE%", !tbaa !2
  %32 = add i16 %31, 2048
  %33 = getelementptr i8, ptr @$data, i16 4
  store i16 %32, ptr %33, !tbaa !2
  %34 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %34, !tbaa !2
  br label %b10

b10:
  %35 = getelementptr i8, ptr @$data, i16 6
  %36 = load i16, ptr %35, !tbaa !2
  %37 = icmp sge i16 %36, 0
  br i1 %37, label %b11, label %b12

b11:
  %38 = load i16, ptr @"O%", !tbaa !2
  %39 = getelementptr i8, ptr @$data, i16 4
  %40 = load i16, ptr %39, !tbaa !2
  %41 = icmp sle i16 %38, %40
  br i1 %41, label %b13, label %b14

b12:
  %42 = load i16, ptr @"O%", !tbaa !2
  %43 = getelementptr i8, ptr @$data, i16 4
  %44 = load i16, ptr %43, !tbaa !2
  %45 = icmp sge i16 %42, %44
  br i1 %45, label %b13, label %b14

b13:
  %46 = load i16, ptr @"O%", !tbaa !2
  %47 = load i16, ptr @b$seg, !tbaa !2
  %48 = inttoptr i16 %47 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 %46
  %51 = load volatile i8, ptr addrspace(1) %50
  %52 = zext i8 %51 to i16
  %53 = load i16, ptr @"LO%", !tbaa !2
  %54 = icmp eq i16 %52, %53
  %55 = sext i1 %54 to i16
  %56 = load i16, ptr @"O%", !tbaa !2
  %57 = add i16 %56, 1
  %58 = load i16, ptr @b$seg, !tbaa !2
  %59 = inttoptr i16 %58 to ptr addrspace(2)
  %60 = addrspacecast ptr addrspace(2) %59 to ptr addrspace(1)
  %61 = getelementptr i8, ptr addrspace(1) %60, i16 %57
  %62 = load volatile i8, ptr addrspace(1) %61
  %63 = zext i8 %62 to i16
  %64 = load i16, ptr @"HI%", !tbaa !2
  %65 = icmp eq i16 %63, %64
  %66 = sext i1 %65 to i16
  %67 = and i16 %55, %66
  %68 = icmp ne i16 %67, 0
  br i1 %68, label %b15, label %b16

b14:
  %69 = load i16, ptr @"FOUND%", !tbaa !2
  %70 = icmp sge i16 %69, 0
  br i1 %70, label %b19, label %b20

b15:
  %71 = load i16, ptr @"O%", !tbaa !2
  store i16 %71, ptr @"FOUND%", !tbaa !2
  br label %b14

b16:
  br label %b17

b17:
  %72 = load i16, ptr @"O%", !tbaa !2
  %73 = getelementptr i8, ptr @$data, i16 6
  %74 = load i16, ptr %73, !tbaa !2
  %75 = add nsw i16 %72, %74
  store i16 %75, ptr @"O%", !tbaa !2
  br label %b10

b19:
  %76 = load i16, ptr @"FOUND%", !tbaa !2
  %77 = load i16, ptr @"ROUND%", !tbaa !2
  %78 = add i16 99, %77
  %79 = trunc i16 %78 to i8
  %80 = load i16, ptr @b$seg, !tbaa !2
  %81 = inttoptr i16 %80 to ptr addrspace(2)
  %82 = addrspacecast ptr addrspace(2) %81 to ptr addrspace(1)
  %83 = getelementptr i8, ptr addrspace(1) %82, i16 %76
  store volatile i8 %79, ptr addrspace(1) %83
  %84 = load i16, ptr @"FOUND%", !tbaa !2
  %85 = add i16 %84, 1
  %86 = load i16, ptr @b$seg, !tbaa !2
  %87 = inttoptr i16 %86 to ptr addrspace(2)
  %88 = addrspacecast ptr addrspace(2) %87 to ptr addrspace(1)
  %89 = getelementptr i8, ptr addrspace(1) %88, i16 %85
  store volatile i8 0, ptr addrspace(1) %89
  br label %b21

b20:
  br label %b21

b21:
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  %90 = load i16, ptr @"ROUND%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %90)
  %91 = load i16, ptr @"FOUND%", !tbaa !2
  %92 = icmp sge i16 %91, 0
  %93 = sext i1 %92 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %93)
  %94 = load i16, ptr @"Z%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %94)
  %95 = load i16, ptr @"ROUND%", !tbaa !2
  %96 = getelementptr i8, ptr @$data, i16 2
  %97 = load i16, ptr %96, !tbaa !2
  %98 = add nsw i16 %95, %97
  store i16 %98, ptr @"ROUND%", !tbaa !2
  br label %b2
}

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
