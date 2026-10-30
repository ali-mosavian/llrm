target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i8
  %6 = alloca i8
  store i8 -128, ptr %6, !tbaa !2
  store i8 -1, ptr %5, !tbaa !2
  store i16 -32768, ptr %4, !tbaa !2
  store i16 -1, ptr %3, !tbaa !2
  store i32 -2147483648, ptr %2, !tbaa !2
  store i32 -1, ptr %1, !tbaa !2
  store i32 0, ptr %0, !tbaa !2
  %7 = load i8, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI1(i8 signext %7)
  %8 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %8)
  %9 = load i8, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %9)
  %10 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %10)
  %11 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %11)
  %12 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %12)
  %13 = load i16, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %13)
  %14 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %14)
  %15 = load i32, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %15)
  %16 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %16)
  %17 = load i32, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU4(i32 %17)
  %18 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %18)
  %19 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %19)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$PI1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
