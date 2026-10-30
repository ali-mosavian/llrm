target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 void @fill(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca ptr addrspace(1)
  %4 = inttoptr i32 -1207959552 to ptr addrspace(1)
  store ptr addrspace(1) %4, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp sle i16 %5, 3998
  br i1 %6, label %b3, label %b4

b3:
  %7 = load ptr addrspace(1), ptr %3, !tbaa !2
  %8 = load i16, ptr %2, !tbaa !2
  %9 = mul i16 %8, 1
  %10 = getelementptr i8, ptr addrspace(1) %7, i16 %9
  %11 = load i16, ptr %2, !tbaa !2
  %12 = and i16 %11, 15
  %13 = add i16 %0, %12
  %14 = trunc i16 %13 to i8
  store i8 %14, ptr addrspace(1) %10
  %15 = load ptr addrspace(1), ptr %3, !tbaa !2
  %16 = load i16, ptr %2, !tbaa !2
  %17 = add i16 %16, 1
  %18 = mul i16 %17, 1
  %19 = getelementptr i8, ptr addrspace(1) %15, i16 %18
  %20 = trunc i16 %1 to i8
  store i8 %20, ptr addrspace(1) %19
  %21 = load i16, ptr %2, !tbaa !2
  %22 = add i16 %21, 2
  store i16 %22, ptr %2, !tbaa !2
  br label %b2

b4:
  ret void
}

define internal cc 1005 i32 @checksum() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca ptr addrspace(1)
  %3 = alloca i32
  store i32 0, ptr %3, !tbaa !2
  %4 = inttoptr i32 -1207959552 to ptr addrspace(1)
  store ptr addrspace(1) %4, ptr %2, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  store i16 4000, ptr %0, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %1, !tbaa !2
  %6 = load i16, ptr %0, !tbaa !2
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %b3, label %b5

b3:
  %8 = load i32, ptr %3, !tbaa !2
  %9 = load ptr addrspace(1), ptr %2, !tbaa !2
  %10 = load i16, ptr %1, !tbaa !2
  %11 = mul i16 %10, 1
  %12 = getelementptr i8, ptr addrspace(1) %9, i16 %11
  %13 = load i8, ptr addrspace(1) %12
  %14 = zext i8 %13 to i32
  %15 = add i32 %8, %14
  store i32 %15, ptr %3, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b2

b5:
  %18 = load i32, ptr %3, !tbaa !2
  ret i32 %18
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
