target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [20 x i8] c"\08\00\0D\00\0D\00sum_three: ok\00"
@$str2 = internal constant [21 x i8] c"\08\00\0E\00\0E\00sum_three: bad\00"

define internal i16 @sum_three(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) {
b1:
  %3 = load i16, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = load i16, ptr addrspace(1) %1
  %7 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  %9 = load i16, ptr addrspace(1) %2
  %10 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  br label %b2

b2:
  %12 = phi i16 [ 0, %b1 ], [ %26, %b10 ]
  %13 = phi i16 [ 0, %b1 ], [ %27, %b10 ]
  %14 = icmp ult i16 %13, %3
  br i1 %14, label %b3, label %b5

b3:
  %15 = shl i16 %13, 1
  %16 = getelementptr i8, ptr addrspace(1) %5, i16 %15
  %17 = load i16, ptr addrspace(1) %16
  %18 = add i16 %12, %17
  %19 = icmp ult i16 %13, %6
  br i1 %19, label %b8, label %b9

b5:
  ret i16 %12

b8:
  %20 = getelementptr i8, ptr addrspace(1) %8, i16 %15
  %21 = load i16, ptr addrspace(1) %20
  %22 = add i16 %18, %21
  %23 = icmp ult i16 %13, %9
  br i1 %23, label %b10, label %b11

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  %24 = getelementptr i8, ptr addrspace(1) %11, i16 %15
  %25 = load i16, ptr addrspace(1) %24
  %26 = add i16 %22, %25
  %27 = add i16 %13, 1
  br label %b2

b11:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %3 = getelementptr inbounds i16, ptr %2, i16 0
  store i16 1, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %2, i16 1
  store i16 2, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %2, i16 2
  store i16 3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %2, i16 3
  store i16 4, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %1, i16 0
  store i16 10, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %1, i16 1
  store i16 20, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %1, i16 2
  store i16 30, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %1, i16 3
  store i16 40, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 100, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i16, ptr %0, i16 1
  store i16 200, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i16, ptr %0, i16 2
  store i16 300, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i16, ptr %0, i16 3
  store i16 400, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %2 to ptr addrspace(1)
  %16 = addrspacecast ptr %1 to ptr addrspace(1)
  %17 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %18

18:
  %19 = phi i16 [ 0, %b1 ], [ %32, %22 ]
  %20 = phi i16 [ 0, %b1 ], [ %33, %22 ]
  %21 = icmp ult i16 %20, 4
  br i1 %21, label %22, label %34

22:
  %23 = shl i16 %20, 1
  %24 = getelementptr i8, ptr addrspace(1) %15, i16 %23
  %25 = load i16, ptr addrspace(1) %24
  %26 = add i16 %19, %25
  %27 = getelementptr i8, ptr addrspace(1) %16, i16 %23
  %28 = load i16, ptr addrspace(1) %27
  %29 = add i16 %26, %28
  %30 = getelementptr i8, ptr addrspace(1) %17, i16 %23
  %31 = load i16, ptr addrspace(1) %30
  %32 = add i16 %29, %31
  %33 = add i16 %20, 1
  br label %18

34:
  %35 = icmp eq i16 %19, 1110
  br i1 %35, label %b2, label %b3

b2:
  %36 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %36)
  call addrspace(1) void @N$PN()
  br label %b4

b3:
  %37 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %37)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  ret i16 %19
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
