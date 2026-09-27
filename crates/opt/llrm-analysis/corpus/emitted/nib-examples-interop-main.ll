target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [16 x i8] c"\08\00\09\00\09\00checksum \00"
@$str2 = internal constant [16 x i8] c"\08\00\09\00\09\00weighted \00"
@$str3 = internal constant [16 x i8] c"\08\00\09\00\09\00centroid \00"
@$str4 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"

define i16 @_weight(i16 %0) addrspace(1) {
b1:
  %1 = icmp slt i16 %0, 0
  %2 = sext i1 %1 to i8
  %3 = icmp ne i8 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = sub i16 0, %0
  %5 = mul i16 %4, 3
  ret i16 %5

b3:
  br label %b4

b4:
  ret i16 %0
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i32
  %1 = alloca i16
  %2 = alloca [4 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [12 x i8]
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [10 x i8]
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [4 x i8]
  store i32 0, ptr %0
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 12, i1 false)
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 10, i1 false)
  store i16 0, ptr %9
  store i16 0, ptr %10
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 4, i1 false)
  store i16 4, ptr %9, !tbaa !2
  store i16 4, ptr %10, !tbaa !2
  %12 = sub i16 0, 0
  %13 = getelementptr inbounds i8, ptr %11, i16 %12
  store i8 81, ptr %13, !tbaa !2
  %14 = sub i16 1, 0
  %15 = getelementptr inbounds i8, ptr %11, i16 %14
  store i8 66, ptr %15, !tbaa !2
  %16 = sub i16 2, 0
  %17 = getelementptr inbounds i8, ptr %11, i16 %16
  store i8 42, ptr %17, !tbaa !2
  %18 = sub i16 3, 0
  %19 = getelementptr inbounds i8, ptr %11, i16 %18
  store i8 7, ptr %19, !tbaa !2
  store i16 5, ptr %6, !tbaa !2
  store i16 5, ptr %7, !tbaa !2
  %20 = sub i16 0, 0
  %21 = getelementptr inbounds i16, ptr %8, i16 %20
  store i16 3, ptr %21, !tbaa !2
  %22 = sub i16 1, 0
  %23 = getelementptr inbounds i16, ptr %8, i16 %22
  store i16 -1, ptr %23, !tbaa !2
  %24 = sub i16 2, 0
  %25 = getelementptr inbounds i16, ptr %8, i16 %24
  store i16 4, ptr %25, !tbaa !2
  %26 = sub i16 3, 0
  %27 = getelementptr inbounds i16, ptr %8, i16 %26
  store i16 -1, ptr %27, !tbaa !2
  %28 = sub i16 4, 0
  %29 = getelementptr inbounds i16, ptr %8, i16 %28
  store i16 5, ptr %29, !tbaa !2
  store i16 3, ptr %3, !tbaa !2
  store i16 3, ptr %4, !tbaa !2
  %30 = sub i16 0, 0
  %31 = getelementptr inbounds [4 x i8], ptr %5, i16 %30
  store i16 0, ptr %31, !tbaa !2
  %32 = sub i16 0, 0
  %33 = getelementptr inbounds [4 x i8], ptr %5, i16 %32
  %34 = getelementptr inbounds i8, ptr %33, i16 2
  store i16 0, ptr %34, !tbaa !2
  %35 = sub i16 1, 0
  %36 = getelementptr inbounds [4 x i8], ptr %5, i16 %35
  store i16 30, ptr %36, !tbaa !2
  %37 = sub i16 1, 0
  %38 = getelementptr inbounds [4 x i8], ptr %5, i16 %37
  %39 = getelementptr inbounds i8, ptr %38, i16 2
  store i16 6, ptr %39, !tbaa !2
  %40 = sub i16 2, 0
  %41 = getelementptr inbounds [4 x i8], ptr %5, i16 %40
  store i16 6, ptr %41, !tbaa !2
  %42 = sub i16 2, 0
  %43 = getelementptr inbounds [4 x i8], ptr %5, i16 %42
  %44 = getelementptr inbounds i8, ptr %43, i16 2
  store i16 30, ptr %44, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 0, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %11 to ptr addrspace(1)
  %47 = call addrspace(1) i16 @_checksum(ptr addrspace(1) %46, i16 4)
  store i16 %47, ptr %1, !tbaa !2
  %48 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  %49 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %49)
  call addrspace(1) void @N$PN()
  %50 = addrspacecast ptr %8 to ptr addrspace(1)
  %51 = call addrspace(1) i32 @_weighted_sum(ptr addrspace(1) %50, i16 5)
  store i32 %51, ptr %0, !tbaa !2
  %52 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %52)
  %53 = load i32, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %53)
  call addrspace(1) void @N$PN()
  %54 = addrspacecast ptr %5 to ptr addrspace(1)
  %55 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @_centroid(ptr addrspace(1) %54, i16 3, ptr addrspace(1) %55)
  %56 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %56)
  %57 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %57)
  %58 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %58)
  %59 = getelementptr inbounds i8, ptr %2, i16 2
  %60 = load i16, ptr %59, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %60)
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
