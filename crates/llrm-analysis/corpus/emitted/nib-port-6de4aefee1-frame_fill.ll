target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [256 x i8]
  %6 = alloca i32
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 256, i1 false)
  store i32 0, ptr %6
  store i32 0, ptr %6, !tbaa !2
  store i16 64, ptr %3, !tbaa !2
  store i16 64, ptr %4, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 64, ptr %1, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp slt i16 %7, %8
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b3, label %b5

b3:
  %12 = load i16, ptr %2, !tbaa !2
  %13 = load i32, ptr %6, !tbaa !2
  %14 = sub i16 %12, 0
  %15 = getelementptr inbounds i32, ptr %5, i16 %14
  store i32 %13, ptr %15, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %2, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %2, !tbaa !2
  br label %b2

b5:
  %18 = icmp ult i16 %0, 64
  %19 = sext i1 %18 to i8
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b6, label %b7

b6:
  %21 = sub i16 %0, 0
  %22 = getelementptr inbounds i32, ptr %5, i16 %21
  store i32 5, ptr %22, !tbaa !2
  %23 = icmp ult i16 %0, 64
  %24 = sext i1 %23 to i8
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %26 = sub i16 %0, 0
  %27 = getelementptr inbounds i32, ptr %5, i16 %26
  %28 = load i32, ptr %27, !tbaa !2
  %29 = add i16 %0, 1
  %30 = icmp ult i16 %29, 64
  %31 = sext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b10, label %b11

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  %33 = sub i16 %29, 0
  %34 = getelementptr inbounds i32, ptr %5, i16 %33
  %35 = load i32, ptr %34, !tbaa !2
  %36 = add i32 %28, %35
  ret i32 %36

b11:
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
