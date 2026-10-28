target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @partition(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, i16 %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = load i16, ptr addrspace(1) %0, !range !5
  %9 = icmp ult i16 %2, %8
  br i1 %9, label %b2, label %b3

b2:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = mul i16 %2, 2
  %13 = getelementptr inbounds i8, ptr addrspace(1) %11, i16 %12
  %14 = load i16, ptr addrspace(1) %13
  store i16 %14, ptr %7, !tbaa !2
  store i16 %1, ptr %6, !tbaa !2
  store i16 %1, ptr %5, !tbaa !2
  store i16 %2, ptr %4, !tbaa !2
  br label %b4

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %15 = load i16, ptr %5, !tbaa !2
  %16 = load i16, ptr %4, !tbaa !2
  %17 = icmp slt i16 %15, %16
  br i1 %17, label %b5, label %b7

b5:
  %18 = load i16, ptr %5, !tbaa !2
  %19 = load i16, ptr addrspace(1) %0, !range !6
  %20 = icmp ult i16 %18, %19
  br i1 %20, label %b11, label %b12

b6:
  %21 = load i16, ptr %5, !tbaa !2
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %5, !tbaa !2
  br label %b4

b7:
  %23 = load i16, ptr %6, !tbaa !2
  ret i16 %23

b8:
  %24 = load i16, ptr %6, !tbaa !2
  %25 = load i16, ptr addrspace(1) %0, !range !7
  %26 = icmp ult i16 %24, %25
  br i1 %26, label %b13, label %b14

b9:
  br label %b10

b10:
  br label %b6

b11:
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %28 = load ptr addrspace(1), ptr addrspace(1) %27
  %29 = mul i16 %18, 2
  %30 = getelementptr inbounds i8, ptr addrspace(1) %28, i16 %29
  %31 = load i16, ptr addrspace(1) %30
  %32 = load i16, ptr %7, !tbaa !2
  %33 = icmp slt i16 %31, %32
  br i1 %33, label %b8, label %b9

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b13:
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %35 = load ptr addrspace(1), ptr addrspace(1) %34
  %36 = mul i16 %24, 2
  %37 = getelementptr inbounds i8, ptr addrspace(1) %35, i16 %36
  %38 = load i16, ptr addrspace(1) %37
  store i16 %38, ptr %3, !tbaa !2
  %39 = load i16, ptr %6, !tbaa !2
  %40 = load i16, ptr addrspace(1) %0, !range !8
  %41 = icmp ult i16 %39, %40
  br i1 %41, label %b15, label %b16

b14:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b15:
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %43 = load ptr addrspace(1), ptr addrspace(1) %42
  %44 = mul i16 %39, 2
  %45 = getelementptr inbounds i8, ptr addrspace(1) %43, i16 %44
  %46 = load i16, ptr %5, !tbaa !2
  %47 = load i16, ptr addrspace(1) %0, !range !9
  %48 = icmp ult i16 %46, %47
  br i1 %48, label %b17, label %b18

b16:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b17:
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %50 = load ptr addrspace(1), ptr addrspace(1) %49
  %51 = mul i16 %46, 2
  %52 = getelementptr inbounds i8, ptr addrspace(1) %50, i16 %51
  %53 = load i16, ptr addrspace(1) %52
  store i16 %53, ptr addrspace(1) %45
  %54 = load i16, ptr %5, !tbaa !2
  %55 = load i16, ptr addrspace(1) %0, !range !10
  %56 = icmp ult i16 %54, %55
  br i1 %56, label %b19, label %b20

b18:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %58 = load ptr addrspace(1), ptr addrspace(1) %57
  %59 = mul i16 %54, 2
  %60 = getelementptr inbounds i8, ptr addrspace(1) %58, i16 %59
  %61 = load i16, ptr %3, !tbaa !2
  store i16 %61, ptr addrspace(1) %60
  %62 = load i16, ptr %6, !tbaa !2
  %63 = add i16 %62, 1
  store i16 %63, ptr %6, !tbaa !2
  br label %b10

b20:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [16 x i8]
  store i16 8, ptr %1, !tbaa !2
  store i16 8, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %3, i16 0
  store i16 5, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %3, i16 1
  store i16 3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %3, i16 2
  store i16 8, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %3, i16 3
  store i16 1, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %3, i16 4
  store i16 9, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %3, i16 5
  store i16 2, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %3, i16 6
  store i16 7, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %3, i16 7
  store i16 4, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 8, ptr %0, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 8, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %0 to ptr addrspace(1)
  %16 = call cc 1005 addrspace(1) i16 @partition(ptr addrspace(1) %15, i16 0, i16 7)
  ret i16 %16
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
!7 = !{i16 0, i16 -32768}
!8 = !{i16 0, i16 -32768}
!9 = !{i16 0, i16 -32768}
!10 = !{i16 0, i16 -32768}
