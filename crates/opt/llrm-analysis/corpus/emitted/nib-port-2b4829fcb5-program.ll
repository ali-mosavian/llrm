target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i32
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [20 x i8]
  %7 = alloca i32
  store i16 0, ptr %0
  store i32 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 20, i1 false)
  store i32 0, ptr %7
  store i32 7, ptr %7, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  store i16 5, ptr %5, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 5, ptr %2, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp slt i16 %8, %9
  %11 = sext i1 %10 to i8
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i16, ptr %3, !tbaa !2
  %14 = load i32, ptr %7, !tbaa !2
  %15 = sub i16 %13, 0
  %16 = getelementptr inbounds i32, ptr %6, i16 %15
  store i32 %14, ptr %16, !tbaa !2
  br label %b4

b4:
  %17 = load i16, ptr %3, !tbaa !2
  %18 = add i16 %17, 1
  store i16 %18, ptr %3, !tbaa !2
  br label %b2

b5:
  %19 = sub i16 2, 0
  %20 = getelementptr inbounds i32, ptr %6, i16 %19
  store i32 1, ptr %20, !tbaa !2
  store i32 0, ptr %1, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  br label %b6

b6:
  %21 = load i16, ptr %0, !tbaa !2
  %22 = icmp ult i16 %21, 5
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b7, label %b9

b7:
  %25 = load i32, ptr %1, !tbaa !2
  %26 = sub i16 %21, 0
  %27 = getelementptr inbounds i32, ptr %6, i16 %26
  %28 = load i32, ptr %27, !tbaa !2
  %29 = add i32 %25, %28
  store i32 %29, ptr %1, !tbaa !2
  br label %b8

b8:
  %30 = load i16, ptr %0, !tbaa !2
  %31 = add i16 %30, 1
  store i16 %31, ptr %0, !tbaa !2
  br label %b6

b9:
  %32 = load i32, ptr %1, !tbaa !2
  ret i32 %32
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
