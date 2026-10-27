target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @calculate() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca [32 x i8]
  store i16 2, ptr %0, !tbaa !2
  store i16 2, ptr %1, !tbaa !2
  %3 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  store i32 1, ptr %3, !tbaa !2
  %4 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  %5 = getelementptr inbounds i8, ptr %4, i16 4
  store i32 2, ptr %5, !tbaa !2
  %6 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  %7 = getelementptr inbounds i8, ptr %6, i16 8
  store i32 3, ptr %7, !tbaa !2
  %8 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  %9 = getelementptr inbounds i8, ptr %8, i16 12
  store i32 4, ptr %9, !tbaa !2
  %10 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  store i32 5, ptr %10, !tbaa !2
  %11 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  %12 = getelementptr inbounds i8, ptr %11, i16 4
  store i32 6, ptr %12, !tbaa !2
  %13 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  %14 = getelementptr inbounds i8, ptr %13, i16 8
  store i32 7, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  %16 = getelementptr inbounds i8, ptr %15, i16 12
  store i32 8, ptr %16, !tbaa !2
  %17 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  %18 = load i32, ptr %17, !tbaa !2
  %19 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  %20 = getelementptr inbounds i8, ptr %19, i16 4
  %21 = load i32, ptr %20, !tbaa !2
  %22 = mul i32 %21, 10
  %23 = add i32 %18, %22
  %24 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  %25 = getelementptr inbounds i8, ptr %24, i16 8
  %26 = load i32, ptr %25, !tbaa !2
  %27 = mul i32 %26, 100
  %28 = add i32 %23, %27
  %29 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  %30 = getelementptr inbounds i8, ptr %29, i16 12
  %31 = load i32, ptr %30, !tbaa !2
  %32 = mul i32 %31, 1000
  %33 = add i32 %28, %32
  %34 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  %35 = load i32, ptr %34, !tbaa !2
  %36 = mul i32 %35, 10000
  %37 = add i32 %33, %36
  %38 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  %39 = getelementptr inbounds i8, ptr %38, i16 4
  %40 = load i32, ptr %39, !tbaa !2
  %41 = mul i32 %40, 100000
  %42 = add i32 %37, %41
  %43 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  %44 = getelementptr inbounds i8, ptr %43, i16 8
  %45 = load i32, ptr %44, !tbaa !2
  %46 = mul i32 %45, 1000000
  %47 = add i32 %42, %46
  %48 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  %49 = getelementptr inbounds i8, ptr %48, i16 12
  %50 = load i32, ptr %49, !tbaa !2
  %51 = mul i32 %50, 10000000
  %52 = add i32 %47, %51
  ret i32 %52
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
