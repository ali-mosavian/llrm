target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @update() addrspace(1) memory(none) willreturn {
b1:
  %0 = alloca [50 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 50, i1 false)
  %1 = getelementptr inbounds [10 x i8], ptr %0, i16 0
  store i16 0, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 2
  store i32 1, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i8, ptr %1, i16 6
  store i32 2, ptr %3, !tbaa !2
  %4 = getelementptr inbounds [10 x i8], ptr %0, i16 1
  store i16 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %4, i16 2
  store i32 2, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %4, i16 6
  store i32 3, ptr %6, !tbaa !2
  %7 = getelementptr inbounds [10 x i8], ptr %0, i16 2
  store i16 0, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %7, i16 2
  store i32 3, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %7, i16 6
  store i32 4, ptr %9, !tbaa !2
  %10 = getelementptr inbounds [10 x i8], ptr %0, i16 3
  store i16 0, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %10, i16 2
  store i32 4, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %10, i16 6
  store i32 5, ptr %12, !tbaa !2
  %13 = getelementptr inbounds [10 x i8], ptr %0, i16 4
  store i16 0, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %13, i16 2
  store i32 5, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %13, i16 6
  store i32 6, ptr %15, !tbaa !2
  br label %b2

b2:
  %16 = phi i16 [ 0, %b1 ], [ %24, %b3 ]
  %17 = icmp ult i16 %16, 5
  br i1 %17, label %b3, label %b5

b3:
  %18 = getelementptr inbounds [10 x i8], ptr %0, i16 %16
  %19 = getelementptr inbounds i8, ptr %18, i16 2
  %20 = load i32, ptr %19, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %18, i16 6
  %22 = load i32, ptr %21, !tbaa !2
  %23 = add i32 %20, %22
  store i32 %23, ptr %19, !tbaa !2
  %24 = add i16 %16, 1
  br label %b2

b5:
  %25 = load i32, ptr %2, !tbaa !2
  %26 = load i32, ptr %14, !tbaa !2
  %27 = add i32 %25, %26
  ret i32 %27
}

define internal i16 @main() addrspace(1) memory(none) willreturn {
b1:
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
