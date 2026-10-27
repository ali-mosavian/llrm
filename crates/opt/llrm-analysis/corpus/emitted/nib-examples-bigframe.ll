target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [8192 x i8]
  %12 = alloca i16
  store i16 0, ptr %12, !tbaa !2
  store i16 64, ptr %8, !tbaa !2
  store i16 64, ptr %9, !tbaa !2
  store i16 4096, ptr %10, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 64, ptr %6, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %7, !tbaa !2
  %14 = load i16, ptr %6, !tbaa !2
  %15 = icmp slt i16 %13, %14
  br i1 %15, label %b3, label %b5

b3:
  store i16 0, ptr %5, !tbaa !2
  store i16 64, ptr %4, !tbaa !2
  br label %b6

b4:
  %16 = load i16, ptr %7, !tbaa !2
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %7, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %3, !tbaa !2
  store i16 64, ptr %2, !tbaa !2
  br label %b10

b6:
  %18 = load i16, ptr %5, !tbaa !2
  %19 = load i16, ptr %4, !tbaa !2
  %20 = icmp slt i16 %18, %19
  br i1 %20, label %b7, label %b9

b7:
  %21 = load i16, ptr %7, !tbaa !2
  %22 = load i16, ptr %5, !tbaa !2
  %23 = load i16, ptr %12, !tbaa !2
  %24 = sub i16 %21, 0
  %25 = sub i16 %22, 0
  %26 = mul i16 %24, 64
  %27 = add i16 %26, %25
  %28 = getelementptr inbounds i16, ptr %11, i16 %27
  store i16 %23, ptr %28, !tbaa !2
  br label %b8

b8:
  %29 = load i16, ptr %5, !tbaa !2
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %5, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %31 = load i16, ptr %3, !tbaa !2
  %32 = load i16, ptr %2, !tbaa !2
  %33 = icmp slt i16 %31, %32
  br i1 %33, label %b11, label %b13

b11:
  store i16 0, ptr %1, !tbaa !2
  store i16 64, ptr %0, !tbaa !2
  br label %b14

b12:
  %34 = load i16, ptr %3, !tbaa !2
  %35 = add nsw i16 %34, 1
  store i16 %35, ptr %3, !tbaa !2
  br label %b10

b13:
  %36 = getelementptr inbounds i16, ptr %11, i16 4095
  %37 = load i16, ptr %36, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %37)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b14:
  %38 = load i16, ptr %1, !tbaa !2
  %39 = load i16, ptr %0, !tbaa !2
  %40 = icmp slt i16 %38, %39
  br i1 %40, label %b15, label %b17

b15:
  %41 = load i16, ptr %3, !tbaa !2
  %42 = load i16, ptr %1, !tbaa !2
  %43 = icmp ult i16 %41, 64
  br i1 %43, label %b18, label %b19

b16:
  %44 = load i16, ptr %1, !tbaa !2
  %45 = add nsw i16 %44, 1
  store i16 %45, ptr %1, !tbaa !2
  br label %b14

b17:
  br label %b12

b18:
  %46 = icmp ult i16 %42, 64
  br i1 %46, label %b20, label %b21

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  %47 = sub i16 %41, 0
  %48 = sub i16 %42, 0
  %49 = mul i16 %47, 64
  %50 = add i16 %49, %48
  %51 = getelementptr inbounds i16, ptr %11, i16 %50
  store i16 7, ptr %51, !tbaa !2
  br label %b16

b21:
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
