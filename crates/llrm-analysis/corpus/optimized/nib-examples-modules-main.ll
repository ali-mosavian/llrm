target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00 left\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00 right\00"

define internal i32 @shapes.geometry.Point.shifted(ptr addrspace(1) %0, i16 %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  %3 = load i16, ptr addrspace(1) %0
  %4 = add i16 %3, 5
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  store i16 %4, ptr %2, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %6, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %2 to ptr addrspace(1)
  %9 = load i32, ptr addrspace(1) %8, !tbaa !2
  ret i32 %9
}

define internal i8 @shapes.geometry.side(ptr addrspace(1) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = icmp slt i16 %1, 0
  br i1 %2, label %b4, label %b3

b3:
  br label %b4

b4:
  %3 = phi i8 [ 0, %b1 ], [ 1, %b3 ]
  ret i8 %3
}

define internal i16 @shapes.geometry.secret() addrspace(1) memory(none) willreturn {
b1:
  ret i16 7
}

define internal ptr @report.describe(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = icmp slt i16 %1, 0
  br i1 %2, label %4, label %3

3:
  br label %4

4:
  %5 = phi i8 [ 0, %b1 ], [ 1, %3 ]
  %6 = icmp eq i8 %5, 0
  br i1 %6, label %b4, label %b3

b3:
  call addrspace(1) void @N$PBEG()
  %7 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @N$PI2(i16 %7)
  %8 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %8)
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %10 = load i16, ptr addrspace(1) %9
  call addrspace(1) void @N$PI2(i16 %10)
  %11 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %11)
  %12 = call addrspace(1) ptr @N$PEND()
  ret ptr %12

b4:
  call addrspace(1) void @N$PBEG()
  %13 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @N$PI2(i16 %13)
  %14 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %14)
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %16 = load i16, ptr addrspace(1) %15
  call addrspace(1) void @N$PI2(i16 %16)
  %17 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %17)
  %18 = call addrspace(1) ptr @N$PEND()
  ret ptr %18
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  store i16 -3, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %4, !tbaa !2
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PI2(i16 -3)
  %6 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %6)
  call addrspace(1) void @N$PI2(i16 4)
  %7 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %7)
  %8 = call addrspace(1) ptr @N$PEND()
  call addrspace(1) void @N$PS(ptr %8)
  call addrspace(1) void @N$PN()
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  %9 = load i16, ptr addrspace(1) %5
  %10 = add i16 %9, 5
  %11 = getelementptr i8, ptr addrspace(1) %5, i16 2
  %12 = load i16, ptr addrspace(1) %11
  store i16 %10, ptr %0
  %13 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %12, ptr %13
  %14 = addrspacecast ptr %0 to ptr addrspace(1)
  %15 = load i32, ptr addrspace(1) %14
  %16 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %15, ptr addrspace(1) %16, !tbaa !2
  %17 = load i16, ptr %1, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %1, i16 2
  %19 = load i16, ptr %18, !tbaa !2
  store i16 %17, ptr %2, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %19, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %2 to ptr addrspace(1)
  %22 = icmp slt i16 %17, 0
  br i1 %22, label %24, label %23

23:
  br label %24

24:
  %25 = phi i8 [ 0, %b1 ], [ 1, %23 ]
  %26 = icmp eq i8 %25, 0
  br i1 %26, label %34, label %27

27:
  call addrspace(1) void @N$PBEG()
  %28 = load i16, ptr addrspace(1) %21
  call addrspace(1) void @N$PI2(i16 %28)
  %29 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %29)
  %30 = getelementptr i8, ptr addrspace(1) %21, i16 2
  %31 = load i16, ptr addrspace(1) %30
  call addrspace(1) void @N$PI2(i16 %31)
  %32 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %32)
  %33 = call addrspace(1) ptr @N$PEND()
  br label %41

34:
  call addrspace(1) void @N$PBEG()
  %35 = load i16, ptr addrspace(1) %21
  call addrspace(1) void @N$PI2(i16 %35)
  %36 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %36)
  %37 = getelementptr i8, ptr addrspace(1) %21, i16 2
  %38 = load i16, ptr addrspace(1) %37
  call addrspace(1) void @N$PI2(i16 %38)
  %39 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %39)
  %40 = call addrspace(1) ptr @N$PEND()
  br label %41

41:
  %42 = phi ptr [ %33, %27 ], [ %40, %34 ]
  call addrspace(1) void @N$PS(ptr %42)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %42)
  call addrspace(1) void @N$BDRP(ptr %8)
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PBEG() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
