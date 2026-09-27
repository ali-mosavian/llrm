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

define internal double @root(double %0) addrspace(1) willreturn {
b1:
  %1 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  %2 = fcmp ogt double %0, %1
  br i1 %2, label %b4, label %b3

b3:
  br label %b4

b4:
  %3 = phi double [ %0, %b1 ], [ %1, %b3 ]
  %4 = load double, ptr @$f64_4000000000000000, !tbaa !2
  br label %b5

b5:
  %5 = phi double [ %3, %b4 ], [ %10, %b6 ]
  %6 = phi i16 [ 0, %b4 ], [ %11, %b6 ]
  %7 = icmp slt i16 %6, 80
  br i1 %7, label %b6, label %b8

b6:
  %8 = fdiv double %0, %5
  %9 = fadd double %5, %8
  %10 = fdiv double %9, %4
  %11 = add i16 %6, 1
  br label %b5

b8:
  ret double %5
}

define internal void @report(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load double, ptr addrspace(1) %2
  %4 = fmul double %1, %3
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %6 = load double, ptr addrspace(1) %5
  %7 = fmul double %6, %6
  %8 = fdiv double %4, %7
  %9 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  %10 = fmul double %1, %9
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 18
  %12 = load double, ptr addrspace(1) %11
  %13 = fdiv double %10, %12
  %14 = load double, ptr @$f64_3ff0000000000000
  %15 = fcmp ogt double %13, %14
  br i1 %15, label %17, label %16

16:
  br label %17

17:
  %18 = phi double [ %13, %b1 ], [ %14, %16 ]
  %19 = load double, ptr @$f64_4000000000000000
  br label %20

20:
  %21 = phi double [ %18, %17 ], [ %27, %24 ]
  %22 = phi i16 [ 0, %17 ], [ %28, %24 ]
  %23 = icmp slt i16 %22, 80
  br i1 %23, label %24, label %29

24:
  %25 = fdiv double %13, %21
  %26 = fadd double %21, %25
  %27 = fdiv double %26, %19
  %28 = add i16 %22, 1
  br label %20

29:
  %30 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %31 = load double, ptr @$f64_400921fb54442d18, !tbaa !2
  %32 = fmul double %30, %31
  %33 = load double, ptr addrspace(1) %11
  %34 = fmul double %32, %33
  %35 = fdiv double %34, %21
  %36 = load double, ptr @$f64_40f5180000000000, !tbaa !2
  %37 = fdiv double %35, %36
  %38 = load ptr, ptr addrspace(1) %0
  call addrspace(1) void @N$PS(ptr %38)
  %39 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %39)
  call addrspace(1) void @N$PR8(double %8)
  %40 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %40)
  call addrspace(1) void @N$PR8(double %21)
  %41 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %41)
  call addrspace(1) void @N$PR8(double %37)
  %42 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %42)
  call addrspace(1) void @N$PN()
  ret void
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [16 x i8]
  %1 = alloca [78 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 16, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 78, i1 false)
  %2 = getelementptr i8, ptr @$str11, i16 6
  %3 = load double, ptr @$f64_44d179b07057cf97, !tbaa !2
  %4 = load double, ptr @$f64_41429d0a00000000, !tbaa !2
  %5 = load double, ptr @$f64_422af768f3000000, !tbaa !2
  %6 = getelementptr inbounds [26 x i8], ptr %1, i16 0
  store ptr %2, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %6, i16 2
  store double %3, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %6, i16 10
  store double %4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %6, i16 18
  store double %5, ptr %9, !tbaa !2
  %10 = getelementptr i8, ptr @$str15, i16 6
  %11 = load double, ptr @$f64_4513c27b13272fb6, !tbaa !2
  %12 = load double, ptr @$f64_41584dae00000000, !tbaa !2
  %13 = load double, ptr @$f64_42416a6d6c000000, !tbaa !2
  %14 = getelementptr inbounds [26 x i8], ptr %1, i16 1
  store ptr %10, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %14, i16 2
  store double %11, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %14, i16 10
  store double %12, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %14, i16 18
  store double %13, ptr %17, !tbaa !2
  %18 = getelementptr i8, ptr @$str19, i16 6
  %19 = load double, ptr @$f64_459887f488aced67, !tbaa !2
  %20 = load double, ptr @$f64_4190ab0760000000, !tbaa !2
  %21 = load double, ptr @$f64_4266a846e9200000, !tbaa !2
  %22 = getelementptr inbounds [26 x i8], ptr %1, i16 2
  store ptr %18, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %22, i16 2
  store double %19, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %22, i16 10
  store double %20, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %22, i16 18
  store double %21, ptr %25, !tbaa !2
  br label %b2

b2:
  %26 = phi i16 [ 0, %b1 ], [ %30, %b3 ]
  %27 = icmp ult i16 %26, 3
  br i1 %27, label %b3, label %b5

b3:
  %28 = getelementptr inbounds [26 x i8], ptr %1, i16 %26
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  call addrspace(1) void @report(ptr addrspace(1) %29)
  %30 = add i16 %26, 1
  br label %b2

b5:
  %31 = getelementptr i8, ptr @$str23, i16 6
  call addrspace(1) void @N$PS(ptr %31)
  %32 = load double, ptr @$f64_3dd25868f4deae16, !tbaa !2
  call addrspace(1) void @N$PR8(double %32)
  %33 = getelementptr i8, ptr @$str24, i16 6
  call addrspace(1) void @N$PS(ptr %33)
  %34 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  call addrspace(1) void @N$PR8(double %34)
  %35 = getelementptr i8, ptr @$str25, i16 6
  call addrspace(1) void @N$PS(ptr %35)
  %36 = load double, ptr @$f64_46391ace3d05aef4, !tbaa !2
  %37 = load double, ptr @$f64_3ff0000000000000, !tbaa !2
  %38 = fdiv double %37, %36
  call addrspace(1) void @N$PR8(double %38)
  call addrspace(1) void @N$PN()
  %39 = load float, ptr @$f32_41ac0000, !tbaa !2
  %40 = getelementptr inbounds float, ptr %0, i16 0
  store float %39, ptr %40, !tbaa !2
  %41 = load float, ptr @$f32_41ae0000, !tbaa !2
  %42 = getelementptr inbounds float, ptr %0, i16 1
  store float %41, ptr %42, !tbaa !2
  %43 = load float, ptr @$f32_404ccccd, !tbaa !2
  %44 = fneg float %43
  %45 = getelementptr inbounds float, ptr %0, i16 2
  store float %44, ptr %45, !tbaa !2
  %46 = load float, ptr @$f32_3dcccccd, !tbaa !2
  %47 = getelementptr inbounds float, ptr %0, i16 3
  store float %46, ptr %47, !tbaa !2
  %48 = load float, ptr @$f32_00000000, !tbaa !2
  br label %b6

b6:
  %49 = phi float [ %48, %b5 ], [ %54, %b7 ]
  %50 = phi i16 [ 0, %b5 ], [ %55, %b7 ]
  %51 = icmp ult i16 %50, 4
  br i1 %51, label %b7, label %b9

b7:
  %52 = getelementptr inbounds float, ptr %0, i16 %50
  %53 = load float, ptr %52, !tbaa !2
  %54 = fadd float %49, %53
  %55 = add i16 %50, 1
  br label %b6

b9:
  %56 = getelementptr i8, ptr @$str31, i16 6
  call addrspace(1) void @N$PS(ptr %56)
  %57 = load float, ptr @$f32_40800000, !tbaa !2
  %58 = fdiv float %49, %57
  call addrspace(1) void @N$PR4(float %58)
  %59 = getelementptr i8, ptr @$str33, i16 6
  call addrspace(1) void @N$PS(ptr %59)
  %60 = load float, ptr %40, !tbaa !2
  call addrspace(1) void @N$PR4(float %60)
  %61 = getelementptr i8, ptr @$str34, i16 6
  call addrspace(1) void @N$PS(ptr %61)
  %62 = load float, ptr @$f32_00000000, !tbaa !2
  %63 = fmul float %49, %62
  call addrspace(1) void @N$PR4(float %63)
  call addrspace(1) void @N$PN()
  %64 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %64)
  %65 = load ptr, ptr %14, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %65)
  %66 = load ptr, ptr %22, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %66)
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
