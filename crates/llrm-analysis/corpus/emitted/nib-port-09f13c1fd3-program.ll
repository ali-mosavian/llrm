target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [6 x i8]
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [4 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 4, i1 false)
  store i16 2, ptr %6, !tbaa !2
  store i16 2, ptr %7, !tbaa !2
  %9 = sub i16 0, 0
  %10 = getelementptr inbounds i16, ptr %8, i16 %9
  store i16 5, ptr %10, !tbaa !2
  %11 = sub i16 1, 0
  %12 = getelementptr inbounds i16, ptr %8, i16 %11
  store i16 6, ptr %12, !tbaa !2
  %13 = icmp ne i8 -1, 0
  br i1 %13, label %b2, label %b3

b2:
  %14 = sub i16 0, 0
  %15 = getelementptr inbounds i16, ptr %8, i16 %14
  %16 = load i16, ptr %15, !tbaa !2
  store i16 %16, ptr %5, !tbaa !2
  store i16 3, ptr %2, !tbaa !2
  store i16 3, ptr %3, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  store i16 3, ptr %0, !tbaa !2
  br label %b5

b3:
  br label %b4

b4:
  ret i16 0

b5:
  %17 = load i16, ptr %1, !tbaa !2
  %18 = load i16, ptr %0, !tbaa !2
  %19 = icmp slt i16 %17, %18
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b6, label %b8

b6:
  %22 = load i16, ptr %1, !tbaa !2
  %23 = load i16, ptr %5, !tbaa !2
  %24 = sub i16 %22, 0
  %25 = getelementptr inbounds i16, ptr %4, i16 %24
  store i16 %23, ptr %25, !tbaa !2
  br label %b7

b7:
  %26 = load i16, ptr %1, !tbaa !2
  %27 = add i16 %26, 1
  store i16 %27, ptr %1, !tbaa !2
  br label %b5

b8:
  %28 = sub i16 0, 0
  %29 = getelementptr inbounds i16, ptr %4, i16 %28
  %30 = load i16, ptr %29, !tbaa !2
  %31 = sub i16 2, 0
  %32 = getelementptr inbounds i16, ptr %4, i16 %31
  %33 = load i16, ptr %32, !tbaa !2
  %34 = add i16 %30, %33
  ret i16 %34
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
