target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X!" = internal global [4 x i8] zeroinitializer
@$arguments$2 = internal global [64 x i8] zeroinitializer
@"Y!" = internal global [4 x i8] zeroinitializer
@$float4 = internal constant [4 x i8] c"\00\00\00?"
@"Z!" = internal global [4 x i8] zeroinitializer
@DONE$ = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 3, ptr @$arguments$2, !tbaa !2
  %0 = load i16, ptr @$arguments$2, !tbaa !2
  %1 = sitofp i16 %0 to float
  %2 = fpround float %1 to float
  store float %2, ptr @"X!", !tbaa !2
  %3 = load float, ptr @$float4, !tbaa !2
  store float %3, ptr @"Y!", !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$2, i16 2
  store i16 -3, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$arguments$2, i16 2
  %6 = load i16, ptr %5, !tbaa !2
  %7 = sitofp i16 %6 to float
  %8 = fpround float %7 to float
  store float %8, ptr @"Z!", !tbaa !2
  %9 = load float, ptr @"X!", !tbaa !2
  %10 = getelementptr i8, ptr @$arguments$2, i16 4
  store i16 1, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$arguments$2, i16 4
  %12 = load i16, ptr %11, !tbaa !2
  %13 = sitofp i16 %12 to float
  %14 = fadd float %9, %13
  %15 = getelementptr i8, ptr @$arguments$2, i16 6
  store i16 2, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$arguments$2, i16 6
  %17 = load i16, ptr %16, !tbaa !2
  %18 = sitofp i16 %17 to float
  %19 = fdiv float %14, %18
  %20 = call float @llvm.rint.f32(float %19)
  %21 = fcmp olt float %19, %20
  %22 = sext i1 %21 to i16
  %23 = getelementptr i8, ptr @$arguments$2, i16 8
  store i16 %22, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$arguments$2, i16 8
  %25 = load i16, ptr %24, !tbaa !2
  %26 = sitofp i16 %25 to float
  %27 = fadd float %20, %26
  %28 = fpround float %27 to float
  %29 = getelementptr i8, ptr @$arguments$2, i16 10
  store float %28, ptr %29, !tbaa !2
  %30 = getelementptr i8, ptr @$arguments$2, i16 10
  %31 = load float, ptr %30, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSR4(float %31)
  %32 = load float, ptr @"X!", !tbaa !2
  %33 = getelementptr i8, ptr @$arguments$2, i16 14
  store i16 -1, ptr %33, !tbaa !2
  %34 = getelementptr i8, ptr @$arguments$2, i16 14
  %35 = load i16, ptr %34, !tbaa !2
  %36 = sitofp i16 %35 to float
  %37 = fadd float %32, %36
  %38 = getelementptr i8, ptr @$arguments$2, i16 16
  store i16 2, ptr %38, !tbaa !2
  %39 = getelementptr i8, ptr @$arguments$2, i16 16
  %40 = load i16, ptr %39, !tbaa !2
  %41 = sitofp i16 %40 to float
  %42 = fdiv float %37, %41
  %43 = call float @llvm.rint.f32(float %42)
  %44 = fcmp olt float %42, %43
  %45 = sext i1 %44 to i16
  %46 = getelementptr i8, ptr @$arguments$2, i16 18
  store i16 %45, ptr %46, !tbaa !2
  %47 = getelementptr i8, ptr @$arguments$2, i16 18
  %48 = load i16, ptr %47, !tbaa !2
  %49 = sitofp i16 %48 to float
  %50 = fadd float %43, %49
  %51 = fpround float %50 to float
  %52 = getelementptr i8, ptr @$arguments$2, i16 20
  store float %51, ptr %52, !tbaa !2
  %53 = getelementptr i8, ptr @$arguments$2, i16 20
  %54 = load float, ptr %53, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSR4(float %54)
  %55 = load float, ptr @"X!", !tbaa !2
  %56 = getelementptr i8, ptr @$arguments$2, i16 24
  store i16 1, ptr %56, !tbaa !2
  %57 = getelementptr i8, ptr @$arguments$2, i16 24
  %58 = load i16, ptr %57, !tbaa !2
  %59 = sitofp i16 %58 to float
  %60 = fadd float %55, %59
  %61 = getelementptr i8, ptr @$arguments$2, i16 26
  store i16 4, ptr %61, !tbaa !2
  %62 = getelementptr i8, ptr @$arguments$2, i16 26
  %63 = load i16, ptr %62, !tbaa !2
  %64 = sitofp i16 %63 to float
  %65 = fdiv float %60, %64
  %66 = call float @llvm.rint.f32(float %65)
  %67 = fcmp olt float %65, %66
  %68 = sext i1 %67 to i16
  %69 = getelementptr i8, ptr @$arguments$2, i16 28
  store i16 %68, ptr %69, !tbaa !2
  %70 = getelementptr i8, ptr @$arguments$2, i16 28
  %71 = load i16, ptr %70, !tbaa !2
  %72 = sitofp i16 %71 to float
  %73 = fadd float %66, %72
  %74 = fpround float %73 to float
  %75 = getelementptr i8, ptr @$arguments$2, i16 30
  store float %74, ptr %75, !tbaa !2
  %76 = getelementptr i8, ptr @$arguments$2, i16 30
  %77 = load float, ptr %76, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %77)
  %78 = load float, ptr @"Y!", !tbaa !2
  %79 = getelementptr i8, ptr @$arguments$2, i16 34
  store i16 1, ptr %79, !tbaa !2
  %80 = getelementptr i8, ptr @$arguments$2, i16 34
  %81 = load i16, ptr %80, !tbaa !2
  %82 = sitofp i16 %81 to float
  %83 = fadd float %78, %82
  %84 = getelementptr i8, ptr @$arguments$2, i16 36
  store i16 2, ptr %84, !tbaa !2
  %85 = getelementptr i8, ptr @$arguments$2, i16 36
  %86 = load i16, ptr %85, !tbaa !2
  %87 = sitofp i16 %86 to float
  %88 = fdiv float %83, %87
  %89 = call float @llvm.rint.f32(float %88)
  %90 = fcmp olt float %88, %89
  %91 = sext i1 %90 to i16
  %92 = getelementptr i8, ptr @$arguments$2, i16 38
  store i16 %91, ptr %92, !tbaa !2
  %93 = getelementptr i8, ptr @$arguments$2, i16 38
  %94 = load i16, ptr %93, !tbaa !2
  %95 = sitofp i16 %94 to float
  %96 = fadd float %89, %95
  %97 = fpround float %96 to float
  %98 = getelementptr i8, ptr @$arguments$2, i16 40
  store float %97, ptr %98, !tbaa !2
  %99 = getelementptr i8, ptr @$arguments$2, i16 40
  %100 = load float, ptr %99, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSR4(float %100)
  %101 = load float, ptr @"Z!", !tbaa !2
  %102 = getelementptr i8, ptr @$arguments$2, i16 44
  store i16 1, ptr %102, !tbaa !2
  %103 = getelementptr i8, ptr @$arguments$2, i16 44
  %104 = load i16, ptr %103, !tbaa !2
  %105 = sitofp i16 %104 to float
  %106 = fadd float %101, %105
  %107 = getelementptr i8, ptr @$arguments$2, i16 46
  store i16 2, ptr %107, !tbaa !2
  %108 = getelementptr i8, ptr @$arguments$2, i16 46
  %109 = load i16, ptr %108, !tbaa !2
  %110 = sitofp i16 %109 to float
  %111 = fdiv float %106, %110
  %112 = call float @llvm.rint.f32(float %111)
  %113 = fcmp olt float %111, %112
  %114 = sext i1 %113 to i16
  %115 = getelementptr i8, ptr @$arguments$2, i16 48
  store i16 %114, ptr %115, !tbaa !2
  %116 = getelementptr i8, ptr @$arguments$2, i16 48
  %117 = load i16, ptr %116, !tbaa !2
  %118 = sitofp i16 %117 to float
  %119 = fadd float %112, %118
  %120 = fpround float %119 to float
  %121 = getelementptr i8, ptr @$arguments$2, i16 50
  store float %120, ptr %121, !tbaa !2
  %122 = getelementptr i8, ptr @$arguments$2, i16 50
  %123 = load float, ptr %122, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSR4(float %123)
  %124 = load float, ptr @"Z!", !tbaa !2
  %125 = getelementptr i8, ptr @$arguments$2, i16 54
  store i16 -1, ptr %125, !tbaa !2
  %126 = getelementptr i8, ptr @$arguments$2, i16 54
  %127 = load i16, ptr %126, !tbaa !2
  %128 = sitofp i16 %127 to float
  %129 = fadd float %124, %128
  %130 = getelementptr i8, ptr @$arguments$2, i16 56
  store i16 4, ptr %130, !tbaa !2
  %131 = getelementptr i8, ptr @$arguments$2, i16 56
  %132 = load i16, ptr %131, !tbaa !2
  %133 = sitofp i16 %132 to float
  %134 = fdiv float %129, %133
  %135 = call float @llvm.rint.f32(float %134)
  %136 = fcmp olt float %134, %135
  %137 = sext i1 %136 to i16
  %138 = getelementptr i8, ptr @$arguments$2, i16 58
  store i16 %137, ptr %138, !tbaa !2
  %139 = getelementptr i8, ptr @$arguments$2, i16 58
  %140 = load i16, ptr %139, !tbaa !2
  %141 = sitofp i16 %140 to float
  %142 = fadd float %135, %141
  %143 = fpround float %142 to float
  %144 = getelementptr i8, ptr @$arguments$2, i16 60
  store float %143, ptr %144, !tbaa !2
  %145 = getelementptr i8, ptr @$arguments$2, i16 60
  %146 = load float, ptr %145, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %146)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string5, ptr @DONE$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @DONE$)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PSR4(float) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
