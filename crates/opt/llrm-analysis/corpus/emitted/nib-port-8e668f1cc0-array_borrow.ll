target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00ok\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00bad\00"

define internal cc 1005 void @bump(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr addrspace(1) %0, !range !5
  %2 = icmp ult i16 1, %1
  br i1 %2, label %b2, label %b3

b2:
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  %5 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = add i16 %6, 3
  store i16 %7, ptr addrspace(1) %5
  ret void

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [6 x i8]
  store i16 3, ptr %1, !tbaa !2
  store i16 3, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %3, i16 0
  store i16 10, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %3, i16 1
  store i16 20, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %3, i16 2
  store i16 30, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %7, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @bump(ptr addrspace(1) %10)
  %11 = getelementptr inbounds i16, ptr %3, i16 1
  %12 = load i16, ptr %11, !tbaa !2
  %13 = icmp eq i16 %12, 23
  br i1 %13, label %b2, label %b3

b2:
  %14 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %14)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b3:
  %15 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %15)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  ret i16 0
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
