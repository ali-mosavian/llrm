target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 i16 @mix(i16 %0, i16 %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  store i16 %0, ptr %3, !tbaa !2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = zext i8 %2 to i16
  %6 = icmp ult i16 %5, 16
  br i1 %6, label %b2, label %b3

b2:
  %7 = zext i8 %2 to i16
  %8 = shl i16 %4, %7
  store i16 %8, ptr %3, !tbaa !2
  %9 = load i16, ptr %3, !tbaa !2
  %10 = or i16 %9, %1
  store i16 %10, ptr %3, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = xor i16 %11, 21845
  store i16 %12, ptr %3, !tbaa !2
  %13 = load i16, ptr %3, !tbaa !2
  %14 = and i16 %13, -256
  store i16 %14, ptr %3, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = zext i8 %2 to i16
  %17 = icmp ult i16 %16, 16
  br i1 %17, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$ESHF()
  unreachable

b4:
  %18 = zext i8 %2 to i16
  %19 = lshr i16 %15, %18
  store i16 %19, ptr %3, !tbaa !2
  %20 = load i16, ptr %3, !tbaa !2
  %21 = xor i16 %1, -1
  %22 = and i16 %21, %0
  %23 = xor i16 %22, %1
  %24 = or i16 %20, %23
  ret i16 %24

b5:
  call cc 1005 addrspace(1) void @N$ESHF()
  unreachable
}

define internal cc 1005 i32 @shifts(i32 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = icmp ult i16 %1, 32
  br i1 %2, label %b2, label %b3

b2:
  %3 = zext i16 %1 to i32
  %4 = ashr i32 %0, %3
  %5 = icmp ult i16 %1, 32
  br i1 %5, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$ESHF()
  unreachable

b4:
  %6 = zext i16 %1 to i32
  %7 = shl i32 %0, %6
  %8 = add i32 %4, %7
  ret i32 %8

b5:
  call cc 1005 addrspace(1) void @N$ESHF()
  unreachable
}

define internal cc 1005 i8 @logic(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = alloca i8
  %4 = icmp slt i16 %0, %1
  %5 = zext i1 %4 to i8
  store i8 %5, ptr %2, !tbaa !2
  br i1 %4, label %b2, label %b3

b2:
  %6 = icmp eq i16 %0, 0
  %7 = zext i1 %6 to i8
  %8 = xor i8 %7, 1
  store i8 %8, ptr %2, !tbaa !2
  br label %b3

b3:
  %9 = load i8, ptr %2, !tbaa !2, !range !5
  store i8 %9, ptr %3, !tbaa !2
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b5, label %b4

b4:
  %11 = icmp eq i16 %1, 7
  %12 = zext i1 %11 to i8
  store i8 %12, ptr %3, !tbaa !2
  br label %b5

b5:
  %13 = load i8, ptr %3, !tbaa !2, !range !6
  ret i8 %13
}

define internal cc 1005 i32 @widen(i8 %0, i8 %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = sext i8 %0 to i32
  %4 = zext i8 %1 to i32
  %5 = add i32 %3, %4
  %6 = zext i8 %2 to i32
  %7 = and i32 %6, 1
  %8 = add i32 %5, %7
  %9 = zext i8 %0 to i16
  %10 = sext i16 %9 to i32
  %11 = add i32 %8, %10
  ret i32 %11
}

define internal cc 1005 i16 @narrow(i32 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = trunc i32 %0 to i8
  %2 = sext i8 %1 to i16
  %3 = trunc i32 %0 to i8
  %4 = zext i8 %3 to i16
  %5 = add i16 %2, %4
  ret i16 %5
}

define internal cc 1005 i16 @filled(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [18 x i8]
  %8 = alloca i16
  %9 = mul i16 %0, 3
  store i16 %9, ptr %8, !tbaa !2
  store i16 9, ptr %5, !tbaa !2
  store i16 9, ptr %6, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 9, ptr %3, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i16, ptr %4, !tbaa !2
  %14 = load i16, ptr %8, !tbaa !2
  %15 = sub i16 %13, 0
  %16 = getelementptr inbounds i16, ptr %7, i16 %15
  store i16 %14, ptr %16, !tbaa !2
  br label %b4

b4:
  %17 = load i16, ptr %4, !tbaa !2
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %4, !tbaa !2
  br label %b2

b5:
  %19 = getelementptr inbounds i16, ptr %7, i16 4
  store i16 1, ptr %19, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  br label %b6

b6:
  %20 = load i16, ptr %1, !tbaa !2
  %21 = icmp ult i16 %20, 9
  br i1 %21, label %b7, label %b9

b7:
  %22 = load i16, ptr %2, !tbaa !2
  %23 = sub i16 %20, 0
  %24 = getelementptr inbounds i16, ptr %7, i16 %23
  %25 = load i16, ptr %24, !tbaa !2
  %26 = add i16 %22, %25
  store i16 %26, ptr %2, !tbaa !2
  br label %b8

b8:
  %27 = load i16, ptr %1, !tbaa !2
  %28 = add i16 %27, 1
  store i16 %28, ptr %1, !tbaa !2
  br label %b6

b9:
  %29 = load i16, ptr %2, !tbaa !2
  ret i16 %29
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i16
  %11 = call cc 1005 addrspace(1) i16 @mix(i16 4660, i16 -21555, i8 zeroext 3)
  store i16 %11, ptr %10, !tbaa !2
  %12 = call cc 1005 addrspace(1) i32 @shifts(i32 -100000, i16 3)
  store i32 %12, ptr %9, !tbaa !2
  %13 = call cc 1005 addrspace(1) i32 @shifts(i32 100000, i16 5)
  store i32 %13, ptr %8, !tbaa !2
  %14 = load i16, ptr %10, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %14)
  %15 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %15)
  %16 = load i32, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %16)
  %17 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %17)
  %18 = load i32, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %18)
  call cc 1005 addrspace(1) void @N$PN()
  %19 = call cc 1005 addrspace(1) i8 @logic(i16 1, i16 2)
  %20 = zext i8 %19 to i16
  %21 = and i16 %20, 1
  store i16 %21, ptr %7, !tbaa !2
  %22 = call cc 1005 addrspace(1) i8 @logic(i16 0, i16 2)
  %23 = zext i8 %22 to i16
  %24 = and i16 %23, 1
  store i16 %24, ptr %6, !tbaa !2
  %25 = call cc 1005 addrspace(1) i8 @logic(i16 3, i16 7)
  %26 = zext i8 %25 to i16
  %27 = and i16 %26, 1
  store i16 %27, ptr %5, !tbaa !2
  %28 = call cc 1005 addrspace(1) i8 @logic(i16 3, i16 2)
  %29 = zext i8 %28 to i16
  %30 = and i16 %29, 1
  store i16 %30, ptr %4, !tbaa !2
  %31 = load i16, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %31)
  %32 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %32)
  %33 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %33)
  %34 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %34)
  %35 = load i16, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %35)
  %36 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %36)
  %37 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %37)
  call cc 1005 addrspace(1) void @N$PN()
  %38 = call cc 1005 addrspace(1) i32 @widen(i8 signext -5, i8 zeroext -56, i8 1)
  store i32 %38, ptr %3, !tbaa !2
  %39 = call cc 1005 addrspace(1) i16 @narrow(i32 -129)
  store i16 %39, ptr %2, !tbaa !2
  %40 = call cc 1005 addrspace(1) i16 @narrow(i32 70000)
  store i16 %40, ptr %1, !tbaa !2
  %41 = call cc 1005 addrspace(1) i16 @filled(i16 5)
  store i16 %41, ptr %0, !tbaa !2
  %42 = load i32, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %42)
  %43 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %43)
  %44 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %44)
  %45 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %45)
  %46 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %46)
  %47 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %47)
  %48 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %48)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$ESHF() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
