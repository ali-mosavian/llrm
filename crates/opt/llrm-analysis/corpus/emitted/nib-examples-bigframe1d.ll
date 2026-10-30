target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [8192 x i8]
  %7 = alloca i16
  store i16 0, ptr %7, !tbaa !2
  store i16 4096, ptr %4, !tbaa !2
  store i16 4096, ptr %5, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 4096, ptr %2, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %b3, label %b5

b3:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %7, !tbaa !2
  %13 = sub i16 %11, 0
  %14 = getelementptr inbounds i16, ptr %6, i16 %13
  store i16 %12, ptr %14, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %3, !tbaa !2
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %3, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %1, !tbaa !2
  store i16 4096, ptr %0, !tbaa !2
  br label %b6

b6:
  %17 = load i16, ptr %1, !tbaa !2
  %18 = load i16, ptr %0, !tbaa !2
  %19 = icmp slt i16 %17, %18
  br i1 %19, label %b7, label %b9

b7:
  %20 = load i16, ptr %1, !tbaa !2
  %21 = icmp ult i16 %20, 4096
  br i1 %21, label %b10, label %b11

b8:
  %22 = load i16, ptr %1, !tbaa !2
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %1, !tbaa !2
  br label %b6

b9:
  %24 = getelementptr inbounds i16, ptr %6, i16 4095
  %25 = load i16, ptr %24, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %25)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b10:
  %26 = sub i16 %20, 0
  %27 = getelementptr inbounds i16, ptr %6, i16 %26
  store i16 7, ptr %27, !tbaa !2
  br label %b8

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
