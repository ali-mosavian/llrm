target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @calculate() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca [32 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 32, i1 false)
  store i16 2, ptr %0, !tbaa !2
  store i16 2, ptr %1, !tbaa !2
  %3 = sub i16 0, 0
  %4 = getelementptr inbounds [16 x i8], ptr %2, i16 %3
  store i32 1, ptr %4, !tbaa !2
  %5 = sub i16 0, 0
  %6 = getelementptr inbounds [16 x i8], ptr %2, i16 %5
  %7 = getelementptr inbounds i8, ptr %6, i16 4
  store i32 2, ptr %7, !tbaa !2
  %8 = sub i16 0, 0
  %9 = getelementptr inbounds [16 x i8], ptr %2, i16 %8
  %10 = getelementptr inbounds i8, ptr %9, i16 8
  store i32 3, ptr %10, !tbaa !2
  %11 = sub i16 0, 0
  %12 = getelementptr inbounds [16 x i8], ptr %2, i16 %11
  %13 = getelementptr inbounds i8, ptr %12, i16 12
  store i32 4, ptr %13, !tbaa !2
  %14 = sub i16 1, 0
  %15 = getelementptr inbounds [16 x i8], ptr %2, i16 %14
  store i32 5, ptr %15, !tbaa !2
  %16 = sub i16 1, 0
  %17 = getelementptr inbounds [16 x i8], ptr %2, i16 %16
  %18 = getelementptr inbounds i8, ptr %17, i16 4
  store i32 6, ptr %18, !tbaa !2
  %19 = sub i16 1, 0
  %20 = getelementptr inbounds [16 x i8], ptr %2, i16 %19
  %21 = getelementptr inbounds i8, ptr %20, i16 8
  store i32 7, ptr %21, !tbaa !2
  %22 = sub i16 1, 0
  %23 = getelementptr inbounds [16 x i8], ptr %2, i16 %22
  %24 = getelementptr inbounds i8, ptr %23, i16 12
  store i32 8, ptr %24, !tbaa !2
  %25 = sub i16 0, 0
  %26 = getelementptr inbounds [16 x i8], ptr %2, i16 %25
  %27 = load i32, ptr %26, !tbaa !2
  %28 = sub i16 0, 0
  %29 = getelementptr inbounds [16 x i8], ptr %2, i16 %28
  %30 = getelementptr inbounds i8, ptr %29, i16 4
  %31 = load i32, ptr %30, !tbaa !2
  %32 = mul i32 %31, 10
  %33 = add i32 %27, %32
  %34 = sub i16 0, 0
  %35 = getelementptr inbounds [16 x i8], ptr %2, i16 %34
  %36 = getelementptr inbounds i8, ptr %35, i16 8
  %37 = load i32, ptr %36, !tbaa !2
  %38 = mul i32 %37, 100
  %39 = add i32 %33, %38
  %40 = sub i16 0, 0
  %41 = getelementptr inbounds [16 x i8], ptr %2, i16 %40
  %42 = getelementptr inbounds i8, ptr %41, i16 12
  %43 = load i32, ptr %42, !tbaa !2
  %44 = mul i32 %43, 1000
  %45 = add i32 %39, %44
  %46 = sub i16 1, 0
  %47 = getelementptr inbounds [16 x i8], ptr %2, i16 %46
  %48 = load i32, ptr %47, !tbaa !2
  %49 = mul i32 %48, 10000
  %50 = add i32 %45, %49
  %51 = sub i16 1, 0
  %52 = getelementptr inbounds [16 x i8], ptr %2, i16 %51
  %53 = getelementptr inbounds i8, ptr %52, i16 4
  %54 = load i32, ptr %53, !tbaa !2
  %55 = mul i32 %54, 100000
  %56 = add i32 %50, %55
  %57 = sub i16 1, 0
  %58 = getelementptr inbounds [16 x i8], ptr %2, i16 %57
  %59 = getelementptr inbounds i8, ptr %58, i16 8
  %60 = load i32, ptr %59, !tbaa !2
  %61 = mul i32 %60, 1000000
  %62 = add i32 %56, %61
  %63 = sub i16 1, 0
  %64 = getelementptr inbounds [16 x i8], ptr %2, i16 %63
  %65 = getelementptr inbounds i8, ptr %64, i16 12
  %66 = load i32, ptr %65, !tbaa !2
  %67 = mul i32 %66, 10000000
  %68 = add i32 %62, %67
  ret i32 %68
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
