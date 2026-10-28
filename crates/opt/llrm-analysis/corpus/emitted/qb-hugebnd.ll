target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [2 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"N&" = internal global [4 x i8] zeroinitializer
@"H&" = internal global [46 x i8] zeroinitializer
@"G%" = internal global [46 x i8] zeroinitializer
@"LO%" = internal global [2 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %4, !tbaa !2
  store i32 32766, ptr @"N&", !tbaa !2
  store i16 -2, ptr @"LO%", !tbaa !2
  %5 = load i32, ptr @"N&", !tbaa !2
  %6 = trunc i32 %5 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 %6, i16 4, i16 257, ptr @"H&")
  %7 = load i16, ptr @"LO%", !tbaa !2
  %8 = load i16, ptr @"LO%", !tbaa !2
  %9 = load i32, ptr @"N&", !tbaa !2
  %10 = load i16, ptr @"LO%", !tbaa !2
  %11 = sext i16 %10 to i32
  %12 = add i32 %9, %11
  %13 = trunc i32 %12 to i16
  store i16 0, ptr %4, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 %7, i16 0, i16 %8, i16 %13, i16 2, i16 258, ptr @"G%")
  store i16 %7, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 %8, ptr %1, !tbaa !2
  store i16 %13, ptr %0, !tbaa !2
  store i16 2, ptr %4, !tbaa !2
  %14 = getelementptr i8, ptr @"H&", i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = getelementptr i8, ptr @"H&", i16 10
  %17 = load i16, ptr %16, !tbaa !2
  %18 = add i16 %17, 0
  %19 = inttoptr i16 %15 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %18
  store i32 5, ptr addrspace(1) %21, !tbaa !6
  %22 = getelementptr i8, ptr @"H&", i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = getelementptr i8, ptr @"H&", i16 10
  %25 = load i16, ptr %24, !tbaa !2
  %26 = add i16 %25, -8
  %27 = inttoptr i16 %23 to ptr addrspace(2)
  %28 = addrspacecast ptr addrspace(2) %27 to ptr addrspace(1)
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 %26
  store i32 7, ptr addrspace(1) %29, !tbaa !6
  %30 = load i16, ptr @"LO%", !tbaa !2
  %31 = load i16, ptr @"LO%", !tbaa !2
  %32 = getelementptr i8, ptr @"G%", i16 18
  %33 = load i16, ptr %32, !tbaa !2
  %34 = mul i16 %31, %33
  %35 = add i16 %34, %30
  %36 = mul i16 %35, 2
  %37 = getelementptr i8, ptr @"G%", i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = getelementptr i8, ptr @"G%", i16 10
  %40 = load i16, ptr %39, !tbaa !2
  %41 = add i16 %40, %36
  %42 = inttoptr i16 %38 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 %41
  store i16 11, ptr addrspace(1) %44, !tbaa !8
  %45 = getelementptr i8, ptr @"G%", i16 18
  %46 = load i16, ptr %45, !tbaa !2
  %47 = mul i16 32764, %46
  %48 = add i16 %47, 0
  %49 = mul i16 %48, 2
  %50 = getelementptr i8, ptr @"G%", i16 2
  %51 = load i16, ptr %50, !tbaa !2
  %52 = getelementptr i8, ptr @"G%", i16 10
  %53 = load i16, ptr %52, !tbaa !2
  %54 = add i16 %53, %49
  %55 = inttoptr i16 %51 to ptr addrspace(2)
  %56 = addrspacecast ptr addrspace(2) %55 to ptr addrspace(1)
  %57 = getelementptr i8, ptr addrspace(1) %56, i16 %54
  store i16 13, ptr addrspace(1) %57, !tbaa !8
  %58 = getelementptr i8, ptr @"G%", i16 18
  %59 = load i16, ptr %58, !tbaa !2
  %60 = mul i16 20000, %59
  %61 = add i16 %60, -1
  %62 = mul i16 %61, 2
  %63 = getelementptr i8, ptr @"G%", i16 2
  %64 = load i16, ptr %63, !tbaa !2
  %65 = getelementptr i8, ptr @"G%", i16 10
  %66 = load i16, ptr %65, !tbaa !2
  %67 = add i16 %66, %62
  %68 = inttoptr i16 %64 to ptr addrspace(2)
  %69 = addrspacecast ptr addrspace(2) %68 to ptr addrspace(1)
  %70 = getelementptr i8, ptr addrspace(1) %69, i16 %67
  store i16 17, ptr addrspace(1) %70, !tbaa !8
  %71 = getelementptr i8, ptr @"H&", i16 2
  %72 = load i16, ptr %71, !tbaa !2
  %73 = getelementptr i8, ptr @"H&", i16 10
  %74 = load i16, ptr %73, !tbaa !2
  %75 = add i16 %74, 0
  %76 = inttoptr i16 %72 to ptr addrspace(2)
  %77 = addrspacecast ptr addrspace(2) %76 to ptr addrspace(1)
  %78 = getelementptr i8, ptr addrspace(1) %77, i16 %75
  %79 = load i32, ptr addrspace(1) %78, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %79)
  %80 = getelementptr i8, ptr @"H&", i16 2
  %81 = load i16, ptr %80, !tbaa !2
  %82 = getelementptr i8, ptr @"H&", i16 10
  %83 = load i16, ptr %82, !tbaa !2
  %84 = add i16 %83, -8
  %85 = inttoptr i16 %81 to ptr addrspace(2)
  %86 = addrspacecast ptr addrspace(2) %85 to ptr addrspace(1)
  %87 = getelementptr i8, ptr addrspace(1) %86, i16 %84
  %88 = load i32, ptr addrspace(1) %87, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %88)
  %89 = getelementptr i8, ptr @"G%", i16 18
  %90 = load i16, ptr %89, !tbaa !2
  %91 = mul i16 -2, %90
  %92 = add i16 %91, -2
  %93 = mul i16 %92, 2
  %94 = getelementptr i8, ptr @"G%", i16 2
  %95 = load i16, ptr %94, !tbaa !2
  %96 = getelementptr i8, ptr @"G%", i16 10
  %97 = load i16, ptr %96, !tbaa !2
  %98 = add i16 %97, %93
  %99 = inttoptr i16 %95 to ptr addrspace(2)
  %100 = addrspacecast ptr addrspace(2) %99 to ptr addrspace(1)
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 %98
  %102 = load i16, ptr addrspace(1) %101, !tbaa !8
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %102)
  %103 = getelementptr i8, ptr @"G%", i16 18
  %104 = load i16, ptr %103, !tbaa !2
  %105 = mul i16 32764, %104
  %106 = add i16 %105, 0
  %107 = mul i16 %106, 2
  %108 = getelementptr i8, ptr @"G%", i16 2
  %109 = load i16, ptr %108, !tbaa !2
  %110 = getelementptr i8, ptr @"G%", i16 10
  %111 = load i16, ptr %110, !tbaa !2
  %112 = add i16 %111, %107
  %113 = inttoptr i16 %109 to ptr addrspace(2)
  %114 = addrspacecast ptr addrspace(2) %113 to ptr addrspace(1)
  %115 = getelementptr i8, ptr addrspace(1) %114, i16 %112
  %116 = load i16, ptr addrspace(1) %115, !tbaa !8
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %116)
  %117 = getelementptr i8, ptr @"G%", i16 18
  %118 = load i16, ptr %117, !tbaa !2
  %119 = mul i16 20000, %118
  %120 = add i16 %119, -1
  %121 = mul i16 %120, 2
  %122 = getelementptr i8, ptr @"G%", i16 2
  %123 = load i16, ptr %122, !tbaa !2
  %124 = getelementptr i8, ptr @"G%", i16 10
  %125 = load i16, ptr %124, !tbaa !2
  %126 = add i16 %125, %121
  %127 = inttoptr i16 %123 to ptr addrspace(2)
  %128 = addrspacecast ptr addrspace(2) %127 to ptr addrspace(1)
  %129 = getelementptr i8, ptr addrspace(1) %128, i16 %126
  %130 = load i16, ptr addrspace(1) %129, !tbaa !8
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %130)
  %131 = load i16, ptr %4, !tbaa !2
  %132 = icmp sge i16 %131, 2
  br i1 %132, label %b3, label %b4

b2:
  %133 = load i16, ptr @$data, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %133)
  ret void

b3:
  %134 = load i16, ptr %1, !tbaa !2
  store i16 %134, ptr @$data, !tbaa !2
  br label %b2

b4:
  %135 = getelementptr i8, ptr @"G%", i16 16
  %136 = load i16, ptr %135, !tbaa !2
  store i16 %136, ptr @$data, !tbaa !2
  br label %b2
}

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
