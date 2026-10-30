target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00..\00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00 clamped to \00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00% -> \00"

define cc 1000 i16 @CLAMP(i16 %0, i16 %1, i16 %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = icmp slt i16 %0, %1
  br i1 %3, label %b2, label %b3

b2:
  ret i16 %1

b3:
  br label %b4

b4:
  %4 = icmp sgt i16 %0, %2
  br i1 %4, label %b5, label %b6

b5:
  ret i16 %2

b6:
  br label %b7

b7:
  ret i16 %0
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca [4 x i8]
  %6 = alloca [4 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [12 x i8]
  store i16 6, ptr %7, !tbaa !2
  store i16 6, ptr %8, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %9, i16 0
  store i16 -40, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %9, i16 1
  store i16 15, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i16, ptr %9, i16 2
  store i16 70, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i16, ptr %9, i16 3
  store i16 99, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i16, ptr %9, i16 4
  store i16 140, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i16, ptr %9, i16 5
  store i16 260, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %9 to ptr addrspace(1)
  %17 = call cc 1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %16, i16 6)
  %18 = addrspacecast ptr %5 to ptr addrspace(1)
  store i32 %17, ptr addrspace(1) %18, !tbaa !2
  %19 = load i16, ptr %5, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %5, i16 2
  %21 = load i16, ptr %20, !tbaa !2
  store i16 %19, ptr %6, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %21, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @CLAMP_ALL(ptr addrspace(1) %23, i16 6, i16 0, i16 100)
  %24 = addrspacecast ptr %9 to ptr addrspace(1)
  %25 = call cc 1000 addrspace(1) i32 @SPAN(ptr addrspace(1) %24, i16 6)
  %26 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %25, ptr addrspace(1) %26, !tbaa !2
  %27 = load i16, ptr %3, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %3, i16 2
  %29 = load i16, ptr %28, !tbaa !2
  store i16 %27, ptr %4, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %29, ptr %30, !tbaa !2
  %31 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %31)
  %32 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %32)
  %33 = getelementptr inbounds i8, ptr %6, i16 2
  %34 = load i16, ptr %33, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %34)
  %35 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %35)
  %36 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %36)
  %37 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %37)
  %38 = getelementptr inbounds i8, ptr %4, i16 2
  %39 = load i16, ptr %38, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %39)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 0, ptr %2, !tbaa !2
  br label %b2

b2:
  %40 = load i16, ptr %2, !tbaa !2
  %41 = icmp ult i16 %40, 6
  br i1 %41, label %b3, label %b5

b3:
  %42 = sub i16 %40, 0
  %43 = getelementptr inbounds i16, ptr %9, i16 %42
  %44 = load i16, ptr %43, !tbaa !2
  store i16 %44, ptr %1, !tbaa !2
  %45 = sub i16 %40, 0
  %46 = getelementptr inbounds i16, ptr %9, i16 %45
  %47 = load i16, ptr %46, !tbaa !2
  %48 = call cc 1000 addrspace(1) i16 @SCALE(i16 %47, i16 255, i16 100)
  store i16 %48, ptr %0, !tbaa !2
  %49 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %49)
  %50 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %50)
  %51 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %51)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  %52 = load i16, ptr %2, !tbaa !2
  %53 = add i16 %52, 1
  store i16 %53, ptr %2, !tbaa !2
  br label %b2

b5:
  ret i16 0
}

declare cc 1000 i32 @SPAN(ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @CLAMP_ALL(ptr addrspace(1), i16, i16, i16) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1000 i16 @SCALE(i16, i16, i16) addrspace(1)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
