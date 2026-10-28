target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @update() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [50 x i8]
  store i16 5, ptr %1, !tbaa !2
  store i16 5, ptr %2, !tbaa !2
  %4 = getelementptr inbounds [10 x i8], ptr %3, i16 0
  store i16 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds [10 x i8], ptr %3, i16 0
  %6 = getelementptr inbounds i8, ptr %5, i16 2
  store i32 1, ptr %6, !tbaa !2
  %7 = getelementptr inbounds [10 x i8], ptr %3, i16 0
  %8 = getelementptr inbounds i8, ptr %7, i16 6
  store i32 2, ptr %8, !tbaa !2
  %9 = getelementptr inbounds [10 x i8], ptr %3, i16 1
  store i16 0, ptr %9, !tbaa !2
  %10 = getelementptr inbounds [10 x i8], ptr %3, i16 1
  %11 = getelementptr inbounds i8, ptr %10, i16 2
  store i32 2, ptr %11, !tbaa !2
  %12 = getelementptr inbounds [10 x i8], ptr %3, i16 1
  %13 = getelementptr inbounds i8, ptr %12, i16 6
  store i32 3, ptr %13, !tbaa !2
  %14 = getelementptr inbounds [10 x i8], ptr %3, i16 2
  store i16 0, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [10 x i8], ptr %3, i16 2
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i32 3, ptr %16, !tbaa !2
  %17 = getelementptr inbounds [10 x i8], ptr %3, i16 2
  %18 = getelementptr inbounds i8, ptr %17, i16 6
  store i32 4, ptr %18, !tbaa !2
  %19 = getelementptr inbounds [10 x i8], ptr %3, i16 3
  store i16 0, ptr %19, !tbaa !2
  %20 = getelementptr inbounds [10 x i8], ptr %3, i16 3
  %21 = getelementptr inbounds i8, ptr %20, i16 2
  store i32 4, ptr %21, !tbaa !2
  %22 = getelementptr inbounds [10 x i8], ptr %3, i16 3
  %23 = getelementptr inbounds i8, ptr %22, i16 6
  store i32 5, ptr %23, !tbaa !2
  %24 = getelementptr inbounds [10 x i8], ptr %3, i16 4
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds [10 x i8], ptr %3, i16 4
  %26 = getelementptr inbounds i8, ptr %25, i16 2
  store i32 5, ptr %26, !tbaa !2
  %27 = getelementptr inbounds [10 x i8], ptr %3, i16 4
  %28 = getelementptr inbounds i8, ptr %27, i16 6
  store i32 6, ptr %28, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  br label %b2

b2:
  %29 = load i16, ptr %0, !tbaa !2
  %30 = icmp ult i16 %29, 5
  br i1 %30, label %b3, label %b5

b3:
  %31 = sub i16 %29, 0
  %32 = getelementptr inbounds [10 x i8], ptr %3, i16 %31
  %33 = getelementptr inbounds i8, ptr %32, i16 2
  %34 = load i32, ptr %33, !tbaa !2
  %35 = sub i16 %29, 0
  %36 = getelementptr inbounds [10 x i8], ptr %3, i16 %35
  %37 = getelementptr inbounds i8, ptr %36, i16 6
  %38 = load i32, ptr %37, !tbaa !2
  %39 = add i32 %34, %38
  %40 = sub i16 %29, 0
  %41 = getelementptr inbounds [10 x i8], ptr %3, i16 %40
  %42 = getelementptr inbounds i8, ptr %41, i16 2
  store i32 %39, ptr %42, !tbaa !2
  br label %b4

b4:
  %43 = load i16, ptr %0, !tbaa !2
  %44 = add i16 %43, 1
  store i16 %44, ptr %0, !tbaa !2
  br label %b2

b5:
  %45 = getelementptr inbounds [10 x i8], ptr %3, i16 0
  %46 = getelementptr inbounds i8, ptr %45, i16 2
  %47 = load i32, ptr %46, !tbaa !2
  %48 = getelementptr inbounds [10 x i8], ptr %3, i16 4
  %49 = getelementptr inbounds i8, ptr %48, i16 2
  %50 = load i32, ptr %49, !tbaa !2
  %51 = add i32 %47, %50
  ret i32 %51
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) i32 @update()
  ret i16 0
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
