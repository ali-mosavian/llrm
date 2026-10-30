target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @dot(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = alloca i32
  store i32 0, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  br label %b3

b2:
  %5 = load i32, ptr %4, !tbaa !2
  ret i32 %5

b3:
  %6 = load i16, ptr %3, !tbaa !2
  %7 = load i16, ptr addrspace(1) %0
  %8 = icmp ult i16 %6, %7
  br i1 %8, label %b6, label %b5

b4:
  %9 = load i16, ptr %3, !tbaa !2
  %10 = load i16, ptr addrspace(1) %0, !range !5
  %11 = icmp ult i16 %9, %10
  br i1 %11, label %b8, label %b9

b5:
  br label %b2

b6:
  %12 = load i16, ptr %3, !tbaa !2
  %13 = load i16, ptr addrspace(1) %1
  %14 = icmp ult i16 %12, %13
  br i1 %14, label %b4, label %b5

b7:
  %15 = load i16, ptr %3, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %3, !tbaa !2
  br label %b3

b8:
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %18 = load ptr addrspace(1), ptr addrspace(1) %17
  %19 = mul i16 %9, 2
  %20 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 %19
  %21 = load i16, ptr %3, !tbaa !2
  %22 = load i16, ptr addrspace(1) %1, !range !6
  %23 = icmp ult i16 %21, %22
  br i1 %23, label %b10, label %b11

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b10:
  %24 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %25 = load ptr addrspace(1), ptr addrspace(1) %24
  %26 = mul i16 %21, 2
  %27 = getelementptr inbounds i8, ptr addrspace(1) %25, i16 %26
  store ptr addrspace(1) %20, ptr %2, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %27, ptr %28, !tbaa !2
  %29 = load ptr addrspace(1), ptr %2, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %2, i16 4
  %31 = load ptr addrspace(1), ptr %30, !tbaa !2
  %32 = load i32, ptr %4, !tbaa !2
  %33 = load i16, ptr addrspace(1) %29
  %34 = sext i16 %33 to i32
  %35 = load i16, ptr addrspace(1) %31
  %36 = sext i16 %35 to i32
  %37 = mul i32 %34, %36
  %38 = add i32 %32, %37
  store i32 %38, ptr %4, !tbaa !2
  br label %b7

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
