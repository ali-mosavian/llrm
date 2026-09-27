target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00area \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00mode \00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00 has \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 columns\00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00text is \00"

define internal i32 @area(ptr addrspace(1) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = load i8, ptr addrspace(1) %0
  %2 = icmp eq i8 %1, 0
  br i1 %2, label %b4, label %b3

b3:
  %3 = icmp eq i8 %1, 1
  br i1 %3, label %b6, label %b5

b4:
  ret i32 0

b5:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 8
  %8 = load i16, ptr addrspace(1) %6
  %9 = load i16, ptr addrspace(1) %4
  %10 = sub i16 %8, %9
  %11 = sext i16 %10 to i32
  %12 = load i16, ptr addrspace(1) %7
  %13 = load i16, ptr addrspace(1) %5
  %14 = sub i16 %12, %13
  %15 = sext i16 %14 to i32
  %16 = mul i32 %11, %15
  ret i32 %16

b6:
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %18 = load i16, ptr addrspace(1) %17
  %19 = zext i16 %18 to i32
  %20 = mul i32 %19, 3
  %21 = mul i32 %20, %19
  ret i32 %21
}

define internal i16 @columns(i8 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = icmp eq i8 %0, 0
  br i1 %1, label %b4, label %b3

b3:
  %2 = icmp eq i8 %0, 1
  br i1 %2, label %b6, label %b5

b4:
  ret i16 80

b5:
  ret i16 320

b6:
  ret i16 40
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [30 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 30, i1 false)
  %1 = getelementptr inbounds [10 x i8], ptr %0, i16 0
  store i8 0, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 1, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 2, ptr %3, !tbaa !2
  %4 = getelementptr inbounds [10 x i8], ptr %0, i16 1
  store i8 1, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 0, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 0, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %4, i16 6
  store i16 5, ptr %7, !tbaa !2
  %8 = getelementptr inbounds [10 x i8], ptr %0, i16 2
  store i8 2, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 1, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %8, i16 4
  store i16 1, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %8, i16 6
  store i16 4, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %8, i16 8
  store i16 5, ptr %12, !tbaa !2
  br label %b2

b2:
  %13 = phi i32 [ 0, %b1 ], [ %45, %43 ]
  %14 = phi i16 [ 0, %b1 ], [ %46, %43 ]
  %15 = icmp ult i16 %14, 3
  br i1 %15, label %b3, label %b5

b3:
  %16 = getelementptr inbounds [10 x i8], ptr %0, i16 %14
  %17 = addrspacecast ptr %16 to ptr addrspace(1)
  %18 = load i8, ptr addrspace(1) %17
  %19 = icmp eq i8 %18, 0
  br i1 %19, label %43, label %20

20:
  %21 = load i8, ptr addrspace(1) %17
  %22 = icmp eq i8 %21, 1
  br i1 %22, label %37, label %23

23:
  %24 = getelementptr i8, ptr addrspace(1) %17, i16 2
  %25 = getelementptr i8, ptr addrspace(1) %17, i16 4
  %26 = getelementptr i8, ptr addrspace(1) %17, i16 6
  %27 = getelementptr i8, ptr addrspace(1) %17, i16 8
  %28 = load i16, ptr addrspace(1) %26
  %29 = load i16, ptr addrspace(1) %24
  %30 = sub i16 %28, %29
  %31 = sext i16 %30 to i32
  %32 = load i16, ptr addrspace(1) %27
  %33 = load i16, ptr addrspace(1) %25
  %34 = sub i16 %32, %33
  %35 = sext i16 %34 to i32
  %36 = mul i32 %31, %35
  br label %43

37:
  %38 = getelementptr i8, ptr addrspace(1) %17, i16 6
  %39 = load i16, ptr addrspace(1) %38
  %40 = zext i16 %39 to i32
  %41 = mul i32 %40, 3
  %42 = mul i32 %41, %40
  br label %43

43:
  %44 = phi i32 [ 0, %b3 ], [ %36, %23 ], [ %42, %37 ]
  %45 = add i32 %13, %44
  %46 = add i16 %14, 1
  br label %b2

b5:
  %47 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %47)
  call addrspace(1) void @N$PI4(i32 %13)
  call addrspace(1) void @N$PN()
  %48 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  call addrspace(1) void @N$PU1(i8 19)
  %49 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %49)
  call addrspace(1) void @N$PU2(i16 320)
  %50 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %50)
  call addrspace(1) void @N$PN()
  %51 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %51)
  call addrspace(1) void @N$PU2(i16 80)
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
