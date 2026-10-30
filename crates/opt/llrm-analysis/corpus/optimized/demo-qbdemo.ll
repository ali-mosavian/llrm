target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"TOTALFRAMECOUNT%" = internal global [2 x i8] zeroinitializer
@"FRACTAL1%" = internal global [18 x i8] zeroinitializer
@"FRACTAL2%" = internal global [18 x i8] zeroinitializer
@"BOBSPRITE%" = internal global [22 x i8] zeroinitializer
@$arguments$17 = internal global [36 x i8] zeroinitializer
@"TI1!" = internal global [4 x i8] zeroinitializer
@b$seg = external global [2 x i8]
@$string8 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string8, i16 4) }>
@$string9 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string9, i16 4), [10 x i8] c"canada.bsv" }>
@$string10 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string10, i16 4) }>
@$string13 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string13, i16 4) }>
@$string14 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string14, i16 4) }>
@$string20 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string20, i16 4), [2 x i8] c"V\00" }>
@$string21 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string21, i16 4), [4 x i8] c".BIN" }>
@$string22 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string22, i16 4), [2 x i8] c"P\00" }>
@$string23 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string23, i16 4), [4 x i8] c".BIN" }>
@$string24 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string24, i16 4), [2 x i8] c"T\00" }>
@$string25 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string25, i16 4), [4 x i8] c".BIN" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL1%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL2%")
  br label %b5

b5:
  %0 = phi i16 [ 0, %b1 ], [ %1, %b5 ]
  %tmp9 = trunc i16 %0 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp9) memory(inaccessiblemem: readwrite)
  %tmp8 = trunc i16 %0 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp8) memory(inaccessiblemem: readwrite)
  %tmp7 = trunc i16 %0 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp7) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %1 = add nuw nsw i16 %0, 1
  %exitcond5 = icmp eq i16 %1, 64
  br i1 %exitcond5, label %b6, label %b5

b6:
  br label %b10

b10:
  %2 = phi i16 [ %4, %b10 ], [ -64, %b6 ]
  %3 = add i16 %2, 128
  %tmp6 = trunc i16 %3 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp6) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  %tmp5 = trunc i16 %3 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp5) memory(inaccessiblemem: readwrite)
  %4 = add nsw i16 %2, 1
  %exitcond4 = icmp eq i16 %4, 0
  br i1 %exitcond4, label %b11, label %b10

b11:
  br label %b15

b15:
  %5 = phi i16 [ %7, %b15 ], [ -64, %b11 ]
  %6 = add i16 %5, 192
  %tmp4 = trunc i16 %6 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp4) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %tmp3 = trunc i16 %6 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp3) memory(inaccessiblemem: readwrite)
  %7 = add nsw i16 %5, 1
  %exitcond3 = icmp eq i16 %7, 0
  br i1 %exitcond3, label %b16, label %b15

b16:
  br label %b20

b20:
  %8 = phi i16 [ %10, %b20 ], [ -65, %b16 ]
  %9 = add i16 %8, 256
  %tmp2 = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp2) memory(inaccessiblemem: readwrite)
  %tmp1 = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp1) memory(inaccessiblemem: readwrite)
  %tmp = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  %10 = add nsw i16 %8, 1
  %exitcond2 = icmp eq i16 %10, 0
  br i1 %exitcond2, label %b21, label %b20

b21:
  tail call void @llrm.ia16.out.i8(i16 968, i8 -1) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 968, i8 127) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  store i16 0, ptr @$arguments$17, align 16, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr @$arguments$17)
  %11 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %12 = load float, ptr %11
  store float %12, ptr @"TI1!", !tbaa !2
  store i16 150, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 2), !tbaa !2
  tail call cc 1000 addrspace(1) void @FRACTALEFFECT(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 2))
  store i16 1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 4), align 4, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 4))
  %13 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %14 = load float, ptr %13
  %15 = load float, ptr @"TI1!", !tbaa !2
  %16 = fsub float %14, %15
  %17 = fpround float %16 to float
  store float %17, ptr @"TI1!", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL1%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL2%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 16, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 6), !tbaa !2
  tail call cc 1000 addrspace(1) void @UNWHITEFADE(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 6))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32, i16 0, i16 32, i16 2, i16 258, ptr @"BOBSPRITE%")
  br label %b25

b25:
  %18 = phi i16 [ %41, %b31 ], [ 0, %b21 ]
  %19 = phi i16 [ 0, %b21 ], [ %40, %b31 ]
  %20 = sub nsw i16 16, %19
  %21 = sitofp i16 %20 to float
  %22 = fpround float %21 to float
  %23 = fmul float %22, %22
  br label %b30

b26:
  store i16 1024, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 22), !tbaa !2
  tail call cc 1000 addrspace(1) void @SHADEBOBEFFECT(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 22))
  store i16 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 24), align 8, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 24))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"BOBSPRITE%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 16, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 26), !tbaa !2
  tail call cc 1000 addrspace(1) void @UNWHITEFADE(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 26))
  store i16 160, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 28), align 4, !tbaa !2
  tail call cc 1000 addrspace(1) void @PLASMA(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 28))
  store i16 3, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 30), !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 30))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 96, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 32), align 16, !tbaa !2
  tail call cc 1000 addrspace(1) void @OHCANADA(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 32))
  store i16 4, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 34), !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 34))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b30:
  %24 = phi i16 [ %39, %b30 ], [ %18, %b25 ]
  %25 = phi i16 [ %38, %b30 ], [ 16, %b25 ]
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"BOBSPRITE%", i16 2), !tbaa !2
  %27 = inttoptr i16 %26 to ptr addrspace(2)
  %28 = addrspacecast ptr addrspace(2) %27 to ptr addrspace(1)
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 %24
  store i16 %20, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 8), align 8, !tbaa !2
  store float %22, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 10), !tbaa !2
  store i16 %25, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 14), !tbaa !2
  %30 = sitofp i16 %25 to float
  %31 = fpround float %30 to float
  store float %31, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 16), align 16, !tbaa !2
  %32 = fmul float %31, %31
  %33 = fadd float %23, %32
  %34 = tail call float @llvm.sqrt.f32(float %33)
  store i16 16, ptr getelementptr inbounds nuw (i8, ptr @$arguments$17, i16 20), align 4, !tbaa !2
  %35 = fsub float 1.600000e+01, %34
  %36 = tail call i16 @llvm.lrint.i16.f32(float %35)
  %37 = tail call i16 @llvm.smax.i16(i16 %36, i16 0)
  store i16 %37, ptr addrspace(1) %29
  %38 = add nsw i16 %25, -1
  %39 = add nuw nsw i16 %24, 66
  %exitcond = icmp eq i16 %38, -17
  br i1 %exitcond, label %b31, label %b30

b31:
  %40 = add nuw nsw i16 %19, 1
  %exitcond1 = icmp eq i16 %40, 33
  %41 = add nuw nsw i16 %18, 2
  br i1 %exitcond1, label %b26, label %b25
}

define cc 1000 void @DRAWBOB(ptr dereferenceable(2) nocapture %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  br label %b5

b5:
  %2 = phi i16 [ %26, %b11 ], [ 0, %b1 ]
  %3 = phi i16 [ %1, %b1 ], [ %23, %b11 ]
  %4 = phi i16 [ 0, %b1 ], [ %25, %b11 ]
  %5 = add i16 %3, 288
  store i16 %5, ptr %0
  br label %b10

b6:
  ret void

b10:
  %6 = phi i16 [ %24, %b10 ], [ -64, %b5 ]
  %7 = phi i16 [ %5, %b5 ], [ %23, %b10 ]
  %8 = load i16, ptr @b$seg, !tbaa !2
  %9 = inttoptr i16 %8 to ptr addrspace(2)
  %10 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 %7
  %12 = load volatile i8, ptr addrspace(1) %11
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"BOBSPRITE%", i16 2), !tbaa !2
  %14 = inttoptr i16 %13 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr addrspace(1) %15, i16 %2
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 %6
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 64
  %19 = load i16, ptr addrspace(1) %18, !tbaa !6
  %20 = trunc i16 %19 to i8
  %21 = add i8 %12, %20
  store volatile i8 %21, ptr addrspace(1) %11
  %22 = load i16, ptr %0
  %23 = add i16 %22, 1
  store i16 %23, ptr %0
  %24 = add nsw i16 %6, 2
  %exitcond = icmp eq i16 %24, 0
  br i1 %exitcond, label %b11, label %b10

b11:
  %25 = add nuw nsw i16 %4, 1
  %exitcond1 = icmp eq i16 %25, 32
  %26 = add nuw nsw i16 %2, 66
  br i1 %exitcond1, label %b6, label %b5
}

define cc 1000 void @FRACLINE(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(8) nocapture readonly %1, ptr dereferenceable(8) nocapture readonly %2, ptr dereferenceable(8) nocapture readonly %3, ptr dereferenceable(8) nocapture readonly %4, ptr dereferenceable(8) nocapture readonly %5) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree nosync norecurse {
b1:
  %6 = load double, ptr %4
  %7 = load double, ptr %3
  %8 = fsub double %6, %7
  %9 = fdiv double %8, 1.600000e+02
  %10 = fpround double %9 to double
  %11 = load double, ptr %2
  %12 = load double, ptr %1
  %13 = fsub double %11, %12
  %14 = fdiv double %13, 1.600000e+02
  %15 = fpround double %14 to double
  %16 = load i16, ptr %0
  %17 = mul i16 %16, 320
  %18 = fmul double %10, 5.000000e-01
  %19 = fadd double %7, %18
  br label %b3

b3:
  %20 = phi double [ %19, %b1 ], [ %70, %b13 ]
  %21 = phi i16 [ 1, %b1 ], [ %69, %b13 ]
  %22 = phi double [ %12, %b1 ], [ %72, %b13 ]
  %23 = fpround double %20 to double
  %24 = load i16, ptr @b$seg, !tbaa !2
  %25 = inttoptr i16 %24 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 %17
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 %21
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 -1
  %30 = load volatile i8, ptr addrspace(1) %29
  %31 = getelementptr i8, ptr addrspace(1) %28, i16 1
  %32 = load volatile i8, ptr addrspace(1) %31
  %33 = icmp eq i8 %30, %32
  %34 = icmp ne i8 %30, 0
  %35 = and i1 %34, %33
  br i1 %35, label %b7, label %b6

b6:
  %36 = load double, ptr %5
  br label %b9

b7:
  %37 = phi i8 [ %tmp1, %b10 ], [ %30, %b3 ]
  store volatile i8 %37, ptr addrspace(1) %28
  %38 = icmp ugt i16 %21, 317
  br i1 %38, label %b11, label %b13

b9:
  %39 = phi i16 [ 0, %b6 ], [ %53, %b9 ]
  %40 = phi double [ 0.000000e+00, %b6 ], [ %52, %b9 ]
  %41 = phi double [ 0.000000e+00, %b6 ], [ %50, %b9 ]
  %42 = fmul double %40, %40
  %43 = fmul double %41, %41
  %44 = fsub double %42, %43
  %45 = fpround double %44 to double
  %46 = fmul double %40, %41
  %47 = fpround double %46 to double
  %48 = fadd double %47, %47
  %49 = fadd double %22, %48
  %50 = fpround double %49 to double
  %51 = fadd double %23, %45
  %52 = fpround double %51 to double
  %53 = add i16 %39, 1
  %54 = fmul double %52, %52
  %55 = fmul double %50, %50
  %56 = fadd double %54, %55
  %57 = fcmp oge double %56, %36
  %58 = sext i1 %57 to i16
  %59 = icmp eq i16 %39, 254
  %60 = sext i1 %59 to i16
  %61 = or i16 %58, %60
  %62 = icmp eq i16 %61, 0
  br i1 %62, label %b9, label %b10

b10:
  %tmp1 = trunc i16 %53 to i8
  br label %b7

b11:
  %63 = load double, ptr %3
  %64 = load double, ptr %4
  %65 = fadd double %63, %64
  %66 = fmul double %65, 5.000000e-01
  %67 = fpround double %66 to double
  %68 = load double, ptr %5
  br label %b16

b13:
  %69 = add nuw nsw i16 %21, 2
  %70 = fadd double %10, %23
  %71 = fadd double %22, %15
  %72 = fpround double %71 to double
  br label %b3

b16:
  %73 = phi i16 [ 0, %b11 ], [ %87, %b16 ]
  %74 = phi double [ 0.000000e+00, %b11 ], [ %86, %b16 ]
  %75 = phi double [ 0.000000e+00, %b11 ], [ %84, %b16 ]
  %76 = fmul double %74, %74
  %77 = fmul double %75, %75
  %78 = fsub double %76, %77
  %79 = fpround double %78 to double
  %80 = fmul double %74, %75
  %81 = fpround double %80 to double
  %82 = fadd double %81, %81
  %83 = fadd double %22, %82
  %84 = fpround double %83 to double
  %85 = fadd double %67, %79
  %86 = fpround double %85 to double
  %87 = add i16 %73, 1
  %88 = fmul double %86, %86
  %89 = fmul double %84, %84
  %90 = fadd double %88, %89
  %91 = fcmp oge double %90, %68
  %92 = sext i1 %91 to i16
  %93 = icmp eq i16 %73, 254
  %94 = sext i1 %93 to i16
  %95 = or i16 %92, %94
  %96 = icmp eq i16 %95, 0
  br i1 %96, label %b16, label %b17

b17:
  %97 = load i16, ptr @b$seg, !tbaa !2
  %98 = inttoptr i16 %97 to ptr addrspace(2)
  %99 = addrspacecast ptr addrspace(2) %98 to ptr addrspace(1)
  %100 = getelementptr i8, ptr addrspace(1) %99, i16 %17
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 160
  %tmp = trunc i16 %87 to i8
  store volatile i8 %tmp, ptr addrspace(1) %101
  ret void
}

define cc 1000 void @FRACLINE2(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(8) nocapture readonly %1, ptr dereferenceable(8) nocapture readonly %2, ptr dereferenceable(8) nocapture readonly %3, ptr dereferenceable(8) nocapture readonly %4, ptr dereferenceable(8) nocapture readonly %5) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %6 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr align 2 %6, i8 0, i16 18, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr nonnull %6)
  %7 = load double, ptr %4
  %8 = load double, ptr %3
  %9 = fsub double %7, %8
  %10 = fdiv double %9, 3.200000e+02
  %11 = fpround double %10 to double
  %12 = load double, ptr %2
  %13 = load double, ptr %1
  %14 = fsub double %12, %13
  %15 = fdiv double %14, 3.200000e+02
  %16 = fpround double %15 to double
  %17 = load i16, ptr %0
  %18 = mul i16 %17, 320
  br label %b3

b3:
  %19 = phi double [ %8, %b1 ], [ %62, %b13 ]
  %20 = phi i16 [ 0, %b1 ], [ %60, %b13 ]
  %21 = phi double [ %13, %b1 ], [ %64, %b13 ]
  %22 = load i16, ptr @b$seg, !tbaa !2
  %23 = inttoptr i16 %22 to ptr addrspace(2)
  %24 = addrspacecast ptr addrspace(2) %23 to ptr addrspace(1)
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 %20
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 %18
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 -320
  %28 = load volatile i8, ptr addrspace(1) %27
  %29 = getelementptr i8, ptr addrspace(1) %26, i16 320
  %30 = load volatile i8, ptr addrspace(1) %29
  %31 = icmp eq i8 %28, %30
  br i1 %31, label %b5, label %b6

b5:
  %32 = getelementptr i8, ptr addrspace(1) %26, i16 -320
  %33 = load volatile i8, ptr addrspace(1) %32
  br label %b7

b6:
  %34 = load double, ptr %5
  br label %b9

b7:
  %35 = phi i8 [ %33, %b5 ], [ %tmp, %b10 ]
  store volatile i8 %35, ptr addrspace(1) %26
  %exitcond = icmp eq i16 %20, 319
  br i1 %exitcond, label %b11, label %b13

b9:
  %36 = phi i16 [ 0, %b6 ], [ %50, %b9 ]
  %37 = phi double [ 0.000000e+00, %b6 ], [ %49, %b9 ]
  %38 = phi double [ 0.000000e+00, %b6 ], [ %47, %b9 ]
  %39 = fmul double %37, %37
  %40 = fmul double %38, %38
  %41 = fsub double %39, %40
  %42 = fpround double %41 to double
  %43 = fmul double %37, %38
  %44 = fpround double %43 to double
  %45 = fadd double %44, %44
  %46 = fadd double %21, %45
  %47 = fpround double %46 to double
  %48 = fadd double %19, %42
  %49 = fpround double %48 to double
  %50 = add i16 %36, 1
  %51 = fmul double %49, %49
  %52 = fmul double %47, %47
  %53 = fadd double %51, %52
  %54 = fcmp oge double %53, %34
  %55 = sext i1 %54 to i16
  %56 = icmp eq i16 %36, 254
  %57 = sext i1 %56 to i16
  %58 = or i16 %55, %57
  %59 = icmp eq i16 %58, 0
  br i1 %59, label %b9, label %b10

b10:
  %tmp = trunc i16 %50 to i8
  br label %b7

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %6)
  ret void

b13:
  %60 = add nuw nsw i16 %20, 1
  %61 = fadd double %19, %11
  %62 = fpround double %61 to double
  %63 = fadd double %21, %16
  %64 = fpround double %63 to double
  br label %b3
}

define cc 1000 void @FRACTALEFFECT(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca double
  %2 = alloca double
  %3 = alloca double
  %4 = alloca double
  %5 = alloca double
  %6 = alloca double
  %7 = alloca double
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca [22 x i8]
  call void @llvm.memset.p0.i16(ptr align 2 %16, i8 0, i16 22, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr nonnull %16)
  %17 = load i16, ptr %0
  %18 = icmp slt i16 %17, 1
  br i1 %18, label %common.ret, label %19

19:
  br label %b5

b5:
  %20 = phi i16 [ %66, %b37 ], [ -100, %19 ]
  %21 = phi double [ %65, %b37 ], [ 1.000000e+00, %19 ]
  %22 = phi i16 [ %139, %b37 ], [ 1, %19 ]
  %23 = add i16 %20, 1
  %24 = add i16 %20, 2
  %25 = add i16 %20, 3
  %26 = sitofp i16 %20 to double
  %27 = fdiv double %26, %21
  %28 = fadd double %27, 0xBFE4322522DA415E
  %29 = fpround double %28 to double
  store double %29, ptr %1, !tbaa !2
  %30 = sitofp i16 %23 to double
  %31 = fdiv double %30, %21
  %32 = fadd double %31, 0xBFE4322522DA415E
  %33 = fpround double %32 to double
  store double %33, ptr %2, !tbaa !2
  %34 = sitofp i16 %24 to double
  %35 = fdiv double %34, %21
  %36 = fadd double %35, 0xBFE4322522DA415E
  %37 = fpround double %36 to double
  store double %37, ptr %3, !tbaa !2
  %38 = sitofp i16 %25 to double
  %39 = fdiv double %38, %21
  %40 = fadd double %39, 0xBFE4322522DA415E
  %41 = fpround double %40 to double
  store double %41, ptr %4, !tbaa !2
  %42 = fdiv double 1.600000e+02, %21
  %43 = fsub double 0xBFE27D77FB73608E, %42
  %44 = fpround double %43 to double
  store double %44, ptr %5, !tbaa !2
  %45 = fadd double %42, 0xBFE27D77FB73608E
  %46 = fpround double %45 to double
  store double %46, ptr %6, !tbaa !2
  %47 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FRACTAL1%", i16 2), !tbaa !2
  %48 = inttoptr i16 %47 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = addrspacecast ptr addrspace(1) %49 to ptr addrspace(2)
  %51 = ptrtoint ptr addrspace(2) %50 to i16
  store i16 %51, ptr @b$seg, !tbaa !2
  store double 4.000000e+00, ptr %7, !tbaa !2
  %52 = add i16 %20, 100
  store i16 %52, ptr %8, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE(ptr nonnull %8, ptr nonnull %1, ptr nonnull %1, ptr nonnull %5, ptr nonnull %6, ptr nonnull %7)
  %53 = add i16 %20, 102
  store i16 %53, ptr %9, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE(ptr nonnull %9, ptr nonnull %3, ptr nonnull %3, ptr nonnull %5, ptr nonnull %6, ptr nonnull %7)
  %54 = add i16 %20, 101
  store i16 %54, ptr %10, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE2(ptr nonnull %10, ptr nonnull %2, ptr nonnull %2, ptr nonnull %5, ptr nonnull %6, ptr nonnull %7)
  %55 = add i16 %20, 103
  store i16 %55, ptr %11, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE2(ptr nonnull %11, ptr nonnull %4, ptr nonnull %4, ptr nonnull %5, ptr nonnull %6, ptr nonnull %7)
  %56 = add i16 %20, 4
  %57 = icmp sgt i16 %56, 99
  br i1 %57, label %b7, label %b9

common.ret:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %16)
  ret void

b7:
  %58 = fmul double %21, 2.000000e+00
  %59 = fpround double %58 to double
  %60 = getelementptr i8, ptr %16, i16 2
  %61 = load i16, ptr %60, !tbaa !2
  %62 = inttoptr i16 %61 to ptr addrspace(2)
  %63 = addrspacecast ptr addrspace(2) %62 to ptr addrspace(1)
  %64 = load i16, ptr @b$seg, !tbaa !2
  br label %b13

b9:
  %65 = phi double [ %21, %b5 ], [ %59, %b34 ]
  %66 = phi i16 [ %56, %b5 ], [ -100, %b34 ]
  %67 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FRACTAL2%", i16 2), !tbaa !2
  %68 = inttoptr i16 %67 to ptr addrspace(2)
  %69 = addrspacecast ptr addrspace(2) %68 to ptr addrspace(1)
  %70 = addrspacecast ptr addrspace(1) %69 to ptr addrspace(2)
  %71 = ptrtoint ptr addrspace(2) %70 to i16
  store i16 %71, ptr @b$seg, !tbaa !2
  %72 = urem i16 %22, 50
  %73 = uitofp nneg i16 %72 to float
  %74 = fmul float %73, 0x3FF99999A0000000
  %75 = call i16 @llvm.lrint.i16.f32(float %74)
  store i16 %75, ptr %12, !tbaa !2
  store i16 %72, ptr %13, !tbaa !2
  %76 = fsub float 3.200000e+02, %74
  %77 = call i16 @llvm.lrint.i16.f32(float %76)
  store i16 %77, ptr %14, !tbaa !2
  %78 = sub nuw nsw i16 200, %72
  store i16 %78, ptr %15, !tbaa !2
  call cc 1000 addrspace(1) void @RENDER(ptr nonnull %12, ptr nonnull %13, ptr nonnull %14, ptr nonnull %15)
  %79 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %80 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %79, ptr @$string8)
  %81 = icmp sgt i16 %80, 0
  br i1 %81, label %common.ret, label %b37

b13:
  %82 = phi ptr addrspace(1) [ %101, %b19 ], [ %63, %b7 ]
  %83 = phi i16 [ 16080, %b7 ], [ %98, %b19 ]
  %84 = phi i16 [ 0, %b7 ], [ %100, %b19 ]
  %85 = add i16 %83, 160
  br label %b18

b14:
  %86 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FRACTAL2%", i16 2), !tbaa !2
  %87 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FRACTAL1%", i16 2), !tbaa !2
  br label %b23

b18:
  %88 = phi i16 [ %99, %b18 ], [ -320, %b13 ]
  %89 = phi i16 [ %98, %b18 ], [ %85, %b13 ]
  %90 = inttoptr i16 %64 to ptr addrspace(2)
  %91 = addrspacecast ptr addrspace(2) %90 to ptr addrspace(1)
  %92 = getelementptr i8, ptr addrspace(1) %82, i16 %88
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 320
  %94 = getelementptr i8, ptr addrspace(1) %91, i16 %89
  %95 = getelementptr i8, ptr addrspace(1) %94, i16 -160
  %96 = load volatile i8, ptr addrspace(1) %95
  %97 = zext i8 %96 to i16
  store i16 %97, ptr addrspace(1) %93, !tbaa !8
  %98 = add i16 %89, 1
  %99 = add nsw i16 %88, 2
  %exitcond3 = icmp eq i16 %99, 0
  br i1 %exitcond3, label %b19, label %b18

b19:
  %100 = add nuw nsw i16 %84, 1
  %exitcond4 = icmp eq i16 %100, 100
  %101 = getelementptr i8, ptr addrspace(1) %82, i16 322
  br i1 %exitcond4, label %b14, label %b13

b23:
  %102 = phi i16 [ %112, %b23 ], [ 0, %b14 ]
  %103 = phi i16 [ %111, %b23 ], [ 32001, %b14 ]
  %104 = inttoptr i16 %87 to ptr addrspace(2)
  %105 = inttoptr i16 %86 to ptr addrspace(2)
  %106 = addrspacecast ptr addrspace(2) %104 to ptr addrspace(1)
  %107 = addrspacecast ptr addrspace(2) %105 to ptr addrspace(1)
  %108 = getelementptr i8, ptr addrspace(1) %107, i16 %102
  %109 = getelementptr i8, ptr addrspace(1) %106, i16 %102
  %110 = load i16, ptr addrspace(1) %109, !tbaa !10
  store i16 %110, ptr addrspace(1) %108, !tbaa !12
  store i16 0, ptr addrspace(1) %109, !tbaa !10
  %111 = add nsw i16 %103, -1
  %112 = add nuw i16 %102, 2
  %exitcond2 = icmp eq i16 %111, 0
  br i1 %exitcond2, label %113, label %b23

113:
  br label %b28

b28:
  %114 = phi i16 [ 0, %113 ], [ %136, %b34 ]
  %115 = phi i16 [ 0, %113 ], [ %135, %b34 ]
  %116 = phi i16 [ %134, %b34 ], [ 0, %113 ]
  br label %b33

b33:
  %117 = phi i16 [ %133, %b33 ], [ -320, %b28 ]
  %118 = getelementptr i8, ptr %16, i16 2
  %119 = load i16, ptr %118, !tbaa !2
  %120 = inttoptr i16 %119 to ptr addrspace(2)
  %121 = addrspacecast ptr addrspace(2) %120 to ptr addrspace(1)
  %122 = getelementptr i8, ptr addrspace(1) %121, i16 %115
  %123 = getelementptr i8, ptr addrspace(1) %122, i16 %117
  %124 = getelementptr i8, ptr addrspace(1) %123, i16 320
  %125 = load i16, ptr addrspace(1) %124, !tbaa !8
  %126 = trunc i16 %125 to i8
  %127 = load i16, ptr @b$seg, !tbaa !2
  %128 = inttoptr i16 %127 to ptr addrspace(2)
  %129 = addrspacecast ptr addrspace(2) %128 to ptr addrspace(1)
  %130 = getelementptr i8, ptr addrspace(1) %129, i16 %114
  %131 = getelementptr i8, ptr addrspace(1) %130, i16 %117
  %132 = getelementptr i8, ptr addrspace(1) %131, i16 320
  store volatile i8 %126, ptr addrspace(1) %132
  %133 = add nsw i16 %117, 2
  %exitcond = icmp eq i16 %133, 0
  br i1 %exitcond, label %b34, label %b33

b34:
  %134 = add nuw nsw i16 %116, 1
  %exitcond1 = icmp eq i16 %134, 100
  %135 = add nuw nsw i16 %115, 322
  %136 = add nuw i16 %114, 640
  br i1 %exitcond1, label %b9, label %b28

b37:
  %137 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %138 = add i16 %137, 1
  store i16 %138, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %139 = add nuw i16 %22, 1
  %140 = icmp eq i16 %22, %17
  br i1 %140, label %common.ret, label %b5
}

define cc 1000 void @OHCANADA(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca i16
  br label %b5

b5:
  %2 = phi i16 [ 0, %b1 ], [ %3, %b5 ]
  %tmp1 = trunc i16 %2 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp1) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  %3 = add nuw nsw i16 %2, 1
  %exitcond2 = icmp eq i16 %3, 256
  br i1 %exitcond2, label %b6, label %b5

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string9, i16 0, i16 1)
  br label %b10

b10:
  %4 = phi i16 [ 0, %b6 ], [ %7, %b10 ]
  %tmp = trunc i16 %4 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  %5 = lshr i16 %4, 2
  %6 = trunc nuw nsw i16 %5 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %6) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %6) memory(inaccessiblemem: readwrite)
  %7 = add nuw nsw i16 %4, 1
  %exitcond1 = icmp eq i16 %7, 256
  br i1 %exitcond1, label %b11, label %b10

b11:
  store i16 256, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @UNWHITEFADE(ptr nonnull %1)
  %8 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %9 = add i16 %8, 16
  store i16 %9, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %10 = load i16, ptr %0
  %11 = icmp slt i16 %10, 1
  br i1 %11, label %common.ret, label %12

12:
  %13 = add i16 %10, -1
  br label %b15

b15:
  %14 = phi i16 [ %13, %12 ], [ %21, %b15 ]
  %15 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %17 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %18 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %17, ptr @$string10)
  %19 = icmp sgt i16 %18, 0
  %20 = icmp eq i16 %14, 0
  %or.cond = select i1 %19, i1 true, i1 %20
  %21 = add i16 %14, -1
  br i1 %or.cond, label %common.ret, label %b15

common.ret:
  ret void
}

define cc 1000 void @PLASMA(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca i16
  %2 = alloca [76 x i8]
  %3 = getelementptr inbounds nuw i8, ptr %2, i16 18
  %4 = getelementptr inbounds nuw i8, ptr %2, i16 36
  %5 = getelementptr inbounds nuw i8, ptr %2, i16 38
  %6 = getelementptr inbounds nuw i8, ptr %2, i16 54
  call void @llvm.memset.p0.i16(ptr align 2 %2, i8 0, i16 76, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr nonnull %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 512, i16 2, i16 257, ptr nonnull %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 128, i16 0, i16 128, i16 2, i16 258, ptr nonnull %6)
  store i16 -24576, ptr @b$seg, !tbaa !2
  %7 = load i16, ptr %5, !tbaa !2
  br label %b5

b5:
  %8 = phi i16 [ %28, %b8 ], [ -513, %b1 ]
  %9 = phi i16 [ %27, %b8 ], [ -1026, %b1 ]
  %10 = add i16 %8, 513
  %11 = uitofp nneg i16 %10 to float
  %12 = fmul float %11, 0x40091EB860000000
  %13 = fmul float %12, 3.906250e-03
  %14 = call float @llvm.fabs.f32(float %13)
  %15 = fcmp ogt float %14, 0x41E0000000000000
  br i1 %15, label %b7, label %b8

b6:
  store i16 1, ptr %1, !tbaa !2
  %16 = load i16, ptr %0
  %17 = icmp slt i16 %16, 1
  br i1 %17, label %common.ret, label %18

18:
  br label %b12

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %19 = inttoptr i16 %7 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %9
  %22 = getelementptr i8, ptr addrspace(1) %21, i16 1026
  %23 = call float @llvm.sin.f32(float %13)
  %24 = fmul float %23, 3.200000e+01
  %25 = fadd float %24, 3.200000e+01
  %26 = call i16 @llvm.lrint.i16.f32(float %25)
  store i16 %26, ptr addrspace(1) %22, !tbaa !14
  %27 = add nsw i16 %9, 2
  %28 = add nsw i16 %8, 1
  %exitcond7 = icmp eq i16 %27, 0
  br i1 %exitcond7, label %b6, label %b5

b12:
  %29 = phi i16 [ 2, %18 ], [ %218, %b46 ]
  %30 = phi i16 [ 14, %18 ], [ %217, %b46 ]
  %31 = phi i16 [ 20, %18 ], [ %216, %b46 ]
  %32 = phi i16 [ 10, %18 ], [ %215, %b46 ]
  %33 = phi i16 [ 746, %18 ], [ %214, %b46 ]
  %34 = phi i16 [ %212, %b46 ], [ 1, %18 ]
  %35 = getelementptr i8, ptr %2, i16 2
  %36 = load i16, ptr %35, !tbaa !2
  %37 = getelementptr i8, ptr %2, i16 38
  %38 = load i16, ptr %37, !tbaa !2
  br label %b17

common.ret:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %6)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %3)
  ret void

b17:
  %39 = phi i16 [ %55, %b17 ], [ %31, %b12 ]
  %40 = phi i16 [ %54, %b17 ], [ 0, %b12 ]
  %41 = inttoptr i16 %38 to ptr addrspace(2)
  %42 = inttoptr i16 %36 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %44 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %45 = getelementptr i8, ptr addrspace(1) %44, i16 %40
  %46 = add i16 %29, %40
  %47 = and i16 %46, 1022
  %48 = getelementptr i8, ptr addrspace(1) %43, i16 %47
  %49 = load i16, ptr addrspace(1) %48, !tbaa !14
  %50 = and i16 %39, 1022
  %51 = getelementptr i8, ptr addrspace(1) %43, i16 %50
  %52 = load i16, ptr addrspace(1) %51, !tbaa !14
  %53 = add i16 %52, %49
  store i16 %53, ptr addrspace(1) %45, !tbaa !16
  %54 = add nuw nsw i16 %40, 2
  %55 = add i16 %39, 6
  %exitcond5 = icmp eq i16 %54, 642
  br i1 %exitcond5, label %b18, label %b17

b18:
  %56 = mul i16 %34, 5
  %57 = getelementptr i8, ptr %2, i16 38
  %58 = load i16, ptr %57, !tbaa !2
  %59 = mul i16 %34, 11
  %60 = getelementptr i8, ptr %2, i16 56
  %61 = load i16, ptr %60, !tbaa !2
  %62 = inttoptr i16 %61 to ptr addrspace(2)
  %63 = addrspacecast ptr addrspace(2) %62 to ptr addrspace(1)
  %64 = getelementptr i8, ptr %2, i16 2
  %65 = load i16, ptr %64, !tbaa !2
  br label %b22

b22:
  %66 = phi ptr addrspace(1) [ %101, %b28 ], [ %63, %b18 ]
  %67 = phi i16 [ 0, %b18 ], [ %100, %b28 ]
  %68 = inttoptr i16 %58 to ptr addrspace(2)
  %69 = addrspacecast ptr addrspace(2) %68 to ptr addrspace(1)
  %70 = mul nuw nsw i16 %67, 7
  %71 = add i16 %70, %56
  %72 = shl i16 %71, 1
  %73 = and i16 %72, 1022
  %74 = getelementptr i8, ptr addrspace(1) %69, i16 %73
  %75 = load i16, ptr addrspace(1) %74, !tbaa !14
  %76 = mul nuw nsw i16 %67, 14
  %77 = add i16 %76, %59
  %78 = shl i16 %77, 1
  %79 = add i16 %78, 814
  %80 = and i16 %79, 1022
  %81 = getelementptr i8, ptr addrspace(1) %69, i16 %80
  %82 = load i16, ptr addrspace(1) %81, !tbaa !14
  %83 = add i16 %82, %75
  br label %b27

b23:
  %84 = getelementptr i8, ptr %2, i16 2
  %85 = load i16, ptr %84, !tbaa !2
  %86 = getelementptr i8, ptr %2, i16 38
  %87 = load i16, ptr %86, !tbaa !2
  %88 = getelementptr i8, ptr %2, i16 20
  %89 = load i16, ptr %88, !tbaa !2
  br label %b32

b27:
  %90 = phi i16 [ %99, %b27 ], [ -258, %b22 ]
  %91 = inttoptr i16 %65 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = getelementptr i8, ptr addrspace(1) %66, i16 %90
  %94 = getelementptr i8, ptr addrspace(1) %93, i16 258
  %95 = getelementptr i8, ptr addrspace(1) %92, i16 %90
  %96 = getelementptr i8, ptr addrspace(1) %95, i16 258
  %97 = load i16, ptr addrspace(1) %96, !tbaa !16
  %98 = add i16 %83, %97
  store i16 %98, ptr addrspace(1) %94, !tbaa !18
  %99 = add nsw i16 %90, 2
  %exitcond3 = icmp eq i16 %99, 0
  br i1 %exitcond3, label %b28, label %b27

b28:
  %100 = add nuw nsw i16 %67, 1
  %exitcond4 = icmp eq i16 %100, 129
  %101 = getelementptr i8, ptr addrspace(1) %66, i16 258
  br i1 %exitcond4, label %b23, label %b22

b32:
  %102 = phi i16 [ %133, %b32 ], [ %30, %b23 ]
  %103 = phi i16 [ %132, %b32 ], [ %31, %b23 ]
  %104 = phi i16 [ %131, %b32 ], [ %32, %b23 ]
  %105 = phi i16 [ %130, %b32 ], [ %33, %b23 ]
  %106 = phi i16 [ %129, %b32 ], [ 0, %b23 ]
  %107 = inttoptr i16 %89 to ptr addrspace(2)
  %108 = inttoptr i16 %87 to ptr addrspace(2)
  %109 = inttoptr i16 %85 to ptr addrspace(2)
  %110 = addrspacecast ptr addrspace(2) %107 to ptr addrspace(1)
  %111 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %112 = addrspacecast ptr addrspace(2) %109 to ptr addrspace(1)
  %113 = getelementptr i8, ptr addrspace(1) %112, i16 %106
  %114 = and i16 %102, 1022
  %115 = getelementptr i8, ptr addrspace(1) %111, i16 %114
  %116 = load i16, ptr addrspace(1) %115, !tbaa !14
  %117 = and i16 %103, 1022
  %118 = getelementptr i8, ptr addrspace(1) %111, i16 %117
  %119 = load i16, ptr addrspace(1) %118, !tbaa !14
  %120 = add i16 %119, %116
  store i16 %120, ptr addrspace(1) %113, !tbaa !16
  %121 = getelementptr i8, ptr addrspace(1) %110, i16 %106
  %122 = and i16 %104, 1022
  %123 = getelementptr i8, ptr addrspace(1) %111, i16 %122
  %124 = load i16, ptr addrspace(1) %123, !tbaa !14
  %125 = and i16 %105, 1022
  %126 = getelementptr i8, ptr addrspace(1) %111, i16 %125
  %127 = load i16, ptr addrspace(1) %126, !tbaa !14
  %128 = add i16 %127, %124
  store i16 %128, ptr addrspace(1) %121, !tbaa !20
  %129 = add nuw nsw i16 %106, 2
  %130 = add i16 %105, 18
  %131 = add i16 %104, 8
  %132 = add i16 %103, 6
  %133 = add i16 %102, 22
  %exitcond2 = icmp eq i16 %129, 642
  br i1 %exitcond2, label %b33, label %b32

b33:
  %134 = mul i16 %34, 6
  %135 = getelementptr i8, ptr %2, i16 38
  %136 = load i16, ptr %135, !tbaa !2
  %137 = shl i16 %34, 3
  %138 = mul i16 %34, 23
  %139 = getelementptr i8, ptr %2, i16 2
  %140 = load i16, ptr %139, !tbaa !2
  %141 = getelementptr i8, ptr %2, i16 20
  %142 = load i16, ptr %141, !tbaa !2
  %143 = getelementptr i8, ptr %2, i16 56
  %144 = load i16, ptr %143, !tbaa !2
  %145 = load i16, ptr @b$seg, !tbaa !2
  br label %b37

b37:
  %146 = phi i16 [ 0, %b33 ], [ %209, %b43 ]
  %147 = phi i16 [ 0, %b33 ], [ %211, %b43 ]
  %148 = inttoptr i16 %136 to ptr addrspace(2)
  %149 = addrspacecast ptr addrspace(2) %148 to ptr addrspace(1)
  %150 = mul nuw nsw i16 %147, 11
  %151 = add i16 %150, %134
  %152 = shl i16 %151, 1
  %153 = and i16 %152, 1022
  %154 = getelementptr i8, ptr addrspace(1) %149, i16 %153
  %155 = load i16, ptr addrspace(1) %154, !tbaa !14
  %156 = mul nuw nsw i16 %147, 14
  %157 = add i16 %156, %59
  %158 = shl i16 %157, 1
  %159 = add i16 %158, 814
  %160 = and i16 %159, 1022
  %161 = getelementptr i8, ptr addrspace(1) %149, i16 %160
  %162 = load i16, ptr addrspace(1) %161, !tbaa !14
  %163 = add i16 %162, %155
  %164 = mul nuw nsw i16 %147, 18
  %165 = add i16 %164, %137
  %166 = and i16 %165, 1022
  %167 = getelementptr i8, ptr addrspace(1) %149, i16 %166
  %168 = load i16, ptr addrspace(1) %167, !tbaa !14
  %169 = mul nuw nsw i16 %147, 17
  %170 = add i16 %169, %138
  %171 = shl i16 %170, 1
  %172 = add i16 %171, 814
  %173 = and i16 %172, 1022
  %174 = getelementptr i8, ptr addrspace(1) %149, i16 %173
  %175 = load i16, ptr addrspace(1) %174, !tbaa !14
  %176 = add i16 %175, %168
  br label %b42

b38:
  call cc 1000 addrspace(1) void @UPDPALPLASMA(ptr nonnull %1)
  %177 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %178 = add i16 %177, 1
  store i16 %178, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %179 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %180 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %179, ptr @$string13)
  %181 = icmp sgt i16 %180, 0
  br i1 %181, label %common.ret, label %b46

b42:
  %182 = phi i16 [ %210, %b42 ], [ -640, %b37 ]
  %183 = phi i16 [ %146, %b37 ], [ %209, %b42 ]
  %184 = inttoptr i16 %145 to ptr addrspace(2)
  %185 = inttoptr i16 %144 to ptr addrspace(2)
  %186 = inttoptr i16 %142 to ptr addrspace(2)
  %187 = inttoptr i16 %140 to ptr addrspace(2)
  %188 = addrspacecast ptr addrspace(2) %184 to ptr addrspace(1)
  %189 = addrspacecast ptr addrspace(2) %185 to ptr addrspace(1)
  %190 = addrspacecast ptr addrspace(2) %186 to ptr addrspace(1)
  %191 = addrspacecast ptr addrspace(2) %187 to ptr addrspace(1)
  %192 = getelementptr i8, ptr addrspace(1) %191, i16 %182
  %193 = getelementptr i8, ptr addrspace(1) %192, i16 640
  %194 = load i16, ptr addrspace(1) %193, !tbaa !16
  %195 = add i16 %163, %194
  %196 = and i16 %195, 127
  %197 = getelementptr i8, ptr addrspace(1) %190, i16 %182
  %198 = getelementptr i8, ptr addrspace(1) %197, i16 640
  %199 = load i16, ptr addrspace(1) %198, !tbaa !20
  %200 = add i16 %176, %199
  %201 = and i16 %200, 127
  %202 = mul nuw nsw i16 %201, 129
  %203 = add nuw nsw i16 %202, %196
  %204 = shl nuw i16 %203, 1
  %205 = getelementptr i8, ptr addrspace(1) %189, i16 %204
  %206 = load i16, ptr addrspace(1) %205, !tbaa !18
  %207 = trunc i16 %206 to i8
  %208 = getelementptr i8, ptr addrspace(1) %188, i16 %183
  store volatile i8 %207, ptr addrspace(1) %208, !tbaa !4
  %209 = add i16 %183, 1
  %210 = add nsw i16 %182, 2
  %exitcond = icmp eq i16 %210, 0
  br i1 %exitcond, label %b43, label %b42

b43:
  %211 = add nuw nsw i16 %147, 1
  %exitcond1 = icmp eq i16 %211, 200
  br i1 %exitcond1, label %b38, label %b37

b46:
  %212 = add nuw i16 %34, 1
  store i16 %212, ptr %1, !tbaa !2
  %213 = icmp eq i16 %34, %16
  %214 = add i16 %33, 4
  %215 = add i16 %32, 10
  %216 = add i16 %31, 14
  %217 = add i16 %30, 14
  %218 = add i16 %29, 2
  br i1 %213, label %common.ret, label %b12
}

define cc 1000 void @RENDER(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1, ptr dereferenceable(2) nocapture readonly %2, ptr dereferenceable(2) nocapture readonly %3) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %4 = alloca [22 x i8]
  %5 = getelementptr inbounds nuw i8, ptr %4, i16 2
  call void @llvm.memset.p0.i16(ptr align 2 %4, i8 0, i16 22, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr nonnull %4)
  %6 = load i16, ptr %3
  %7 = load i16, ptr %1
  %8 = sub i16 %6, %7
  %9 = load i16, ptr %2
  %10 = load i16, ptr %0
  %11 = sub i16 %9, %10
  %12 = sdiv i16 %8, 100
  %13 = sdiv i16 %11, 160
  %14 = srem i16 %11, 160
  %15 = srem i16 %8, 100
  %16 = load i16, ptr %5, !tbaa !2
  %17 = inttoptr i16 %16 to ptr addrspace(2)
  %18 = addrspacecast ptr addrspace(2) %17 to ptr addrspace(1)
  %19 = load i16, ptr @b$seg, !tbaa !2
  br label %b5

b5:
  %20 = phi ptr addrspace(1) [ %55, %b11 ], [ %18, %b1 ]
  %21 = phi i16 [ 0, %b1 ], [ %51, %b11 ]
  %22 = phi i16 [ %7, %b1 ], [ %53, %b11 ]
  %23 = phi i16 [ 0, %b1 ], [ %54, %b11 ]
  %24 = inttoptr i16 %19 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %26 = load i16, ptr %0
  %27 = mul i16 %22, 320
  %invariant.gep = getelementptr i8, ptr addrspace(1) %25, i16 %27
  br label %b10

b6:
  %28 = getelementptr i8, ptr %4, i16 2
  %29 = load i16, ptr %28, !tbaa !2
  %30 = inttoptr i16 %29 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  br label %b21

b10:
  %32 = phi i16 [ %46, %b10 ], [ -320, %b5 ]
  %33 = phi i16 [ %26, %b5 ], [ %45, %b10 ]
  %34 = phi i16 [ 0, %b5 ], [ %42, %b10 ]
  %35 = getelementptr i8, ptr addrspace(1) %20, i16 %32
  %36 = getelementptr i8, ptr addrspace(1) %35, i16 320
  %gep = getelementptr i8, ptr addrspace(1) %invariant.gep, i16 %33
  %37 = load volatile i8, ptr addrspace(1) %gep
  %38 = zext i8 %37 to i16
  store i16 %38, ptr addrspace(1) %36, !tbaa !22
  %39 = add i16 %34, %14
  %40 = icmp sgt i16 %39, 160
  %41 = add nsw i16 %39, -160
  %42 = select i1 %40, i16 %41, i16 %39
  %43 = zext i1 %40 to i16
  %44 = add i16 %33, %13
  %45 = add i16 %44, %43
  %46 = add nsw i16 %32, 2
  %exitcond2 = icmp eq i16 %46, 0
  br i1 %exitcond2, label %b11, label %b10

b11:
  %47 = add i16 %21, %15
  %48 = icmp sgt i16 %47, 100
  %49 = add nsw i16 %47, -100
  %50 = zext i1 %48 to i16
  %51 = select i1 %48, i16 %49, i16 %47
  %52 = add i16 %22, %12
  %53 = add i16 %52, %50
  %54 = add nuw nsw i16 %23, 1
  %exitcond3 = icmp eq i16 %54, 100
  %55 = getelementptr i8, ptr addrspace(1) %20, i16 322
  br i1 %exitcond3, label %b6, label %b5

b21:
  %56 = phi ptr addrspace(1) [ %75, %b27 ], [ %31, %b6 ]
  %57 = phi i16 [ 0, %b6 ], [ %74, %b27 ]
  %58 = phi i16 [ 16080, %b6 ], [ %73, %b27 ]
  br label %b26

b22:
  %59 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FRACTAL2%", i16 2), !tbaa !2
  %60 = inttoptr i16 %59 to ptr addrspace(2)
  %61 = addrspacecast ptr addrspace(2) %60 to ptr addrspace(1)
  %62 = addrspacecast ptr addrspace(1) %61 to ptr addrspace(2)
  %63 = ptrtoint ptr addrspace(2) %62 to i16
  store i16 %63, ptr @b$seg, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %4)
  ret void

b26:
  %64 = phi ptr addrspace(1) [ %72, %b26 ], [ addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), %b21 ]
  %65 = phi i16 [ %71, %b26 ], [ -320, %b21 ]
  %66 = getelementptr i8, ptr addrspace(1) %56, i16 %65
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 320
  %68 = load i16, ptr addrspace(1) %67, !tbaa !22
  %69 = trunc i16 %68 to i8
  %70 = getelementptr i8, ptr addrspace(1) %64, i16 %58
  store volatile i8 %69, ptr addrspace(1) %70, !tbaa !4
  %71 = add nsw i16 %65, 2
  %72 = getelementptr i8, ptr addrspace(1) %64, i16 1
  %exitcond = icmp eq i16 %71, 0
  br i1 %exitcond, label %b27, label %b26

b27:
  %73 = add nuw i16 %58, 320
  %74 = add nuw nsw i16 %57, 1
  %exitcond1 = icmp eq i16 %74, 100
  %75 = getelementptr i8, ptr addrspace(1) %56, i16 322
  br i1 %exitcond1, label %b22, label %b21
}

define cc 1000 void @SHADEBOBEFFECT(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr align 2 %1, i8 0, i16 18, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr nonnull %1)
  store i16 -24576, ptr @b$seg, !tbaa !2
  %2 = load i16, ptr %0
  %3 = icmp slt i16 %2, 1
  br i1 %3, label %b6, label %4

4:
  br label %5

5:
  %6 = phi i16 [ %127, %b25 ], [ 0, %4 ]
  %7 = phi i16 [ %128, %b25 ], [ 1, %4 ]
  %8 = shl i16 %6, 1
  %9 = getelementptr i8, ptr %1, i16 2
  %10 = load i16, ptr %9, !tbaa !2
  %11 = inttoptr i16 %10 to ptr addrspace(2)
  %12 = addrspacecast ptr addrspace(2) %11 to ptr addrspace(1)
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 %8
  %14 = load i16, ptr addrspace(1) %13, !tbaa !24
  br label %15

15:
  %16 = phi i16 [ %41, %39 ], [ 0, %5 ]
  %17 = phi i16 [ %14, %5 ], [ %37, %39 ]
  %18 = phi i16 [ 0, %5 ], [ %40, %39 ]
  %19 = add i16 %17, 288
  br label %20

20:
  %21 = phi i16 [ %38, %20 ], [ -64, %15 ]
  %22 = phi i16 [ %19, %15 ], [ %37, %20 ]
  %23 = load i16, ptr @b$seg, !tbaa !2
  %24 = inttoptr i16 %23 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 %22
  %27 = load volatile i8, ptr addrspace(1) %26
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"BOBSPRITE%", i16 2), !tbaa !2
  %29 = inttoptr i16 %28 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %16
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %21
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 64
  %34 = load i16, ptr addrspace(1) %33, !tbaa !6
  %35 = trunc i16 %34 to i8
  %36 = sub i8 %27, %35
  store volatile i8 %36, ptr addrspace(1) %26
  %37 = add i16 %22, 1
  %38 = add nsw i16 %21, 2
  %exitcond2 = icmp eq i16 %38, 0
  br i1 %exitcond2, label %39, label %20

39:
  %40 = add nuw nsw i16 %18, 1
  %exitcond11 = icmp eq i16 %40, 32
  %41 = add nuw nsw i16 %16, 66
  br i1 %exitcond11, label %42, label %15

42:
  %43 = sitofp i16 %7 to float
  %44 = fdiv float %43, 7.100000e+01
  %45 = call float @llvm.fabs.f32(float %44)
  %46 = fcmp ogt float %45, 0x41E0000000000000
  br i1 %46, label %b7, label %b8

common.ret:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %1)
  ret void

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %1)
  br label %common.ret

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %47 = fdiv float %43, 4.700000e+01
  %48 = fadd float %47, 2.000000e+00
  %49 = call float @llvm.fabs.f32(float %48)
  %50 = fcmp ogt float %49, 0x41E0000000000000
  br i1 %50, label %b9, label %b10

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b10:
  %51 = fdiv float %43, 9.100000e+01
  %52 = fadd float %51, 7.000000e+00
  %53 = call float @llvm.fabs.f32(float %52)
  %54 = fcmp ogt float %53, 0x41E0000000000000
  br i1 %54, label %b11, label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %55 = call float @llvm.sin.f32(float %44)
  %56 = call float @llvm.cos.f32(float %48)
  %57 = fadd float %55, %56
  %58 = call float @llvm.cos.f32(float %52)
  %59 = fadd float %58, %57
  %60 = fmul float %59, 4.800000e+01
  %61 = fadd float %60, 1.600000e+02
  %62 = call i16 @llvm.lrint.i16.f32(float %61)
  %63 = fdiv float %43, 4.900000e+01
  %64 = fadd float %63, 3.000000e+00
  %65 = call float @llvm.fabs.f32(float %64)
  %66 = fcmp ogt float %65, 0x41E0000000000000
  br i1 %66, label %b13, label %b14

b13:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b14:
  %67 = fdiv float %43, 4.100000e+01
  %68 = fadd float %67, 2.000000e+00
  %69 = call float @llvm.fabs.f32(float %68)
  %70 = fcmp ogt float %69, 0x41E0000000000000
  br i1 %70, label %b15, label %b16

b15:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b16:
  %71 = fdiv float %43, 9.700000e+01
  %72 = fadd float %71, 7.000000e+00
  %73 = call float @llvm.fabs.f32(float %72)
  %74 = fcmp ogt float %73, 0x41E0000000000000
  br i1 %74, label %b17, label %75

b17:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

75:
  %76 = call float @llvm.cos.f32(float %64)
  %77 = call float @llvm.sin.f32(float %68)
  %78 = fadd float %76, %77
  %79 = call float @llvm.sin.f32(float %72)
  %80 = fadd float %79, %78
  %81 = fmul float %80, 2.800000e+01
  %82 = fadd float %81, 1.000000e+02
  %83 = call i16 @llvm.lrint.i16.f32(float %82)
  %84 = getelementptr i8, ptr %1, i16 2
  %85 = load i16, ptr %84, !tbaa !2
  %86 = inttoptr i16 %85 to ptr addrspace(2)
  %87 = addrspacecast ptr addrspace(2) %86 to ptr addrspace(1)
  %88 = getelementptr i8, ptr addrspace(1) %87, i16 %8
  %89 = mul i16 %83, 320
  %90 = add i16 %89, %62
  store i16 %90, ptr addrspace(1) %88, !tbaa !24
  br label %91

91:
  %92 = phi i16 [ %117, %115 ], [ 0, %75 ]
  %93 = phi i16 [ %90, %75 ], [ %113, %115 ]
  %94 = phi i16 [ 0, %75 ], [ %116, %115 ]
  %95 = add i16 %93, 288
  br label %96

96:
  %97 = phi i16 [ %114, %96 ], [ -64, %91 ]
  %98 = phi i16 [ %95, %91 ], [ %113, %96 ]
  %99 = load i16, ptr @b$seg, !tbaa !2
  %100 = inttoptr i16 %99 to ptr addrspace(2)
  %101 = addrspacecast ptr addrspace(2) %100 to ptr addrspace(1)
  %102 = getelementptr i8, ptr addrspace(1) %101, i16 %98
  %103 = load volatile i8, ptr addrspace(1) %102
  %104 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"BOBSPRITE%", i16 2), !tbaa !2
  %105 = inttoptr i16 %104 to ptr addrspace(2)
  %106 = addrspacecast ptr addrspace(2) %105 to ptr addrspace(1)
  %107 = getelementptr i8, ptr addrspace(1) %106, i16 %92
  %108 = getelementptr i8, ptr addrspace(1) %107, i16 %97
  %109 = getelementptr i8, ptr addrspace(1) %108, i16 64
  %110 = load i16, ptr addrspace(1) %109, !tbaa !6
  %111 = trunc i16 %110 to i8
  %112 = add i8 %103, %111
  store volatile i8 %112, ptr addrspace(1) %102
  %113 = add i16 %98, 1
  %114 = add nsw i16 %97, 2
  %exitcond = icmp eq i16 %114, 0
  br i1 %exitcond, label %115, label %96

115:
  %116 = add nuw nsw i16 %94, 1
  %exitcond1 = icmp eq i16 %116, 32
  %117 = add nuw nsw i16 %92, 66
  br i1 %exitcond1, label %118, label %91

118:
  %119 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %120 = add i16 %119, 1
  store i16 %120, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %121 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %122 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %121, ptr @$string14)
  %123 = icmp sgt i16 %122, 0
  br i1 %123, label %common.ret, label %b25

b25:
  %124 = lshr i16 %7, 1
  %125 = add nuw nsw i16 %124, 1
  %126 = add i16 %6, 1
  %127 = srem i16 %126, %125
  %128 = add nuw i16 %7, 1
  %129 = icmp eq i16 %7, %2
  br i1 %129, label %b6, label %5
}

define cc 1000 void @UNDRAWBOB(ptr dereferenceable(2) nocapture %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  br label %b5

b5:
  %2 = phi i16 [ %26, %b11 ], [ 0, %b1 ]
  %3 = phi i16 [ %1, %b1 ], [ %23, %b11 ]
  %4 = phi i16 [ 0, %b1 ], [ %25, %b11 ]
  %5 = add i16 %3, 288
  store i16 %5, ptr %0
  br label %b10

b6:
  ret void

b10:
  %6 = phi i16 [ %24, %b10 ], [ -64, %b5 ]
  %7 = phi i16 [ %5, %b5 ], [ %23, %b10 ]
  %8 = load i16, ptr @b$seg, !tbaa !2
  %9 = inttoptr i16 %8 to ptr addrspace(2)
  %10 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 %7
  %12 = load volatile i8, ptr addrspace(1) %11
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"BOBSPRITE%", i16 2), !tbaa !2
  %14 = inttoptr i16 %13 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr addrspace(1) %15, i16 %2
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 %6
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 64
  %19 = load i16, ptr addrspace(1) %18, !tbaa !6
  %20 = trunc i16 %19 to i8
  %21 = sub i8 %12, %20
  store volatile i8 %21, ptr addrspace(1) %11
  %22 = load i16, ptr %0
  %23 = add i16 %22, 1
  store i16 %23, ptr %0
  %24 = add nsw i16 %6, 2
  %exitcond = icmp eq i16 %24, 0
  br i1 %exitcond, label %b11, label %b10

b11:
  %25 = add nuw nsw i16 %4, 1
  %exitcond1 = icmp eq i16 %25, 32
  %26 = add nuw nsw i16 %2, 66
  br i1 %exitcond1, label %b6, label %b5
}

define cc 1000 void @UNWHITEFADE(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca [22 x i8]
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 2
  call void @llvm.memset.p0.i16(ptr align 2 %1, i8 0, i16 22, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr nonnull %1)
  %3 = load i16, ptr %2, !tbaa !2
  %4 = inttoptr i16 %3 to ptr addrspace(2)
  br label %b5

b5:
  %5 = phi i8 [ %21, %b5 ], [ 0, %b1 ]
  %6 = phi i16 [ %20, %b5 ], [ -512, %b1 ]
  %7 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)
  call void @llrm.ia16.out.i8(i16 967, i8 %5) memory(inaccessiblemem: readwrite)
  %8 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %9 = zext i8 %8 to i16
  %10 = getelementptr i8, ptr addrspace(1) %7, i16 %6
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 512
  store i16 %9, ptr addrspace(1) %11, !tbaa !26
  %12 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %13 = zext i8 %12 to i16
  %14 = getelementptr i8, ptr addrspace(1) %7, i16 %6
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 1026
  store i16 %13, ptr addrspace(1) %15, !tbaa !26
  %16 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %17 = zext i8 %16 to i16
  %18 = getelementptr i8, ptr addrspace(1) %7, i16 %6
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 1540
  store i16 %17, ptr addrspace(1) %19, !tbaa !26
  %20 = add nsw i16 %6, 2
  %21 = add i8 %5, 1
  %exitcond2 = icmp eq i16 %20, 0
  br i1 %exitcond2, label %b6, label %b5

b6:
  %22 = load i16, ptr %0
  %23 = icmp slt i16 %22, 0
  br i1 %23, label %b11, label %24

24:
  %promoted = load i16, ptr @"TOTALFRAMECOUNT%", align 1
  %25 = uitofp nneg i16 %22 to float
  %26 = getelementptr i8, ptr %1, i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = inttoptr i16 %27 to ptr addrspace(2)
  br label %b10

b10:
  %29 = phi i16 [ 0, %24 ], [ %66, %b16 ]
  %30 = phi i16 [ %promoted, %24 ], [ %65, %b16 ]
  %31 = sitofp i16 %29 to float
  %32 = fdiv float %31, %25
  %33 = fpround float %32 to float
  %34 = fsub float 1.000000e+00, %33
  %35 = fmul float %34, 6.300000e+01
  br label %b15

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %1)
  ret void

b15:
  %36 = phi i8 [ %64, %b15 ], [ 0, %b10 ]
  %37 = phi i16 [ %63, %b15 ], [ -512, %b10 ]
  %38 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  call void @llrm.ia16.out.i8(i16 968, i8 %36) memory(inaccessiblemem: readwrite)
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 %37
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 512
  %41 = load i16, ptr addrspace(1) %40, !tbaa !26
  %42 = sitofp i16 %41 to float
  %43 = fmul float %33, %42
  %44 = fadd float %35, %43
  %45 = call i16 @llvm.lrint.i16.f32(float %44)
  %46 = trunc i16 %45 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %46) memory(inaccessiblemem: readwrite)
  %47 = getelementptr i8, ptr addrspace(1) %38, i16 %37
  %48 = getelementptr i8, ptr addrspace(1) %47, i16 1026
  %49 = load i16, ptr addrspace(1) %48, !tbaa !26
  %50 = sitofp i16 %49 to float
  %51 = fmul float %33, %50
  %52 = fadd float %35, %51
  %53 = call i16 @llvm.lrint.i16.f32(float %52)
  %54 = trunc i16 %53 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %54) memory(inaccessiblemem: readwrite)
  %55 = getelementptr i8, ptr addrspace(1) %38, i16 %37
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 1540
  %57 = load i16, ptr addrspace(1) %56, !tbaa !26
  %58 = sitofp i16 %57 to float
  %59 = fmul float %33, %58
  %60 = fadd float %35, %59
  %61 = call i16 @llvm.lrint.i16.f32(float %60)
  %62 = trunc i16 %61 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %62) memory(inaccessiblemem: readwrite)
  %63 = add nsw i16 %37, 2
  %64 = add i8 %36, 1
  %exitcond = icmp eq i16 %63, 0
  br i1 %exitcond, label %b16, label %b15

b16:
  %65 = add i16 %30, 1
  store i16 %65, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %66 = add nuw i16 %29, 1
  %67 = icmp eq i16 %29, %22
  br i1 %67, label %b11, label %b10
}

define cc 1000 void @UPDPALPLASMA(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = load i16, ptr %0
  %2 = sitofp i16 %1 to float
  %3 = fmul float %2, 0x3F5719F800000000
  %4 = fpext float %3 to double
  %5 = fmul float %2, 0x3F8CE07600000000
  %6 = fpext float %5 to double
  %7 = fmul float %2, 0x3F8BDA5120000000
  %8 = fpext float %7 to double
  br label %b5

b5:
  %9 = phi i16 [ 0, %b1 ], [ %38, %b12 ]
  %tmp = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp) memory(inaccessiblemem: readwrite)
  %10 = uitofp nneg i16 %9 to double
  %11 = fmul double %10, 0x400921FB54442D18
  %12 = fmul double %11, 7.812500e-03
  %13 = fadd double %12, %4
  %14 = tail call double @llvm.fabs.f64(double %13)
  %15 = fcmp ogt double %14, 0x41E0000000000000
  br i1 %15, label %b7, label %b8

b6:
  ret void

b7:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %16 = tail call double @llvm.cos.f64(double %13)
  %17 = fmul double %16, 3.100000e+01
  %18 = fsub double 3.200000e+01, %17
  %19 = tail call i16 @llvm.lrint.i16.f64(double %18)
  %20 = trunc i16 %19 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %20) memory(inaccessiblemem: readwrite)
  %21 = fadd double %12, %6
  %22 = tail call double @llvm.fabs.f64(double %21)
  %23 = fcmp ogt double %22, 0x41E0000000000000
  br i1 %23, label %b9, label %b10

b9:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b10:
  %24 = tail call double @llvm.cos.f64(double %21)
  %25 = fmul double %24, 3.100000e+01
  %26 = fsub double 3.200000e+01, %25
  %27 = tail call i16 @llvm.lrint.i16.f64(double %26)
  %28 = trunc i16 %27 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %28) memory(inaccessiblemem: readwrite)
  %29 = fmul double %11, 1.562500e-02
  %30 = fadd double %29, %8
  %31 = tail call double @llvm.fabs.f64(double %30)
  %32 = fcmp ogt double %31, 0x41E0000000000000
  br i1 %32, label %b11, label %b12

b11:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %33 = tail call double @llvm.cos.f64(double %30)
  %34 = fmul double %33, 3.100000e+01
  %35 = fsub double 3.200000e+01, %34
  %36 = tail call i16 @llvm.lrint.i16.f64(double %35)
  %37 = trunc i16 %36 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %37) memory(inaccessiblemem: readwrite)
  %38 = add nuw nsw i16 %9, 1
  %exitcond = icmp eq i16 %38, 256
  br i1 %exitcond, label %b6, label %b5
}

define cc 1000 void @WHITEFADE(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca [22 x i8]
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 2
  call void @llvm.memset.p0.i16(ptr align 2 %1, i8 0, i16 22, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr nonnull %1)
  %3 = load i16, ptr %2, !tbaa !2
  %4 = inttoptr i16 %3 to ptr addrspace(2)
  br label %b5

b5:
  %5 = phi i8 [ %21, %b5 ], [ 0, %b1 ]
  %6 = phi i16 [ %20, %b5 ], [ -512, %b1 ]
  %7 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)
  call void @llrm.ia16.out.i8(i16 967, i8 %5) memory(inaccessiblemem: readwrite)
  %8 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %9 = zext i8 %8 to i16
  %10 = getelementptr i8, ptr addrspace(1) %7, i16 %6
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 512
  store i16 %9, ptr addrspace(1) %11, !tbaa !28
  %12 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %13 = zext i8 %12 to i16
  %14 = getelementptr i8, ptr addrspace(1) %7, i16 %6
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 1026
  store i16 %13, ptr addrspace(1) %15, !tbaa !28
  %16 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %17 = zext i8 %16 to i16
  %18 = getelementptr i8, ptr addrspace(1) %7, i16 %6
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 1540
  store i16 %17, ptr addrspace(1) %19, !tbaa !28
  %20 = add nsw i16 %6, 2
  %21 = add i8 %5, 1
  %exitcond1 = icmp eq i16 %20, 0
  br i1 %exitcond1, label %b6, label %b5

b6:
  %22 = load i16, ptr %0
  %23 = icmp sgt i16 %22, -1
  br i1 %23, label %24, label %b11

24:
  %promoted = load i16, ptr @"TOTALFRAMECOUNT%", align 1
  %25 = uitofp nneg i16 %22 to float
  %26 = getelementptr i8, ptr %1, i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = inttoptr i16 %27 to ptr addrspace(2)
  br label %b10

b10:
  %29 = phi i16 [ %22, %24 ], [ %66, %b16 ]
  %30 = phi i16 [ %promoted, %24 ], [ %65, %b16 ]
  %31 = uitofp nneg i16 %29 to float
  %32 = fdiv float %31, %25
  %33 = fpround float %32 to float
  %34 = fsub float 1.000000e+00, %33
  %35 = fmul float %34, 6.300000e+01
  br label %b15

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %1)
  ret void

b15:
  %36 = phi i8 [ %64, %b15 ], [ 0, %b10 ]
  %37 = phi i16 [ %63, %b15 ], [ -512, %b10 ]
  %38 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  call void @llrm.ia16.out.i8(i16 968, i8 %36) memory(inaccessiblemem: readwrite)
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 %37
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 512
  %41 = load i16, ptr addrspace(1) %40, !tbaa !28
  %42 = sitofp i16 %41 to float
  %43 = fmul float %33, %42
  %44 = fadd float %35, %43
  %45 = call i16 @llvm.lrint.i16.f32(float %44)
  %46 = trunc i16 %45 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %46) memory(inaccessiblemem: readwrite)
  %47 = getelementptr i8, ptr addrspace(1) %38, i16 %37
  %48 = getelementptr i8, ptr addrspace(1) %47, i16 1026
  %49 = load i16, ptr addrspace(1) %48, !tbaa !28
  %50 = sitofp i16 %49 to float
  %51 = fmul float %33, %50
  %52 = fadd float %35, %51
  %53 = call i16 @llvm.lrint.i16.f32(float %52)
  %54 = trunc i16 %53 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %54) memory(inaccessiblemem: readwrite)
  %55 = getelementptr i8, ptr addrspace(1) %38, i16 %37
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 1540
  %57 = load i16, ptr addrspace(1) %56, !tbaa !28
  %58 = sitofp i16 %57 to float
  %59 = fmul float %33, %58
  %60 = fadd float %35, %59
  %61 = call i16 @llvm.lrint.i16.f32(float %60)
  %62 = trunc i16 %61 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %62) memory(inaccessiblemem: readwrite)
  %63 = add nsw i16 %37, 2
  %64 = add i8 %36, 1
  %exitcond = icmp eq i16 %63, 0
  br i1 %exitcond, label %b16, label %b15

b16:
  %65 = add i16 %30, 1
  store i16 %65, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %66 = add nsw i16 %29, -1
  %67 = icmp eq i16 %29, 0
  br i1 %67, label %b11, label %b10
}

define cc 1000 void @BENCHMARK(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca [36 x i8]
  %4 = getelementptr inbounds nuw i8, ptr %3, i16 18
  call void @llvm.memset.p0.i16(ptr align 2 %3, i8 0, i16 36, i1 false)
  store i32 0, ptr %1, !tbaa !2
  store i32 0, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 767, i16 2, i16 257, ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 5, i16 4, i16 257, ptr nonnull %4)
  call cc 1000 addrspace(1) void @TSCSNAP(ptr nonnull %1, ptr nonnull %2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  br label %b5

b5:
  %5 = phi ptr addrspace(1) [ %16, %b5 ], [ addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), %b1 ]
  %6 = phi i32 [ %15, %b5 ], [ 64000, %b1 ]
  %7 = phi i32 [ 1, %b1 ], [ %12, %b5 ]
  %8 = phi i32 [ 0, %b1 ], [ %14, %b5 ]
  %9 = load volatile i8, ptr addrspace(1) %5
  %10 = zext i8 %9 to i32
  %11 = add i32 %7, %10
  %12 = srem i32 %11, 65521
  %13 = add i32 %12, %8
  %14 = srem i32 %13, 65521
  %15 = add nsw i32 %6, -1
  %16 = getelementptr i8, ptr addrspace(1) %5, i16 1
  %exitcond1 = icmp eq i32 %15, 0
  br i1 %exitcond1, label %b6, label %b5

b6:
  %17 = load i16, ptr %0
  %18 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %17)
  %19 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %18)
  %20 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string20, ptr %19)
  %21 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %20, ptr @$string21)
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %21, i16 0, i16 -1536)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %22 = getelementptr i8, ptr %3, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  br label %b10

b10:
  %24 = phi i16 [ %31, %b10 ], [ -1536, %b6 ]
  %25 = inttoptr i16 %23 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 %24
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 1536
  %29 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %30 = zext i8 %29 to i16
  store i16 %30, ptr addrspace(1) %28, !tbaa !30
  %31 = add nsw i16 %24, 2
  %exitcond = icmp eq i16 %31, 0
  br i1 %exitcond, label %b11, label %b10

b11:
  %32 = getelementptr inbounds nuw i8, ptr %3, i16 20
  %33 = getelementptr i8, ptr %3, i16 2
  %34 = load i16, ptr %33, !tbaa !2
  %35 = inttoptr i16 %34 to ptr addrspace(2)
  %36 = addrspacecast ptr addrspace(2) %35 to ptr addrspace(1)
  %37 = addrspacecast ptr addrspace(1) %36 to ptr addrspace(2)
  %38 = ptrtoint ptr addrspace(2) %37 to i16
  store i16 %38, ptr @b$seg, !tbaa !2
  %39 = load i16, ptr %0
  %40 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %39)
  %41 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %40)
  %42 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string22, ptr %41)
  %43 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %42, ptr @$string23)
  %44 = load i16, ptr %33, !tbaa !2
  %45 = inttoptr i16 %44 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = ptrtoint ptr addrspace(1) %46 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %43, i16 %47, i16 1536)
  %48 = load i16, ptr %32, !tbaa !2
  %49 = inttoptr i16 %48 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = load i32, ptr @"BENCHHI&", !tbaa !2
  store i32 %51, ptr addrspace(1) %50, !tbaa !32
  %52 = getelementptr i8, ptr addrspace(1) %50, i16 4
  %53 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %53, ptr addrspace(1) %52, !tbaa !32
  %54 = getelementptr i8, ptr addrspace(1) %50, i16 8
  %55 = load i32, ptr %1, !tbaa !2
  store i32 %55, ptr addrspace(1) %54, !tbaa !32
  %56 = getelementptr i8, ptr addrspace(1) %50, i16 12
  %57 = load i32, ptr %2, !tbaa !2
  store i32 %57, ptr addrspace(1) %56, !tbaa !32
  %58 = getelementptr i8, ptr addrspace(1) %50, i16 16
  store i32 %14, ptr addrspace(1) %58, !tbaa !32
  %59 = getelementptr i8, ptr addrspace(1) %50, i16 20
  store i32 %12, ptr addrspace(1) %59, !tbaa !32
  %60 = addrspacecast ptr addrspace(1) %50 to ptr addrspace(2)
  %61 = ptrtoint ptr addrspace(2) %60 to i16
  store i16 %61, ptr @b$seg, !tbaa !2
  %62 = load i16, ptr %0
  %63 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %62)
  %64 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %63)
  %65 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string24, ptr %64)
  %66 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %65, ptr @$string25)
  %67 = load i16, ptr %32, !tbaa !2
  %68 = inttoptr i16 %67 to ptr addrspace(2)
  %69 = addrspacecast ptr addrspace(2) %68 to ptr addrspace(1)
  %70 = ptrtoint ptr addrspace(1) %69 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %66, i16 %70, i16 24)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc 1000 addrspace(1) void @TSCSNAP(ptr @"BENCHHI&", ptr @"BENCHLO&")
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %3)
  ret void
}

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$TIMR() addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$SCLS(i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare float @llvm.sqrt.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 ptr @llrm.qb.B$INKY() addrspace(1) nocallback

declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1) threeway nocallback nounwind flagsresult

declare cc 1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1) nocallback

declare float @llvm.fabs.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.cos.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare double @llvm.fabs.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.cos.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1) nocallback nounwind

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
!9 = !{!"allocation.2", !3, i64 0}
!10 = !{!9, !9, i64 0}
!11 = !{!"allocation.3", !3, i64 0}
!12 = !{!11, !11, i64 0}
!13 = !{!"allocation.4", !3, i64 0}
!14 = !{!13, !13, i64 0}
!15 = !{!"allocation.5", !3, i64 0}
!16 = !{!15, !15, i64 0}
!17 = !{!"allocation.6", !3, i64 0}
!18 = !{!17, !17, i64 0}
!19 = !{!"allocation.7", !3, i64 0}
!20 = !{!19, !19, i64 0}
!21 = !{!"allocation.8", !3, i64 0}
!22 = !{!21, !21, i64 0}
!23 = !{!"allocation.9", !3, i64 0}
!24 = !{!23, !23, i64 0}
!25 = !{!"allocation.10", !3, i64 0}
!26 = !{!25, !25, i64 0}
!27 = !{!"allocation.11", !3, i64 0}
!28 = !{!27, !27, i64 0}
!29 = !{!"allocation.12", !3, i64 0}
!30 = !{!29, !29, i64 0}
!31 = !{!"allocation.13", !3, i64 0}
!32 = !{!31, !31, i64 0}
