target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00..\00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00 clamped to \00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00% -> \00"

define cc1000 i16 @CLAMP(i16 %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = icmp slt i16 %0, %1
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  ret i16 %1

b3:
  br label %b4

b4:
  %6 = icmp sgt i16 %0, %2
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b5, label %b6

b5:
  ret i16 %2

b6:
  br label %b7

b7:
  ret i16 %0
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca [4 x i8]
  %6 = alloca [4 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [12 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 12, i1 false)
  store i16 6, ptr %7, !tbaa !2
  store i16 6, ptr %8, !tbaa !2
  %10 = sub i16 0, 0
  %11 = getelementptr inbounds i16, ptr %9, i16 %10
  store i16 -40, ptr %11, !tbaa !2
  %12 = sub i16 1, 0
  %13 = getelementptr inbounds i16, ptr %9, i16 %12
  store i16 15, ptr %13, !tbaa !2
  %14 = sub i16 2, 0
  %15 = getelementptr inbounds i16, ptr %9, i16 %14
  store i16 70, ptr %15, !tbaa !2
  %16 = sub i16 3, 0
  %17 = getelementptr inbounds i16, ptr %9, i16 %16
  store i16 99, ptr %17, !tbaa !2
  %18 = sub i16 4, 0
  %19 = getelementptr inbounds i16, ptr %9, i16 %18
  store i16 140, ptr %19, !tbaa !2
  %20 = sub i16 5, 0
  %21 = getelementptr inbounds i16, ptr %9, i16 %20
  store i16 260, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %9 to ptr addrspace(1)
  %23 = call cc1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %22, i16 6)
  %24 = addrspacecast ptr %5 to ptr addrspace(1)
  store i32 %23, ptr addrspace(1) %24, !tbaa !2
  %25 = load i16, ptr %5, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %5, i16 2
  %27 = load i16, ptr %26, !tbaa !2
  store i16 %25, ptr %6, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %27, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc1000 addrspace(1) void @CLAMP_ALL(ptr addrspace(1) %29, i16 6, i16 0, i16 100)
  %30 = addrspacecast ptr %9 to ptr addrspace(1)
  %31 = call cc1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %30, i16 6)
  %32 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %31, ptr addrspace(1) %32, !tbaa !2
  %33 = load i16, ptr %3, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %3, i16 2
  %35 = load i16, ptr %34, !tbaa !2
  store i16 %33, ptr %4, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %35, ptr %36, !tbaa !2
  %37 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %37)
  %38 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %38)
  %39 = getelementptr inbounds i8, ptr %6, i16 2
  %40 = load i16, ptr %39, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %40)
  %41 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %41)
  %42 = load i16, ptr %4, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %42)
  %43 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %43)
  %44 = getelementptr inbounds i8, ptr %4, i16 2
  %45 = load i16, ptr %44, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %45)
  call addrspace(1) void @N$PN()
  store i16 0, ptr %2, !tbaa !2
  br label %b2

b2:
  %46 = load i16, ptr %2, !tbaa !2
  %47 = icmp ult i16 %46, 6
  %48 = sext i1 %47 to i8
  %49 = icmp ne i8 %48, 0
  br i1 %49, label %b3, label %b5

b3:
  %50 = sub i16 %46, 0
  %51 = getelementptr inbounds i16, ptr %9, i16 %50
  %52 = load i16, ptr %51, !tbaa !2
  store i16 %52, ptr %1, !tbaa !2
  %53 = sub i16 %46, 0
  %54 = getelementptr inbounds i16, ptr %9, i16 %53
  %55 = load i16, ptr %54, !tbaa !2
  %56 = call cc1000 addrspace(1) i16 @SCALE(i16 %55, i16 255, i16 100)
  store i16 %56, ptr %0, !tbaa !2
  %57 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %57)
  %58 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %58)
  %59 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %59)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  %60 = load i16, ptr %2, !tbaa !2
  %61 = add i16 %60, 1
  store i16 %61, ptr %2, !tbaa !2
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
