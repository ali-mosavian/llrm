target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f64_3ff0000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\F0?"
@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"
@$f64_3dd25868f4deae16 = internal constant [8 x i8] c"\16\AE\DE\F4hX\D2="
@$f64_46391ace3d05aef4 = internal constant [8 x i8] c"\F4\AE\05=\CE\1A9F"
@$f64_400921fb54442d18 = internal constant [8 x i8] c"\18-DT\FB!\09@"
@$f64_40f5180000000000 = internal constant [8 x i8] c"\00\00\00\00\00\18\F5@"
@$str7 = internal constant [11 x i8] c"\08\00\04\00\04\00: g \00"
@$str8 = internal constant [20 x i8] c"\08\00\0D\00\0D\00 m/s2, orbit \00"
@$str9 = internal constant [18 x i8] c"\08\00\0B\00\0B\00 m/s, year \00"
@$str10 = internal constant [12 x i8] c"\08\00\05\00\05\00 days\00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00Mercury\00"
@$f64_44d179b07057cf97 = internal constant [8 x i8] c"\97\CFWp\B0y\D1D"
@$f64_41429d0a00000000 = internal constant [8 x i8] c"\00\00\00\00\0A\9DBA"
@$f64_422af768f3000000 = internal constant [8 x i8] c"\00\00\00\F3h\F7*B"
@$str15 = internal constant [12 x i8] c"\08\00\05\00\05\00Earth\00"
@$f64_4513c27b13272fb6 = internal constant [8 x i8] c"\B6/'\13{\C2\13E"
@$f64_41584dae00000000 = internal constant [8 x i8] c"\00\00\00\00\AEMXA"
@$f64_42416a6d6c000000 = internal constant [8 x i8] c"\00\00\00lmjAB"
@$str19 = internal constant [14 x i8] c"\08\00\07\00\07\00Jupiter\00"
@$f64_459887f488aced67 = internal constant [8 x i8] c"g\ED\AC\88\F4\87\98E"
@$f64_4190ab0760000000 = internal constant [8 x i8] c"\00\00\00`\07\AB\90A"
@$f64_4266a846e9200000 = internal constant [8 x i8] c"\00\00 \E9F\A8fB"
@$str23 = internal constant [9 x i8] c"\08\00\02\00\02\00G \00"
@$str24 = internal constant [13 x i8] c"\08\00\06\00\06\00, sun \00"
@$str25 = internal constant [24 x i8] c"\08\00\11\00\11\00 kg, one part in \00"
@$f32_41ac0000 = internal constant [4 x i8] c"\00\00\ACA"
@$f32_41ae0000 = internal constant [4 x i8] c"\00\00\AEA"
@$f32_404ccccd = internal constant [4 x i8] c"\CD\CCL@"
@$f32_3dcccccd = internal constant [4 x i8] c"\CD\CC\CC="
@$f32_00000000 = internal constant [4 x i8] zeroinitializer
@$str31 = internal constant [24 x i8] c"\08\00\11\00\11\00thermometer mean \00"
@$f32_40800000 = internal constant [4 x i8] c"\00\00\80@"
@$str33 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 C, first \00"
@$str34 = internal constant [14 x i8] c"\08\00\07\00\07\00, zero \00"

define internal double @root(double %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca double
  %4 = alloca double
  store i16 0, ptr %1
  store i16 0, ptr %2
  store double 0.000000e+00, ptr %3
  store double 0.000000e+00, ptr %4
  %5 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  %6 = fcmp ogt double %0, %5
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b2, label %b3

b2:
  store double %0, ptr %4, !tbaa !2
  br label %b4

b3:
  %9 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  store double %9, ptr %4, !tbaa !2
  br label %b4

b4:
  %10 = load double, ptr %4, !tbaa !2
  store double %10, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 80, ptr %1, !tbaa !2
  br label %b5

b5:
  %11 = load i16, ptr %2, !tbaa !2
  %12 = load i16, ptr %1, !tbaa !2
  %13 = icmp slt i16 %11, %12
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b6, label %b8

b6:
  %16 = load double, ptr %3, !tbaa !2
  %17 = load double, ptr %3, !tbaa !2
  %18 = fdiv double %0, %17
  %19 = fadd double %16, %18
  %20 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %21 = fdiv double %19, %20
  store double %21, ptr %3, !tbaa !2
  br label %b7

b7:
  %22 = load i16, ptr %2, !tbaa !2
  %23 = add i16 %22, 1
  store i16 %23, ptr %2, !tbaa !2
  br label %b5

b8:
  %24 = load double, ptr %3, !tbaa !2
  ret double %24
}

define internal void @report(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca double
  %2 = alloca double
  %3 = alloca double
  %4 = alloca double
  store double 0.000000e+00, ptr %1
  store double 0.000000e+00, ptr %2
  store double 0.000000e+00, ptr %3
  store double 0.000000e+00, ptr %4
  %5 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load double, ptr addrspace(1) %6
  %8 = fmul double %5, %7
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %10 = load double, ptr addrspace(1) %9
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %12 = load double, ptr addrspace(1) %11
  %13 = fmul double %10, %12
  %14 = fdiv double %8, %13
  store double %14, ptr %4, !tbaa !2
  %15 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  %16 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  %17 = fmul double %15, %16
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 18
  %19 = load double, ptr addrspace(1) %18
  %20 = fdiv double %17, %19
  store double %20, ptr %3, !tbaa !2
  %21 = load double, ptr %3, !tbaa !2
  %22 = call addrspace(1) double @root(double %21)
  store double %22, ptr %2, !tbaa !2
  %23 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %24 = load double, ptr @$f64_400921fb54442d18, !tbaa !2
  %25 = fmul double %23, %24
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 18
  %27 = load double, ptr addrspace(1) %26
  %28 = fmul double %25, %27
  %29 = load double, ptr %2, !tbaa !2
  %30 = fdiv double %28, %29
  %31 = load double, ptr @$f64_40f5180000000000, !tbaa !2
  %32 = fdiv double %30, %31
  store double %32, ptr %1, !tbaa !2
  %33 = load ptr, ptr addrspace(1) %0
  call addrspace(1) void @N$PS(ptr %33)
  %34 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %34)
  %35 = load double, ptr %4, !tbaa !2
  call addrspace(1) void @N$PR8(double %35)
  %36 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %36)
  %37 = load double, ptr %2, !tbaa !2
  call addrspace(1) void @N$PR8(double %37)
  %38 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %38)
  %39 = load double, ptr %1, !tbaa !2
  call addrspace(1) void @N$PR8(double %39)
  %40 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %40)
  call addrspace(1) void @N$PN()
  ret void
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca float
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [16 x i8]
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [78 x i8]
  store i16 0, ptr %0
  store float 0.000000e+00, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 16, i1 false)
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 78, i1 false)
  store i16 3, ptr %6, !tbaa !2
  store i16 3, ptr %7, !tbaa !2
  %9 = getelementptr i8, ptr @$str11, i16 6
  %10 = load double, ptr @$f64_44d179b07057cf97, !tbaa !2
  %11 = load double, ptr @$f64_41429d0a00000000, !tbaa !2
  %12 = load double, ptr @$f64_422af768f3000000, !tbaa !2
  %13 = sub i16 0, 0
  %14 = getelementptr inbounds [26 x i8], ptr %8, i16 %13
  store ptr %9, ptr %14, !tbaa !2
  %15 = sub i16 0, 0
  %16 = getelementptr inbounds [26 x i8], ptr %8, i16 %15
  %17 = getelementptr inbounds i8, ptr %16, i16 2
  store double %10, ptr %17, !tbaa !2
  %18 = sub i16 0, 0
  %19 = getelementptr inbounds [26 x i8], ptr %8, i16 %18
  %20 = getelementptr inbounds i8, ptr %19, i16 10
  store double %11, ptr %20, !tbaa !2
  %21 = sub i16 0, 0
  %22 = getelementptr inbounds [26 x i8], ptr %8, i16 %21
  %23 = getelementptr inbounds i8, ptr %22, i16 18
  store double %12, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$str15, i16 6
  %25 = load double, ptr @$f64_4513c27b13272fb6, !tbaa !2
  %26 = load double, ptr @$f64_41584dae00000000, !tbaa !2
  %27 = load double, ptr @$f64_42416a6d6c000000, !tbaa !2
  %28 = sub i16 1, 0
  %29 = getelementptr inbounds [26 x i8], ptr %8, i16 %28
  store ptr %24, ptr %29, !tbaa !2
  %30 = sub i16 1, 0
  %31 = getelementptr inbounds [26 x i8], ptr %8, i16 %30
  %32 = getelementptr inbounds i8, ptr %31, i16 2
  store double %25, ptr %32, !tbaa !2
  %33 = sub i16 1, 0
  %34 = getelementptr inbounds [26 x i8], ptr %8, i16 %33
  %35 = getelementptr inbounds i8, ptr %34, i16 10
  store double %26, ptr %35, !tbaa !2
  %36 = sub i16 1, 0
  %37 = getelementptr inbounds [26 x i8], ptr %8, i16 %36
  %38 = getelementptr inbounds i8, ptr %37, i16 18
  store double %27, ptr %38, !tbaa !2
  %39 = getelementptr i8, ptr @$str19, i16 6
  %40 = load double, ptr @$f64_459887f488aced67, !tbaa !2
  %41 = load double, ptr @$f64_4190ab0760000000, !tbaa !2
  %42 = load double, ptr @$f64_4266a846e9200000, !tbaa !2
  %43 = sub i16 2, 0
  %44 = getelementptr inbounds [26 x i8], ptr %8, i16 %43
  store ptr %39, ptr %44, !tbaa !2
  %45 = sub i16 2, 0
  %46 = getelementptr inbounds [26 x i8], ptr %8, i16 %45
  %47 = getelementptr inbounds i8, ptr %46, i16 2
  store double %40, ptr %47, !tbaa !2
  %48 = sub i16 2, 0
  %49 = getelementptr inbounds [26 x i8], ptr %8, i16 %48
  %50 = getelementptr inbounds i8, ptr %49, i16 10
  store double %41, ptr %50, !tbaa !2
  %51 = sub i16 2, 0
  %52 = getelementptr inbounds [26 x i8], ptr %8, i16 %51
  %53 = getelementptr inbounds i8, ptr %52, i16 18
  store double %42, ptr %53, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  br label %b2

b2:
  %54 = load i16, ptr %5, !tbaa !2
  %55 = icmp ult i16 %54, 3
  %56 = sext i1 %55 to i8
  %57 = icmp ne i8 %56, 0
  br i1 %57, label %b3, label %b5

b3:
  %58 = sub i16 %54, 0
  %59 = getelementptr inbounds [26 x i8], ptr %8, i16 %58
  %60 = addrspacecast ptr %59 to ptr addrspace(1)
  call addrspace(1) void @report(ptr addrspace(1) %60)
  br label %b4

b4:
  %61 = load i16, ptr %5, !tbaa !2
  %62 = add i16 %61, 1
  store i16 %62, ptr %5, !tbaa !2
  br label %b2

b5:
  %63 = getelementptr i8, ptr @$str23, i16 6
  call addrspace(1) void @N$PS(ptr %63)
  %64 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  call addrspace(1) void @N$PR8(double %64)
  %65 = getelementptr i8, ptr @$str24, i16 6
  call addrspace(1) void @N$PS(ptr %65)
  %66 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  call addrspace(1) void @N$PR8(double %66)
  %67 = getelementptr i8, ptr @$str25, i16 6
  call addrspace(1) void @N$PS(ptr %67)
  %68 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  %69 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  %70 = fdiv double %69, %68
  call addrspace(1) void @N$PR8(double %70)
  call addrspace(1) void @N$PN()
  store i16 4, ptr %2, !tbaa !2
  store i16 4, ptr %3, !tbaa !2
  %71 = load float, ptr @$f32_41ac0000, !tbaa !2
  %72 = sub i16 0, 0
  %73 = getelementptr inbounds float, ptr %4, i16 %72
  store float %71, ptr %73, !tbaa !2
  %74 = load float, ptr @$f32_41ae0000, !tbaa !2
  %75 = sub i16 1, 0
  %76 = getelementptr inbounds float, ptr %4, i16 %75
  store float %74, ptr %76, !tbaa !2
  %77 = load float, ptr @$f32_404ccccd, !tbaa !2
  %78 = fneg float %77
  %79 = sub i16 2, 0
  %80 = getelementptr inbounds float, ptr %4, i16 %79
  store float %78, ptr %80, !tbaa !2
  %81 = load float, ptr @$f32_3dcccccd, !tbaa !2
  %82 = sub i16 3, 0
  %83 = getelementptr inbounds float, ptr %4, i16 %82
  store float %81, ptr %83, !tbaa !2
  %84 = load float, ptr @$f32_00000000, !tbaa !2
  store float %84, ptr %1, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  br label %b6

b6:
  %85 = load i16, ptr %0, !tbaa !2
  %86 = icmp ult i16 %85, 4
  %87 = sext i1 %86 to i8
  %88 = icmp ne i8 %87, 0
  br i1 %88, label %b7, label %b9

b7:
  %89 = load float, ptr %1, !tbaa !2
  %90 = sub i16 %85, 0
  %91 = getelementptr inbounds float, ptr %4, i16 %90
  %92 = load float, ptr %91, !tbaa !2
  %93 = fadd float %89, %92
  store float %93, ptr %1, !tbaa !2
  br label %b8

b8:
  %94 = load i16, ptr %0, !tbaa !2
  %95 = add i16 %94, 1
  store i16 %95, ptr %0, !tbaa !2
  br label %b6

b9:
  %96 = getelementptr i8, ptr @$str31, i16 6
  call addrspace(1) void @N$PS(ptr %96)
  %97 = load float, ptr %1, !tbaa !2
  %98 = load float, ptr @$f32_40800000, !tbaa !2
  %99 = fdiv float %97, %98
  call addrspace(1) void @N$PR4(float %99)
  %100 = getelementptr i8, ptr @$str33, i16 6
  call addrspace(1) void @N$PS(ptr %100)
  %101 = sub i16 0, 0
  %102 = getelementptr inbounds float, ptr %4, i16 %101
  %103 = load float, ptr %102, !tbaa !2
  call addrspace(1) void @N$PR4(float %103)
  %104 = getelementptr i8, ptr @$str34, i16 6
  call addrspace(1) void @N$PS(ptr %104)
  %105 = load float, ptr %1, !tbaa !2
  %106 = load float, ptr @$f32_00000000, !tbaa !2
  %107 = fmul float %105, %106
  call addrspace(1) void @N$PR4(float %107)
  call addrspace(1) void @N$PN()
  %108 = sub i16 0, 0
  %109 = getelementptr inbounds [26 x i8], ptr %8, i16 %108
  %110 = load ptr, ptr %109, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %110)
  %111 = sub i16 1, 0
  %112 = getelementptr inbounds [26 x i8], ptr %8, i16 %111
  %113 = load ptr, ptr %112, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %113)
  %114 = sub i16 2, 0
  %115 = getelementptr inbounds [26 x i8], ptr %8, i16 %114
  %116 = load ptr, ptr %115, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %116)
  ret i16 0
}

declare void @N$PS(ptr) addrspace(1)

declare void @N$PR8(double) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PR4(float) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
