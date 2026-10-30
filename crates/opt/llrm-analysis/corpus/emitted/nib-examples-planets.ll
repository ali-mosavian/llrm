target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$f64_3ff0000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\F0?"
@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"
@$f64_408f400000000000 = internal constant [8 x i8] c"\00\00\00\00\00@\8F@"
@$f64_3fe0000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0?"
@$f64_c1e0000000200000 = internal constant [8 x i8] c"\00\00 \00\00\00\E0\C1"
@$f64_41e0000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0A"
@$f64_3dd25868f4deae16 = internal constant [8 x i8] c"\16\AE\DE\F4hX\D2="
@$f64_46391ace3d05aef4 = internal constant [8 x i8] c"\F4\AE\05=\CE\1A9F"
@$f64_400921fb54442d18 = internal constant [8 x i8] c"\18-DT\FB!\09@"
@$f64_40f5180000000000 = internal constant [8 x i8] c"\00\00\00\00\00\18\F5@"
@$str11 = internal constant [11 x i8] c"\08\00\04\00\04\00: g \00"
@$str12 = internal constant [20 x i8] c"\08\00\0D\00\0D\00 m/s2, orbit \00"
@$str13 = internal constant [18 x i8] c"\08\00\0B\00\0B\00 m/s, year \00"
@$str14 = internal constant [12 x i8] c"\08\00\05\00\05\00 days\00"
@$str15 = internal constant [14 x i8] c"\08\00\07\00\07\00Mercury\00"
@$f64_44d179b07057cf97 = internal constant [8 x i8] c"\97\CFWp\B0y\D1D"
@$f64_41429d0a00000000 = internal constant [8 x i8] c"\00\00\00\00\0A\9DBA"
@$f64_422af768f3000000 = internal constant [8 x i8] c"\00\00\00\F3h\F7*B"
@$str19 = internal constant [12 x i8] c"\08\00\05\00\05\00Earth\00"
@$f64_4513c27b13272fb6 = internal constant [8 x i8] c"\B6/'\13{\C2\13E"
@$f64_41584dae00000000 = internal constant [8 x i8] c"\00\00\00\00\AEMXA"
@$f64_42416a6d6c000000 = internal constant [8 x i8] c"\00\00\00lmjAB"
@$str23 = internal constant [14 x i8] c"\08\00\07\00\07\00Jupiter\00"
@$f64_459887f488aced67 = internal constant [8 x i8] c"g\ED\AC\88\F4\87\98E"
@$f64_4190ab0760000000 = internal constant [8 x i8] c"\00\00\00`\07\AB\90A"
@$f64_4266a846e9200000 = internal constant [8 x i8] c"\00\00 \E9F\A8fB"
@$str27 = internal constant [9 x i8] c"\08\00\02\00\02\00G \00"
@$str28 = internal constant [13 x i8] c"\08\00\06\00\06\00, sun \00"
@$str29 = internal constant [24 x i8] c"\08\00\11\00\11\00 kg, one part in \00"
@$f32_41ac0000 = internal constant [4 x i8] c"\00\00\ACA"
@$f32_41ae0000 = internal constant [4 x i8] c"\00\00\AEA"
@$f32_404ccccd = internal constant [4 x i8] c"\CD\CCL@"
@$f32_3dcccccd = internal constant [4 x i8] c"\CD\CC\CC="
@$f32_00000000 = internal constant [4 x i8] zeroinitializer
@$f32_40800000 = internal constant [4 x i8] c"\00\00\80@"
@$f32_42c80000 = internal constant [4 x i8] c"\00\00\C8B"
@$f32_3f000000 = internal constant [4 x i8] c"\00\00\00?"
@$f32_cf000000 = internal constant [4 x i8] c"\00\00\00\CF"
@$f32_4f000000 = internal constant [4 x i8] c"\00\00\00O"
@$str40 = internal constant [24 x i8] c"\08\00\11\00\11\00thermometer mean \00"
@$str41 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 C, first \00"
@$str42 = internal constant [14 x i8] c"\08\00\07\00\07\00, zero \00"

define internal cc 1005 double @root(double %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca double
  %4 = alloca double
  %5 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  %6 = fcmp ogt double %0, %5
  br i1 %6, label %b2, label %b3

b2:
  store double %0, ptr %4, !tbaa !2
  br label %b4

b3:
  %7 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  store double %7, ptr %4, !tbaa !2
  br label %b4

b4:
  %8 = load double, ptr %4, !tbaa !2
  store double %8, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 80, ptr %1, !tbaa !2
  br label %b5

b5:
  %9 = load i16, ptr %2, !tbaa !2
  %10 = load i16, ptr %1, !tbaa !2
  %11 = icmp slt i16 %9, %10
  br i1 %11, label %b6, label %b8

b6:
  %12 = load double, ptr %3, !tbaa !2
  %13 = load double, ptr %3, !tbaa !2
  %14 = fdiv double %0, %13
  %15 = fadd double %12, %14
  %16 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %17 = fdiv double %15, %16
  store double %17, ptr %3, !tbaa !2
  br label %b7

b7:
  %18 = load i16, ptr %2, !tbaa !2
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %2, !tbaa !2
  br label %b5

b8:
  %20 = load double, ptr %3, !tbaa !2
  ret double %20
}

define internal cc 1005 double @tidy(double %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load double, ptr @$f64_408f400000000000, !tbaa !2
  %2 = fmul double %0, %1
  %3 = load double, ptr @$f64_3fe0000000000000, !tbaa !2
  %4 = fadd double %2, %3
  %5 = load double, ptr @$f64_c1e0000000200000, !tbaa !2
  %6 = load double, ptr @$f64_41e0000000000000, !tbaa !2
  %7 = fcmp ogt double %4, %5
  %8 = zext i1 %7 to i8
  %9 = fcmp olt double %4, %6
  %10 = zext i1 %9 to i8
  %11 = and i8 %8, %10
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b2, label %b3

b2:
  %13 = fptosi double %4 to i32
  %14 = sitofp i32 %13 to double
  %15 = load double, ptr @$f64_408f400000000000, !tbaa !2
  %16 = fdiv double %14, %15
  ret double %16

b3:
  call cc 1005 addrspace(1) void @N$ECNV()
  unreachable
}

define internal cc 1005 void @report(ptr addrspace(1) nonnull dereferenceable(26) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca double
  %2 = alloca double
  %3 = alloca double
  %4 = alloca double
  %5 = alloca double
  %6 = alloca double
  %7 = alloca double
  %8 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %10 = load double, ptr addrspace(1) %9
  %11 = fmul double %8, %10
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %13 = load double, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %15 = load double, ptr addrspace(1) %14
  %16 = fmul double %13, %15
  %17 = fdiv double %11, %16
  store double %17, ptr %7, !tbaa !2
  %18 = load double, ptr %7, !tbaa !2
  %19 = call cc 1005 addrspace(1) double @tidy(double %18)
  store double %19, ptr %6, !tbaa !2
  %20 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  %21 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  %22 = fmul double %20, %21
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 18
  %24 = load double, ptr addrspace(1) %23
  %25 = fdiv double %22, %24
  store double %25, ptr %5, !tbaa !2
  %26 = load double, ptr %5, !tbaa !2
  %27 = call cc 1005 addrspace(1) double @root(double %26)
  store double %27, ptr %4, !tbaa !2
  %28 = load double, ptr %4, !tbaa !2
  %29 = call cc 1005 addrspace(1) double @tidy(double %28)
  store double %29, ptr %3, !tbaa !2
  %30 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %31 = load double, ptr @$f64_400921fb54442d18, !tbaa !2
  %32 = fmul double %30, %31
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 18
  %34 = load double, ptr addrspace(1) %33
  %35 = fmul double %32, %34
  %36 = load double, ptr %3, !tbaa !2
  %37 = fdiv double %35, %36
  %38 = load double, ptr @$f64_40f5180000000000, !tbaa !2
  %39 = fdiv double %37, %38
  store double %39, ptr %2, !tbaa !2
  %40 = load double, ptr %2, !tbaa !2
  %41 = call cc 1005 addrspace(1) double @tidy(double %40)
  store double %41, ptr %1, !tbaa !2
  %42 = load ptr, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PS(ptr %42)
  %43 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %43)
  %44 = load double, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PR8(double %44)
  %45 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %45)
  %46 = load double, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PR8(double %46)
  %47 = getelementptr i8, ptr @$str13, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %47)
  %48 = load double, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PR8(double %48)
  %49 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %49)
  call cc 1005 addrspace(1) void @N$PN()
  ret void
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca float
  %1 = alloca i16
  %2 = alloca float
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [16 x i8]
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [78 x i8]
  store i16 3, ptr %7, !tbaa !2
  store i16 3, ptr %8, !tbaa !2
  %10 = getelementptr i8, ptr @$str15, i16 6
  %11 = load double, ptr @$f64_44d179b07057cf97, !tbaa !2
  %12 = load double, ptr @$f64_41429d0a00000000, !tbaa !2
  %13 = load double, ptr @$f64_422af768f3000000, !tbaa !2
  %14 = getelementptr inbounds [26 x i8], ptr %9, i16 0
  store ptr %10, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [26 x i8], ptr %9, i16 0
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store double %11, ptr %16, !tbaa !2
  %17 = getelementptr inbounds [26 x i8], ptr %9, i16 0
  %18 = getelementptr inbounds i8, ptr %17, i16 10
  store double %12, ptr %18, !tbaa !2
  %19 = getelementptr inbounds [26 x i8], ptr %9, i16 0
  %20 = getelementptr inbounds i8, ptr %19, i16 18
  store double %13, ptr %20, !tbaa !2
  %21 = getelementptr i8, ptr @$str19, i16 6
  %22 = load double, ptr @$f64_4513c27b13272fb6, !tbaa !2
  %23 = load double, ptr @$f64_41584dae00000000, !tbaa !2
  %24 = load double, ptr @$f64_42416a6d6c000000, !tbaa !2
  %25 = getelementptr inbounds [26 x i8], ptr %9, i16 1
  store ptr %21, ptr %25, !tbaa !2
  %26 = getelementptr inbounds [26 x i8], ptr %9, i16 1
  %27 = getelementptr inbounds i8, ptr %26, i16 2
  store double %22, ptr %27, !tbaa !2
  %28 = getelementptr inbounds [26 x i8], ptr %9, i16 1
  %29 = getelementptr inbounds i8, ptr %28, i16 10
  store double %23, ptr %29, !tbaa !2
  %30 = getelementptr inbounds [26 x i8], ptr %9, i16 1
  %31 = getelementptr inbounds i8, ptr %30, i16 18
  store double %24, ptr %31, !tbaa !2
  %32 = getelementptr i8, ptr @$str23, i16 6
  %33 = load double, ptr @$f64_459887f488aced67, !tbaa !2
  %34 = load double, ptr @$f64_4190ab0760000000, !tbaa !2
  %35 = load double, ptr @$f64_4266a846e9200000, !tbaa !2
  %36 = getelementptr inbounds [26 x i8], ptr %9, i16 2
  store ptr %32, ptr %36, !tbaa !2
  %37 = getelementptr inbounds [26 x i8], ptr %9, i16 2
  %38 = getelementptr inbounds i8, ptr %37, i16 2
  store double %33, ptr %38, !tbaa !2
  %39 = getelementptr inbounds [26 x i8], ptr %9, i16 2
  %40 = getelementptr inbounds i8, ptr %39, i16 10
  store double %34, ptr %40, !tbaa !2
  %41 = getelementptr inbounds [26 x i8], ptr %9, i16 2
  %42 = getelementptr inbounds i8, ptr %41, i16 18
  store double %35, ptr %42, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b2

b2:
  %43 = load i16, ptr %6, !tbaa !2
  %44 = icmp ult i16 %43, 3
  br i1 %44, label %b3, label %b5

b3:
  %45 = sub i16 %43, 0
  %46 = getelementptr inbounds [26 x i8], ptr %9, i16 %45
  %47 = addrspacecast ptr %46 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @report(ptr addrspace(1) %47)
  br label %b4

b4:
  %48 = load i16, ptr %6, !tbaa !2
  %49 = add i16 %48, 1
  store i16 %49, ptr %6, !tbaa !2
  br label %b2

b5:
  %50 = getelementptr i8, ptr @$str27, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %50)
  %51 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  call cc 1005 addrspace(1) void @N$PR8(double %51)
  %52 = getelementptr i8, ptr @$str28, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %52)
  %53 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PR8(double %53)
  %54 = getelementptr i8, ptr @$str29, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %54)
  %55 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  %56 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  %57 = fdiv double %56, %55
  call cc 1005 addrspace(1) void @N$PR8(double %57)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 4, ptr %3, !tbaa !2
  store i16 4, ptr %4, !tbaa !2
  %58 = load float, ptr @$f32_41ac0000, !tbaa !2
  %59 = getelementptr inbounds float, ptr %5, i16 0
  store float %58, ptr %59, !tbaa !2
  %60 = load float, ptr @$f32_41ae0000, !tbaa !2
  %61 = getelementptr inbounds float, ptr %5, i16 1
  store float %60, ptr %61, !tbaa !2
  %62 = load float, ptr @$f32_404ccccd, !tbaa !2
  %63 = fneg float %62
  %64 = getelementptr inbounds float, ptr %5, i16 2
  store float %63, ptr %64, !tbaa !2
  %65 = load float, ptr @$f32_3dcccccd, !tbaa !2
  %66 = getelementptr inbounds float, ptr %5, i16 3
  store float %65, ptr %66, !tbaa !2
  %67 = load float, ptr @$f32_00000000, !tbaa !2
  store float %67, ptr %2, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  br label %b6

b6:
  %68 = load i16, ptr %1, !tbaa !2
  %69 = icmp ult i16 %68, 4
  br i1 %69, label %b7, label %b9

b7:
  %70 = load float, ptr %2, !tbaa !2
  %71 = sub i16 %68, 0
  %72 = getelementptr inbounds float, ptr %5, i16 %71
  %73 = load float, ptr %72, !tbaa !2
  %74 = fadd float %70, %73
  store float %74, ptr %2, !tbaa !2
  br label %b8

b8:
  %75 = load i16, ptr %1, !tbaa !2
  %76 = add i16 %75, 1
  store i16 %76, ptr %1, !tbaa !2
  br label %b6

b9:
  %77 = load float, ptr %2, !tbaa !2
  %78 = load float, ptr @$f32_40800000, !tbaa !2
  %79 = fdiv float %77, %78
  %80 = load float, ptr @$f32_42c80000, !tbaa !2
  %81 = fmul float %79, %80
  %82 = load float, ptr @$f32_3f000000, !tbaa !2
  %83 = fadd float %81, %82
  %84 = load float, ptr @$f32_cf000000, !tbaa !2
  %85 = load float, ptr @$f32_4f000000, !tbaa !2
  %86 = fcmp oge float %83, %84
  %87 = zext i1 %86 to i8
  %88 = fcmp olt float %83, %85
  %89 = zext i1 %88 to i8
  %90 = and i8 %87, %89
  %91 = icmp ne i8 %90, 0
  br i1 %91, label %b10, label %b11

b10:
  %92 = fptosi float %83 to i32
  %93 = sitofp i32 %92 to float
  %94 = load float, ptr @$f32_42c80000, !tbaa !2
  %95 = fdiv float %93, %94
  store float %95, ptr %0, !tbaa !2
  %96 = getelementptr i8, ptr @$str40, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %96)
  %97 = load float, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PR4(float %97)
  %98 = getelementptr i8, ptr @$str41, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %98)
  %99 = getelementptr inbounds float, ptr %5, i16 0
  %100 = load float, ptr %99, !tbaa !2
  call cc 1005 addrspace(1) void @N$PR4(float %100)
  %101 = getelementptr i8, ptr @$str42, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %101)
  %102 = load float, ptr %2, !tbaa !2
  %103 = load float, ptr @$f32_00000000, !tbaa !2
  %104 = fmul float %102, %103
  call cc 1005 addrspace(1) void @N$PR4(float %104)
  call cc 1005 addrspace(1) void @N$PN()
  %105 = getelementptr inbounds [26 x i8], ptr %9, i16 0
  %106 = load ptr, ptr %105, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %106)
  %107 = getelementptr inbounds [26 x i8], ptr %9, i16 1
  %108 = load ptr, ptr %107, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %108)
  %109 = getelementptr inbounds [26 x i8], ptr %9, i16 2
  %110 = load ptr, ptr %109, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %110)
  ret i16 0

b11:
  call cc 1005 addrspace(1) void @N$ECNV()
  unreachable
}

declare cc 1005 void @N$ECNV() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PR8(double) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PR4(float) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
