target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca ptr
  %4 = alloca i16
  %5 = alloca ptr
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [8 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store ptr null, ptr %3
  store i16 0, ptr %4
  store ptr null, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  store i16 4, ptr %6, !tbaa !2
  store i16 4, ptr %7, !tbaa !2
  %9 = sub i16 0, 0
  %10 = getelementptr inbounds i16, ptr %8, i16 %9
  store i16 1, ptr %10, !tbaa !2
  %11 = sub i16 1, 0
  %12 = getelementptr inbounds i16, ptr %8, i16 %11
  store i16 2, ptr %12, !tbaa !2
  %13 = sub i16 2, 0
  %14 = getelementptr inbounds i16, ptr %8, i16 %13
  store i16 3, ptr %14, !tbaa !2
  %15 = sub i16 3, 0
  %16 = getelementptr inbounds i16, ptr %8, i16 %15
  store i16 4, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$str1, i16 6
  store ptr %17, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  br label %b2

b2:
  %18 = load i16, ptr %4, !tbaa !2
  %19 = icmp ult i16 %18, 4
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b3, label %b5

b3:
  %22 = load ptr, ptr %5, !tbaa !2
  %23 = getelementptr i8, ptr %22, i16 -4
  %24 = load i16, ptr %23
  %25 = call addrspace(1) ptr @N$BGRW(ptr %22, i16 1, i16 2)
  store ptr %25, ptr %5, !tbaa !2
  %26 = mul i16 %24, 2
  %27 = getelementptr i8, ptr %25, i16 %26
  %28 = sub i16 %18, 0
  %29 = getelementptr inbounds i16, ptr %8, i16 %28
  %30 = load i16, ptr %29, !tbaa !2
  %31 = mul i16 %30, 2
  store i16 %31, ptr %27
  br label %b4

b4:
  %32 = load i16, ptr %4, !tbaa !2
  %33 = add i16 %32, 1
  store i16 %33, ptr %4, !tbaa !2
  br label %b2

b5:
  %34 = load ptr, ptr %5, !tbaa !2
  store ptr null, ptr %5, !tbaa !2
  store ptr %34, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %35 = load ptr, ptr %3, !tbaa !2
  %36 = getelementptr i8, ptr %35, i16 -4
  %37 = load i16, ptr %36
  store i16 0, ptr %1, !tbaa !2
  br label %b7

b6:
  %38 = load i16, ptr %2, !tbaa !2
  %39 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %39)
  %40 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %40)
  ret i16 %38

b7:
  %41 = load i16, ptr %1, !tbaa !2
  %42 = icmp ult i16 %41, %37
  %43 = sext i1 %42 to i8
  %44 = icmp ne i8 %43, 0
  br i1 %44, label %b8, label %b10

b8:
  %45 = mul i16 %41, 2
  %46 = getelementptr i8, ptr %35, i16 %45
  %47 = load i16, ptr %46
  %48 = add i16 %47, 1
  store i16 %48, ptr %0, !tbaa !2
  %49 = load i16, ptr %2, !tbaa !2
  %50 = load i16, ptr %0, !tbaa !2
  %51 = add i16 %49, %50
  store i16 %51, ptr %2, !tbaa !2
  br label %b11

b9:
  %52 = load i16, ptr %1, !tbaa !2
  %53 = add i16 %52, 1
  store i16 %53, ptr %1, !tbaa !2
  br label %b7

b10:
  br label %b6

b11:
  br label %b9
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
