target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [60 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"TOTALFRAMECOUNT%" = internal global [2 x i8] zeroinitializer
@"FRACTAL1%" = internal global [18 x i8] zeroinitializer
@"FRACTAL2%" = internal global [18 x i8] zeroinitializer
@"BOBSPRITE%" = internal global [22 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"TI1!" = internal global [4 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@b$seg = global [2 x i8] zeroinitializer
@$float4 = internal constant [8 x i8] c"^A\DA\22%2\E4\BF"
@$float5 = internal constant [8 x i8] c"\8E`s\FBw}\E2\BF"
@$float6 = internal constant [4 x i8] c"\CD\CC\CC?"
@$string7 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string7, i16 4) }>
@$string8 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string8, i16 4), [10 x i8] c"canada.bsv" }>
@$string9 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string9, i16 4) }>
@$float10 = internal constant [4 x i8] c"\C3\F5H@"
@$string11 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string11, i16 4) }>
@$string12 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string12, i16 4) }>
@$float13 = internal constant [8 x i8] c"\18-DT\FB!\09@"
@$float14 = internal constant [4 x i8] c"\C0\CF\B8:"
@$float15 = internal constant [4 x i8] c"\B0\03g<"
@$float16 = internal constant [4 x i8] c"\89\D2^<"
@$string17 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string17, i16 4), [2 x i8] c"V\00" }>
@$string18 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string18, i16 4), [4 x i8] c".BIN" }>
@$string19 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string19, i16 4), [2 x i8] c"P\00" }>
@$string20 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string20, i16 4), [4 x i8] c".BIN" }>
@$string21 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string21, i16 4), [2 x i8] c"T\00" }>
@$string22 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string22, i16 4), [4 x i8] c".BIN" }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL1%")
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL2%")
  store i16 0, ptr @"X%", !tbaa !2
  store i16 63, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = load i16, ptr %0, !tbaa !2
  %2 = icmp sge i16 %1, 0
  br i1 %2, label %b3, label %b4

b3:
  %3 = load i16, ptr @"X%", !tbaa !2
  %4 = load i16, ptr @$data, !tbaa !2
  %5 = icmp sle i16 %3, %4
  br i1 %5, label %b5, label %b6

b4:
  %6 = load i16, ptr @"X%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sge i16 %6, %7
  br i1 %8, label %b5, label %b6

b5:
  %9 = load i16, ptr @"X%", !tbaa !2
  %10 = trunc i16 %9 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %10)
  %11 = load i16, ptr @"X%", !tbaa !2
  %12 = trunc i16 %11 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %12)
  %13 = load i16, ptr @"X%", !tbaa !2
  %14 = trunc i16 %13 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %14)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %15 = load i16, ptr @"X%", !tbaa !2
  %16 = load i16, ptr %0, !tbaa !2
  %17 = add i16 %15, %16
  store i16 %17, ptr @"X%", !tbaa !2
  br label %b2

b6:
  store i16 64, ptr @"X%", !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 4
  store i16 127, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %19, !tbaa !2
  br label %b7

b7:
  %20 = load i16, ptr %19, !tbaa !2
  %21 = icmp sge i16 %20, 0
  br i1 %21, label %b8, label %b9

b8:
  %22 = load i16, ptr @"X%", !tbaa !2
  %23 = load i16, ptr %18, !tbaa !2
  %24 = icmp sle i16 %22, %23
  br i1 %24, label %b10, label %b11

b9:
  %25 = load i16, ptr @"X%", !tbaa !2
  %26 = load i16, ptr %18, !tbaa !2
  %27 = icmp sge i16 %25, %26
  br i1 %27, label %b10, label %b11

b10:
  %28 = load i16, ptr @"X%", !tbaa !2
  %29 = trunc i16 %28 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %29)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %30 = load i16, ptr @"X%", !tbaa !2
  %31 = trunc i16 %30 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %31)
  %32 = load i16, ptr @"X%", !tbaa !2
  %33 = load i16, ptr %19, !tbaa !2
  %34 = add i16 %32, %33
  store i16 %34, ptr @"X%", !tbaa !2
  br label %b7

b11:
  store i16 128, ptr @"X%", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 8
  store i16 191, ptr %35, !tbaa !2
  %36 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %36, !tbaa !2
  br label %b12

b12:
  %37 = load i16, ptr %36, !tbaa !2
  %38 = icmp sge i16 %37, 0
  br i1 %38, label %b13, label %b14

b13:
  %39 = load i16, ptr @"X%", !tbaa !2
  %40 = load i16, ptr %35, !tbaa !2
  %41 = icmp sle i16 %39, %40
  br i1 %41, label %b15, label %b16

b14:
  %42 = load i16, ptr @"X%", !tbaa !2
  %43 = load i16, ptr %35, !tbaa !2
  %44 = icmp sge i16 %42, %43
  br i1 %44, label %b15, label %b16

b15:
  %45 = load i16, ptr @"X%", !tbaa !2
  %46 = trunc i16 %45 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %46)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %47 = load i16, ptr @"X%", !tbaa !2
  %48 = trunc i16 %47 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %48)
  %49 = load i16, ptr @"X%", !tbaa !2
  %50 = load i16, ptr %36, !tbaa !2
  %51 = add i16 %49, %50
  store i16 %51, ptr @"X%", !tbaa !2
  br label %b12

b16:
  store i16 191, ptr @"X%", !tbaa !2
  %52 = getelementptr i8, ptr @$data, i16 12
  store i16 255, ptr %52, !tbaa !2
  %53 = getelementptr i8, ptr @$data, i16 14
  store i16 1, ptr %53, !tbaa !2
  br label %b17

b17:
  %54 = load i16, ptr %53, !tbaa !2
  %55 = icmp sge i16 %54, 0
  br i1 %55, label %b18, label %b19

b18:
  %56 = load i16, ptr @"X%", !tbaa !2
  %57 = load i16, ptr %52, !tbaa !2
  %58 = icmp sle i16 %56, %57
  br i1 %58, label %b20, label %b21

b19:
  %59 = load i16, ptr @"X%", !tbaa !2
  %60 = load i16, ptr %52, !tbaa !2
  %61 = icmp sge i16 %59, %60
  br i1 %61, label %b20, label %b21

b20:
  %62 = load i16, ptr @"X%", !tbaa !2
  %63 = trunc i16 %62 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %63)
  %64 = load i16, ptr @"X%", !tbaa !2
  %65 = trunc i16 %64 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %65)
  %66 = load i16, ptr @"X%", !tbaa !2
  %67 = trunc i16 %66 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %67)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %68 = load i16, ptr @"X%", !tbaa !2
  %69 = load i16, ptr %53, !tbaa !2
  %70 = add i16 %68, %69
  store i16 %70, ptr @"X%", !tbaa !2
  br label %b17

b21:
  call void @llrm.ia16.out.i8(i16 968, i8 -1)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 968, i8 127)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %71 = getelementptr i8, ptr @$data, i16 16
  store i16 0, ptr %71, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr %71)
  %72 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %73 = load float, ptr %72
  store float %73, ptr @"TI1!", !tbaa !2
  %74 = getelementptr i8, ptr @$data, i16 18
  store i16 150, ptr %74, !tbaa !2
  call cc1000 addrspace(1) void @FRACTALEFFECT(ptr %74)
  %75 = getelementptr i8, ptr @$data, i16 20
  store i16 1, ptr %75, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr %75)
  %76 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %77 = load float, ptr %76
  %78 = load float, ptr @"TI1!", !tbaa !2
  %79 = fsub float %77, %78
  store float %79, ptr @"TI1!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL1%")
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL2%")
  call cc1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %80 = getelementptr i8, ptr @$data, i16 22
  store i16 16, ptr %80, !tbaa !2
  call cc1000 addrspace(1) void @UNWHITEFADE(ptr %80)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32, i16 0, i16 32, i16 2, i16 258, ptr @"BOBSPRITE%")
  store i16 0, ptr @"X%", !tbaa !2
  %81 = getelementptr i8, ptr @$data, i16 24
  store i16 32, ptr %81, !tbaa !2
  %82 = getelementptr i8, ptr @$data, i16 26
  store i16 1, ptr %82, !tbaa !2
  %83 = getelementptr i8, ptr @$data, i16 28
  %84 = getelementptr i8, ptr @$data, i16 30
  %85 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %86 = load i16, ptr %85, !tbaa !2
  %87 = inttoptr i16 %86 to ptr addrspace(2)
  %88 = addrspacecast ptr addrspace(2) %87 to ptr addrspace(1)
  %89 = getelementptr i8, ptr @$data, i16 32
  %90 = getelementptr i8, ptr @$data, i16 34
  %91 = getelementptr i8, ptr @$data, i16 38
  %92 = getelementptr i8, ptr @$data, i16 40
  %93 = getelementptr i8, ptr @$data, i16 44
  br label %b22

b22:
  %94 = load i16, ptr @"X%", !tbaa !2
  %95 = icmp sle i16 %94, 32
  br i1 %95, label %b25, label %b26

b25:
  store i16 0, ptr @"Y%", !tbaa !2
  store i16 32, ptr %83, !tbaa !2
  store i16 1, ptr %84, !tbaa !2
  %96 = sub i16 16, %94
  %97 = sitofp i16 %96 to float
  %98 = fmul float %97, %97
  br label %b28

b26:
  %99 = getelementptr i8, ptr @$data, i16 46
  store i16 1024, ptr %99, !tbaa !2
  call cc1000 addrspace(1) void @SHADEBOBEFFECT(ptr %99)
  %100 = getelementptr i8, ptr @$data, i16 48
  store i16 2, ptr %100, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr %100)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"BOBSPRITE%")
  call cc1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %101 = getelementptr i8, ptr @$data, i16 50
  store i16 16, ptr %101, !tbaa !2
  call cc1000 addrspace(1) void @UNWHITEFADE(ptr %101)
  %102 = getelementptr i8, ptr @$data, i16 52
  store i16 160, ptr %102, !tbaa !2
  call cc1000 addrspace(1) void @PLASMA(ptr %102)
  %103 = getelementptr i8, ptr @$data, i16 54
  store i16 3, ptr %103, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr %103)
  call cc1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %104 = getelementptr i8, ptr @$data, i16 56
  store i16 96, ptr %104, !tbaa !2
  call cc1000 addrspace(1) void @OHCANADA(ptr %104)
  %105 = getelementptr i8, ptr @$data, i16 58
  store i16 4, ptr %105, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr %105)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b28:
  %106 = load i16, ptr @"Y%", !tbaa !2
  %107 = icmp sle i16 %106, 32
  br i1 %107, label %b30, label %b31

b30:
  %108 = mul i16 %106, 33
  %109 = add i16 %108, %94
  %110 = shl i16 %109, 1
  %111 = getelementptr i8, ptr addrspace(1) %88, i16 %110
  store i16 %96, ptr %89, !tbaa !2
  store float %97, ptr %90, !tbaa !2
  %112 = sub i16 16, %106
  store i16 %112, ptr %91, !tbaa !2
  %113 = sitofp i16 %112 to float
  store float %113, ptr %92, !tbaa !2
  %114 = fmul float %113, %113
  %115 = fadd float %98, %114
  %116 = call float @llvm.sqrt.f32(float %115)
  store i16 16, ptr %93, !tbaa !2
  %117 = fsub float 1.600000e+01, %116
  %118 = call i16 @llvm.lrint.i16.f32(float %117)
  store i16 %118, ptr addrspace(1) %111, !tbaa !4
  %119 = load i16, ptr addrspace(1) %111, !tbaa !4
  %120 = icmp slt i16 %119, 0
  br i1 %120, label %b32, label %b34

b31:
  %121 = load i16, ptr @"X%", !tbaa !2
  %122 = add i16 %121, 1
  store i16 %122, ptr @"X%", !tbaa !2
  br label %b22

b32:
  store i16 0, ptr addrspace(1) %111, !tbaa !4
  br label %b34

b34:
  %123 = load i16, ptr @"Y%", !tbaa !2
  %124 = add i16 %123, 1
  store i16 %124, ptr @"Y%", !tbaa !2
  br label %b28
}

define cc1000 void @DRAWBOB(ptr %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %29, %b11 ]
  %3 = icmp sle i16 %2, 31
  br i1 %3, label %b5, label %b6

b5:
  %4 = load i16, ptr %0
  %5 = add i16 %4, 288
  store i16 %5, ptr %0
  %6 = mul i16 %2, 33
  %7 = shl i16 %6, 1
  br label %b7

b6:
  ret void

b7:
  %8 = phi i16 [ %7, %b5 ], [ %28, %b10 ]
  %9 = phi i16 [ 0, %b5 ], [ %27, %b10 ]
  %10 = icmp sle i16 %9, 31
  br i1 %10, label %b10, label %b11

b10:
  %11 = load i16, ptr %0
  %12 = load i16, ptr @b$seg, !tbaa !2
  %13 = inttoptr i16 %12 to ptr addrspace(2)
  %14 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 %11
  %16 = load i8, ptr addrspace(1) %15
  %17 = zext i8 %16 to i16
  %18 = load i16, ptr %1, !tbaa !2
  %19 = inttoptr i16 %18 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %8
  %22 = load i16, ptr addrspace(1) %21, !tbaa !4
  %23 = add i16 %17, %22
  %24 = trunc i16 %23 to i8
  store i8 %24, ptr addrspace(1) %15
  %25 = load i16, ptr %0
  %26 = add i16 %25, 1
  store i16 %26, ptr %0
  %27 = add i16 %9, 1
  %28 = add i16 %8, 2
  br label %b7

b11:
  %29 = add i16 %2, 1
  br label %b2
}

define cc1000 void @FRACLINE(ptr %0, ptr %1, ptr %2, ptr %3, ptr %4, ptr %5) addrspace(1) {
b1:
  %6 = load double, ptr %4
  %7 = load double, ptr %3
  %8 = fsub double %6, %7
  %9 = fdiv double %8, 1.600000e+02
  %10 = load double, ptr %2
  %11 = load double, ptr %1
  %12 = fsub double %10, %11
  %13 = fdiv double %12, 1.600000e+02
  %14 = load i16, ptr %0
  %15 = mul i16 %14, 320
  %16 = fdiv double %9, 2.000000e+00
  %17 = fadd double %7, %16
  br label %b3

b3:
  %18 = phi double [ %11, %b1 ], [ %73, %b13 ]
  %19 = phi i16 [ 1, %b1 ], [ %45, %b13 ]
  %20 = phi double [ %17, %b1 ], [ %72, %b13 ]
  %21 = add i16 %15, %19
  %22 = add i16 %21, -1
  %23 = load i16, ptr @b$seg, !tbaa !2
  %24 = inttoptr i16 %23 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 %22
  %27 = load i8, ptr addrspace(1) %26
  %28 = zext i8 %27 to i16
  %29 = add i16 %21, 1
  %30 = getelementptr i8, ptr addrspace(1) %25, i16 %29
  %31 = load i8, ptr addrspace(1) %30
  %32 = zext i8 %31 to i16
  %33 = icmp eq i16 %28, %32
  %34 = sext i1 %33 to i16
  %35 = icmp sgt i16 %28, 0
  %36 = sext i1 %35 to i16
  %37 = and i16 %34, %36
  %38 = icmp ne i16 %37, 0
  br i1 %38, label %b7, label %b6

b6:
  br label %b9

b7:
  %39 = phi i16 [ %28, %b3 ], [ %57, %b10 ]
  %40 = trunc i16 %39 to i8
  %41 = load i16, ptr @b$seg, !tbaa !2
  %42 = inttoptr i16 %41 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 %21
  store i8 %40, ptr addrspace(1) %44
  %45 = add i16 %19, 2
  %46 = icmp sge i16 %45, 320
  br i1 %46, label %b11, label %b13

b9:
  %47 = phi i16 [ 0, %b6 ], [ %57, %b9 ]
  %48 = phi double [ 0.000000e+00, %b6 ], [ %56, %b9 ]
  %49 = phi double [ 0.000000e+00, %b6 ], [ %55, %b9 ]
  %50 = fmul double %48, %48
  %51 = fmul double %49, %49
  %52 = fsub double %50, %51
  %53 = fmul double %48, %49
  %54 = fadd double %53, %53
  %55 = fadd double %54, %18
  %56 = fadd double %52, %20
  %57 = add i16 %47, 1
  %58 = fmul double %56, %56
  %59 = fmul double %55, %55
  %60 = fadd double %58, %59
  %61 = load double, ptr %5
  %62 = fcmp oge double %60, %61
  %63 = sext i1 %62 to i16
  %64 = icmp eq i16 %57, 255
  %65 = sext i1 %64 to i16
  %66 = or i16 %63, %65
  %67 = icmp ne i16 %66, 0
  br i1 %67, label %b10, label %b9

b10:
  br label %b7

b11:
  %68 = load double, ptr %3
  %69 = load double, ptr %4
  %70 = fadd double %68, %69
  %71 = fdiv double %70, 2.000000e+00
  br label %b16

b13:
  %72 = fadd double %20, %9
  %73 = fadd double %18, %13
  br label %b3

b16:
  %74 = phi i16 [ 0, %b11 ], [ %84, %b16 ]
  %75 = phi double [ 0.000000e+00, %b11 ], [ %83, %b16 ]
  %76 = phi double [ 0.000000e+00, %b11 ], [ %82, %b16 ]
  %77 = fmul double %75, %75
  %78 = fmul double %76, %76
  %79 = fsub double %77, %78
  %80 = fmul double %75, %76
  %81 = fadd double %80, %80
  %82 = fadd double %81, %18
  %83 = fadd double %79, %71
  %84 = add i16 %74, 1
  %85 = fmul double %83, %83
  %86 = fmul double %82, %82
  %87 = fadd double %85, %86
  %88 = load double, ptr %5
  %89 = fcmp oge double %87, %88
  %90 = sext i1 %89 to i16
  %91 = icmp eq i16 %84, 255
  %92 = sext i1 %91 to i16
  %93 = or i16 %90, %92
  %94 = icmp ne i16 %93, 0
  br i1 %94, label %b17, label %b16

b17:
  %95 = add i16 %15, 160
  %96 = trunc i16 %84 to i8
  %97 = load i16, ptr @b$seg, !tbaa !2
  %98 = inttoptr i16 %97 to ptr addrspace(2)
  %99 = addrspacecast ptr addrspace(2) %98 to ptr addrspace(1)
  %100 = getelementptr i8, ptr addrspace(1) %99, i16 %95
  store i8 %96, ptr addrspace(1) %100
  ret void
}

define cc1000 void @FRACLINE2(ptr %0, ptr %1, ptr %2, ptr %3, ptr %4, ptr %5) addrspace(1) {
b1:
  %6 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %6)
  %7 = load double, ptr %4
  %8 = load double, ptr %3
  %9 = fsub double %7, %8
  %10 = fdiv double %9, 3.200000e+02
  %11 = load double, ptr %2
  %12 = load double, ptr %1
  %13 = fsub double %11, %12
  %14 = fdiv double %13, 3.200000e+02
  %15 = load i16, ptr %0
  %16 = mul i16 %15, 320
  br label %b3

b3:
  %17 = phi double [ %12, %b1 ], [ %65, %b13 ]
  %18 = phi i16 [ 0, %b1 ], [ %41, %b13 ]
  %19 = phi double [ %8, %b1 ], [ %64, %b13 ]
  %20 = add i16 %18, %16
  %21 = add i16 %20, -320
  %22 = load i16, ptr @b$seg, !tbaa !2
  %23 = inttoptr i16 %22 to ptr addrspace(2)
  %24 = addrspacecast ptr addrspace(2) %23 to ptr addrspace(1)
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 %21
  %26 = load i8, ptr addrspace(1) %25
  %27 = zext i8 %26 to i16
  %28 = add i16 %20, 320
  %29 = getelementptr i8, ptr addrspace(1) %24, i16 %28
  %30 = load i8, ptr addrspace(1) %29
  %31 = zext i8 %30 to i16
  %32 = icmp eq i16 %27, %31
  br i1 %32, label %b5, label %b6

b5:
  %33 = load i8, ptr addrspace(1) %25
  %34 = zext i8 %33 to i16
  br label %b7

b6:
  br label %b9

b7:
  %35 = phi i16 [ %34, %b5 ], [ %53, %b10 ]
  %36 = trunc i16 %35 to i8
  %37 = load i16, ptr @b$seg, !tbaa !2
  %38 = inttoptr i16 %37 to ptr addrspace(2)
  %39 = addrspacecast ptr addrspace(2) %38 to ptr addrspace(1)
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 %20
  store i8 %36, ptr addrspace(1) %40
  %41 = add i16 %18, 1
  %42 = icmp sge i16 %41, 320
  br i1 %42, label %b11, label %b13

b9:
  %43 = phi i16 [ 0, %b6 ], [ %53, %b9 ]
  %44 = phi double [ 0.000000e+00, %b6 ], [ %52, %b9 ]
  %45 = phi double [ 0.000000e+00, %b6 ], [ %51, %b9 ]
  %46 = fmul double %44, %44
  %47 = fmul double %45, %45
  %48 = fsub double %46, %47
  %49 = fmul double %44, %45
  %50 = fadd double %49, %49
  %51 = fadd double %50, %17
  %52 = fadd double %48, %19
  %53 = add i16 %43, 1
  %54 = fmul double %52, %52
  %55 = fmul double %51, %51
  %56 = fadd double %54, %55
  %57 = load double, ptr %5
  %58 = fcmp oge double %56, %57
  %59 = sext i1 %58 to i16
  %60 = icmp eq i16 %53, 255
  %61 = sext i1 %60 to i16
  %62 = or i16 %59, %61
  %63 = icmp ne i16 %62, 0
  br i1 %63, label %b10, label %b9

b10:
  br label %b7

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %6)
  ret void

b13:
  %64 = fadd double %19, %10
  %65 = fadd double %17, %14
  br label %b3
}

define cc1000 void @FRACTALEFFECT(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca double
  %10 = alloca double
  %11 = alloca double
  %12 = alloca double
  %13 = alloca double
  %14 = alloca double
  %15 = alloca double
  %16 = alloca [22 x i8]
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store double 0.000000e+00, ptr %9
  store double 0.000000e+00, ptr %10
  store double 0.000000e+00, ptr %11
  store double 0.000000e+00, ptr %12
  store double 0.000000e+00, ptr %13
  store double 0.000000e+00, ptr %14
  store double 0.000000e+00, ptr %15
  call void @llvm.memset.p0.i16(ptr %16, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr %16)
  %17 = load i16, ptr %0
  %18 = getelementptr i8, ptr @"FRACTAL1%", i16 2
  %19 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %20 = getelementptr i8, ptr %16, i16 2
  br label %b2

b2:
  %21 = phi i16 [ -100, %b1 ], [ %64, %b37 ]
  %22 = phi double [ 1.000000e+00, %b1 ], [ %65, %b37 ]
  %23 = phi i16 [ 1, %b1 ], [ %141, %b37 ]
  %24 = icmp sle i16 %23, %17
  br i1 %24, label %b5, label %b6

b5:
  %25 = add i16 %21, 1
  %26 = add i16 %21, 2
  %27 = add i16 %21, 3
  %28 = sitofp i16 %21 to double
  %29 = fdiv double %28, %22
  %30 = load double, ptr @$float4, !tbaa !2
  %31 = fadd double %29, %30
  store double %31, ptr %15, !tbaa !2
  %32 = sitofp i16 %25 to double
  %33 = fdiv double %32, %22
  %34 = fadd double %33, %30
  store double %34, ptr %14, !tbaa !2
  %35 = sitofp i16 %26 to double
  %36 = fdiv double %35, %22
  %37 = fadd double %36, %30
  store double %37, ptr %13, !tbaa !2
  %38 = sitofp i16 %27 to double
  %39 = fdiv double %38, %22
  %40 = fadd double %39, %30
  store double %40, ptr %12, !tbaa !2
  %41 = load double, ptr @$float5, !tbaa !2
  %42 = fdiv double 1.600000e+02, %22
  %43 = fsub double %41, %42
  store double %43, ptr %11, !tbaa !2
  %44 = fadd double %41, %42
  store double %44, ptr %10, !tbaa !2
  %45 = load i16, ptr %18, !tbaa !2
  %46 = inttoptr i16 %45 to ptr addrspace(2)
  %47 = addrspacecast ptr addrspace(2) %46 to ptr addrspace(1)
  %48 = getelementptr i8, ptr addrspace(1) %47, i16 0
  %49 = addrspacecast ptr addrspace(1) %48 to ptr addrspace(2)
  %50 = ptrtoint ptr addrspace(2) %49 to i16
  store i16 %50, ptr @b$seg, !tbaa !2
  store double 4.000000e+00, ptr %9, !tbaa !2
  %51 = add i16 %21, 100
  store i16 %51, ptr %8, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE(ptr %8, ptr %15, ptr %15, ptr %11, ptr %10, ptr %9)
  %52 = add i16 %21, 102
  store i16 %52, ptr %7, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE(ptr %7, ptr %13, ptr %13, ptr %11, ptr %10, ptr %9)
  %53 = add i16 %21, 101
  store i16 %53, ptr %6, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE2(ptr %6, ptr %14, ptr %14, ptr %11, ptr %10, ptr %9)
  %54 = add i16 %21, 103
  store i16 %54, ptr %5, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE2(ptr %5, ptr %12, ptr %12, ptr %11, ptr %10, ptr %9)
  %55 = add i16 %21, 4
  %56 = icmp sge i16 %55, 100
  br i1 %56, label %b7, label %b9

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %16)
  ret void

b7:
  %57 = fmul double %22, 2.000000e+00
  %58 = load i16, ptr %20, !tbaa !2
  %59 = inttoptr i16 %58 to ptr addrspace(2)
  %60 = addrspacecast ptr addrspace(2) %59 to ptr addrspace(1)
  %61 = load i16, ptr @b$seg, !tbaa !2
  %62 = inttoptr i16 %61 to ptr addrspace(2)
  %63 = addrspacecast ptr addrspace(2) %62 to ptr addrspace(1)
  br label %b10

b9:
  %64 = phi i16 [ %55, %b5 ], [ -100, %b29 ]
  %65 = phi double [ %22, %b5 ], [ %57, %b29 ]
  %66 = load i16, ptr %19, !tbaa !2
  %67 = inttoptr i16 %66 to ptr addrspace(2)
  %68 = addrspacecast ptr addrspace(2) %67 to ptr addrspace(1)
  %69 = getelementptr i8, ptr addrspace(1) %68, i16 0
  %70 = addrspacecast ptr addrspace(1) %69 to ptr addrspace(2)
  %71 = ptrtoint ptr addrspace(2) %70 to i16
  store i16 %71, ptr @b$seg, !tbaa !2
  %72 = sext i16 %23 to i32
  %73 = srem i32 %72, 50
  %74 = trunc i32 %73 to i16
  %75 = load float, ptr @$float6, !tbaa !2
  %76 = sitofp i16 %74 to float
  %77 = fmul float %76, %75
  %78 = call i16 @llvm.lrint.i16.f32(float %77)
  store i16 %78, ptr %4, !tbaa !2
  store i16 %74, ptr %3, !tbaa !2
  %79 = fsub float 3.200000e+02, %77
  %80 = call i16 @llvm.lrint.i16.f32(float %79)
  store i16 %80, ptr %2, !tbaa !2
  %81 = sub i16 200, %74
  store i16 %81, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @RENDER(ptr %4, ptr %3, ptr %2, ptr %1)
  %82 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %83 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %82, ptr @$string7)
  %84 = icmp sgt i16 %83, 0
  br i1 %84, label %b35, label %b37

b10:
  %85 = phi i16 [ 16080, %b7 ], [ %107, %b19 ]
  %86 = phi i16 [ 0, %b7 ], [ %108, %b19 ]
  %87 = icmp sle i16 %86, 99
  br i1 %87, label %b13, label %b14

b13:
  %88 = mul i16 %86, 161
  %89 = shl i16 %88, 1
  br label %b15

b14:
  %90 = load i16, ptr %19, !tbaa !2
  %91 = inttoptr i16 %90 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = load i16, ptr %18, !tbaa !2
  %94 = inttoptr i16 %93 to ptr addrspace(2)
  %95 = addrspacecast ptr addrspace(2) %94 to ptr addrspace(1)
  br label %b20

b15:
  %96 = phi i16 [ %89, %b13 ], [ %106, %b18 ]
  %97 = phi i16 [ %85, %b13 ], [ %104, %b18 ]
  %98 = phi i16 [ 0, %b13 ], [ %105, %b18 ]
  %99 = icmp sle i16 %98, 159
  br i1 %99, label %b18, label %b19

b18:
  %100 = getelementptr i8, ptr addrspace(1) %60, i16 %96
  %101 = getelementptr i8, ptr addrspace(1) %63, i16 %97
  %102 = load i8, ptr addrspace(1) %101
  %103 = zext i8 %102 to i16
  store i16 %103, ptr addrspace(1) %100, !tbaa !4
  %104 = add i16 %97, 1
  %105 = add i16 %98, 1
  %106 = add i16 %96, 2
  br label %b15

b19:
  %107 = add i16 %97, 160
  %108 = add i16 %86, 1
  br label %b10

b20:
  %109 = phi i16 [ 0, %b14 ], [ %115, %b23 ]
  %110 = icmp sle i16 %109, 32000
  br i1 %110, label %b23, label %b24

b23:
  %111 = shl i16 %109, 1
  %112 = getelementptr i8, ptr addrspace(1) %92, i16 %111
  %113 = getelementptr i8, ptr addrspace(1) %95, i16 %111
  %114 = load i16, ptr addrspace(1) %113, !tbaa !4
  store i16 %114, ptr addrspace(1) %112, !tbaa !4
  store i16 0, ptr addrspace(1) %113, !tbaa !4
  %115 = add i16 %109, 1
  br label %b20

b24:
  br label %b25

b25:
  %116 = phi i16 [ 0, %b24 ], [ %138, %b34 ]
  %117 = icmp sle i16 %116, 99
  br i1 %117, label %b28, label %b29

b28:
  %118 = mul i16 %116, 640
  %119 = mul i16 %116, 161
  %120 = shl i16 %119, 1
  br label %b30

b29:
  br label %b9

b30:
  %121 = phi i16 [ %120, %b28 ], [ %137, %b33 ]
  %122 = phi i16 [ %118, %b28 ], [ %136, %b33 ]
  %123 = phi i16 [ 0, %b28 ], [ %135, %b33 ]
  %124 = icmp sle i16 %123, 159
  br i1 %124, label %b33, label %b34

b33:
  %125 = load i16, ptr %20, !tbaa !2
  %126 = inttoptr i16 %125 to ptr addrspace(2)
  %127 = addrspacecast ptr addrspace(2) %126 to ptr addrspace(1)
  %128 = getelementptr i8, ptr addrspace(1) %127, i16 %121
  %129 = load i16, ptr addrspace(1) %128, !tbaa !4
  %130 = trunc i16 %129 to i8
  %131 = load i16, ptr @b$seg, !tbaa !2
  %132 = inttoptr i16 %131 to ptr addrspace(2)
  %133 = addrspacecast ptr addrspace(2) %132 to ptr addrspace(1)
  %134 = getelementptr i8, ptr addrspace(1) %133, i16 %122
  store i8 %130, ptr addrspace(1) %134
  %135 = add i16 %123, 1
  %136 = add i16 %122, 2
  %137 = add i16 %121, 2
  br label %b30

b34:
  %138 = add i16 %116, 1
  br label %b25

b35:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %16)
  ret void

b37:
  %139 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %140 = add i16 %139, 1
  store i16 %140, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %141 = add i16 %23, 1
  br label %b2
}

define cc1000 void @OHCANADA(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  store i16 0, ptr %1
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %5, %b5 ]
  %3 = icmp sle i16 %2, 255
  br i1 %3, label %b5, label %b6

b5:
  %4 = trunc i16 %2 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %4)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %5 = add i16 %2, 1
  br label %b2

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string8, i16 0, i16 1)
  br label %b7

b7:
  %6 = phi i16 [ 0, %b6 ], [ %12, %b10 ]
  %7 = icmp sle i16 %6, 255
  br i1 %7, label %b10, label %b11

b10:
  %8 = trunc i16 %6 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %8)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %9 = sext i16 %6 to i32
  %10 = sdiv i32 %9, 4
  %11 = trunc i32 %10 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %11)
  call void @llrm.ia16.out.i8(i16 969, i8 %11)
  %12 = add i16 %6, 1
  br label %b7

b11:
  store i16 256, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @UNWHITEFADE(ptr %1)
  %13 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %14 = add i16 %13, 16
  store i16 %14, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %15 = load i16, ptr %0
  br label %b12

b12:
  %16 = phi i16 [ 1, %b11 ], [ %24, %b19 ]
  %17 = icmp sle i16 %16, %15
  br i1 %17, label %b15, label %b16

b15:
  %18 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %18, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %18, ptr @"BENCHFRAME&", !tbaa !2
  %19 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %20 = add i16 %19, 1
  store i16 %20, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %21 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %22 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %21, ptr @$string9)
  %23 = icmp sgt i16 %22, 0
  br i1 %23, label %b17, label %b19

b16:
  ret void

b17:
  ret void

b19:
  %24 = add i16 %16, 1
  br label %b12
}

define cc1000 void @PLASMA(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca [22 x i8]
  %3 = alloca [18 x i8]
  %4 = alloca [18 x i8]
  %5 = alloca [18 x i8]
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 22, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %5)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %4)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 512, i16 2, i16 257, ptr %3)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 128, i16 0, i16 128, i16 2, i16 258, ptr %2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  %6 = getelementptr i8, ptr %3, i16 2
  %7 = load i16, ptr %6, !tbaa !2
  %8 = inttoptr i16 %7 to ptr addrspace(2)
  %9 = addrspacecast ptr addrspace(2) %8 to ptr addrspace(1)
  %10 = load float, ptr @$float10, !tbaa !2
  br label %b2

b2:
  %11 = phi i16 [ 0, %b1 ], [ %22, %b5 ]
  %12 = icmp sle i16 %11, 512
  br i1 %12, label %b5, label %b6

b5:
  %13 = shl i16 %11, 1
  %14 = getelementptr i8, ptr addrspace(1) %9, i16 %13
  %15 = sitofp i16 %11 to float
  %16 = fmul float %15, %10
  %17 = fdiv float %16, 2.560000e+02
  %18 = call float @llvm.sin.f32(float %17)
  %19 = fmul float %18, 3.200000e+01
  %20 = fadd float %19, 3.200000e+01
  %21 = call i16 @llvm.lrint.i16.f32(float %20)
  store i16 %21, ptr addrspace(1) %14, !tbaa !4
  %22 = add i16 %11, 1
  br label %b2

b6:
  store i16 1, ptr %1, !tbaa !2
  %23 = load i16, ptr %0
  %24 = getelementptr i8, ptr %5, i16 2
  %25 = getelementptr i8, ptr %2, i16 2
  %26 = getelementptr i8, ptr %4, i16 2
  br label %b8

b8:
  %27 = load i16, ptr %1, !tbaa !2
  %28 = icmp sle i16 %27, %23
  br i1 %28, label %b10, label %b11

b10:
  %29 = load i16, ptr %24, !tbaa !2
  %30 = inttoptr i16 %29 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = load i16, ptr %6, !tbaa !2
  %33 = inttoptr i16 %32 to ptr addrspace(2)
  %34 = addrspacecast ptr addrspace(2) %33 to ptr addrspace(1)
  %35 = mul i16 %27, 7
  %36 = add i16 %35, 3
  br label %b12

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %2)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %3)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %5)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %4)
  ret void

b12:
  %37 = phi i16 [ %36, %b10 ], [ %53, %b15 ]
  %38 = phi i16 [ 0, %b10 ], [ %52, %b15 ]
  %39 = icmp sle i16 %38, 320
  br i1 %39, label %b15, label %b16

b15:
  %40 = shl i16 %38, 1
  %41 = getelementptr i8, ptr addrspace(1) %31, i16 %40
  %42 = add i16 %38, %27
  %43 = and i16 %42, 511
  %44 = shl i16 %43, 1
  %45 = getelementptr i8, ptr addrspace(1) %34, i16 %44
  %46 = load i16, ptr addrspace(1) %45, !tbaa !4
  %47 = and i16 %37, 511
  %48 = shl i16 %47, 1
  %49 = getelementptr i8, ptr addrspace(1) %34, i16 %48
  %50 = load i16, ptr addrspace(1) %49, !tbaa !4
  %51 = add i16 %46, %50
  store i16 %51, ptr addrspace(1) %41, !tbaa !4
  %52 = add i16 %38, 1
  %53 = add i16 %37, 3
  br label %b12

b16:
  %54 = load i16, ptr %1, !tbaa !2
  %55 = mul i16 %54, 5
  %56 = load i16, ptr %6, !tbaa !2
  %57 = inttoptr i16 %56 to ptr addrspace(2)
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = mul i16 %54, 11
  %60 = load i16, ptr %25, !tbaa !2
  %61 = inttoptr i16 %60 to ptr addrspace(2)
  %62 = addrspacecast ptr addrspace(2) %61 to ptr addrspace(1)
  %63 = load i16, ptr %24, !tbaa !2
  %64 = inttoptr i16 %63 to ptr addrspace(2)
  %65 = addrspacecast ptr addrspace(2) %64 to ptr addrspace(1)
  %66 = add i16 %59, 1943
  br label %b17

b17:
  %67 = phi i16 [ %66, %b16 ], [ %108, %b26 ]
  %68 = phi i16 [ %55, %b16 ], [ %107, %b26 ]
  %69 = phi i16 [ 0, %b16 ], [ %106, %b26 ]
  %70 = icmp sle i16 %69, 128
  br i1 %70, label %b20, label %b21

b20:
  %71 = and i16 %68, 511
  %72 = shl i16 %71, 1
  %73 = getelementptr i8, ptr addrspace(1) %58, i16 %72
  %74 = load i16, ptr addrspace(1) %73, !tbaa !4
  %75 = and i16 %67, 511
  %76 = shl i16 %75, 1
  %77 = getelementptr i8, ptr addrspace(1) %58, i16 %76
  %78 = load i16, ptr addrspace(1) %77, !tbaa !4
  %79 = add i16 %74, %78
  %80 = mul i16 %69, 129
  %81 = shl i16 %80, 1
  br label %b22

b21:
  %82 = load i16, ptr %24, !tbaa !2
  %83 = inttoptr i16 %82 to ptr addrspace(2)
  %84 = addrspacecast ptr addrspace(2) %83 to ptr addrspace(1)
  %85 = load i16, ptr %1, !tbaa !2
  %86 = mul i16 %85, 7
  %87 = load i16, ptr %6, !tbaa !2
  %88 = inttoptr i16 %87 to ptr addrspace(2)
  %89 = addrspacecast ptr addrspace(2) %88 to ptr addrspace(1)
  %90 = load i16, ptr %26, !tbaa !2
  %91 = inttoptr i16 %90 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = mul i16 %85, 5
  %94 = shl i16 %85, 1
  %95 = add i16 %94, 371
  br label %b27

b22:
  %96 = phi i16 [ %81, %b20 ], [ %105, %b25 ]
  %97 = phi i16 [ 0, %b20 ], [ %104, %b25 ]
  %98 = icmp sle i16 %97, 128
  br i1 %98, label %b25, label %b26

b25:
  %99 = getelementptr i8, ptr addrspace(1) %62, i16 %96
  %100 = shl i16 %97, 1
  %101 = getelementptr i8, ptr addrspace(1) %65, i16 %100
  %102 = load i16, ptr addrspace(1) %101, !tbaa !4
  %103 = add i16 %102, %79
  store i16 %103, ptr addrspace(1) %99, !tbaa !4
  %104 = add i16 %97, 1
  %105 = add i16 %96, 2
  br label %b22

b26:
  %106 = add i16 %69, 1
  %107 = add i16 %68, 7
  %108 = add i16 %67, 14
  br label %b17

b27:
  %109 = phi i16 [ %95, %b21 ], [ %141, %b30 ]
  %110 = phi i16 [ %93, %b21 ], [ %140, %b30 ]
  %111 = phi i16 [ 0, %b21 ], [ %139, %b30 ]
  %112 = icmp sle i16 %111, 320
  br i1 %112, label %b30, label %b31

b30:
  %113 = shl i16 %111, 1
  %114 = getelementptr i8, ptr addrspace(1) %84, i16 %113
  %115 = mul i16 %111, 11
  %116 = add i16 %115, %86
  %117 = and i16 %116, 511
  %118 = shl i16 %117, 1
  %119 = getelementptr i8, ptr addrspace(1) %89, i16 %118
  %120 = load i16, ptr addrspace(1) %119, !tbaa !4
  %121 = mul i16 %111, 3
  %122 = add i16 %121, %86
  %123 = add i16 %122, 3
  %124 = and i16 %123, 511
  %125 = shl i16 %124, 1
  %126 = getelementptr i8, ptr addrspace(1) %89, i16 %125
  %127 = load i16, ptr addrspace(1) %126, !tbaa !4
  %128 = add i16 %120, %127
  store i16 %128, ptr addrspace(1) %114, !tbaa !4
  %129 = getelementptr i8, ptr addrspace(1) %92, i16 %113
  %130 = and i16 %110, 511
  %131 = shl i16 %130, 1
  %132 = getelementptr i8, ptr addrspace(1) %89, i16 %131
  %133 = load i16, ptr addrspace(1) %132, !tbaa !4
  %134 = and i16 %109, 511
  %135 = shl i16 %134, 1
  %136 = getelementptr i8, ptr addrspace(1) %89, i16 %135
  %137 = load i16, ptr addrspace(1) %136, !tbaa !4
  %138 = add i16 %133, %137
  store i16 %138, ptr addrspace(1) %129, !tbaa !4
  %139 = add i16 %111, 1
  %140 = add i16 %110, 4
  %141 = add i16 %109, 9
  br label %b27

b31:
  br label %b32

b32:
  %142 = phi i16 [ 0, %b31 ], [ %187, %b41 ]
  %143 = phi i16 [ 0, %b31 ], [ %219, %b41 ]
  %144 = icmp sle i16 %143, 199
  br i1 %144, label %b35, label %b36

b35:
  %145 = mul i16 %143, 11
  %146 = load i16, ptr %1, !tbaa !2
  %147 = mul i16 %146, 6
  %148 = add i16 %145, %147
  %149 = and i16 %148, 511
  %150 = shl i16 %149, 1
  %151 = load i16, ptr %6, !tbaa !2
  %152 = inttoptr i16 %151 to ptr addrspace(2)
  %153 = addrspacecast ptr addrspace(2) %152 to ptr addrspace(1)
  %154 = getelementptr i8, ptr addrspace(1) %153, i16 %150
  %155 = load i16, ptr addrspace(1) %154, !tbaa !4
  %156 = mul i16 %143, 14
  %157 = mul i16 %146, 11
  %158 = add i16 %156, %157
  %159 = add i16 %158, 1943
  %160 = and i16 %159, 511
  %161 = shl i16 %160, 1
  %162 = getelementptr i8, ptr addrspace(1) %153, i16 %161
  %163 = load i16, ptr addrspace(1) %162, !tbaa !4
  %164 = add i16 %155, %163
  %165 = mul i16 %143, 9
  %166 = shl i16 %146, 2
  %167 = add i16 %165, %166
  %168 = and i16 %167, 511
  %169 = shl i16 %168, 1
  %170 = getelementptr i8, ptr addrspace(1) %153, i16 %169
  %171 = load i16, ptr addrspace(1) %170, !tbaa !4
  %172 = mul i16 %143, 17
  %173 = mul i16 %146, 23
  %174 = add i16 %172, %173
  %175 = add i16 %174, 1943
  %176 = and i16 %175, 511
  %177 = shl i16 %176, 1
  %178 = getelementptr i8, ptr addrspace(1) %153, i16 %177
  %179 = load i16, ptr addrspace(1) %178, !tbaa !4
  %180 = add i16 %171, %179
  br label %b37

b36:
  call cc1000 addrspace(1) void @UPDPALPLASMA(ptr %1)
  %181 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %182 = add i16 %181, 1
  store i16 %182, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %183 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %184 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %183, ptr @$string11)
  %185 = icmp sgt i16 %184, 0
  br i1 %185, label %b42, label %b44

b37:
  %186 = phi i16 [ 0, %b35 ], [ %218, %b40 ]
  %187 = phi i16 [ %142, %b35 ], [ %217, %b40 ]
  %188 = icmp sle i16 %186, 319
  br i1 %188, label %b40, label %b41

b40:
  %189 = shl i16 %186, 1
  %190 = load i16, ptr %24, !tbaa !2
  %191 = inttoptr i16 %190 to ptr addrspace(2)
  %192 = addrspacecast ptr addrspace(2) %191 to ptr addrspace(1)
  %193 = getelementptr i8, ptr addrspace(1) %192, i16 %189
  %194 = load i16, ptr addrspace(1) %193, !tbaa !4
  %195 = add i16 %194, %164
  %196 = and i16 %195, 127
  %197 = load i16, ptr %26, !tbaa !2
  %198 = inttoptr i16 %197 to ptr addrspace(2)
  %199 = addrspacecast ptr addrspace(2) %198 to ptr addrspace(1)
  %200 = getelementptr i8, ptr addrspace(1) %199, i16 %189
  %201 = load i16, ptr addrspace(1) %200, !tbaa !4
  %202 = add i16 %201, %180
  %203 = and i16 %202, 127
  %204 = mul i16 %203, 129
  %205 = add i16 %204, %196
  %206 = shl i16 %205, 1
  %207 = load i16, ptr %25, !tbaa !2
  %208 = inttoptr i16 %207 to ptr addrspace(2)
  %209 = addrspacecast ptr addrspace(2) %208 to ptr addrspace(1)
  %210 = getelementptr i8, ptr addrspace(1) %209, i16 %206
  %211 = load i16, ptr addrspace(1) %210, !tbaa !4
  %212 = trunc i16 %211 to i8
  %213 = load i16, ptr @b$seg, !tbaa !2
  %214 = inttoptr i16 %213 to ptr addrspace(2)
  %215 = addrspacecast ptr addrspace(2) %214 to ptr addrspace(1)
  %216 = getelementptr i8, ptr addrspace(1) %215, i16 %187
  store i8 %212, ptr addrspace(1) %216
  %217 = add i16 %187, 1
  %218 = add i16 %186, 1
  br label %b37

b41:
  %219 = add i16 %143, 1
  br label %b32

b42:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %2)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %3)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %5)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %4)
  ret void

b44:
  %220 = load i16, ptr %1, !tbaa !2
  %221 = add i16 %220, 1
  store i16 %221, ptr %1, !tbaa !2
  br label %b8
}

define cc1000 void @RENDER(ptr %0, ptr %1, ptr %2, ptr %3) addrspace(1) {
b1:
  %4 = alloca [22 x i8]
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr %4)
  %5 = load i16, ptr %3
  %6 = load i16, ptr %1
  %7 = sub i16 %5, %6
  %8 = load i16, ptr %2
  %9 = load i16, ptr %0
  %10 = sub i16 %8, %9
  %11 = sext i16 %7 to i32
  %12 = sdiv i32 %11, 100
  %13 = trunc i32 %12 to i16
  %14 = sext i16 %10 to i32
  %15 = sdiv i32 %14, 160
  %16 = trunc i32 %15 to i16
  %17 = srem i32 %14, 160
  %18 = trunc i32 %17 to i16
  %19 = srem i32 %11, 100
  %20 = trunc i32 %19 to i16
  %21 = getelementptr i8, ptr %4, i16 2
  %22 = load i16, ptr %21, !tbaa !2
  %23 = inttoptr i16 %22 to ptr addrspace(2)
  %24 = addrspacecast ptr addrspace(2) %23 to ptr addrspace(1)
  %25 = load i16, ptr @b$seg, !tbaa !2
  %26 = inttoptr i16 %25 to ptr addrspace(2)
  %27 = addrspacecast ptr addrspace(2) %26 to ptr addrspace(1)
  br label %b2

b2:
  %28 = phi i16 [ %6, %b1 ], [ %61, %b17 ]
  %29 = phi i16 [ 0, %b1 ], [ %62, %b17 ]
  %30 = phi i16 [ 0, %b1 ], [ %60, %b17 ]
  %31 = icmp sle i16 %29, 99
  br i1 %31, label %b5, label %b6

b5:
  %32 = load i16, ptr %0
  %33 = mul i16 %28, 320
  %34 = mul i16 %29, 161
  %35 = shl i16 %34, 1
  br label %b7

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  br label %b18

b7:
  %36 = phi i16 [ %35, %b5 ], [ %56, %b14 ]
  %37 = phi i16 [ %32, %b5 ], [ %54, %b14 ]
  %38 = phi i16 [ 0, %b5 ], [ %53, %b14 ]
  %39 = phi i16 [ 0, %b5 ], [ %55, %b14 ]
  %40 = icmp sle i16 %39, 159
  br i1 %40, label %b10, label %b11

b10:
  %41 = getelementptr i8, ptr addrspace(1) %24, i16 %36
  %42 = add i16 %37, %33
  %43 = getelementptr i8, ptr addrspace(1) %27, i16 %42
  %44 = load i8, ptr addrspace(1) %43
  %45 = zext i8 %44 to i16
  store i16 %45, ptr addrspace(1) %41, !tbaa !4
  %46 = add i16 %38, %18
  %47 = icmp sgt i16 %46, 160
  br i1 %47, label %b12, label %b14

b11:
  %48 = add i16 %30, %20
  %49 = icmp sgt i16 %48, 100
  br i1 %49, label %b15, label %b17

b12:
  %50 = add i16 %46, -160
  %51 = add i16 %37, 1
  br label %b14

b14:
  %52 = phi i16 [ %51, %b12 ], [ %37, %b10 ]
  %53 = phi i16 [ %50, %b12 ], [ %46, %b10 ]
  %54 = add i16 %52, %16
  %55 = add i16 %39, 1
  %56 = add i16 %36, 2
  br label %b7

b15:
  %57 = add i16 %48, -100
  %58 = add i16 %28, 1
  br label %b17

b17:
  %59 = phi i16 [ %58, %b15 ], [ %28, %b11 ]
  %60 = phi i16 [ %57, %b15 ], [ %48, %b11 ]
  %61 = add i16 %59, %13
  %62 = add i16 %29, 1
  br label %b2

b18:
  %63 = phi i16 [ 0, %b6 ], [ %92, %b27 ]
  %64 = phi i16 [ 16080, %b6 ], [ %91, %b27 ]
  %65 = icmp sle i16 %63, 99
  br i1 %65, label %b21, label %b22

b21:
  %66 = mul i16 %63, 161
  %67 = shl i16 %66, 1
  br label %b23

b22:
  %68 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %69 = load i16, ptr %68, !tbaa !2
  %70 = inttoptr i16 %69 to ptr addrspace(2)
  %71 = addrspacecast ptr addrspace(2) %70 to ptr addrspace(1)
  %72 = getelementptr i8, ptr addrspace(1) %71, i16 0
  %73 = addrspacecast ptr addrspace(1) %72 to ptr addrspace(2)
  %74 = ptrtoint ptr addrspace(2) %73 to i16
  store i16 %74, ptr @b$seg, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %4)
  ret void

b23:
  %75 = phi i16 [ %67, %b21 ], [ %90, %b26 ]
  %76 = phi i16 [ 0, %b21 ], [ %89, %b26 ]
  %77 = icmp sle i16 %76, 159
  br i1 %77, label %b26, label %b27

b26:
  %78 = add i16 %64, %76
  %79 = load i16, ptr %21, !tbaa !2
  %80 = inttoptr i16 %79 to ptr addrspace(2)
  %81 = addrspacecast ptr addrspace(2) %80 to ptr addrspace(1)
  %82 = getelementptr i8, ptr addrspace(1) %81, i16 %75
  %83 = load i16, ptr addrspace(1) %82, !tbaa !4
  %84 = trunc i16 %83 to i8
  %85 = load i16, ptr @b$seg, !tbaa !2
  %86 = inttoptr i16 %85 to ptr addrspace(2)
  %87 = addrspacecast ptr addrspace(2) %86 to ptr addrspace(1)
  %88 = getelementptr i8, ptr addrspace(1) %87, i16 %78
  store i8 %84, ptr addrspace(1) %88
  %89 = add i16 %76, 1
  %90 = add i16 %75, 2
  br label %b23

b27:
  %91 = add i16 %64, 320
  %92 = add i16 %63, 1
  br label %b18
}

define cc1000 void @SHADEBOBEFFECT(ptr %0) addrspace(1) {
b1:
  %1 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr %1)
  store i16 -24576, ptr @b$seg, !tbaa !2
  %2 = load i16, ptr %0
  %3 = getelementptr i8, ptr %1, i16 2
  %4 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %123, %b9 ]
  %6 = phi i16 [ 1, %b1 ], [ %129, %b9 ]
  %7 = icmp sle i16 %6, %2
  br i1 %7, label %b5, label %b6

b5:
  %8 = shl i16 %5, 1
  %9 = load i16, ptr %3, !tbaa !2
  %10 = inttoptr i16 %9 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 %8
  %13 = load i16, ptr addrspace(1) %12, !tbaa !4
  br label %14

14:
  %15 = phi i16 [ %13, %b5 ], [ %24, %44 ]
  %16 = phi i16 [ 0, %b5 ], [ %45, %44 ]
  %17 = icmp sle i16 %16, 31
  br i1 %17, label %18, label %46

18:
  %19 = add i16 %15, 288
  %20 = mul i16 %16, 33
  %21 = shl i16 %20, 1
  br label %22

22:
  %23 = phi i16 [ %21, %18 ], [ %43, %27 ]
  %24 = phi i16 [ %19, %18 ], [ %41, %27 ]
  %25 = phi i16 [ 0, %18 ], [ %42, %27 ]
  %26 = icmp sle i16 %25, 31
  br i1 %26, label %27, label %44

27:
  %28 = load i16, ptr @b$seg
  %29 = inttoptr i16 %28 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %24
  %32 = load i8, ptr addrspace(1) %31
  %33 = zext i8 %32 to i16
  %34 = load i16, ptr %4
  %35 = inttoptr i16 %34 to ptr addrspace(2)
  %36 = addrspacecast ptr addrspace(2) %35 to ptr addrspace(1)
  %37 = getelementptr i8, ptr addrspace(1) %36, i16 %23
  %38 = load i16, ptr addrspace(1) %37
  %39 = sub i16 %33, %38
  %40 = trunc i16 %39 to i8
  store i8 %40, ptr addrspace(1) %31
  %41 = add i16 %24, 1
  %42 = add i16 %25, 1
  %43 = add i16 %23, 2
  br label %22

44:
  %45 = add i16 %16, 1
  br label %14

46:
  %47 = sitofp i16 %6 to float
  %48 = fdiv float %47, 7.100000e+01
  %49 = call float @llvm.sin.f32(float %48)
  %50 = fdiv float %47, 4.700000e+01
  %51 = fadd float %50, 2.000000e+00
  %52 = call float @llvm.cos.f32(float %51)
  %53 = fadd float %49, %52
  %54 = fdiv float %47, 9.100000e+01
  %55 = fadd float %54, 7.000000e+00
  %56 = call float @llvm.cos.f32(float %55)
  %57 = fadd float %53, %56
  %58 = fmul float %57, 4.800000e+01
  %59 = fadd float %58, 1.600000e+02
  %60 = call i16 @llvm.lrint.i16.f32(float %59)
  %61 = fdiv float %47, 4.900000e+01
  %62 = fadd float %61, 3.000000e+00
  %63 = call float @llvm.cos.f32(float %62)
  %64 = fdiv float %47, 4.100000e+01
  %65 = fadd float %64, 2.000000e+00
  %66 = call float @llvm.sin.f32(float %65)
  %67 = fadd float %63, %66
  %68 = fdiv float %47, 9.700000e+01
  %69 = fadd float %68, 7.000000e+00
  %70 = call float @llvm.sin.f32(float %69)
  %71 = fadd float %67, %70
  %72 = fmul float %71, 2.800000e+01
  %73 = fadd float %72, 1.000000e+02
  %74 = call i16 @llvm.lrint.i16.f32(float %73)
  %75 = load i16, ptr %3, !tbaa !2
  %76 = inttoptr i16 %75 to ptr addrspace(2)
  %77 = addrspacecast ptr addrspace(2) %76 to ptr addrspace(1)
  %78 = getelementptr i8, ptr addrspace(1) %77, i16 %8
  %79 = mul i16 %74, 320
  %80 = add i16 %60, %79
  store i16 %80, ptr addrspace(1) %78, !tbaa !4
  %81 = load i16, ptr addrspace(1) %78, !tbaa !4
  br label %82

82:
  %83 = phi i16 [ %81, %46 ], [ %92, %112 ]
  %84 = phi i16 [ 0, %46 ], [ %113, %112 ]
  %85 = icmp sle i16 %84, 31
  br i1 %85, label %86, label %114

86:
  %87 = add i16 %83, 288
  %88 = mul i16 %84, 33
  %89 = shl i16 %88, 1
  br label %90

90:
  %91 = phi i16 [ %89, %86 ], [ %111, %95 ]
  %92 = phi i16 [ %87, %86 ], [ %109, %95 ]
  %93 = phi i16 [ 0, %86 ], [ %110, %95 ]
  %94 = icmp sle i16 %93, 31
  br i1 %94, label %95, label %112

95:
  %96 = load i16, ptr @b$seg
  %97 = inttoptr i16 %96 to ptr addrspace(2)
  %98 = addrspacecast ptr addrspace(2) %97 to ptr addrspace(1)
  %99 = getelementptr i8, ptr addrspace(1) %98, i16 %92
  %100 = load i8, ptr addrspace(1) %99
  %101 = zext i8 %100 to i16
  %102 = load i16, ptr %4
  %103 = inttoptr i16 %102 to ptr addrspace(2)
  %104 = addrspacecast ptr addrspace(2) %103 to ptr addrspace(1)
  %105 = getelementptr i8, ptr addrspace(1) %104, i16 %91
  %106 = load i16, ptr addrspace(1) %105
  %107 = add i16 %101, %106
  %108 = trunc i16 %107 to i8
  store i8 %108, ptr addrspace(1) %99
  %109 = add i16 %92, 1
  %110 = add i16 %93, 1
  %111 = add i16 %91, 2
  br label %90

112:
  %113 = add i16 %84, 1
  br label %82

114:
  %115 = sext i16 %6 to i32
  %116 = sdiv i32 %115, 2
  %117 = trunc i32 %116 to i16
  %118 = add i16 %117, 1
  %119 = add i16 %5, 1
  %120 = sext i16 %119 to i32
  %121 = sext i16 %118 to i32
  %122 = srem i32 %120, %121
  %123 = trunc i32 %122 to i16
  %124 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %125 = add i16 %124, 1
  store i16 %125, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %126 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %127 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %126, ptr @$string12)
  %128 = icmp sgt i16 %127, 0
  br i1 %128, label %b7, label %b9

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %1)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %1)
  ret void

b7:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %1)
  ret void

b9:
  %129 = add i16 %6, 1
  br label %b2
}

define cc1000 void @UNDRAWBOB(ptr %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %29, %b11 ]
  %3 = icmp sle i16 %2, 31
  br i1 %3, label %b5, label %b6

b5:
  %4 = load i16, ptr %0
  %5 = add i16 %4, 288
  store i16 %5, ptr %0
  %6 = mul i16 %2, 33
  %7 = shl i16 %6, 1
  br label %b7

b6:
  ret void

b7:
  %8 = phi i16 [ %7, %b5 ], [ %28, %b10 ]
  %9 = phi i16 [ 0, %b5 ], [ %27, %b10 ]
  %10 = icmp sle i16 %9, 31
  br i1 %10, label %b10, label %b11

b10:
  %11 = load i16, ptr %0
  %12 = load i16, ptr @b$seg, !tbaa !2
  %13 = inttoptr i16 %12 to ptr addrspace(2)
  %14 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 %11
  %16 = load i8, ptr addrspace(1) %15
  %17 = zext i8 %16 to i16
  %18 = load i16, ptr %1, !tbaa !2
  %19 = inttoptr i16 %18 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %8
  %22 = load i16, ptr addrspace(1) %21, !tbaa !4
  %23 = sub i16 %17, %22
  %24 = trunc i16 %23 to i8
  store i8 %24, ptr addrspace(1) %15
  %25 = load i16, ptr %0
  %26 = add i16 %25, 1
  store i16 %26, ptr %0
  %27 = add i16 %9, 1
  %28 = add i16 %8, 2
  br label %b7

b11:
  %29 = add i16 %2, 1
  br label %b2
}

define cc1000 void @UNWHITEFADE(ptr %0) addrspace(1) {
b1:
  %1 = alloca [22 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr %1)
  %2 = getelementptr i8, ptr %1, i16 2
  br label %b2

b2:
  %3 = phi i16 [ 0, %b1 ], [ %29, %b5 ]
  %4 = icmp sle i16 %3, 255
  br i1 %4, label %b5, label %b6

b5:
  %5 = trunc i16 %3 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %5)
  %6 = shl i16 %3, 1
  %7 = load i16, ptr %2, !tbaa !2
  %8 = inttoptr i16 %7 to ptr addrspace(2)
  %9 = addrspacecast ptr addrspace(2) %8 to ptr addrspace(1)
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 %6
  %11 = call i8 @llrm.ia16.in.i8(i16 969)
  %12 = zext i8 %11 to i16
  store i16 %12, ptr addrspace(1) %10, !tbaa !4
  %13 = add i16 %3, 257
  %14 = shl i16 %13, 1
  %15 = load i16, ptr %2, !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2)
  %17 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 %14
  %19 = call i8 @llrm.ia16.in.i8(i16 969)
  %20 = zext i8 %19 to i16
  store i16 %20, ptr addrspace(1) %18, !tbaa !4
  %21 = add i16 %3, 514
  %22 = shl i16 %21, 1
  %23 = load i16, ptr %2, !tbaa !2
  %24 = inttoptr i16 %23 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 %22
  %27 = call i8 @llrm.ia16.in.i8(i16 969)
  %28 = zext i8 %27 to i16
  store i16 %28, ptr addrspace(1) %26, !tbaa !4
  %29 = add i16 %3, 1
  br label %b2

b6:
  %30 = load i16, ptr %0
  br label %b7

b7:
  %31 = phi i16 [ 0, %b6 ], [ %81, %b16 ]
  %32 = icmp sle i16 %31, %30
  br i1 %32, label %b10, label %b11

b10:
  %33 = load i16, ptr %0
  %34 = sitofp i16 %31 to float
  %35 = sitofp i16 %33 to float
  %36 = fdiv float %34, %35
  %37 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %37, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %37, ptr @"BENCHFRAME&", !tbaa !2
  %38 = fsub float 1.000000e+00, %36
  %39 = fmul float 6.300000e+01, %38
  br label %b12

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %1)
  ret void

b12:
  %40 = phi i16 [ 0, %b10 ], [ %78, %b15 ]
  %41 = icmp sle i16 %40, 255
  br i1 %41, label %b15, label %b16

b15:
  %42 = trunc i16 %40 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %42)
  %43 = shl i16 %40, 1
  %44 = load i16, ptr %2, !tbaa !2
  %45 = inttoptr i16 %44 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %43
  %48 = load i16, ptr addrspace(1) %47, !tbaa !4
  %49 = sitofp i16 %48 to float
  %50 = fmul float %49, %36
  %51 = fadd float %50, %39
  %52 = call i16 @llvm.lrint.i16.f32(float %51)
  %53 = trunc i16 %52 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %53)
  %54 = add i16 %40, 257
  %55 = shl i16 %54, 1
  %56 = load i16, ptr %2, !tbaa !2
  %57 = inttoptr i16 %56 to ptr addrspace(2)
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 %55
  %60 = load i16, ptr addrspace(1) %59, !tbaa !4
  %61 = sitofp i16 %60 to float
  %62 = fmul float %61, %36
  %63 = fadd float %62, %39
  %64 = call i16 @llvm.lrint.i16.f32(float %63)
  %65 = trunc i16 %64 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %65)
  %66 = add i16 %40, 514
  %67 = shl i16 %66, 1
  %68 = load i16, ptr %2, !tbaa !2
  %69 = inttoptr i16 %68 to ptr addrspace(2)
  %70 = addrspacecast ptr addrspace(2) %69 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 %67
  %72 = load i16, ptr addrspace(1) %71, !tbaa !4
  %73 = sitofp i16 %72 to float
  %74 = fmul float %73, %36
  %75 = fadd float %74, %39
  %76 = call i16 @llvm.lrint.i16.f32(float %75)
  %77 = trunc i16 %76 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %77)
  %78 = add i16 %40, 1
  br label %b12

b16:
  %79 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %80 = add i16 %79, 1
  store i16 %80, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %81 = add i16 %31, 1
  br label %b7
}

define cc1000 void @UPDPALPLASMA(ptr %0) addrspace(1) {
b1:
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %47, %b5 ]
  %2 = icmp sle i16 %1, 255
  br i1 %2, label %b5, label %b6

b5:
  %3 = trunc i16 %1 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %3)
  %4 = load double, ptr @$float13, !tbaa !2
  %5 = sitofp i16 %1 to double
  %6 = fmul double %5, %4
  %7 = fdiv double %6, 1.280000e+02
  %8 = load i16, ptr %0
  %9 = load float, ptr @$float14, !tbaa !2
  %10 = sitofp i16 %8 to float
  %11 = fmul float %10, %9
  %12 = fpext float %11 to double
  %13 = fadd double %7, %12
  %14 = call double @llvm.cos.f64(double %13)
  %15 = fmul double 3.100000e+01, %14
  %16 = fsub double 3.200000e+01, %15
  %17 = call i16 @llvm.lrint.i16.f64(double %16)
  %18 = trunc i16 %17 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %18)
  %19 = load double, ptr @$float13, !tbaa !2
  %20 = fmul double %5, %19
  %21 = fdiv double %20, 1.280000e+02
  %22 = load i16, ptr %0
  %23 = load float, ptr @$float15, !tbaa !2
  %24 = sitofp i16 %22 to float
  %25 = fmul float %24, %23
  %26 = fpext float %25 to double
  %27 = fadd double %21, %26
  %28 = call double @llvm.cos.f64(double %27)
  %29 = fmul double 3.100000e+01, %28
  %30 = fsub double 3.200000e+01, %29
  %31 = call i16 @llvm.lrint.i16.f64(double %30)
  %32 = trunc i16 %31 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %32)
  %33 = load double, ptr @$float13, !tbaa !2
  %34 = fmul double %5, %33
  %35 = fdiv double %34, 6.400000e+01
  %36 = load i16, ptr %0
  %37 = load float, ptr @$float16, !tbaa !2
  %38 = sitofp i16 %36 to float
  %39 = fmul float %38, %37
  %40 = fpext float %39 to double
  %41 = fadd double %35, %40
  %42 = call double @llvm.cos.f64(double %41)
  %43 = fmul double 3.100000e+01, %42
  %44 = fsub double 3.200000e+01, %43
  %45 = call i16 @llvm.lrint.i16.f64(double %44)
  %46 = trunc i16 %45 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %46)
  %47 = add i16 %1, 1
  br label %b2

b6:
  ret void
}

define cc1000 void @WHITEFADE(ptr %0) addrspace(1) {
b1:
  %1 = alloca [22 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr %1)
  %2 = getelementptr i8, ptr %1, i16 2
  br label %b2

b2:
  %3 = phi i16 [ 0, %b1 ], [ %29, %b5 ]
  %4 = icmp sle i16 %3, 255
  br i1 %4, label %b5, label %b6

b5:
  %5 = trunc i16 %3 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %5)
  %6 = shl i16 %3, 1
  %7 = load i16, ptr %2, !tbaa !2
  %8 = inttoptr i16 %7 to ptr addrspace(2)
  %9 = addrspacecast ptr addrspace(2) %8 to ptr addrspace(1)
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 %6
  %11 = call i8 @llrm.ia16.in.i8(i16 969)
  %12 = zext i8 %11 to i16
  store i16 %12, ptr addrspace(1) %10, !tbaa !4
  %13 = add i16 %3, 257
  %14 = shl i16 %13, 1
  %15 = load i16, ptr %2, !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2)
  %17 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 %14
  %19 = call i8 @llrm.ia16.in.i8(i16 969)
  %20 = zext i8 %19 to i16
  store i16 %20, ptr addrspace(1) %18, !tbaa !4
  %21 = add i16 %3, 514
  %22 = shl i16 %21, 1
  %23 = load i16, ptr %2, !tbaa !2
  %24 = inttoptr i16 %23 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 %22
  %27 = call i8 @llrm.ia16.in.i8(i16 969)
  %28 = zext i8 %27 to i16
  store i16 %28, ptr addrspace(1) %26, !tbaa !4
  %29 = add i16 %3, 1
  br label %b2

b6:
  %30 = load i16, ptr %0
  br label %b7

b7:
  %31 = phi i16 [ %30, %b6 ], [ %81, %b16 ]
  %32 = icmp sge i16 %31, 0
  br i1 %32, label %b10, label %b11

b10:
  %33 = load i16, ptr %0
  %34 = sitofp i16 %31 to float
  %35 = sitofp i16 %33 to float
  %36 = fdiv float %34, %35
  %37 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %37, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %37, ptr @"BENCHFRAME&", !tbaa !2
  %38 = fsub float 1.000000e+00, %36
  %39 = fmul float 6.300000e+01, %38
  br label %b12

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %1)
  ret void

b12:
  %40 = phi i16 [ 0, %b10 ], [ %78, %b15 ]
  %41 = icmp sle i16 %40, 255
  br i1 %41, label %b15, label %b16

b15:
  %42 = trunc i16 %40 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %42)
  %43 = shl i16 %40, 1
  %44 = load i16, ptr %2, !tbaa !2
  %45 = inttoptr i16 %44 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %43
  %48 = load i16, ptr addrspace(1) %47, !tbaa !4
  %49 = sitofp i16 %48 to float
  %50 = fmul float %49, %36
  %51 = fadd float %50, %39
  %52 = call i16 @llvm.lrint.i16.f32(float %51)
  %53 = trunc i16 %52 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %53)
  %54 = add i16 %40, 257
  %55 = shl i16 %54, 1
  %56 = load i16, ptr %2, !tbaa !2
  %57 = inttoptr i16 %56 to ptr addrspace(2)
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 %55
  %60 = load i16, ptr addrspace(1) %59, !tbaa !4
  %61 = sitofp i16 %60 to float
  %62 = fmul float %61, %36
  %63 = fadd float %62, %39
  %64 = call i16 @llvm.lrint.i16.f32(float %63)
  %65 = trunc i16 %64 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %65)
  %66 = add i16 %40, 514
  %67 = shl i16 %66, 1
  %68 = load i16, ptr %2, !tbaa !2
  %69 = inttoptr i16 %68 to ptr addrspace(2)
  %70 = addrspacecast ptr addrspace(2) %69 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 %67
  %72 = load i16, ptr addrspace(1) %71, !tbaa !4
  %73 = sitofp i16 %72 to float
  %74 = fmul float %73, %36
  %75 = fadd float %74, %39
  %76 = call i16 @llvm.lrint.i16.f32(float %75)
  %77 = trunc i16 %76 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %77)
  %78 = add i16 %40, 1
  br label %b12

b16:
  %79 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %80 = add i16 %79, 1
  store i16 %80, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %81 = add i16 %31, -1
  br label %b7
}

define cc1000 void @BENCHMARK(ptr %0) addrspace(1) {
b1:
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca [18 x i8]
  %4 = alloca [18 x i8]
  store i32 0, ptr %1
  store i32 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 767, i16 2, i16 257, ptr %4)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 5, i16 4, i16 257, ptr %3)
  call cc1000 addrspace(1) void @TSCSNAP(ptr %2, ptr %1)
  store i16 -24576, ptr @b$seg, !tbaa !2
  %5 = inttoptr i16 -24576 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  br label %b2

b2:
  %7 = phi i32 [ 0, %b1 ], [ %19, %b5 ]
  %8 = phi i32 [ 1, %b1 ], [ %16, %b5 ]
  %9 = phi i32 [ 0, %b1 ], [ %18, %b5 ]
  %10 = icmp sle i32 %7, 63999
  br i1 %10, label %b5, label %b6

b5:
  %11 = trunc i32 %7 to i16
  %12 = getelementptr i8, ptr addrspace(1) %6, i16 %11
  %13 = load i8, ptr addrspace(1) %12
  %14 = zext i8 %13 to i32
  %15 = add i32 %8, %14
  %16 = srem i32 %15, 65521
  %17 = add i32 %9, %16
  %18 = srem i32 %17, 65521
  %19 = add i32 %7, 1
  br label %b2

b6:
  %20 = load i16, ptr %0
  %21 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %20)
  %22 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %21)
  %23 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string17, ptr %22)
  %24 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %23, ptr @$string18)
  call cc1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %24, i16 0, i16 -1536)
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  call void @llrm.ia16.out.i8(i16 967, i8 0)
  %25 = getelementptr i8, ptr %4, i16 2
  br label %b7

b7:
  %26 = phi i16 [ 0, %b6 ], [ %35, %b10 ]
  %27 = icmp sle i16 %26, 767
  br i1 %27, label %b10, label %b11

b10:
  %28 = shl i16 %26, 1
  %29 = load i16, ptr %25, !tbaa !2
  %30 = inttoptr i16 %29 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %28
  %33 = call i8 @llrm.ia16.in.i8(i16 969)
  %34 = zext i8 %33 to i16
  store i16 %34, ptr addrspace(1) %32, !tbaa !4
  %35 = add i16 %26, 1
  br label %b7

b11:
  %36 = load i16, ptr %25, !tbaa !2
  %37 = inttoptr i16 %36 to ptr addrspace(2)
  %38 = addrspacecast ptr addrspace(2) %37 to ptr addrspace(1)
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 0
  %40 = addrspacecast ptr addrspace(1) %39 to ptr addrspace(2)
  %41 = ptrtoint ptr addrspace(2) %40 to i16
  store i16 %41, ptr @b$seg, !tbaa !2
  %42 = load i16, ptr %0
  %43 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %42)
  %44 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %43)
  %45 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string19, ptr %44)
  %46 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %45, ptr @$string20)
  %47 = load i16, ptr %25, !tbaa !2
  %48 = inttoptr i16 %47 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 0
  %51 = ptrtoint ptr addrspace(1) %50 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %46, i16 %51, i16 1536)
  %52 = getelementptr i8, ptr %3, i16 2
  %53 = load i16, ptr %52, !tbaa !2
  %54 = inttoptr i16 %53 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 0
  %57 = load i32, ptr @"BENCHHI&", !tbaa !2
  store i32 %57, ptr addrspace(1) %56, !tbaa !4
  %58 = getelementptr i8, ptr addrspace(1) %55, i16 4
  %59 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %59, ptr addrspace(1) %58, !tbaa !4
  %60 = getelementptr i8, ptr addrspace(1) %55, i16 8
  %61 = load i32, ptr %2, !tbaa !2
  store i32 %61, ptr addrspace(1) %60, !tbaa !4
  %62 = getelementptr i8, ptr addrspace(1) %55, i16 12
  %63 = load i32, ptr %1, !tbaa !2
  store i32 %63, ptr addrspace(1) %62, !tbaa !4
  %64 = getelementptr i8, ptr addrspace(1) %55, i16 16
  store i32 %9, ptr addrspace(1) %64, !tbaa !4
  %65 = getelementptr i8, ptr addrspace(1) %55, i16 20
  store i32 %8, ptr addrspace(1) %65, !tbaa !4
  %66 = addrspacecast ptr addrspace(1) %56 to ptr addrspace(2)
  %67 = ptrtoint ptr addrspace(2) %66 to i16
  store i16 %67, ptr @b$seg, !tbaa !2
  %68 = load i16, ptr %0
  %69 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %68)
  %70 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %69)
  %71 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string21, ptr %70)
  %72 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %71, ptr @$string22)
  %73 = load i16, ptr %52, !tbaa !2
  %74 = inttoptr i16 %73 to ptr addrspace(2)
  %75 = addrspacecast ptr addrspace(2) %74 to ptr addrspace(1)
  %76 = getelementptr i8, ptr addrspace(1) %75, i16 0
  %77 = ptrtoint ptr addrspace(1) %76 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %72, i16 %77, i16 24)
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc1000 addrspace(1) void @TSCSNAP(ptr @"BENCHHI&", ptr @"BENCHLO&")
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %3)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %4)
  ret void
}

declare cc1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare cc1000 ptr @llrm.qb.B$TIMR() addrspace(1)

declare cc1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$SCLS(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

declare float @llvm.sqrt.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 ptr @llrm.qb.B$INKY() addrspace(1)

declare cc1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1)

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.cos.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare double @llvm.cos.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$DSG0() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
