target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"C@" = internal global [8 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@$float3 = internal constant [8 x i8] c"-C\1C\EB\E26\0A?"
@$float4 = internal constant [4 x i8] c"\00@\1CF"
@$arguments$5 = internal global [16 x i8] zeroinitializer
@$float6 = internal constant [8 x i8] c"\FC\A9\F1\D2Mb0?"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = load double, ptr @$float3, !tbaa !2
  store double %0, ptr @"D#", !tbaa !2
  %1 = load double, ptr @"D#", !tbaa !2
  %2 = fpext double %1 to x86_fp80
  %3 = load float, ptr @$float4, !tbaa !2
  %4 = fpext float %3 to x86_fp80
  %5 = fmul x86_fp80 %2, %4
  %6 = call i64 @llvm.lrint.i64.f80(x86_fp80 %5)
  store i64 %6, ptr @"C@", !tbaa !2
  %7 = load i64, ptr @"C@", !tbaa !2
  store i64 %7, ptr @$arguments$5, !tbaa !2
  %8 = load i64, ptr @$arguments$5, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PECY(i64 %8)
  %9 = load double, ptr @$float6, !tbaa !2
  store double %9, ptr @"D#", !tbaa !2
  %10 = load double, ptr @"D#", !tbaa !2
  %11 = fpext double %10 to x86_fp80
  %12 = load float, ptr @$float4, !tbaa !2
  %13 = fpext float %12 to x86_fp80
  %14 = fmul x86_fp80 %11, %13
  %15 = call i64 @llvm.lrint.i64.f80(x86_fp80 %14)
  store i64 %15, ptr @"C@", !tbaa !2
  %16 = load i64, ptr @"C@", !tbaa !2
  %17 = getelementptr i8, ptr @$arguments$5, i16 8
  store i64 %16, ptr %17, !tbaa !2
  %18 = getelementptr i8, ptr @$arguments$5, i16 8
  %19 = load i64, ptr %18, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PECY(i64 %19)
  ret void
}

declare i64 @llvm.lrint.i64.f80(x86_fp80) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PECY(i64) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
