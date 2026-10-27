target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @value(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  store i16 8, ptr %5, !tbaa !2
  store i16 8, ptr %6, !tbaa !2
  store i16 64, ptr %7, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 8, ptr %3, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %b3, label %b5

b3:
  store i16 0, ptr %2, !tbaa !2
  store i16 8, ptr %1, !tbaa !2
  br label %b6

b4:
  %13 = load i16, ptr %4, !tbaa !2
  %14 = add nsw i16 %13, 1
  store i16 %14, ptr %4, !tbaa !2
  br label %b2

b5:
  %15 = icmp ult i16 %0, 8
  br i1 %15, label %b10, label %b11

b6:
  %16 = load i16, ptr %2, !tbaa !2
  %17 = load i16, ptr %1, !tbaa !2
  %18 = icmp slt i16 %16, %17
  br i1 %18, label %b7, label %b9

b7:
  %19 = load i16, ptr %4, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = load i32, ptr %9, !tbaa !2
  %22 = sub i16 %19, 0
  %23 = sub i16 %20, 0
  %24 = mul i16 %22, 8
  %25 = add i16 %24, %23
  %26 = getelementptr inbounds i32, ptr %8, i16 %25
  store i32 %21, ptr %26, !tbaa !2
  br label %b8

b8:
  %27 = load i16, ptr %2, !tbaa !2
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %2, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %29 = sub i16 %0, 0
  %30 = mul i16 %29, 8
  %31 = add i16 %30, 1
  %32 = getelementptr inbounds i32, ptr %8, i16 %31
  store i32 5, ptr %32, !tbaa !2
  %33 = icmp ult i16 %0, 8
  br i1 %33, label %b12, label %b13

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %34 = sub i16 %0, 0
  %35 = mul i16 %34, 8
  %36 = add i16 %35, 2
  %37 = getelementptr inbounds i32, ptr %8, i16 %36
  %38 = load i32, ptr %37, !tbaa !2
  ret i32 %38

b13:
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
