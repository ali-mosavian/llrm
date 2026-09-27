target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00ok\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00bad\00"

define internal i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = mul i16 %4, 2
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %12 = load i16, ptr %2, !tbaa !2
  %13 = load i16, ptr addrspace(1) %11
  %14 = add i16 %12, %13
  store i16 %14, ptr %2, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %1, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %1, !tbaa !2
  br label %b2

b5:
  %17 = load i16, ptr %2, !tbaa !2
  ret i16 %17
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [12 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 12, i1 false)
  store i16 6, ptr %2, !tbaa !2
  store i16 6, ptr %3, !tbaa !2
  %5 = sub i16 0, 0
  %6 = getelementptr inbounds i16, ptr %4, i16 %5
  store i16 1, ptr %6, !tbaa !2
  %7 = sub i16 1, 0
  %8 = getelementptr inbounds i16, ptr %4, i16 %7
  store i16 2, ptr %8, !tbaa !2
  %9 = sub i16 2, 0
  %10 = getelementptr inbounds i16, ptr %4, i16 %9
  store i16 3, ptr %10, !tbaa !2
  %11 = sub i16 3, 0
  %12 = getelementptr inbounds i16, ptr %4, i16 %11
  store i16 4, ptr %12, !tbaa !2
  %13 = sub i16 4, 0
  %14 = getelementptr inbounds i16, ptr %4, i16 %13
  store i16 5, ptr %14, !tbaa !2
  %15 = sub i16 5, 0
  %16 = getelementptr inbounds i16, ptr %4, i16 %15
  store i16 6, ptr %16, !tbaa !2
  %17 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 6, ptr %1, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 6, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %17, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %1 to ptr addrspace(1)
  %21 = call addrspace(1) i16 @sum(ptr addrspace(1) %20)
  store i16 %21, ptr %0, !tbaa !2
  %22 = load i16, ptr %0, !tbaa !2
  %23 = icmp eq i16 %22, 21
  %24 = sext i1 %23 to i8
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b2, label %b3

b2:
  %26 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %26)
  call addrspace(1) void @N$PN()
  br label %b4

b3:
  %27 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  %28 = load i16, ptr %0, !tbaa !2
  ret i16 %28
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
