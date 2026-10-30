target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str11 = internal constant [11 x i8] c"\08\00\04\00\04\00: g \00"
@$str12 = internal constant [20 x i8] c"\08\00\0D\00\0D\00 m/s2, orbit \00"
@$str13 = internal constant [18 x i8] c"\08\00\0B\00\0B\00 m/s, year \00"
@$str14 = internal constant [12 x i8] c"\08\00\05\00\05\00 days\00"
@$str15 = internal constant [14 x i8] c"\08\00\07\00\07\00Mercury\00"
@$str19 = internal constant [12 x i8] c"\08\00\05\00\05\00Earth\00"
@$str23 = internal constant [14 x i8] c"\08\00\07\00\07\00Jupiter\00"
@$str27 = internal constant [9 x i8] c"\08\00\02\00\02\00G \00"
@$str28 = internal constant [13 x i8] c"\08\00\06\00\06\00, sun \00"
@$str29 = internal constant [24 x i8] c"\08\00\11\00\11\00 kg, one part in \00"
@$str40 = internal constant [24 x i8] c"\08\00\11\00\11\00thermometer mean \00"
@$str41 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 C, first \00"
@$str42 = internal constant [14 x i8] c"\08\00\07\00\07\00, zero \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [78 x i8]
  store ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6), ptr %0, !tbaa !2
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 2
  store double 3.301000e+23, ptr %1, !tbaa !2
  %2 = getelementptr inbounds nuw i8, ptr %0, i16 10
  store double 2.439700e+06, ptr %2, !tbaa !2
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 18
  store double 5.791000e+10, ptr %3, !tbaa !2
  %4 = getelementptr inbounds nuw i8, ptr %0, i16 26
  store ptr getelementptr inbounds nuw (i8, ptr @$str19, i16 6), ptr %4, !tbaa !2
  %5 = getelementptr inbounds nuw i8, ptr %0, i16 28
  store double 5.972000e+24, ptr %5, !tbaa !2
  %6 = getelementptr inbounds nuw i8, ptr %0, i16 36
  store double 6.371000e+06, ptr %6, !tbaa !2
  %7 = getelementptr inbounds nuw i8, ptr %0, i16 44
  store double 1.496000e+11, ptr %7, !tbaa !2
  %8 = getelementptr inbounds nuw i8, ptr %0, i16 52
  store ptr getelementptr inbounds nuw (i8, ptr @$str23, i16 6), ptr %8, align 4, !tbaa !2
  %9 = getelementptr inbounds nuw i8, ptr %0, i16 54
  store double 1.898000e+27, ptr %9, !tbaa !2
  %10 = getelementptr inbounds nuw i8, ptr %0, i16 62
  store double 6.991100e+07, ptr %10, !tbaa !2
  %11 = getelementptr inbounds nuw i8, ptr %0, i16 70
  store double 7.785000e+11, ptr %11, !tbaa !2
  %12 = fmul double 6.674000e-11, 1.989000e+30
  br label %13

13:
  %14 = phi i16 [ 0, %b1 ], [ %71, %63 ]
  %15 = getelementptr inbounds nuw [26 x i8], ptr %0, i16 %14
  %16 = addrspacecast ptr %15 to ptr addrspace(5)
  %17 = getelementptr inbounds nuw i8, ptr addrspace(5) %16, i16 2
  %18 = load double, ptr addrspace(5) %17
  %19 = fmul double %18, 6.674000e-11
  %20 = getelementptr inbounds nuw i8, ptr addrspace(5) %16, i16 10
  %21 = load double, ptr addrspace(5) %20
  %22 = fmul double %21, %21
  %23 = fdiv double %19, %22
  %24 = fmul double %23, 1.000000e+03
  %25 = fadd double %24, 5.000000e-01
  %26 = fcmp ogt double %25, 0xC1E0000000200000
  %27 = fcmp olt double %25, 0x41E0000000000000
  %28 = and i1 %26, %27
  br i1 %28, label %30, label %29

29:
  tail call cc 1005 addrspace(1) void @N$ECNV()
  unreachable

30:
  %31 = getelementptr inbounds nuw i8, ptr addrspace(5) %16, i16 18
  %32 = load double, ptr addrspace(5) %31
  %33 = fdiv double %12, %32
  %34 = fcmp ogt double %33, 1.000000e+00
  %35 = select i1 %34, double %33, double 1.000000e+00
  br label %36

36:
  %37 = phi i16 [ %42, %36 ], [ 80, %30 ]
  %38 = phi double [ %35, %30 ], [ %41, %36 ]
  %39 = fdiv double %33, %38
  %40 = fadd double %38, %39
  %41 = fmul double %40, 5.000000e-01
  %42 = add nsw i16 %37, -1
  %exitcond = icmp eq i16 %42, 0
  br i1 %exitcond, label %43, label %36

43:
  %44 = fmul double %41, 1.000000e+03
  %45 = fadd double %44, 5.000000e-01
  %46 = fcmp ogt double %45, 0xC1E0000000200000
  %47 = fcmp olt double %45, 0x41E0000000000000
  %48 = and i1 %46, %47
  br i1 %48, label %50, label %49

49:
  tail call cc 1005 addrspace(1) void @N$ECNV()
  unreachable

50:
  %51 = fptosi double %45 to i32
  %52 = sitofp i32 %51 to double
  %53 = fdiv double %52, 1.000000e+03
  %54 = fmul double %32, 0x401921FB54442D18
  %55 = fdiv double %54, %53
  %56 = fdiv double %55, 8.640000e+04
  %57 = fmul double %56, 1.000000e+03
  %58 = fadd double %57, 5.000000e-01
  %59 = fcmp ogt double %58, 0xC1E0000000200000
  %60 = fcmp olt double %58, 0x41E0000000000000
  %61 = and i1 %59, %60
  br i1 %61, label %63, label %62

62:
  tail call cc 1005 addrspace(1) void @N$ECNV()
  unreachable

63:
  %64 = fptosi double %25 to i32
  %65 = sitofp i32 %64 to double
  %66 = fdiv double %65, 1.000000e+03
  %67 = fptosi double %58 to i32
  %68 = sitofp i32 %67 to double
  %69 = fdiv double %68, 1.000000e+03
  %70 = load ptr, ptr addrspace(5) %16
  tail call cc 1005 addrspace(1) void @N$PS(ptr %70)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  tail call cc 1005 addrspace(1) void @N$PR8(double %66)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  tail call cc 1005 addrspace(1) void @N$PR8(double %53)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6))
  tail call cc 1005 addrspace(1) void @N$PR8(double %69)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %71 = add nuw nsw i16 %14, 1
  %exitcond2 = icmp eq i16 %71, 3
  br i1 %exitcond2, label %b5, label %13

b5:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str27, i16 6))
  tail call cc 1005 addrspace(1) void @N$PR8(double 6.674000e-11)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str28, i16 6))
  tail call cc 1005 addrspace(1) void @N$PR8(double 1.989000e+30)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str29, i16 6))
  %72 = fdiv double 1.000000e+00, 1.989000e+30
  tail call cc 1005 addrspace(1) void @N$PR8(double %72)
  tail call cc 1005 addrspace(1) void @N$PN()
  %73 = fadd float 4.325000e+01, 0xC0099999A0000000
  %74 = fadd float %73, 0x3FB99999A0000000
  %75 = fmul float %74, 2.500000e-01
  %76 = fmul float %75, 1.000000e+02
  %77 = fadd float %76, 5.000000e-01
  %78 = fcmp oge float %77, 0xC1E0000000000000
  %79 = fcmp olt float %77, 0x41E0000000000000
  %80 = and i1 %78, %79
  br i1 %80, label %b10, label %b11

b10:
  %81 = fptosi float %77 to i32
  %82 = sitofp i32 %81 to float
  %83 = fdiv float %82, 1.000000e+02
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str40, i16 6))
  tail call cc 1005 addrspace(1) void @N$PR4(float %83)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str41, i16 6))
  tail call cc 1005 addrspace(1) void @N$PR4(float 2.150000e+01)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str42, i16 6))
  %84 = fmul float %74, 0.000000e+00
  tail call cc 1005 addrspace(1) void @N$PR4(float %84)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6))
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str19, i16 6))
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str23, i16 6))
  ret i16 0

b11:
  tail call cc 1005 addrspace(1) void @N$ECNV()
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
