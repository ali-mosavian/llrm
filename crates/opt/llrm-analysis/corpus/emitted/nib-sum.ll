target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00ok\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00bad\00"

define internal cc 1005 i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = mul i16 %4, 2
  %9 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %8
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr addrspace(1) %9
  %12 = add i16 %10, %11
  store i16 %12, ptr %2, !tbaa !2
  br label %b4

b4:
  %13 = load i16, ptr %1, !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr %1, !tbaa !2
  br label %b2

b5:
  %15 = load i16, ptr %2, !tbaa !2
  ret i16 %15
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [12 x i8]
  store i16 6, ptr %2, !tbaa !2
  store i16 6, ptr %3, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %4, i16 0
  store i16 1, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %4, i16 1
  store i16 2, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %4, i16 2
  store i16 3, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %4, i16 3
  store i16 4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %4, i16 4
  store i16 5, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %4, i16 5
  store i16 6, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 6, ptr %1, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 6, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %11, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %1 to ptr addrspace(1)
  %15 = call cc 1005 addrspace(1) i16 @sum(ptr addrspace(1) %14)
  store i16 %15, ptr %0, !tbaa !2
  %16 = load i16, ptr %0, !tbaa !2
  %17 = icmp eq i16 %16, 21
  br i1 %17, label %b2, label %b3

b2:
  %18 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %18)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b3:
  %19 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %19)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  %20 = load i16, ptr %0, !tbaa !2
  ret i16 %20
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
