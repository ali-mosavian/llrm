target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00..\00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00 clamped to \00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00% -> \00"

define cc1000 i16 @CLAMP(i16 %0, i16 %1, i16 %2) addrspace(1) memory(none) willreturn {
b1:
  %3 = icmp slt i16 %0, %1
  br i1 %3, label %b2, label %b3

b2:
  ret i16 %1

b3:
  %4 = icmp sgt i16 %0, %2
  br i1 %4, label %b5, label %b7

b5:
  ret i16 %2

b7:
  ret i16 %0
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [4 x i8]
  %2 = alloca [12 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 12, i1 false)
  %3 = getelementptr inbounds i16, ptr %2, i16 0
  store i16 -40, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %2, i16 1
  store i16 15, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %2, i16 2
  store i16 70, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %2, i16 3
  store i16 99, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %2, i16 4
  store i16 140, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %2, i16 5
  store i16 260, ptr %8, !tbaa !2
  %9 = addrspacecast ptr %2 to ptr addrspace(1)
  %10 = call cc1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %9, i16 6)
  %11 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %10, ptr addrspace(1) %11, !tbaa !2
  %12 = load i16, ptr %1, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 2
  %14 = load i16, ptr %13, !tbaa !2
  call cc1000 addrspace(1) void @CLAMP_ALL(ptr addrspace(1) %9, i16 6, i16 0, i16 100)
  %15 = call cc1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %9, i16 6)
  %16 = addrspacecast ptr %0 to ptr addrspace(1)
  store i32 %15, ptr addrspace(1) %16, !tbaa !2
  %17 = load i16, ptr %0, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %0, i16 2
  %19 = load i16, ptr %18, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %12)
  %20 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %20)
  call addrspace(1) void @N$PI2(i16 %14)
  %21 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %21)
  call addrspace(1) void @N$PI2(i16 %17)
  call addrspace(1) void @N$PS(ptr %20)
  call addrspace(1) void @N$PI2(i16 %19)
  call addrspace(1) void @N$PN()
  %22 = getelementptr i8, ptr @$str3, i16 6
  br label %b2

b2:
  %23 = phi i16 [ 0, %b1 ], [ %29, %b3 ]
  %24 = icmp ult i16 %23, 6
  br i1 %24, label %b3, label %b5

b3:
  %25 = getelementptr inbounds i16, ptr %2, i16 %23
  %26 = load i16, ptr %25, !tbaa !2
  %27 = load i16, ptr %25, !tbaa !2
  %28 = call cc1000 addrspace(1) i16 @SCALE(i16 %27, i16 255, i16 100)
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %26)
  call addrspace(1) void @N$PS(ptr %22)
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %28)
  call addrspace(1) void @N$PN()
  %29 = add i16 %23, 1
  br label %b2

b5:
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 i32 @SPAN(ptr addrspace(1), i16) addrspace(1)

declare cc1000 void @CLAMP_ALL(ptr addrspace(1), i16, i16, i16) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare cc1000 i16 @SCALE(i16, i16, i16) addrspace(1)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
