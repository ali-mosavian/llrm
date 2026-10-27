target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00abc\00"

define internal cc 1005 i16 @arr(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
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
  %8 = icmp ult i16 %0, 4
  br i1 %8, label %b2, label %b3

b2:
  %9 = sub i16 %0, 0
  %10 = getelementptr inbounds i16, ptr %3, i16 %9
  store i16 9, ptr %10, !tbaa !2
  %11 = icmp ult i16 %0, 4
  br i1 %11, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %12 = sub i16 %0, 0
  %13 = getelementptr inbounds i16, ptr %3, i16 %12
  %14 = load i16, ptr %13, !tbaa !2
  ret i16 %14

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @vecx(ptr addrspace(1) nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load ptr, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr %2, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp ult i16 %1, %4
  br i1 %5, label %b2, label %b3

b2:
  %6 = mul i16 %1, 2
  %7 = getelementptr inbounds i8, ptr %2, i16 %6
  store i16 5, ptr %7
  %8 = load ptr, ptr addrspace(1) %0
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = icmp ult i16 %1, %10
  br i1 %11, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %12 = mul i16 %1, 2
  %13 = getelementptr inbounds i8, ptr %8, i16 %12
  %14 = load i16, ptr %13
  ret i16 %14

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i8 @text(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp ult i16 %1, %2
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 %1
  %7 = load i8, ptr addrspace(1) %6
  ret i8 %7

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @field(ptr addrspace(1) nonnull dereferenceable(8) noalias nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = icmp ult i16 %1, 4
  br i1 %2, label %b2, label %b3

b2:
  %3 = mul i16 %1, 2
  %4 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %3
  store i16 7, ptr addrspace(1) %4
  %5 = icmp ult i16 %1, 4
  br i1 %5, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %6 = mul i16 %1, 2
  %7 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %6
  %8 = load i16, ptr addrspace(1) %7
  ret i16 %8

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @grid(ptr addrspace(1) nonnull dereferenceable(12) noalias nocapture %0, i16 %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = icmp ult i16 %1, 2
  br i1 %3, label %b2, label %b3

b2:
  %4 = icmp ult i16 %2, 3
  br i1 %4, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %5 = mul i16 %1, 3
  %6 = mul i16 %2, 1
  %7 = add i16 %5, %6
  %8 = mul i16 %7, 2
  %9 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %8
  store i16 3, ptr addrspace(1) %9
  %10 = icmp ult i16 %1, 2
  br i1 %10, label %b6, label %b7

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b6:
  %11 = icmp ult i16 %2, 3
  br i1 %11, label %b8, label %b9

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %12 = mul i16 %1, 3
  %13 = mul i16 %2, 1
  %14 = add i16 %12, %13
  %15 = mul i16 %14, 2
  %16 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %15
  %17 = load i16, ptr addrspace(1) %16
  ret i16 %17

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @view(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i16, ptr addrspace(1) %0, !range !5
  %3 = icmp ult i16 %1, %2
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = mul i16 %1, 2
  %7 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 %6
  store i16 1, ptr addrspace(1) %7
  %8 = load i16, ptr addrspace(1) %0, !range !6
  %9 = icmp ult i16 %1, %8
  br i1 %9, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = mul i16 %1, 2
  %13 = getelementptr inbounds i8, ptr addrspace(1) %11, i16 %12
  %14 = load i16, ptr addrspace(1) %13
  ret i16 %14

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [6 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca [12 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca ptr
  %13 = getelementptr i8, ptr @$str1, i16 6
  %14 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %13, i16 3, i16 2)
  %15 = getelementptr inbounds i8, ptr %14, i16 0
  store i16 1, ptr %15
  %16 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 2, ptr %16
  %17 = getelementptr inbounds i8, ptr %14, i16 4
  store i16 3, ptr %17
  store ptr %14, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 2, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %11, i16 4
  store i16 3, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %11, i16 6
  store i16 4, ptr %20, !tbaa !2
  store i16 2, ptr %7, !tbaa !2
  store i16 3, ptr %8, !tbaa !2
  store i16 6, ptr %9, !tbaa !2
  %21 = getelementptr inbounds i16, ptr %10, i16 0
  store i16 1, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i16, ptr %10, i16 1
  store i16 2, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i16, ptr %10, i16 2
  store i16 3, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i16, ptr %10, i16 3
  store i16 4, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i16, ptr %10, i16 4
  store i16 5, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i16, ptr %10, i16 5
  store i16 6, ptr %26, !tbaa !2
  store i16 3, ptr %4, !tbaa !2
  store i16 3, ptr %5, !tbaa !2
  %27 = getelementptr inbounds i16, ptr %6, i16 0
  store i16 1, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i16, ptr %6, i16 1
  store i16 2, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i16, ptr %6, i16 2
  store i16 3, ptr %29, !tbaa !2
  %30 = call cc 1005 addrspace(1) i16 @arr(i16 1)
  %31 = addrspacecast ptr %12 to ptr addrspace(1)
  %32 = call cc 1005 addrspace(1) i16 @vecx(ptr addrspace(1) %31, i16 1)
  %33 = add i16 %30, %32
  %34 = addrspacecast ptr %11 to ptr addrspace(1)
  %35 = call cc 1005 addrspace(1) i16 @field(ptr addrspace(1) %34, i16 1)
  %36 = add i16 %33, %35
  %37 = addrspacecast ptr %10 to ptr addrspace(1)
  %38 = call cc 1005 addrspace(1) i16 @grid(ptr addrspace(1) %37, i16 1, i16 2)
  %39 = add i16 %36, %38
  %40 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %3 to ptr addrspace(1)
  %44 = call cc 1005 addrspace(1) i16 @view(ptr addrspace(1) %43, i16 1)
  %45 = add i16 %39, %44
  store i16 %45, ptr %2, !tbaa !2
  %46 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %46)
  call cc 1005 addrspace(1) void @N$PN()
  %47 = getelementptr i8, ptr @$str2, i16 6
  %48 = getelementptr i8, ptr %47, i16 -4
  %49 = load i16, ptr %48
  %50 = addrspacecast ptr %47 to ptr addrspace(1)
  store i16 %49, ptr %1, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %49, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %50, ptr %52, !tbaa !2
  %53 = addrspacecast ptr %1 to ptr addrspace(1)
  %54 = call cc 1005 addrspace(1) i8 @text(ptr addrspace(1) %53, i16 1)
  store i8 %54, ptr %0, !tbaa !2
  %55 = load i8, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PC(i8 zeroext %55)
  call cc 1005 addrspace(1) void @N$PN()
  %56 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %56)
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PC(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
