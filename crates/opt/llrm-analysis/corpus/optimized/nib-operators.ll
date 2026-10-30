target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [18 x i8]
  tail call cc 1005 addrspace(1) void @N$PU2(i16 -16387)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI4(i32 -812500)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI4(i32 3203125)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PI2(i16 1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 0)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 0)
  tail call cc 1005 addrspace(1) void @N$PN()
  call void @llvm.lifetime.start.p0(i64 18, ptr nonnull %1)
  call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr nonnull %1, i16 15, i16 9, i1 false)
  %2 = load i16, ptr %1, !tbaa !2
  %3 = getelementptr inbounds nuw i8, ptr %1, i16 2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = getelementptr inbounds nuw i8, ptr %1, i16 4
  %6 = load i16, ptr %5, !tbaa !2
  %7 = getelementptr inbounds nuw i8, ptr %1, i16 6
  %8 = load i16, ptr %7, !tbaa !2
  %9 = getelementptr inbounds nuw i8, ptr %1, i16 10
  %10 = load i16, ptr %9, !tbaa !2
  %11 = getelementptr inbounds nuw i8, ptr %1, i16 12
  %12 = load i16, ptr %11, !tbaa !2
  %13 = getelementptr inbounds nuw i8, ptr %1, i16 14
  %14 = load i16, ptr %13, !tbaa !2
  %15 = getelementptr inbounds nuw i8, ptr %1, i16 16
  %16 = load i16, ptr %15, !tbaa !2
  %17 = add i16 %2, 1
  %18 = add i16 %17, %4
  %19 = add i16 %18, %6
  %20 = add i16 %19, %8
  %21 = add i16 %20, %10
  %22 = add i16 %21, %12
  %23 = add i16 %22, %14
  %24 = add i16 %23, %16
  call void @llvm.lifetime.end.p0(i64 18, ptr nonnull %1)
  tail call cc 1005 addrspace(1) void @N$PI4(i32 447)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 254)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 224)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %24)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$ESHF() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.experimental.memset.pattern.p0.i16.i16(ptr nocapture writeonly, i16, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
