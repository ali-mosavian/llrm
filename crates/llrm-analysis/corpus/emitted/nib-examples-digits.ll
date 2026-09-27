target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str2 = internal constant [18 x i8] c"\08\00\0B\00\0B\00not a digit\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00too big\00"
@$str4 = internal constant [11 x i8] c"\08\00\04\00\04\001234\00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\0012x4\00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\0099999\00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00first even \00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00no even value\00"

define internal void @digit(ptr addrspace(1) %0, i8 %1) addrspace(1) {
b1:
  %2 = alloca i8
  store i8 0, ptr %2
  %3 = icmp ult i8 %1, 48
  %4 = sext i1 %3 to i8
  store i8 %4, ptr %2, !tbaa !2
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b3, label %b2

b2:
  %6 = icmp ugt i8 %1, 57
  %7 = sext i1 %6 to i8
  store i8 %7, ptr %2, !tbaa !2
  br label %b3

b3:
  %8 = load i8, ptr %2, !tbaa !2
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b4, label %b5

b4:
  store i8 1, ptr addrspace(1) %0
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 1, ptr addrspace(1) %10
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr addrspace(1) %11
  ret void

b5:
  br label %b6

b6:
  %12 = zext i8 %1 to i16
  %13 = zext i8 48 to i16
  %14 = sub i16 %12, %13
  %15 = trunc i16 %14 to i8
  store i8 0, ptr addrspace(1) %0
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 %15, ptr addrspace(1) %16
  ret void
}

define internal void @parse(ptr addrspace(1) %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i32
  %3 = alloca [6 x i8]
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca ptr
  store i32 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store i16 0, ptr %4
  store i32 0, ptr %5
  store ptr null, ptr %6
  store ptr %1, ptr %6, !tbaa !2
  %7 = load ptr, ptr %6, !tbaa !2
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = icmp eq i16 %9, 0
  %11 = sext i1 %10 to i8
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %13
  %14 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %14)
  ret void

b3:
  br label %b4

b4:
  store i32 0, ptr %5, !tbaa !2
  %15 = load ptr, ptr %6, !tbaa !2
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  store i16 0, ptr %4, !tbaa !2
  br label %b5

b5:
  %18 = load i16, ptr %4, !tbaa !2
  %19 = icmp ult i16 %18, %17
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b6, label %b8

b6:
  %22 = getelementptr i8, ptr %15, i16 %18
  %23 = load i32, ptr %5, !tbaa !2
  %24 = mul i32 %23, 10
  %25 = addrspacecast ptr %3 to ptr addrspace(1)
  %26 = load i8, ptr %22
  call addrspace(1) void @digit(ptr addrspace(1) %25, i8 %26)
  %27 = load i8, ptr %3, !tbaa !2
  %28 = icmp eq i8 %27, 1
  %29 = sext i1 %28 to i8
  %30 = icmp ne i8 %29, 0
  br i1 %30, label %b9, label %b10

b7:
  %31 = load i16, ptr %4, !tbaa !2
  %32 = add i16 %31, 1
  store i16 %32, ptr %4, !tbaa !2
  br label %b5

b8:
  %33 = load i32, ptr %5, !tbaa !2
  %34 = trunc i32 %33 to i16
  store i8 0, ptr addrspace(1) %0
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %34, ptr addrspace(1) %35
  %36 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %36)
  ret void

b9:
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 4
  %40 = load i16, ptr %39, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %38, ptr addrspace(1) %41
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %40, ptr addrspace(1) %42
  %43 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %43)
  ret void

b10:
  %44 = getelementptr inbounds i8, ptr %3, i16 2
  %45 = load i8, ptr %44, !tbaa !2
  %46 = zext i8 %45 to i32
  %47 = add i32 %24, %46
  store i32 %47, ptr %2, !tbaa !2
  %48 = load i32, ptr %2, !tbaa !2
  store i32 %48, ptr %5, !tbaa !2
  %49 = load i32, ptr %5, !tbaa !2
  %50 = icmp ugt i32 %49, 65535
  %51 = sext i1 %50 to i8
  %52 = icmp ne i8 %51, 0
  br i1 %52, label %b11, label %b12

b11:
  store i8 1, ptr addrspace(1) %0
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %53
  %54 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %54)
  ret void

b12:
  br label %b13

b13:
  br label %b7
}

define internal i32 @first_even(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca [4 x i8]
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
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
  %12 = load i16, ptr addrspace(1) %11
  %13 = srem i16 %12, 2
  %14 = icmp eq i16 %13, 0
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b6, label %b7

b4:
  %17 = load i16, ptr %1, !tbaa !2
  %18 = add i16 %17, 1
  store i16 %18, ptr %1, !tbaa !2
  br label %b2

b5:
  store i8 1, ptr %2, !tbaa !2
  %19 = addrspacecast ptr %2 to ptr addrspace(1)
  %20 = load i32, ptr addrspace(1) %19, !tbaa !2
  ret i32 %20

b6:
  %21 = load i16, ptr addrspace(1) %11
  store i8 0, ptr %2, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %21, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %2 to ptr addrspace(1)
  %24 = load i32, ptr addrspace(1) %23, !tbaa !2
  ret i32 %24

b7:
  br label %b8

b8:
  br label %b4
}

define internal void @report(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca [6 x i8]
  %3 = alloca ptr
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  store ptr null, ptr %3
  store ptr %0, ptr %3, !tbaa !2
  %4 = addrspacecast ptr %2 to ptr addrspace(1)
  %5 = load ptr, ptr %3, !tbaa !2
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @parse(ptr addrspace(1) %4, ptr %5)
  %6 = load i8, ptr %2, !tbaa !2
  %7 = icmp eq i8 %6, 0
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b4, label %b3

b2:
  %10 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %10)
  ret void

b3:
  %11 = load i8, ptr %2, !tbaa !2
  %12 = icmp eq i8 %11, 1
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b6, label %b5

b4:
  %15 = getelementptr inbounds i8, ptr %2, i16 2
  %16 = load i16, ptr %15, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %2, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  store i16 %18, ptr %1, !tbaa !2
  %19 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %19)
  call addrspace(1) void @N$PN()
  br label %b2

b5:
  %20 = load i8, ptr %2, !tbaa !2
  %21 = icmp eq i8 %20, 1
  %22 = sext i1 %21 to i8
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b9, label %b8

b6:
  %24 = getelementptr inbounds i8, ptr %2, i16 2
  %25 = load i8, ptr %24, !tbaa !2
  %26 = icmp eq i8 %25, 0
  %27 = sext i1 %26 to i8
  %28 = icmp ne i8 %27, 0
  br i1 %28, label %b7, label %b5

b7:
  %29 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %29)
  call addrspace(1) void @N$PN()
  br label %b2

b8:
  %30 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %30)
  call addrspace(1) void @N$PN()
  br label %b2

b9:
  %31 = getelementptr inbounds i8, ptr %2, i16 2
  %32 = load i8, ptr %31, !tbaa !2
  %33 = icmp eq i8 %32, 1
  %34 = sext i1 %33 to i8
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b10, label %b8

b10:
  %36 = getelementptr inbounds i8, ptr %2, i16 4
  %37 = load i16, ptr %36, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %2, i16 4
  %39 = load i16, ptr %38, !tbaa !2
  %40 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %40)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [8 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  %6 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @report(ptr %6)
  %7 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @report(ptr %7)
  %8 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @report(ptr %8)
  %9 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @report(ptr %9)
  store i16 4, ptr %3, !tbaa !2
  store i16 4, ptr %4, !tbaa !2
  %10 = sub i16 0, 0
  %11 = getelementptr inbounds i16, ptr %5, i16 %10
  store i16 3, ptr %11, !tbaa !2
  %12 = sub i16 1, 0
  %13 = getelementptr inbounds i16, ptr %5, i16 %12
  store i16 7, ptr %13, !tbaa !2
  %14 = sub i16 2, 0
  %15 = getelementptr inbounds i16, ptr %5, i16 %14
  store i16 8, ptr %15, !tbaa !2
  %16 = sub i16 3, 0
  %17 = getelementptr inbounds i16, ptr %5, i16 %16
  store i16 9, ptr %17, !tbaa !2
  %18 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(1)
  %22 = call addrspace(1) i32 @first_even(ptr addrspace(1) %21)
  %23 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 %22, ptr addrspace(1) %23, !tbaa !2
  %24 = load i8, ptr %2, !tbaa !2
  %25 = icmp eq i8 %24, 0
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b4, label %b3

b2:
  ret i16 0

b3:
  %28 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %28)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %29 = getelementptr inbounds i8, ptr %2, i16 2
  %30 = load i16, ptr %29, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %2, i16 2
  %32 = load i16, ptr %31, !tbaa !2
  store i16 %32, ptr %0, !tbaa !2
  %33 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %33)
  %34 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %34)
  call addrspace(1) void @N$PN()
  br label %b2
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
