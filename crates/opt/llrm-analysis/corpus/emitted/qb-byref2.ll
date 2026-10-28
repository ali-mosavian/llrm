target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@$float3 = internal constant [4 x i8] c"\00\00\80@"
@$float4 = internal constant [4 x i8] c"\00\00\00A"
@$arguments$5 = internal global [24 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = load float, ptr @$float3, !tbaa !2
  store float %0, ptr @"S!", !tbaa !2
  %1 = load float, ptr @$float4, !tbaa !2
  %2 = fpext float %1 to double
  %3 = fpround double %2 to double
  store double %3, ptr @"D#", !tbaa !2
  %4 = call cc 1000 addrspace(1) ptr @"HALF!"(ptr @"S!", ptr @$arguments$5)
  %5 = load float, ptr %4
  %6 = fpround float %5 to float
  %7 = getelementptr i8, ptr @$arguments$5, i16 4
  store float %6, ptr %7, !tbaa !2
  %8 = getelementptr i8, ptr @$arguments$5, i16 4
  %9 = load float, ptr %8, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %9)
  %10 = getelementptr i8, ptr @$arguments$5, i16 8
  %11 = call cc 1000 addrspace(1) ptr @"DOUBLED#"(ptr @"D#", ptr %10)
  %12 = load double, ptr %11
  %13 = fpround double %12 to double
  %14 = getelementptr i8, ptr @$arguments$5, i16 16
  store double %13, ptr %14, !tbaa !2
  %15 = getelementptr i8, ptr @$arguments$5, i16 16
  %16 = load double, ptr %15, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %16)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  ret void
}

define cc 1000 ptr @"HALF!"(ptr dereferenceable(4) %0, ptr %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca float
  %4 = load float, ptr %0
  store i16 2, ptr %2, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = sitofp i16 %5 to float
  %7 = fdiv float %4, %6
  %8 = fpround float %7 to float
  store float %8, ptr %3, !tbaa !2
  br label %b2

b2:
  %9 = load float, ptr %3, !tbaa !2
  store float %9, ptr %1
  ret ptr %1
}

define cc 1000 ptr @"DOUBLED#"(ptr dereferenceable(8) %0, ptr %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca double
  %4 = load double, ptr %0
  store i16 2, ptr %2, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = sitofp i16 %5 to double
  %7 = fmul double %4, %6
  %8 = fpround double %7 to double
  store double %8, ptr %3, !tbaa !2
  br label %b2

b2:
  %9 = load double, ptr %3, !tbaa !2
  store double %9, ptr %1
  ret ptr %1
}

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
