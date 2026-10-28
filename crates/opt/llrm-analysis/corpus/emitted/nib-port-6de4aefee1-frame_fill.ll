target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @value(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [256 x i8]
  %6 = alloca i32
  store i32 0, ptr %6, !tbaa !2
  store i16 64, ptr %3, !tbaa !2
  store i16 64, ptr %4, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 64, ptr %1, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %b3, label %b5

b3:
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i32, ptr %6, !tbaa !2
  %12 = sub i16 %10, 0
  %13 = getelementptr inbounds i32, ptr %5, i16 %12
  store i32 %11, ptr %13, !tbaa !2
  br label %b4

b4:
  %14 = load i16, ptr %2, !tbaa !2
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %2, !tbaa !2
  br label %b2

b5:
  %16 = icmp ult i16 %0, 64
  br i1 %16, label %b6, label %b7

b6:
  %17 = sub i16 %0, 0
  %18 = getelementptr inbounds i32, ptr %5, i16 %17
  store i32 5, ptr %18, !tbaa !2
  %19 = icmp ult i16 %0, 64
  br i1 %19, label %b8, label %b9

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %20 = sub i16 %0, 0
  %21 = getelementptr inbounds i32, ptr %5, i16 %20
  %22 = load i32, ptr %21, !tbaa !2
  %23 = add i16 %0, 1
  %24 = icmp ult i16 %23, 64
  br i1 %24, label %b10, label %b11

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b10:
  %25 = sub i16 %23, 0
  %26 = getelementptr inbounds i32, ptr %5, i16 %25
  %27 = load i32, ptr %26, !tbaa !2
  %28 = add i32 %22, %27
  ret i32 %28

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) i32 @value(i16 3)
  %1 = trunc i32 %0 to i16
  ret i16 %1
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
