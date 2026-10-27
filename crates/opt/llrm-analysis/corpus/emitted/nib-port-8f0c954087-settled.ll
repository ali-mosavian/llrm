target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @value(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 8, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %2, !tbaa !2
  %5 = load i16, ptr %1, !tbaa !2
  %6 = icmp slt i16 %4, %5
  br i1 %6, label %b3, label %b5

b3:
  %7 = load i16, ptr %3, !tbaa !2
  %8 = load i16, ptr %2, !tbaa !2
  %9 = load i16, ptr addrspace(1) %0, !range !5
  %10 = icmp ult i16 %8, %9
  br i1 %10, label %b6, label %b7

b4:
  %11 = load i16, ptr %2, !tbaa !2
  %12 = add nsw i16 %11, 1
  store i16 %12, ptr %2, !tbaa !2
  br label %b2

b5:
  %13 = load i16, ptr %3, !tbaa !2
  ret i16 %13

b6:
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %15 = load ptr addrspace(1), ptr addrspace(1) %14
  %16 = mul i16 %8, 2
  %17 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 %16
  %18 = load i16, ptr addrspace(1) %17
  %19 = add i16 %7, %18
  store i16 %19, ptr %3, !tbaa !2
  br label %b4

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
