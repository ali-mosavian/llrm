target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @value(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [256 x i8]
  %9 = alloca i32
  store i32 0, ptr %9, !tbaa !2
  store i16 64, ptr %6, !tbaa !2
  store i16 64, ptr %7, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 64, ptr %4, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i16, ptr %5, !tbaa !2
  %14 = load i32, ptr %9, !tbaa !2
  %15 = sub i16 %13, 0
  %16 = getelementptr inbounds i32, ptr %8, i16 %15
  store i32 %14, ptr %16, !tbaa !2
  br label %b4

b4:
  %17 = load i16, ptr %5, !tbaa !2
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %5, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 4, ptr %1, !tbaa !2
  br label %b6

b6:
  %19 = load i16, ptr %2, !tbaa !2
  %20 = load i16, ptr %1, !tbaa !2
  %21 = icmp slt i16 %19, %20
  br i1 %21, label %b7, label %b9

b7:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %2, !tbaa !2
  %24 = load i16, ptr addrspace(1) %0, !range !5
  %25 = icmp ult i16 %23, %24
  br i1 %25, label %b10, label %b11

b8:
  %26 = load i16, ptr %2, !tbaa !2
  %27 = add nsw i16 %26, 1
  store i16 %27, ptr %2, !tbaa !2
  br label %b6

b9:
  %28 = load i16, ptr %3, !tbaa !2
  %29 = icmp ult i16 %28, 64
  br i1 %29, label %b12, label %b13

b10:
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %31 = load ptr addrspace(1), ptr addrspace(1) %30
  %32 = mul i16 %23, 2
  %33 = getelementptr inbounds i8, ptr addrspace(1) %31, i16 %32
  %34 = load i16, ptr addrspace(1) %33
  %35 = add i16 %22, %34
  store i16 %35, ptr %3, !tbaa !2
  br label %b8

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %36 = sub i16 %28, 0
  %37 = getelementptr inbounds i32, ptr %8, i16 %36
  store i32 5, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i32, ptr %8, i16 1
  %39 = load i32, ptr %38, !tbaa !2
  ret i32 %39

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  store i16 4, ptr %1, !tbaa !2
  store i16 4, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %3, i16 0
  store i16 1, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %3, i16 1
  store i16 2, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %3, i16 2
  store i16 3, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %3, i16 3
  store i16 4, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 4, ptr %0, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 4, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %0 to ptr addrspace(1)
  %12 = call cc 1005 addrspace(1) i32 @value(ptr addrspace(1) %11)
  %13 = trunc i32 %12 to i16
  ret i16 %13
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
