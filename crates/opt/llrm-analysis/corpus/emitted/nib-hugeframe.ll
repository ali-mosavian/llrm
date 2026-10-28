target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @last(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode noipa "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr addrspace(1) %0, !range !5
  %2 = icmp ult i16 31999, %1
  br i1 %2, label %b2, label %b3

b2:
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  %5 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 -1538
  %6 = load i16, ptr addrspace(1) %5
  ret i16 %6

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [64000 x i8]
  %9 = alloca i16
  store i16 0, ptr %9, !tbaa !2
  store i16 32000, ptr %6, !tbaa !2
  store i16 32000, ptr %7, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 32000, ptr %4, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i16, ptr %5, !tbaa !2
  %14 = load i16, ptr %9, !tbaa !2
  %15 = sub i16 %13, 0
  %16 = getelementptr inbounds i16, ptr %8, i16 %15
  store i16 %14, ptr %16, !tbaa !2
  br label %b4

b4:
  %17 = load i16, ptr %5, !tbaa !2
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %5, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %3, !tbaa !2
  store i16 32000, ptr %2, !tbaa !2
  br label %b6

b6:
  %19 = load i16, ptr %3, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = icmp slt i16 %19, %20
  br i1 %21, label %b7, label %b9

b7:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = icmp ult i16 %22, 32000
  br i1 %23, label %b10, label %b11

b8:
  %24 = load i16, ptr %3, !tbaa !2
  %25 = add nsw i16 %24, 1
  store i16 %25, ptr %3, !tbaa !2
  br label %b6

b9:
  %26 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 32000, ptr %1, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 32000, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %1 to ptr addrspace(1)
  %30 = call cc 1005 addrspace(1) i16 @last(ptr addrspace(1) %29)
  store i16 %30, ptr %0, !tbaa !2
  %31 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %31)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b10:
  %32 = load i16, ptr %3, !tbaa !2
  %33 = sub i16 %22, 0
  %34 = getelementptr inbounds i16, ptr %8, i16 %33
  store i16 %32, ptr %34, !tbaa !2
  br label %b8

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
