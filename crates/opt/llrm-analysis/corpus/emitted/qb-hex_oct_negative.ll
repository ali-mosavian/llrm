target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"Y&" = internal global [4 x i8] zeroinitializer
@$float3 = internal constant [4 x i8] c"\00\00\C0?"
@$float4 = internal constant [4 x i8] c"\00P\C3G"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65535)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %0)
  %1 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65520)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %1)
  %2 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65534)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %2)
  %3 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 255)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %3)
  %4 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 32767)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %4)
  %5 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65535)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %5)
  %6 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65520)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %6)
  %7 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65534)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %7)
  %8 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 255)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %8)
  %9 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 -1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %9)
  %10 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 -16)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %10)
  %11 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 -1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %11)
  store i16 -1, ptr @"X%", !tbaa !2
  store i32 -16, ptr @"Y&", !tbaa !2
  %12 = load i16, ptr @"X%", !tbaa !2
  %13 = zext i16 %12 to i32
  %14 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 %13)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %14)
  %15 = load i16, ptr @"X%", !tbaa !2
  %16 = zext i16 %15 to i32
  %17 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 %16)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %17)
  %18 = load i32, ptr @"Y&", !tbaa !2
  %19 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 %18)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %19)
  %20 = load i32, ptr @"Y&", !tbaa !2
  %21 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 %20)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %21)
  %22 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65535)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %22)
  %23 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65536)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %23)
  %24 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 -32768)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %24)
  %25 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 65535)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %25)
  %26 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FOCT(i32 65535)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %26)
  %27 = load float, ptr @$float3, !tbaa !2
  %28 = fneg float %27
  %29 = call i32 @llvm.lrint.i32.f32(float %28)
  %30 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 %29)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %30)
  %31 = load float, ptr @$float4, !tbaa !2
  %32 = call i32 @llvm.lrint.i32.f32(float %31)
  %33 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FHEX(i32 %32)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %33)
  ret void
}

declare cc 1000 ptr @llrm.qb.B$FHEX(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FOCT(i32) addrspace(1)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
