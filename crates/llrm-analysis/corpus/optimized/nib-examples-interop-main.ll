target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [16 x i8] c"\08\00\09\00\09\00checksum \00"
@$str2 = internal constant [16 x i8] c"\08\00\09\00\09\00weighted \00"
@$str3 = internal constant [16 x i8] c"\08\00\09\00\09\00centroid \00"
@$str4 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"

define i16 @_weight(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = icmp slt i16 %0, 0
  br i1 %1, label %b2, label %b3

b2:
  %2 = sub i16 0, %0
  %3 = mul i16 %2, 3
  ret i16 %3

b3:
  ret i16 %0
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [12 x i8]
  %2 = alloca [10 x i8]
  %3 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 12, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 10, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  %4 = getelementptr inbounds i8, ptr %3, i16 0
  store i8 81, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %3, i16 1
  store i8 66, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %3, i16 2
  store i8 42, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %3, i16 3
  store i8 7, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %2, i16 0
  store i16 3, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %2, i16 1
  store i16 -1, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %2, i16 2
  store i16 4, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %2, i16 3
  store i16 -1, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i16, ptr %2, i16 4
  store i16 5, ptr %12, !tbaa !2
  %13 = getelementptr inbounds [4 x i8], ptr %1, i16 0
  store i16 0, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 0, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [4 x i8], ptr %1, i16 1
  store i16 30, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 6, ptr %16, !tbaa !2
  %17 = getelementptr inbounds [4 x i8], ptr %1, i16 2
  store i16 6, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 30, ptr %18, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 0, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %3 to ptr addrspace(1)
  %21 = call addrspace(1) i16 @_checksum(ptr addrspace(1) %20, i16 4)
  %22 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %22)
  call addrspace(1) void @N$PU2(i16 %21)
  call addrspace(1) void @N$PN()
  %23 = addrspacecast ptr %2 to ptr addrspace(1)
  %24 = call addrspace(1) i32 @_weighted_sum(ptr addrspace(1) %23, i16 5)
  %25 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %25)
  call addrspace(1) void @N$PI4(i32 %24)
  call addrspace(1) void @N$PN()
  %26 = addrspacecast ptr %1 to ptr addrspace(1)
  %27 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @_centroid(ptr addrspace(1) %26, i16 3, ptr addrspace(1) %27)
  %28 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %28)
  %29 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %29)
  %30 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %30)
  %31 = load i16, ptr %19, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %31)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare i16 @_checksum(ptr addrspace(1), i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare i32 @_weighted_sum(ptr addrspace(1), i16) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @_centroid(ptr addrspace(1), i16, ptr addrspace(1)) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
