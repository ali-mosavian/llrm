target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [36 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"DSPBASE%" = internal global [2 x i8] zeroinitializer
@BUFFER$ = internal global [32767 x i8] zeroinitializer
@BUFFER$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @BUFFER$ to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @BUFFER$, i16 -32767), [6 x i8] c"\FF\7F\01\00\01\00" }>
@"BUFOFS&" = internal global [4 x i8] zeroinitializer
@"SND%" = internal global [2 x i8] zeroinitializer
@"TEXT%" = internal global [9062 x i8] zeroinitializer
@TEXT$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"TEXT%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"TEXT%", [6 x i8] c"\02\00\B3\11\00\00" }>
@"MASK%" = internal global [9062 x i8] zeroinitializer
@MASK$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"MASK%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"MASK%", [6 x i8] c"\02\00\B3\11\00\00" }>
@b$seg = global [2 x i8] zeroinitializer
@$string7 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string7, i16 4), [8 x i8] c"text.bsv" }>
@$string8 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string8, i16 4), [8 x i8] c"mask.bsv" }>
@$string9 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string9, i16 4), [8 x i8] c"dem1.raw" }>
@"OLDTIMER#" = internal global [8 x i8] zeroinitializer
@"OLDTIMER2#" = internal global [8 x i8] zeroinitializer
@"ADDX%" = internal global [2 x i8] zeroinitializer
@"X2%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@$string10 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string10, i16 4), [20 x i8] c"press Escape to exit" }>
@$float11 = internal constant [4 x i8] zeroinitializer
@$string12 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string12, i16 4), [8 x i8] c"-NOSOUND" }>
@$string13 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string13, i16 4), [8 x i8] c"pal.dat\00" }>
@$string14 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string14, i16 4), [2 x i8] c"V\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string15, i16 4), [4 x i8] c".BIN" }>
@$string16 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string16, i16 4), [2 x i8] c"P\00" }>
@$string17 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string17, i16 4), [4 x i8] c".BIN" }>
@$string18 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string18, i16 4), [2 x i8] c"T\00" }>
@$string19 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string19, i16 4), [4 x i8] c".BIN" }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  %0 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 0
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = addrspacecast ptr addrspace(1) %1 to ptr addrspace(2)
  %3 = ptrtoint ptr addrspace(2) %2 to i16
  %4 = sext i16 %3 to i32
  %5 = shl i32 %4, 4
  %6 = ptrtoint ptr addrspace(1) %1 to i16
  %7 = sext i16 %6 to i32
  %8 = add i32 %5, %7
  store i32 %8, ptr @"BUFOFS&", !tbaa !2
  %9 = getelementptr inbounds i16, ptr @"TEXT%", i16 0
  %10 = addrspacecast ptr %9 to ptr addrspace(1)
  %11 = addrspacecast ptr addrspace(1) %10 to ptr addrspace(2)
  %12 = ptrtoint ptr addrspace(2) %11 to i16
  store i16 %12, ptr @b$seg, !tbaa !2
  %13 = ptrtoint ptr addrspace(1) %10 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string7, i16 %13, i16 1)
  %14 = getelementptr inbounds i16, ptr @"MASK%", i16 0
  %15 = addrspacecast ptr %14 to ptr addrspace(1)
  %16 = addrspacecast ptr addrspace(1) %15 to ptr addrspace(2)
  %17 = ptrtoint ptr addrspace(2) %16 to i16
  store i16 %17, ptr @b$seg, !tbaa !2
  %18 = ptrtoint ptr addrspace(1) %15 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string8, i16 %18, i16 1)
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  store i16 544, ptr @"DSPBASE%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string9, i16 1, i16 -1, i16 32)
  call cc1000 addrspace(1) void @INIT()
  store i16 0, ptr @$data, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr @$data)
  %19 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %20 = load float, ptr %19
  %21 = fpext float %20 to double
  store double %21, ptr @"OLDTIMER#", !tbaa !2
  %22 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %23 = load float, ptr %22
  %24 = fpext float %23 to double
  store double %24, ptr @"OLDTIMER2#", !tbaa !2
  store i16 1, ptr @"ADDX%", !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 2
  %26 = getelementptr i8, ptr @$data, i16 4
  %27 = getelementptr i8, ptr @$data, i16 8
  %28 = getelementptr i8, ptr @$data, i16 10
  %29 = getelementptr i8, ptr @$data, i16 12
  %30 = getelementptr i8, ptr @$data, i16 30
  %31 = getelementptr i8, ptr @$data, i16 14
  %32 = getelementptr i8, ptr @$data, i16 16
  %33 = getelementptr i8, ptr @$data, i16 18
  %34 = getelementptr i8, ptr @$data, i16 20
  %35 = getelementptr inbounds i16, ptr @"MASK%", i16 1500
  %36 = addrspacecast ptr %35 to ptr addrspace(1)
  %37 = getelementptr i8, ptr @$data, i16 22
  %38 = getelementptr i8, ptr @$data, i16 24
  %39 = getelementptr i8, ptr @$data, i16 26
  %40 = getelementptr i8, ptr @$data, i16 28
  %41 = getelementptr inbounds i16, ptr @"TEXT%", i16 1500
  %42 = addrspacecast ptr %41 to ptr addrspace(1)
  br label %b3

b3:
  %43 = load i16, ptr @"X2%", !tbaa !2
  %44 = load i16, ptr @"ADDX%", !tbaa !2
  %45 = add i16 %43, %44
  store i16 %45, ptr @"X2%", !tbaa !2
  %46 = icmp sgt i16 %45, 150
  br i1 %46, label %b5, label %b7

b4:
  %47 = getelementptr i8, ptr @$data, i16 32
  store i16 1, ptr %47, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr %47)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b5:
  store i16 -1, ptr @"ADDX%", !tbaa !2
  br label %b7

b7:
  %48 = load i16, ptr @"X2%", !tbaa !2
  %49 = icmp slt i16 %48, 20
  br i1 %49, label %b8, label %b10

b8:
  store i16 1, ptr @"ADDX%", !tbaa !2
  br label %b10

b10:
  %50 = load i16, ptr @"X2%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %50, i16 100)
  store i16 16, ptr %25, !tbaa !2
  store float 1.600000e+01, ptr %26, !tbaa !2
  %51 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %52 = load float, ptr %51
  store i16 64, ptr %27, !tbaa !2
  %53 = fmul float %52, 6.400000e+01
  %54 = call float @llvm.rint.f32(float %53)
  %55 = fcmp olt float %53, %54
  %56 = sext i1 %55 to i16
  store i16 %56, ptr %28, !tbaa !2
  %57 = sitofp i16 %56 to float
  %58 = fadd float %54, %57
  store i16 200, ptr %29, !tbaa !2
  %59 = fadd float %58, 2.000000e+02
  %60 = call i16 @llvm.lrint.i16.f32(float %59)
  %61 = load float, ptr %26, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$CIRC(float %61, i16 %60)
  %62 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %62, ptr @"BENCHFRAME&", !tbaa !2
  %63 = srem i32 %62, 8
  %64 = icmp eq i32 %63, 0
  br i1 %64, label %b11, label %b13

b11:
  %65 = load i16, ptr @"X%", !tbaa !2
  %66 = add i16 %65, 15
  %67 = load i16, ptr @"Y%", !tbaa !2
  %68 = add i16 %67, 15
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %66, i16 %68)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %15, ptr @MASK$descriptor, i16 1)
  %69 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %70 = load float, ptr %69
  store i16 10, ptr %31, !tbaa !2
  %71 = fmul float %70, 1.000000e+01
  %72 = call float @llvm.rint.f32(float %71)
  %73 = fcmp olt float %71, %72
  %74 = sext i1 %73 to i16
  store i16 %74, ptr %32, !tbaa !2
  %75 = sitofp i16 %74 to float
  %76 = fadd float %72, %75
  %77 = call i16 @llvm.lrint.i16.f32(float %76)
  store i16 %77, ptr @"X%", !tbaa !2
  %78 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %79 = load float, ptr %78
  store i16 10, ptr %33, !tbaa !2
  %80 = fmul float %79, 1.000000e+01
  %81 = call float @llvm.rint.f32(float %80)
  %82 = fcmp olt float %80, %81
  %83 = sext i1 %82 to i16
  store i16 %83, ptr %34, !tbaa !2
  %84 = sitofp i16 %83 to float
  %85 = fadd float %81, %84
  %86 = call i16 @llvm.lrint.i16.f32(float %85)
  store i16 %86, ptr @"Y%", !tbaa !2
  %87 = load i16, ptr @"X%", !tbaa !2
  %88 = add i16 %87, 15
  %89 = add i16 %86, 15
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %88, i16 %89)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %15, ptr @MASK$descriptor, i16 1)
  %90 = load i16, ptr @"X%", !tbaa !2
  %91 = add i16 %90, 15
  %92 = load i16, ptr @"Y%", !tbaa !2
  %93 = add i16 %92, 15
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %91, i16 %93)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %10, ptr @TEXT$descriptor, i16 0)
  %94 = load i16, ptr @"X%", !tbaa !2
  %95 = add i16 %94, 80
  %96 = load i16, ptr @"Y%", !tbaa !2
  %97 = add i16 %96, 150
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %95, i16 %97)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %36, ptr @MASK$descriptor, i16 1)
  %98 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %99 = load float, ptr %98
  store i16 10, ptr %37, !tbaa !2
  %100 = fmul float %99, 1.000000e+01
  %101 = call float @llvm.rint.f32(float %100)
  %102 = fcmp olt float %100, %101
  %103 = sext i1 %102 to i16
  store i16 %103, ptr %38, !tbaa !2
  %104 = sitofp i16 %103 to float
  %105 = fadd float %101, %104
  %106 = call i16 @llvm.lrint.i16.f32(float %105)
  store i16 %106, ptr @"X%", !tbaa !2
  %107 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %108 = load float, ptr %107
  store i16 10, ptr %39, !tbaa !2
  %109 = fmul float %108, 1.000000e+01
  %110 = call float @llvm.rint.f32(float %109)
  %111 = fcmp olt float %109, %110
  %112 = sext i1 %111 to i16
  store i16 %112, ptr %40, !tbaa !2
  %113 = sitofp i16 %112 to float
  %114 = fadd float %110, %113
  %115 = call i16 @llvm.lrint.i16.f32(float %114)
  store i16 %115, ptr @"Y%", !tbaa !2
  %116 = load i16, ptr @"X%", !tbaa !2
  %117 = add i16 %116, 80
  %118 = add i16 %115, 150
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %117, i16 %118)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %36, ptr @MASK$descriptor, i16 1)
  %119 = load i16, ptr @"X%", !tbaa !2
  %120 = add i16 %119, 80
  %121 = load i16, ptr @"Y%", !tbaa !2
  %122 = add i16 %121, 150
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %120, i16 %122)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %42, ptr @TEXT$descriptor, i16 0)
  %123 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %123, ptr @"BENCHFRAME&", !tbaa !2
  %124 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %125 = load float, ptr %124
  %126 = fpext float %125 to double
  store double %126, ptr @"OLDTIMER#", !tbaa !2
  br label %b13

b13:
  %127 = load i16, ptr @"SND%", !tbaa !2
  %128 = icmp ne i16 %127, 0
  br i1 %128, label %b14, label %b16

b14:
  store i16 32767, ptr %30, !tbaa !2
  %129 = call cc1000 addrspace(1) i16 @"DMADONE%"(ptr %30)
  %130 = icmp ne i16 %129, 0
  br i1 %130, label %b17, label %b16

b16:
  %131 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %132 = icmp sgt i32 %131, 1500
  br i1 %132, label %b20, label %b22

b17:
  call cc1000 addrspace(1) void @PLAYMUSIC()
  br label %b16

b20:
  call cc1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 150, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 8, i16 1, i16 7, i16 4)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string10)
  br label %b22

b22:
  %133 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %134 = add i32 %133, 1
  store i32 %134, ptr @"BENCHFRAME&", !tbaa !2
  %135 = icmp sge i32 %134, 3000
  br i1 %135, label %b4, label %b3
}

define cc1000 i16 @DMADONE(ptr %0) addrspace(1) willreturn {
b1:
  %1 = call i8 @llrm.ia16.in.i8(i16 3)
  %2 = call i8 @llrm.ia16.in.i8(i16 3)
  %3 = zext i8 %2 to i32
  %4 = shl i32 %3, 8
  %5 = zext i8 %1 to i32
  %6 = add i32 %4, %5
  %7 = load i16, ptr %0
  %8 = add i16 %7, -1
  %9 = sext i16 %8 to i32
  %10 = icmp sgt i32 %6, %9
  br i1 %10, label %b2, label %b4

b2:
  %11 = load i16, ptr @"DSPBASE%", !tbaa !2
  %12 = add i16 %11, 14
  %13 = call i8 @llrm.ia16.in.i8(i16 %12)
  br label %b4

b4:
  %14 = phi i16 [ 1, %b2 ], [ 0, %b1 ]
  ret i16 %14
}

define cc1000 void @INIT() addrspace(1) {
b1:
  %0 = load i16, ptr @"DSPBASE%", !tbaa !2
  %1 = add i16 %0, 6
  call void @llrm.ia16.out.i8(i16 %1, i8 1)
  br label %b2

b2:
  %2 = phi float [ 1.000000e+00, %b1 ], [ %10, %b5 ]
  %3 = load float, ptr @$float11, !tbaa !2
  %4 = fcmp oge float 1.000000e+00, %3
  br i1 %4, label %b3, label %b4

b3:
  %5 = fcmp ole float %2, 4.000000e+00
  br i1 %5, label %b5, label %b6

b4:
  %6 = fcmp oge float %2, 4.000000e+00
  br i1 %6, label %b5, label %b6

b5:
  %7 = load i16, ptr @"DSPBASE%", !tbaa !2
  %8 = add i16 %7, 6
  %9 = call i8 @llrm.ia16.in.i8(i16 %8)
  %10 = fadd float %2, 1.000000e+00
  br label %b2

b6:
  %11 = load i16, ptr @"DSPBASE%", !tbaa !2
  %12 = add i16 %11, 6
  call void @llrm.ia16.out.i8(i16 %12, i8 0)
  %13 = call cc1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %14 = call cc1000 addrspace(1) i16 @llrm.qb.B$INS2(ptr %13, ptr @$string12)
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b9, label %b8

b8:
  br label %b9

b9:
  %16 = phi float [ 0.000000e+00, %b6 ], [ -1.000000e+00, %b8 ]
  %17 = load i16, ptr @"DSPBASE%", !tbaa !2
  %18 = add i16 %17, 14
  %19 = call i8 @llrm.ia16.in.i8(i16 %18)
  %20 = zext i8 %19 to i16
  %21 = and i16 %20, 128
  %22 = icmp eq i16 %21, 128
  %23 = sext i1 %22 to i16
  %24 = load i16, ptr @"DSPBASE%", !tbaa !2
  %25 = add i16 %24, 10
  %26 = call i8 @llrm.ia16.in.i8(i16 %25)
  %27 = zext i8 %26 to i16
  %28 = icmp eq i16 %27, 170
  %29 = sext i1 %28 to i16
  %30 = and i16 %23, %29
  %31 = icmp ne i16 %30, 0
  br i1 %31, label %b12, label %b11

b11:
  br label %b12

b12:
  %32 = phi float [ 0.000000e+00, %b9 ], [ %16, %b11 ]
  br label %33

33:
  %34 = load i16, ptr @"DSPBASE%"
  %35 = add i16 %34, 12
  %36 = call i8 @llrm.ia16.in.i8(i16 %35)
  %37 = zext i8 %36 to i16
  %38 = and i16 %37, 128
  %39 = icmp ne i16 %38, 0
  br i1 %39, label %40, label %41

40:
  br label %33

41:
  %42 = load i16, ptr @"DSPBASE%"
  %43 = add i16 %42, 12
  call void @llrm.ia16.out.i8(i16 %43, i8 -47)
  %44 = load float, ptr @$float11, !tbaa !2
  %45 = fcmp une float %32, %44
  br i1 %45, label %b13, label %b15

b13:
  call cc1000 addrspace(1) void @PLAYMUSIC()
  br label %b15

b15:
  call cc1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string13, i16 -1536, i16 1)
  br label %b16

b16:
  %46 = phi float [ 0.000000e+00, %b15 ], [ %78, %b19 ]
  %47 = load float, ptr @$float11, !tbaa !2
  %48 = fcmp oge float 1.000000e+00, %47
  br i1 %48, label %b17, label %b18

b17:
  %49 = fcmp ole float %46, 2.550000e+02
  br i1 %49, label %b19, label %b20

b18:
  %50 = fcmp oge float %46, 2.550000e+02
  br i1 %50, label %b19, label %b20

b19:
  %51 = call i16 @llvm.lrint.i16.f32(float %46)
  %52 = trunc i16 %51 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %52)
  %53 = fmul float %46, 3.000000e+00
  %54 = fadd float 6.400000e+04, %53
  %55 = call i32 @llvm.lrint.i32.f32(float %54)
  %56 = trunc i32 %55 to i16
  %57 = load i16, ptr @b$seg, !tbaa !2
  %58 = inttoptr i16 %57 to ptr addrspace(2)
  %59 = addrspacecast ptr addrspace(2) %58 to ptr addrspace(1)
  %60 = getelementptr i8, ptr addrspace(1) %59, i16 %56
  %61 = load i8, ptr addrspace(1) %60
  call void @llrm.ia16.out.i8(i16 969, i8 %61)
  %62 = fadd float %54, 1.000000e+00
  %63 = call i32 @llvm.lrint.i32.f32(float %62)
  %64 = trunc i32 %63 to i16
  %65 = load i16, ptr @b$seg, !tbaa !2
  %66 = inttoptr i16 %65 to ptr addrspace(2)
  %67 = addrspacecast ptr addrspace(2) %66 to ptr addrspace(1)
  %68 = getelementptr i8, ptr addrspace(1) %67, i16 %64
  %69 = load i8, ptr addrspace(1) %68
  call void @llrm.ia16.out.i8(i16 969, i8 %69)
  %70 = fadd float %54, 2.000000e+00
  %71 = call i32 @llvm.lrint.i32.f32(float %70)
  %72 = trunc i32 %71 to i16
  %73 = load i16, ptr @b$seg, !tbaa !2
  %74 = inttoptr i16 %73 to ptr addrspace(2)
  %75 = addrspacecast ptr addrspace(2) %74 to ptr addrspace(1)
  %76 = getelementptr i8, ptr addrspace(1) %75, i16 %72
  %77 = load i8, ptr addrspace(1) %76
  call void @llrm.ia16.out.i8(i16 969, i8 %77)
  %78 = fadd float %46, 1.000000e+00
  br label %b16

b20:
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  ret void
}

define cc1000 void @PLAYMUSIC() addrspace(1) {
b1:
  %0 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 0
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %1, i16 32767)
  call void @llrm.ia16.out.i8(i16 10, i8 5)
  call void @llrm.ia16.out.i8(i16 12, i8 2)
  call void @llrm.ia16.out.i8(i16 11, i8 73)
  call void @llrm.ia16.out.i8(i16 2, i8 0)
  call void @llrm.ia16.out.i8(i16 2, i8 0)
  %2 = load i32, ptr @"BUFOFS&", !tbaa !2
  %3 = and i32 %2, 255
  %4 = trunc i32 %3 to i16
  %5 = sext i16 %4 to i32
  %6 = sub i32 %2, %5
  %7 = sdiv i32 %6, 255
  %8 = trunc i32 %3 to i8
  call void @llrm.ia16.out.i8(i16 3, i8 %8)
  %9 = trunc i32 %7 to i8
  call void @llrm.ia16.out.i8(i16 3, i8 %9)
  %10 = load i32, ptr @"BUFOFS&", !tbaa !2
  %11 = sdiv i32 %10, 65536
  %12 = trunc i32 %11 to i8
  call void @llrm.ia16.out.i8(i16 131, i8 %12)
  call void @llrm.ia16.out.i8(i16 10, i8 1)
  br label %13

13:
  %14 = load i16, ptr @"DSPBASE%"
  %15 = add i16 %14, 12
  %16 = call i8 @llrm.ia16.in.i8(i16 %15)
  %17 = zext i8 %16 to i16
  %18 = and i16 %17, 128
  %19 = icmp ne i16 %18, 0
  br i1 %19, label %20, label %21

20:
  br label %13

21:
  %22 = load i16, ptr @"DSPBASE%"
  %23 = add i16 %22, 12
  call void @llrm.ia16.out.i8(i16 %23, i8 64)
  br label %24

24:
  %25 = load i16, ptr @"DSPBASE%"
  %26 = add i16 %25, 12
  %27 = call i8 @llrm.ia16.in.i8(i16 %26)
  %28 = zext i8 %27 to i16
  %29 = and i16 %28, 128
  %30 = icmp ne i16 %29, 0
  br i1 %30, label %31, label %32

31:
  br label %24

32:
  %33 = load i16, ptr @"DSPBASE%"
  %34 = add i16 %33, 12
  call void @llrm.ia16.out.i8(i16 %34, i8 6)
  br label %35

35:
  %36 = load i16, ptr @"DSPBASE%"
  %37 = add i16 %36, 12
  %38 = call i8 @llrm.ia16.in.i8(i16 %37)
  %39 = zext i8 %38 to i16
  %40 = and i16 %39, 128
  %41 = icmp ne i16 %40, 0
  br i1 %41, label %42, label %43

42:
  br label %35

43:
  %44 = load i16, ptr @"DSPBASE%"
  %45 = add i16 %44, 12
  call void @llrm.ia16.out.i8(i16 %45, i8 20)
  br label %46

46:
  %47 = load i16, ptr @"DSPBASE%"
  %48 = add i16 %47, 12
  %49 = call i8 @llrm.ia16.in.i8(i16 %48)
  %50 = zext i8 %49 to i16
  %51 = and i16 %50, 128
  %52 = icmp ne i16 %51, 0
  br i1 %52, label %53, label %54

53:
  br label %46

54:
  %55 = load i16, ptr @"DSPBASE%"
  %56 = add i16 %55, 12
  call void @llrm.ia16.out.i8(i16 %56, i8 %8)
  br label %57

57:
  %58 = load i16, ptr @"DSPBASE%"
  %59 = add i16 %58, 12
  %60 = call i8 @llrm.ia16.in.i8(i16 %59)
  %61 = zext i8 %60 to i16
  %62 = and i16 %61, 128
  %63 = icmp ne i16 %62, 0
  br i1 %63, label %64, label %65

64:
  br label %57

65:
  %66 = load i16, ptr @"DSPBASE%"
  %67 = add i16 %66, 12
  call void @llrm.ia16.out.i8(i16 %67, i8 %9)
  ret void
}

define cc1000 void @SENDDSP(ptr %0) addrspace(1) {
b1:
  br label %b2

b2:
  %1 = load i16, ptr @"DSPBASE%", !tbaa !2
  %2 = add i16 %1, 12
  %3 = call i8 @llrm.ia16.in.i8(i16 %2)
  %4 = zext i8 %3 to i16
  %5 = and i16 %4, 128
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  br label %b2

b4:
  %7 = load i16, ptr @"DSPBASE%", !tbaa !2
  %8 = add i16 %7, 12
  %9 = load i16, ptr %0
  %10 = trunc i16 %9 to i8
  call void @llrm.ia16.out.i8(i16 %8, i8 %10)
  ret void
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
  %23 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string14, ptr %22)
  %24 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %23, ptr @$string15)
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
  %45 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string16, ptr %44)
  %46 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %45, ptr @$string17)
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
  %71 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string18, ptr %70)
  %72 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %71, ptr @$string19)
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

declare cc1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$DSG0() addrspace(1)

declare cc1000 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$TIMR() addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc1000 void @llrm.qb.B$N1I2(i16, i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$RND0() addrspace(1)

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$CIRC(float, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$GPUT(ptr addrspace(1), ptr, i16) addrspace(1)

declare cc1000 i16 @"DMADONE%"(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$COLR(i16, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$LOCT(i16, i16, i16, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare cc1000 ptr @llrm.qb.B$FCMD() addrspace(1)

declare cc1000 i16 @llrm.qb.B$INS2(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$GET3(i16, ptr addrspace(1), i16) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
