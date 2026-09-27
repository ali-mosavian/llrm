target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00= \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00level\00"
@$str3 = internal constant [31 x i8] c"\08\00\18\00\18\00          twice that is \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00name=Ada;role=pilot;level=7\00"
@$str5 = internal constant [23 x i8] c"\08\00\10\00\10\00 settings, kept \00"

define internal i16 @digits(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %12, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %13, %b3 ]
  %6 = icmp ult i16 %5, %1
  br i1 %6, label %b3, label %b5

b3:
  %7 = getelementptr i8, ptr addrspace(1) %3, i16 %5
  %8 = mul i16 %4, 10
  %9 = load i8, ptr addrspace(1) %7
  %10 = zext i8 %9 to i16
  %11 = add i16 %8, %10
  %12 = add i16 %11, -48
  %13 = add i16 %5, 1
  br label %b2

b5:
  ret i16 %4
}

define internal void @show(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call addrspace(1) void @N$PFLD(i8 8, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %3 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %3)
  call addrspace(1) void @N$PV(ptr addrspace(1) %1)
  call addrspace(1) void @N$PN()
  %4 = getelementptr i8, ptr @$str2, i16 6
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  %7 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 %6, ptr %2, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %6, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %7, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %2 to ptr addrspace(1)
  %11 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %0, ptr addrspace(1) %10)
  %12 = icmp eq i8 %11, 0
  br i1 %12, label %b2, label %b4

b2:
  %13 = load i16, ptr addrspace(1) %1
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %15 = load ptr addrspace(1), ptr addrspace(1) %14
  br label %16

16:
  %17 = phi i16 [ 0, %b2 ], [ %26, %20 ]
  %18 = phi i16 [ 0, %b2 ], [ %27, %20 ]
  %19 = icmp ult i16 %18, %13
  br i1 %19, label %20, label %28

20:
  %21 = getelementptr i8, ptr addrspace(1) %15, i16 %18
  %22 = mul i16 %17, 10
  %23 = load i8, ptr addrspace(1) %21
  %24 = zext i8 %23 to i16
  %25 = add i16 %22, %24
  %26 = add i16 %25, -48
  %27 = add i16 %18, 1
  br label %16

28:
  %29 = shl i16 %17, 1
  %30 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %30)
  call addrspace(1) void @N$PI2(i16 %29)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  ret void
}

define internal ptr @keep(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %0)
  ret ptr %1
}

define internal i16 @each_setting(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %3 = load i16, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = getelementptr inbounds i8, ptr %2, i16 2
  %7 = getelementptr inbounds i8, ptr %2, i16 4
  %8 = addrspacecast ptr %2 to ptr addrspace(1)
  %9 = getelementptr inbounds i8, ptr %1, i16 2
  %10 = getelementptr inbounds i8, ptr %1, i16 4
  %11 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %b2

b2:
  %12 = phi i16 [ 0, %b1 ], [ %25, %b11 ]
  %13 = phi i16 [ 0, %b1 ], [ %26, %b11 ]
  %14 = phi i16 [ 0, %b1 ], [ %27, %b11 ]
  %15 = icmp ule i16 %14, %3
  br i1 %15, label %b3, label %b4

b3:
  %16 = icmp eq i16 %14, %3
  %17 = sext i1 %16 to i8
  br i1 %16, label %b6, label %b5

b4:
  ret i16 %12

b5:
  %18 = icmp ult i16 %14, %3
  br i1 %18, label %b7, label %b8

b6:
  %19 = phi i8 [ %17, %b3 ], [ %24, %b7 ]
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b9, label %b11

b7:
  %21 = getelementptr i8, ptr addrspace(1) %5, i16 %14
  %22 = load i8, ptr addrspace(1) %21
  %23 = icmp eq i8 %22, 59
  %24 = sext i1 %23 to i8
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  br label %b12

b11:
  %25 = phi i16 [ %12, %b6 ], [ %47, %b25 ]
  %26 = phi i16 [ %13, %b6 ], [ %48, %b25 ]
  %27 = add i16 %14, 1
  br label %b2

b12:
  %28 = phi i16 [ %13, %b9 ], [ %31, %b13 ]
  %29 = icmp ult i16 %28, %14
  %30 = sext i1 %29 to i8
  br i1 %29, label %b15, label %b16

b13:
  %31 = add i16 %28, 1
  br label %b12

b14:
  %32 = icmp ule i16 %28, %3
  br i1 %32, label %b19, label %b20

b15:
  %33 = icmp ult i16 %28, %3
  br i1 %33, label %b17, label %b18

b16:
  %34 = phi i8 [ %30, %b12 ], [ %39, %b17 ]
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b13, label %b14

b17:
  %36 = getelementptr i8, ptr addrspace(1) %5, i16 %28
  %37 = load i8, ptr addrspace(1) %36
  %38 = icmp ne i8 %37, 61
  %39 = sext i1 %38 to i8
  br label %b16

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %40 = icmp ule i16 %13, %28
  br i1 %40, label %b21, label %b22

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  %41 = getelementptr i8, ptr addrspace(1) %5, i16 %13
  %42 = sub i16 %28, %13
  store i16 %42, ptr %2, !tbaa !2
  store i16 %42, ptr %6, !tbaa !2
  store ptr addrspace(1) %41, ptr %7, !tbaa !2
  %43 = add i16 %28, 1
  %44 = icmp ule i16 %43, %14
  br i1 %44, label %b25, label %b26

b22:
  call addrspace(1) void @N$EBND()
  unreachable

b25:
  %45 = getelementptr i8, ptr addrspace(1) %5, i16 %43
  %46 = sub i16 %14, %43
  store i16 %46, ptr %1, !tbaa !2
  store i16 %46, ptr %9, !tbaa !2
  store ptr addrspace(1) %45, ptr %10, !tbaa !2
  call addrspace(1) void @show(ptr addrspace(1) %8, ptr addrspace(1) %11)
  %47 = add i16 %12, 1
  %48 = add i16 %14, 1
  br label %b11

b26:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %2 = getelementptr i8, ptr @$str4, i16 6
  %3 = getelementptr i8, ptr %2, i16 -4
  %4 = load i16, ptr %3
  %5 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 %4, ptr %1, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %4, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %5, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  %9 = call addrspace(1) i16 @each_setting(ptr addrspace(1) %8)
  %10 = load i16, ptr %3
  %11 = icmp uge i16 %10, 8
  br i1 %11, label %b2, label %b3

b2:
  %12 = getelementptr i8, ptr addrspace(1) %5, i16 5
  store i16 3, ptr %0, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %0 to ptr addrspace(1)
  %16 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %15)
  call addrspace(1) void @N$PU2(i16 %9)
  %17 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %17)
  call addrspace(1) void @N$PS(ptr %16)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %16)
  call addrspace(1) void @N$BDRP(ptr %2)
  ret i16 0

b3:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
