target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @at(ptr addrspace(1) noalias readonly dereferenceable(10) nocapture %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i16, ptr addrspace(1) %0, !range !5
  %3 = zext i8 %1 to i16
  %4 = icmp ult i16 %3, %2
  br i1 %4, label %b2, label %b3

b2:
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5, !range !6
  %7 = icmp ult i16 1, %6
  br i1 %7, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %9 = load i16, ptr addrspace(1) %8
  %10 = mul i16 %9, 1
  %11 = zext i8 %1 to i16
  %12 = mul i16 %11, %10
  %13 = add i16 %12, 1
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %15 = load ptr addrspace(1), ptr addrspace(1) %14
  %16 = mul i16 %13, 2
  %17 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 %16
  %18 = load i16, ptr addrspace(1) %17
  ret i16 %18

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [10 x i8]
  %1 = alloca i8
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [800 x i8]
  %10 = alloca i16
  store i16 0, ptr %10, !tbaa !2
  store i16 20, ptr %6, !tbaa !2
  store i16 20, ptr %7, !tbaa !2
  store i16 400, ptr %8, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 20, ptr %4, !tbaa !2
  br label %b2

b2:
  %11 = load i16, ptr %5, !tbaa !2
  %12 = load i16, ptr %4, !tbaa !2
  %13 = icmp slt i16 %11, %12
  br i1 %13, label %b3, label %b5

b3:
  store i16 0, ptr %3, !tbaa !2
  store i16 20, ptr %2, !tbaa !2
  br label %b6

b4:
  %14 = load i16, ptr %5, !tbaa !2
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %5, !tbaa !2
  br label %b2

b5:
  %16 = getelementptr inbounds i16, ptr %9, i16 381
  store i16 7, ptr %16, !tbaa !2
  store i8 19, ptr %1, !tbaa !2
  %17 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 20, ptr %0, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 20, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %0, i16 4
  store i16 400, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %0, i16 6
  store ptr addrspace(1) %17, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %0 to ptr addrspace(1)
  %22 = load i8, ptr %1, !tbaa !2
  %23 = call cc 1005 addrspace(1) i16 @at(ptr addrspace(1) %21, i8 zeroext %22)
  ret i16 %23

b6:
  %24 = load i16, ptr %3, !tbaa !2
  %25 = load i16, ptr %2, !tbaa !2
  %26 = icmp slt i16 %24, %25
  br i1 %26, label %b7, label %b9

b7:
  %27 = load i16, ptr %5, !tbaa !2
  %28 = load i16, ptr %3, !tbaa !2
  %29 = load i16, ptr %10, !tbaa !2
  %30 = sub i16 %27, 0
  %31 = sub i16 %28, 0
  %32 = mul i16 %30, 20
  %33 = add i16 %32, %31
  %34 = getelementptr inbounds i16, ptr %9, i16 %33
  store i16 %29, ptr %34, !tbaa !2
  br label %b8

b8:
  %35 = load i16, ptr %3, !tbaa !2
  %36 = add nsw i16 %35, 1
  store i16 %36, ptr %3, !tbaa !2
  br label %b6

b9:
  br label %b4
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
