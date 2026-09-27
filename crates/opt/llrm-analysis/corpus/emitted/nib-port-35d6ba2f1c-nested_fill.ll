target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [256 x i8]
  %9 = alloca i32
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 256, i1 false)
  store i32 0, ptr %9
  store i32 0, ptr %9, !tbaa !2
  store i16 8, ptr %5, !tbaa !2
  store i16 8, ptr %6, !tbaa !2
  store i16 64, ptr %7, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 8, ptr %3, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = icmp slt i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b3, label %b5

b3:
  store i16 0, ptr %2, !tbaa !2
  store i16 8, ptr %1, !tbaa !2
  br label %b6

b4:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %4, !tbaa !2
  br label %b2

b5:
  %17 = icmp ult i16 %0, 8
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b10, label %b11

b6:
  %20 = load i16, ptr %2, !tbaa !2
  %21 = load i16, ptr %1, !tbaa !2
  %22 = icmp slt i16 %20, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b7, label %b9

b7:
  %25 = load i16, ptr %4, !tbaa !2
  %26 = load i16, ptr %2, !tbaa !2
  %27 = load i32, ptr %9, !tbaa !2
  %28 = sub i16 %25, 0
  %29 = sub i16 %26, 0
  %30 = mul i16 %28, 8
  %31 = add i16 %30, %29
  %32 = getelementptr inbounds i32, ptr %8, i16 %31
  store i32 %27, ptr %32, !tbaa !2
  br label %b8

b8:
  %33 = load i16, ptr %2, !tbaa !2
  %34 = add i16 %33, 1
  store i16 %34, ptr %2, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %35 = sub i16 %0, 0
  %36 = sub i16 1, 0
  %37 = mul i16 %35, 8
  %38 = add i16 %37, %36
  %39 = getelementptr inbounds i32, ptr %8, i16 %38
  store i32 5, ptr %39, !tbaa !2
  %40 = icmp ult i16 %0, 8
  %41 = sext i1 %40 to i8
  %42 = icmp ne i8 %41, 0
  br i1 %42, label %b12, label %b13

b11:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  %43 = sub i16 %0, 0
  %44 = sub i16 2, 0
  %45 = mul i16 %43, 8
  %46 = add i16 %45, %44
  %47 = getelementptr inbounds i32, ptr %8, i16 %46
  %48 = load i32, ptr %47, !tbaa !2
  ret i32 %48

b13:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @value(i16 3)
  %1 = trunc i32 %0 to i16
  ret i16 %1
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
