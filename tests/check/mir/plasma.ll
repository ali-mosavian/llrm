; RUN: llrm-mir %s
; CHECK: define {{.*}} @PLASMA(


@"TOTALFRAMECOUNT%" = internal global [2 x i8] zeroinitializer
@b$seg = internal global [2 x i8] zeroinitializer
@$string11 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string11, i16 4) }>

declare cc1000 void @UPDPALPLASMA(ptr) addrspace(1)
define cc1000 void @PLASMA(ptr nocapture %0) addrspace(1) memory(readwrite, argmem: read) {
b1:
  %1 = alloca i16
  %2 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 17795, i16 2, i16 257, ptr %2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  %3 = getelementptr i8, ptr %2, i16 2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = inttoptr i16 %4 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 1284
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 1026
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 0
  br label %b5

b5:
  %10 = phi i16 [ 0, %b1 ], [ %20, %b5 ]
  %11 = phi i16 [ -1026, %b1 ], [ %21, %b5 ]
  %12 = sitofp i16 %10 to float
  %13 = fmul float %12, 0x40091EB860000000
  %14 = fdiv float %13, 2.560000e+02
  %15 = call float @llvm.sin.f32(float %14)
  %16 = fmul float %15, 3.200000e+01
  %17 = fadd float %16, 3.200000e+01
  %18 = call i16 @llvm.lrint.i16.f32(float %17)
  %19 = getelementptr i8, ptr addrspace(1) %9, i16 %11
  store i16 %18, ptr addrspace(1) %19, !tbaa !4
  %20 = add nsw i16 %10, 1
  %21 = add i16 %11, 2
  %22 = icmp ne i16 %21, 0
  br i1 %22, label %b5, label %b6

b6:
  store i16 1, ptr %1, !tbaa !2
  %23 = load i16, ptr %0
  %24 = inttoptr i16 -24576 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  br label %b7

b7:
  %26 = phi i16 [ 1, %b6 ], [ %152, %b44 ]
  %27 = icmp sle i16 %26, %23
  br i1 %27, label %b10, label %b11

b10:
  %28 = load i16, ptr %3, !tbaa !2
  %29 = inttoptr i16 %28 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %31 = mul i16 %26, 2
  %32 = mul i16 %26, 14
  %33 = add i16 %32, 6
  %34 = getelementptr i8, ptr addrspace(1) %30, i16 0
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 1284
  br label %b15

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %2)
  ret void

b15:
  %36 = phi i16 [ -1284, %b10 ], [ %47, %b15 ]
  %lsr.iv261 = phi i16 [ %33, %b10 ], [ %lsr.iv.next26, %b15 ]
  %lsr.iv301 = phi i16 [ %31, %b10 ], [ %lsr.iv.next30, %b15 ]
  %37 = and i16 %lsr.iv301, 1022
  %38 = add i16 %37, 1284
  %39 = getelementptr i8, ptr addrspace(1) %30, i16 %38
  %40 = load i16, ptr addrspace(1) %39, !tbaa !4
  %41 = and i16 %lsr.iv261, 1022
  %42 = add i16 %41, 1284
  %43 = getelementptr i8, ptr addrspace(1) %30, i16 %42
  %44 = load i16, ptr addrspace(1) %43, !tbaa !4
  %45 = add i16 %40, %44
  %46 = getelementptr i8, ptr addrspace(1) %35, i16 %36
  store i16 %45, ptr addrspace(1) %46, !tbaa !4
  %47 = add i16 %36, 4
  %lsr.iv.next26 = add i16 %lsr.iv261, 6
  %lsr.iv.next30 = add i16 %lsr.iv301, 2
  %48 = icmp ne i16 %47, 0
  br i1 %48, label %b15, label %b16

b16:
  %49 = mul i16 %26, 10
  %50 = mul i16 %26, 22
  %51 = add i16 %50, 3886
  %52 = getelementptr i8, ptr addrspace(1) %30, i16 2310
  %53 = getelementptr i8, ptr addrspace(1) %52, i16 -32254
  %54 = getelementptr i8, ptr addrspace(1) %34, i16 516
  br label %b17

b17:
  %55 = phi i16 [ 32254, %b16 ], [ %77, %b26 ]
  %lsr.iv11 = phi i16 [ %51, %b16 ], [ %lsr.iv.next11, %b26 ]
  %lsr.iv10 = phi i16 [ %49, %b16 ], [ %lsr.iv.next10, %b26 ]
  %56 = getelementptr i8, ptr addrspace(1) %53, i16 %55
  %57 = icmp ne i16 %55, 0
  br i1 %57, label %b20, label %b21

b20:
  %58 = and i16 %lsr.iv10, 1022
  %59 = add i16 %58, 1284
  %60 = getelementptr i8, ptr addrspace(1) %30, i16 %59
  %61 = load i16, ptr addrspace(1) %60, !tbaa !4
  %62 = and i16 %lsr.iv11, 1022
  %63 = add i16 %62, 1284
  %64 = getelementptr i8, ptr addrspace(1) %30, i16 %63
  %65 = load i16, ptr addrspace(1) %64, !tbaa !4
  %66 = add i16 %61, %65
  br label %b25

b21:
  %67 = mul i16 %31, 2
  %68 = getelementptr i8, ptr addrspace(1) %30, i16 2
  %69 = add i16 %67, 742
  %70 = getelementptr i8, ptr addrspace(1) %35, i16 0
  br label %b30

b25:
  %lsr.iv31 = phi ptr addrspace(1) [ %56, %b20 ], [ %lsr.iv.next3, %b25 ]
  %71 = phi i16 [ -516, %b20 ], [ %75, %b25 ]
  %72 = getelementptr i8, ptr addrspace(1) %54, i16 %71
  %73 = load i16, ptr addrspace(1) %72, !tbaa !4
  %74 = add i16 %73, %66
  store i16 %74, ptr addrspace(1) %lsr.iv31, !tbaa !4
  %lsr.iv.next3 = getelementptr i8, ptr addrspace(1) %lsr.iv31, i16 2
  %75 = add i16 %71, 4
  %76 = icmp ne i16 %75, 0
  br i1 %76, label %b25, label %b26

b26:
  %lsr.iv.next10 = add i16 %lsr.iv10, 14
  %lsr.iv.next11 = add i16 %lsr.iv11, 28
  %77 = add i16 %55, 258
  br label %b17

b30:
  %lsr.iv51 = phi i16 [ %33, %b21 ], [ %lsr.iv.next5, %b30 ]
  %lsr.iv61 = phi ptr addrspace(1) [ %68, %b21 ], [ %lsr.iv.next6, %b30 ]
  %lsr.iv71 = phi i16 [ %69, %b21 ], [ %lsr.iv.next7, %b30 ]
  %78 = phi i16 [ -1284, %b21 ], [ %98, %b30 ]
  %lsr.iv281 = phi i16 [ %32, %b21 ], [ %lsr.iv.next28, %b30 ]
  %lsr.iv291 = phi i16 [ %49, %b21 ], [ %lsr.iv.next29, %b30 ]
  %79 = and i16 %lsr.iv281, 1022
  %80 = add i16 %79, 1284
  %81 = getelementptr i8, ptr addrspace(1) %30, i16 %80
  %82 = load i16, ptr addrspace(1) %81, !tbaa !4
  %83 = and i16 %lsr.iv51, 1022
  %84 = add i16 %83, 1284
  %85 = getelementptr i8, ptr addrspace(1) %30, i16 %84
  %86 = load i16, ptr addrspace(1) %85, !tbaa !4
  %87 = add i16 %82, %86
  %88 = getelementptr i8, ptr addrspace(1) %70, i16 %78
  store i16 %87, ptr addrspace(1) %88, !tbaa !4
  %89 = and i16 %lsr.iv291, 1022
  %90 = add i16 %89, 1284
  %91 = getelementptr i8, ptr addrspace(1) %30, i16 %90
  %92 = load i16, ptr addrspace(1) %91, !tbaa !4
  %93 = and i16 %lsr.iv71, 1022
  %94 = add i16 %93, 1284
  %95 = getelementptr i8, ptr addrspace(1) %30, i16 %94
  %96 = load i16, ptr addrspace(1) %95, !tbaa !4
  %97 = add i16 %92, %96
  store i16 %97, ptr addrspace(1) %lsr.iv61, !tbaa !4
  %lsr.iv.next5 = add i16 %lsr.iv51, 6
  %lsr.iv.next6 = getelementptr i8, ptr addrspace(1) %lsr.iv61, i16 4
  %lsr.iv.next7 = add i16 %lsr.iv71, 18
  %98 = add i16 %78, 4
  %lsr.iv.next28 = add i16 %lsr.iv281, 22
  %lsr.iv.next29 = add i16 %lsr.iv291, 8
  %99 = icmp ne i16 %98, 0
  br i1 %99, label %b30, label %b31

b31:
  %100 = mul i16 %26, 12
  %101 = mul i16 %26, 8
  %102 = mul i16 %26, 46
  %103 = add i16 %102, 3886
  %104 = add i16 %103, 6800
  br label %b35

b35:
  %105 = phi i16 [ 0, %b31 ], [ %149, %b41 ]
  %lsr.iv131 = phi i16 [ %100, %b31 ], [ %lsr.iv.next13, %b41 ]
  %lsr.iv141 = phi i16 [ %51, %b31 ], [ %lsr.iv.next14, %b41 ]
  %lsr.iv151 = phi i16 [ %101, %b31 ], [ %lsr.iv.next15, %b41 ]
  %lsr.iv161 = phi i16 [ %103, %b31 ], [ %lsr.iv.next16, %b41 ]
  %106 = and i16 %lsr.iv131, 1022
  %107 = add i16 %106, 1284
  %108 = getelementptr i8, ptr addrspace(1) %30, i16 %107
  %109 = load i16, ptr addrspace(1) %108, !tbaa !4
  %110 = and i16 %lsr.iv141, 1022
  %111 = add i16 %110, 1284
  %112 = getelementptr i8, ptr addrspace(1) %30, i16 %111
  %113 = load i16, ptr addrspace(1) %112, !tbaa !4
  %114 = add i16 %109, %113
  %115 = and i16 %lsr.iv151, 1022
  %116 = add i16 %115, 1284
  %117 = getelementptr i8, ptr addrspace(1) %30, i16 %116
  %118 = load i16, ptr addrspace(1) %117, !tbaa !4
  %119 = and i16 %lsr.iv161, 1022
  %120 = add i16 %119, 1284
  %121 = getelementptr i8, ptr addrspace(1) %30, i16 %120
  %122 = load i16, ptr addrspace(1) %121, !tbaa !4
  %123 = add i16 %118, %122
  %124 = add i16 %105, 320
  %125 = getelementptr i8, ptr addrspace(1) %25, i16 %105
  %126 = getelementptr i8, ptr addrspace(1) %125, i16 320
  br label %b40

b36:
  call cc1000 addrspace(1) void @UPDPALPLASMA(ptr %1)
  %127 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %128 = add i16 %127, 1
  store i16 %128, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %129 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %130 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %129, ptr @$string11)
  %131 = icmp sgt i16 %130, 0
  br i1 %131, label %b42, label %b44

b40:
  %lsr.iv91 = phi ptr addrspace(1) [ %68, %b35 ], [ %lsr.iv.next9, %b40 ]
  %lsr.iv201 = phi ptr addrspace(1) [ %34, %b35 ], [ %lsr.iv.next20, %b40 ]
  %132 = phi i16 [ -320, %b35 ], [ %147, %b40 ]
  %133 = load i16, ptr addrspace(1) %lsr.iv201, !tbaa !4
  %134 = add i16 %133, %114
  %135 = and i16 %134, 127
  %136 = load i16, ptr addrspace(1) %lsr.iv91, !tbaa !4
  %137 = add i16 %136, %123
  %138 = and i16 %137, 127
  %139 = mul i16 %138, 129
  %140 = add i16 %139, %135
  %141 = mul i16 %140, 2
  %142 = add i16 %141, 2310
  %143 = getelementptr i8, ptr addrspace(1) %30, i16 %142
  %144 = load i16, ptr addrspace(1) %143, !tbaa !4
  %145 = trunc i16 %144 to i8
  %146 = getelementptr i8, ptr addrspace(1) %126, i16 %132
  store i8 %145, ptr addrspace(1) %146
  %lsr.iv.next9 = getelementptr i8, ptr addrspace(1) %lsr.iv91, i16 4
  %lsr.iv.next20 = getelementptr i8, ptr addrspace(1) %lsr.iv201, i16 4
  %147 = add i16 %132, 1
  %148 = icmp ne i16 %147, 0
  br i1 %148, label %b40, label %b41

b41:
  %149 = phi i16 [ %124, %b40 ]
  %lsr.iv.next13 = add i16 %lsr.iv131, 22
  %lsr.iv.next14 = add i16 %lsr.iv141, 28
  %lsr.iv.next15 = add i16 %lsr.iv151, 18
  %lsr.iv.next16 = add i16 %lsr.iv161, 34
  %150 = icmp ne i16 %lsr.iv.next16, %104
  br i1 %150, label %b35, label %b36

b42:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %2)
  ret void

b44:
  %151 = load i16, ptr %1, !tbaa !2
  %152 = add nsw i16 %151, 1
  store i16 %152, ptr %1, !tbaa !2
  br label %b7
}

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc1000 void @llrm.qb.B$ERAS(ptr) addrspace(1) nocallback

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 ptr @llrm.qb.B$INKY() addrspace(1) nocallback

declare cc1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1) nocallback

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
