target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00area \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00mode \00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00 has \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 columns\00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00text is \00"

define internal i32 @area(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i32
  store i32 0, ptr %1
  %2 = load i8, ptr addrspace(1) %0
  %3 = icmp eq i8 %2, 0
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b4, label %b3

b3:
  %6 = load i8, ptr addrspace(1) %0
  %7 = icmp eq i8 %6, 1
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b6, label %b5

b4:
  ret i32 0

b5:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %11 = load i16, ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %13 = load i16, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %17 = load i16, ptr addrspace(1) %16
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 8
  %19 = load i16, ptr addrspace(1) %18
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 8
  %22 = load i16, ptr addrspace(1) %20
  %23 = load i16, ptr addrspace(1) %14
  %24 = sub i16 %22, %23
  %25 = sext i16 %24 to i32
  %26 = load i16, ptr addrspace(1) %21
  %27 = load i16, ptr addrspace(1) %15
  %28 = sub i16 %26, %27
  %29 = sext i16 %28 to i32
  %30 = mul i32 %25, %29
  ret i32 %30

b6:
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %32 = load i16, ptr addrspace(1) %31
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %34 = load i16, ptr addrspace(1) %33
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %36 = load i16, ptr addrspace(1) %35
  %37 = zext i16 %36 to i32
  store i32 %37, ptr %1, !tbaa !2
  %38 = load i32, ptr %1, !tbaa !2
  %39 = mul i32 3, %38
  %40 = load i32, ptr %1, !tbaa !2
  %41 = mul i32 %39, %40
  ret i32 %41
}

define internal i16 @columns(i8 %0) addrspace(1) {
b1:
  %1 = icmp eq i8 %0, 0
  %2 = sext i1 %1 to i8
  %3 = icmp ne i8 %2, 0
  br i1 %3, label %b4, label %b3

b3:
  %4 = icmp eq i8 %0, 1
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b6, label %b5

b4:
  ret i16 80

b5:
  ret i16 320

b6:
  ret i16 40
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i8
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [30 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i8 0, ptr %2
  store i8 0, ptr %3
  store i16 0, ptr %4
  store i32 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 30, i1 false)
  store i16 3, ptr %6, !tbaa !2
  store i16 3, ptr %7, !tbaa !2
  %9 = sub i16 0, 0
  %10 = getelementptr inbounds [10 x i8], ptr %8, i16 %9
  store i8 0, ptr %10, !tbaa !2
  %11 = sub i16 0, 0
  %12 = getelementptr inbounds [10 x i8], ptr %8, i16 %11
  %13 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 1, ptr %13, !tbaa !2
  %14 = sub i16 0, 0
  %15 = getelementptr inbounds [10 x i8], ptr %8, i16 %14
  %16 = getelementptr inbounds i8, ptr %15, i16 4
  store i16 2, ptr %16, !tbaa !2
  %17 = sub i16 1, 0
  %18 = getelementptr inbounds [10 x i8], ptr %8, i16 %17
  store i8 1, ptr %18, !tbaa !2
  %19 = sub i16 1, 0
  %20 = getelementptr inbounds [10 x i8], ptr %8, i16 %19
  %21 = getelementptr inbounds i8, ptr %20, i16 2
  store i16 0, ptr %21, !tbaa !2
  %22 = sub i16 1, 0
  %23 = getelementptr inbounds [10 x i8], ptr %8, i16 %22
  %24 = getelementptr inbounds i8, ptr %23, i16 4
  store i16 0, ptr %24, !tbaa !2
  %25 = sub i16 1, 0
  %26 = getelementptr inbounds [10 x i8], ptr %8, i16 %25
  %27 = getelementptr inbounds i8, ptr %26, i16 6
  store i16 5, ptr %27, !tbaa !2
  %28 = sub i16 2, 0
  %29 = getelementptr inbounds [10 x i8], ptr %8, i16 %28
  store i8 2, ptr %29, !tbaa !2
  %30 = sub i16 2, 0
  %31 = getelementptr inbounds [10 x i8], ptr %8, i16 %30
  %32 = getelementptr inbounds i8, ptr %31, i16 2
  store i16 1, ptr %32, !tbaa !2
  %33 = sub i16 2, 0
  %34 = getelementptr inbounds [10 x i8], ptr %8, i16 %33
  %35 = getelementptr inbounds i8, ptr %34, i16 4
  store i16 1, ptr %35, !tbaa !2
  %36 = sub i16 2, 0
  %37 = getelementptr inbounds [10 x i8], ptr %8, i16 %36
  %38 = getelementptr inbounds i8, ptr %37, i16 6
  store i16 4, ptr %38, !tbaa !2
  %39 = sub i16 2, 0
  %40 = getelementptr inbounds [10 x i8], ptr %8, i16 %39
  %41 = getelementptr inbounds i8, ptr %40, i16 8
  store i16 5, ptr %41, !tbaa !2
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  br label %b2

b2:
  %42 = load i16, ptr %4, !tbaa !2
  %43 = icmp ult i16 %42, 3
  %44 = sext i1 %43 to i8
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %b3, label %b5

b3:
  %46 = load i32, ptr %5, !tbaa !2
  %47 = sub i16 %42, 0
  %48 = getelementptr inbounds [10 x i8], ptr %8, i16 %47
  %49 = addrspacecast ptr %48 to ptr addrspace(1)
  %50 = call addrspace(1) i32 @area(ptr addrspace(1) %49)
  %51 = add i32 %46, %50
  store i32 %51, ptr %5, !tbaa !2
  br label %b4

b4:
  %52 = load i16, ptr %4, !tbaa !2
  %53 = add i16 %52, 1
  store i16 %53, ptr %4, !tbaa !2
  br label %b2

b5:
  %54 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %54)
  %55 = load i32, ptr %5, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %55)
  call addrspace(1) void @N$PN()
  store i8 19, ptr %3, !tbaa !2
  %56 = load i8, ptr %3, !tbaa !2
  store i8 %56, ptr %2, !tbaa !2
  %57 = load i8, ptr %3, !tbaa !2
  %58 = call addrspace(1) i16 @columns(i8 %57)
  store i16 %58, ptr %1, !tbaa !2
  %59 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %59)
  %60 = load i8, ptr %2, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %60)
  %61 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %61)
  %62 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %62)
  %63 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %63)
  call addrspace(1) void @N$PN()
  %64 = call addrspace(1) i16 @columns(i8 0)
  store i16 %64, ptr %0, !tbaa !2
  %65 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %65)
  %66 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %66)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PU1(i8) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
