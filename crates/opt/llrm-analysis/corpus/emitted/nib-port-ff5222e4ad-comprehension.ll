target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  store i16 4, ptr %6, !tbaa !2
  store i16 4, ptr %7, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %8, i16 0
  store i16 1, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %8, i16 1
  store i16 2, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %8, i16 2
  store i16 3, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i16, ptr %8, i16 3
  store i16 4, ptr %12, !tbaa !2
  %13 = getelementptr i8, ptr @$str1, i16 6
  store ptr %13, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  br label %b2

b2:
  %14 = load i16, ptr %4, !tbaa !2
  %15 = icmp ult i16 %14, 4
  br i1 %15, label %b3, label %b5

b3:
  %16 = load ptr, ptr %5, !tbaa !2
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %16, i16 1, i16 2)
  store ptr %19, ptr %5, !tbaa !2
  %20 = mul i16 %18, 2
  %21 = getelementptr inbounds i8, ptr %19, i16 %20
  %22 = sub i16 %14, 0
  %23 = getelementptr inbounds i16, ptr %8, i16 %22
  %24 = load i16, ptr %23, !tbaa !2
  %25 = mul i16 %24, 2
  store i16 %25, ptr %21
  br label %b4

b4:
  %26 = load i16, ptr %4, !tbaa !2
  %27 = add i16 %26, 1
  store i16 %27, ptr %4, !tbaa !2
  br label %b2

b5:
  %28 = load ptr, ptr %5, !tbaa !2
  store ptr null, ptr %5, !tbaa !2
  store ptr %28, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %29 = load ptr, ptr %3, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  store i16 0, ptr %1, !tbaa !2
  br label %b7

b6:
  %32 = load i16, ptr %2, !tbaa !2
  %33 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %33)
  %34 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %34)
  ret i16 %32

b7:
  %35 = load i16, ptr %1, !tbaa !2
  %36 = icmp ult i16 %35, %31
  br i1 %36, label %b8, label %b10

b8:
  %37 = mul i16 %35, 2
  %38 = getelementptr inbounds i8, ptr %29, i16 %37
  %39 = load i16, ptr %38
  %40 = add i16 %39, 1
  store i16 %40, ptr %0, !tbaa !2
  %41 = load i16, ptr %2, !tbaa !2
  %42 = load i16, ptr %0, !tbaa !2
  %43 = add i16 %41, %42
  store i16 %43, ptr %2, !tbaa !2
  br label %b11

b9:
  %44 = load i16, ptr %1, !tbaa !2
  %45 = add i16 %44, 1
  store i16 %45, ptr %1, !tbaa !2
  br label %b7

b10:
  br label %b6

b11:
  br label %b9
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
