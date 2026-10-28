target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal cc 1005 i16 @bump(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = add i16 %0, 1
  ret i16 %1
}

define internal cc 1005 void @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca ptr
  %3 = getelementptr i8, ptr @$str1, i16 6
  %4 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %3, i16 3, i16 2)
  %5 = getelementptr inbounds i8, ptr %4, i16 0
  store i16 1, ptr %5
  %6 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 2, ptr %6
  %7 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 3, ptr %7
  store ptr %4, ptr %2, !tbaa !2
  %8 = load ptr, ptr %2, !tbaa !2
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = icmp ult i16 0, %10
  br i1 %11, label %b2, label %b3

b2:
  %12 = getelementptr inbounds i8, ptr %8, i16 0
  %13 = load i16, ptr %12
  %14 = call cc 1005 addrspace(1) i16 @bump(i16 %13)
  store i16 %14, ptr %1, !tbaa !2
  %15 = load ptr, ptr %2, !tbaa !2
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  %18 = icmp ult i16 1, %17
  br i1 %18, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %19 = getelementptr inbounds i8, ptr %15, i16 2
  %20 = load i16, ptr %19
  %21 = call cc 1005 addrspace(1) i16 @bump(i16 %20)
  store i16 %21, ptr %0, !tbaa !2
  %22 = load i16, ptr %1, !tbaa !2
  %23 = load i16, ptr %0, !tbaa !2
  %24 = add i16 %22, %23
  %25 = load ptr, ptr %2, !tbaa !2
  %26 = getelementptr i8, ptr %25, i16 -4
  %27 = load i16, ptr %26
  %28 = icmp ult i16 2, %27
  br i1 %28, label %b6, label %b7

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b6:
  %29 = getelementptr inbounds i8, ptr %25, i16 4
  %30 = load i16, ptr %29
  %31 = add i16 %24, %30
  call cc 1005 addrspace(1) void @N$PI2(i16 %31)
  call cc 1005 addrspace(1) void @N$PN()
  %32 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %32)
  ret void

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
