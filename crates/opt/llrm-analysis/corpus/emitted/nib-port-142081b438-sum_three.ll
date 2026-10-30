target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [20 x i8] c"\08\00\0D\00\0D\00sum_three: ok\00"
@$str2 = internal constant [21 x i8] c"\08\00\0E\00\0E\00sum_three: bad\00"

define internal cc 1005 i16 @sum_three(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  store i16 0, ptr %5, !tbaa !2
  %6 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %4, !tbaa !2
  store i16 %6, ptr %3, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = load i16, ptr %3, !tbaa !2
  %9 = icmp ult i16 %7, %8
  br i1 %9, label %b3, label %b5

b3:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = load i16, ptr addrspace(1) %0, !range !5
  %13 = icmp ult i16 %11, %12
  br i1 %13, label %b6, label %b7

b4:
  %14 = load i16, ptr %4, !tbaa !2
  %15 = add nuw i16 %14, 1
  store i16 %15, ptr %4, !tbaa !2
  br label %b2

b5:
  %16 = load i16, ptr %5, !tbaa !2
  ret i16 %16

b6:
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %18 = load ptr addrspace(1), ptr addrspace(1) %17
  %19 = mul i16 %11, 2
  %20 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 %19
  %21 = load i16, ptr addrspace(1) %20
  %22 = add i16 %10, %21
  store i16 %22, ptr %5, !tbaa !2
  %23 = load i16, ptr %5, !tbaa !2
  %24 = load i16, ptr %4, !tbaa !2
  %25 = load i16, ptr addrspace(1) %1, !range !6
  %26 = icmp ult i16 %24, %25
  br i1 %26, label %b8, label %b9

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %28 = load ptr addrspace(1), ptr addrspace(1) %27
  %29 = mul i16 %24, 2
  %30 = getelementptr inbounds i8, ptr addrspace(1) %28, i16 %29
  %31 = load i16, ptr addrspace(1) %30
  %32 = add i16 %23, %31
  store i16 %32, ptr %5, !tbaa !2
  %33 = load i16, ptr %5, !tbaa !2
  %34 = load i16, ptr %4, !tbaa !2
  %35 = load i16, ptr addrspace(1) %2, !range !7
  %36 = icmp ult i16 %34, %35
  br i1 %36, label %b10, label %b11

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b10:
  %37 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %38 = load ptr addrspace(1), ptr addrspace(1) %37
  %39 = mul i16 %34, 2
  %40 = getelementptr inbounds i8, ptr addrspace(1) %38, i16 %39
  %41 = load i16, ptr addrspace(1) %40
  %42 = add i16 %33, %41
  store i16 %42, ptr %5, !tbaa !2
  br label %b4

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [8 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [8 x i8]
  store i16 4, ptr %10, !tbaa !2
  store i16 4, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i16, ptr %12, i16 0
  store i16 1, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i16, ptr %12, i16 1
  store i16 2, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i16, ptr %12, i16 2
  store i16 3, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i16, ptr %12, i16 3
  store i16 4, ptr %16, !tbaa !2
  store i16 4, ptr %7, !tbaa !2
  store i16 4, ptr %8, !tbaa !2
  %17 = getelementptr inbounds i16, ptr %9, i16 0
  store i16 10, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i16, ptr %9, i16 1
  store i16 20, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i16, ptr %9, i16 2
  store i16 30, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i16, ptr %9, i16 3
  store i16 40, ptr %20, !tbaa !2
  store i16 4, ptr %4, !tbaa !2
  store i16 4, ptr %5, !tbaa !2
  %21 = getelementptr inbounds i16, ptr %6, i16 0
  store i16 100, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i16, ptr %6, i16 1
  store i16 200, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i16, ptr %6, i16 2
  store i16 300, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i16, ptr %6, i16 3
  store i16 400, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %25, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %3 to ptr addrspace(1)
  %29 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %2 to ptr addrspace(1)
  %33 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %1 to ptr addrspace(1)
  %37 = call cc 1005 addrspace(1) i16 @sum_three(ptr addrspace(1) %28, ptr addrspace(1) %32, ptr addrspace(1) %36)
  store i16 %37, ptr %0, !tbaa !2
  %38 = load i16, ptr %0, !tbaa !2
  %39 = icmp eq i16 %38, 1110
  br i1 %39, label %b2, label %b3

b2:
  %40 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %40)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b3:
  %41 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %41)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  %42 = load i16, ptr %0, !tbaa !2
  ret i16 %42
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
!7 = !{i16 0, i16 -32768}
