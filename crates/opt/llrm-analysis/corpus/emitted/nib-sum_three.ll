target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [20 x i8] c"\08\00\0D\00\0D\00sum_three: ok\00"
@$str2 = internal constant [21 x i8] c"\08\00\0E\00\0E\00sum_three: bad\00"

define internal cc 1005 i16 @sum_three(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  store i16 0, ptr %5, !tbaa !2
  %6 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %4, !tbaa !2
  store i16 %6, ptr %3, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = load i16, ptr %3, !tbaa !2
  %9 = icmp ult i16 %7, %8
  br i1 %9, label %b3, label %b5

b3:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %13 = load ptr addrspace(1), ptr addrspace(1) %12
  %14 = mul i16 %11, 2
  %15 = getelementptr inbounds i8, ptr addrspace(1) %13, i16 %14
  %16 = load i16, ptr addrspace(1) %15
  %17 = add i16 %10, %16
  store i16 %17, ptr %5, !tbaa !2
  %18 = load i16, ptr %5, !tbaa !2
  %19 = load i16, ptr %4, !tbaa !2
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20
  %22 = mul i16 %19, 2
  %23 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 %22
  %24 = load i16, ptr addrspace(1) %23
  %25 = add i16 %18, %24
  store i16 %25, ptr %5, !tbaa !2
  %26 = load i16, ptr %5, !tbaa !2
  %27 = load i16, ptr %4, !tbaa !2
  %28 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %29 = load ptr addrspace(1), ptr addrspace(1) %28
  %30 = mul i16 %27, 2
  %31 = getelementptr inbounds i8, ptr addrspace(1) %29, i16 %30
  %32 = load i16, ptr addrspace(1) %31
  %33 = add i16 %26, %32
  store i16 %33, ptr %5, !tbaa !2
  br label %b4

b4:
  %34 = load i16, ptr %4, !tbaa !2
  %35 = add nuw i16 %34, 1
  store i16 %35, ptr %4, !tbaa !2
  br label %b2

b5:
  %36 = load i16, ptr %5, !tbaa !2
  ret i16 %36
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [8 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [8 x i8]
  store i16 4, ptr %10, !tbaa !2
  store i16 4, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i16, ptr %12, i16 0
  store i16 1, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i16, ptr %12, i16 1
  store i16 2, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i16, ptr %12, i16 2
  store i16 3, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i16, ptr %12, i16 3
  store i16 4, ptr %16, !tbaa !2
  store i16 4, ptr %7, !tbaa !2
  store i16 4, ptr %8, !tbaa !2
  %17 = getelementptr inbounds i16, ptr %9, i16 0
  store i16 10, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i16, ptr %9, i16 1
  store i16 20, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i16, ptr %9, i16 2
  store i16 30, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i16, ptr %9, i16 3
  store i16 40, ptr %20, !tbaa !2
  store i16 4, ptr %4, !tbaa !2
  store i16 4, ptr %5, !tbaa !2
  %21 = getelementptr inbounds i16, ptr %6, i16 0
  store i16 100, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i16, ptr %6, i16 1
  store i16 200, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i16, ptr %6, i16 2
  store i16 300, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i16, ptr %6, i16 3
  store i16 400, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %25, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %3 to ptr addrspace(1)
  %29 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %2 to ptr addrspace(1)
  %33 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %1 to ptr addrspace(1)
  %37 = call cc 1005 addrspace(1) i16 @sum_three(ptr addrspace(1) %28, ptr addrspace(1) %32, ptr addrspace(1) %36)
  store i16 %37, ptr %0, !tbaa !2
  %38 = load i16, ptr %0, !tbaa !2
  %39 = icmp eq i16 %38, 1110
  br i1 %39, label %b2, label %b3

b2:
  %40 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %40)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b3:
  %41 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %41)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  %42 = load i16, ptr %0, !tbaa !2
  ret i16 %42
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
