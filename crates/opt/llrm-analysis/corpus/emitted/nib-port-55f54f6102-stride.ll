target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @update() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [50 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 50, i1 false)
  store i16 5, ptr %1, !tbaa !2
  store i16 5, ptr %2, !tbaa !2
  %4 = sub i16 0, 0
  %5 = getelementptr inbounds [10 x i8], ptr %3, i16 %4
  store i16 0, ptr %5, !tbaa !2
  %6 = sub i16 0, 0
  %7 = getelementptr inbounds [10 x i8], ptr %3, i16 %6
  %8 = getelementptr inbounds i8, ptr %7, i16 2
  store i32 1, ptr %8, !tbaa !2
  %9 = sub i16 0, 0
  %10 = getelementptr inbounds [10 x i8], ptr %3, i16 %9
  %11 = getelementptr inbounds i8, ptr %10, i16 6
  store i32 2, ptr %11, !tbaa !2
  %12 = sub i16 1, 0
  %13 = getelementptr inbounds [10 x i8], ptr %3, i16 %12
  store i16 0, ptr %13, !tbaa !2
  %14 = sub i16 1, 0
  %15 = getelementptr inbounds [10 x i8], ptr %3, i16 %14
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i32 2, ptr %16, !tbaa !2
  %17 = sub i16 1, 0
  %18 = getelementptr inbounds [10 x i8], ptr %3, i16 %17
  %19 = getelementptr inbounds i8, ptr %18, i16 6
  store i32 3, ptr %19, !tbaa !2
  %20 = sub i16 2, 0
  %21 = getelementptr inbounds [10 x i8], ptr %3, i16 %20
  store i16 0, ptr %21, !tbaa !2
  %22 = sub i16 2, 0
  %23 = getelementptr inbounds [10 x i8], ptr %3, i16 %22
  %24 = getelementptr inbounds i8, ptr %23, i16 2
  store i32 3, ptr %24, !tbaa !2
  %25 = sub i16 2, 0
  %26 = getelementptr inbounds [10 x i8], ptr %3, i16 %25
  %27 = getelementptr inbounds i8, ptr %26, i16 6
  store i32 4, ptr %27, !tbaa !2
  %28 = sub i16 3, 0
  %29 = getelementptr inbounds [10 x i8], ptr %3, i16 %28
  store i16 0, ptr %29, !tbaa !2
  %30 = sub i16 3, 0
  %31 = getelementptr inbounds [10 x i8], ptr %3, i16 %30
  %32 = getelementptr inbounds i8, ptr %31, i16 2
  store i32 4, ptr %32, !tbaa !2
  %33 = sub i16 3, 0
  %34 = getelementptr inbounds [10 x i8], ptr %3, i16 %33
  %35 = getelementptr inbounds i8, ptr %34, i16 6
  store i32 5, ptr %35, !tbaa !2
  %36 = sub i16 4, 0
  %37 = getelementptr inbounds [10 x i8], ptr %3, i16 %36
  store i16 0, ptr %37, !tbaa !2
  %38 = sub i16 4, 0
  %39 = getelementptr inbounds [10 x i8], ptr %3, i16 %38
  %40 = getelementptr inbounds i8, ptr %39, i16 2
  store i32 5, ptr %40, !tbaa !2
  %41 = sub i16 4, 0
  %42 = getelementptr inbounds [10 x i8], ptr %3, i16 %41
  %43 = getelementptr inbounds i8, ptr %42, i16 6
  store i32 6, ptr %43, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  br label %b2

b2:
  %44 = load i16, ptr %0, !tbaa !2
  %45 = icmp ult i16 %44, 5
  %46 = sext i1 %45 to i8
  %47 = icmp ne i8 %46, 0
  br i1 %47, label %b3, label %b5

b3:
  %48 = sub i16 %44, 0
  %49 = getelementptr inbounds [10 x i8], ptr %3, i16 %48
  %50 = getelementptr inbounds i8, ptr %49, i16 2
  %51 = load i32, ptr %50, !tbaa !2
  %52 = sub i16 %44, 0
  %53 = getelementptr inbounds [10 x i8], ptr %3, i16 %52
  %54 = getelementptr inbounds i8, ptr %53, i16 6
  %55 = load i32, ptr %54, !tbaa !2
  %56 = add i32 %51, %55
  %57 = sub i16 %44, 0
  %58 = getelementptr inbounds [10 x i8], ptr %3, i16 %57
  %59 = getelementptr inbounds i8, ptr %58, i16 2
  store i32 %56, ptr %59, !tbaa !2
  br label %b4

b4:
  %60 = load i16, ptr %0, !tbaa !2
  %61 = add i16 %60, 1
  store i16 %61, ptr %0, !tbaa !2
  br label %b2

b5:
  %62 = sub i16 0, 0
  %63 = getelementptr inbounds [10 x i8], ptr %3, i16 %62
  %64 = getelementptr inbounds i8, ptr %63, i16 2
  %65 = load i32, ptr %64, !tbaa !2
  %66 = sub i16 4, 0
  %67 = getelementptr inbounds [10 x i8], ptr %3, i16 %66
  %68 = getelementptr inbounds i8, ptr %67, i16 2
  %69 = load i32, ptr %68, !tbaa !2
  %70 = add i32 %65, %69
  ret i32 %70
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @update()
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
