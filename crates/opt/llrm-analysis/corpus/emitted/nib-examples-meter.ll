target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00health \00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 void @Player.hit(ptr addrspace(1) nonnull dereferenceable(6) noalias nocapture %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i8, ptr addrspace(1) %0
  %3 = call cc 1005 addrspace(1) i8 @u8.saturating_sub(i8 zeroext %2, i8 zeroext %1)
  store i8 %3, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 void @Player.heal(ptr addrspace(1) nonnull dereferenceable(6) noalias nocapture %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i8, ptr addrspace(1) %0
  %3 = call cc 1005 addrspace(1) i8 @u8.saturating_add(i8 zeroext %2, i8 zeroext %1)
  store i8 %3, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 i8 @Player.award(ptr addrspace(1) nonnull dereferenceable(6) noalias nocapture %0, i32 %1, i32 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i32
  %4 = alloca [6 x i8]
  %5 = alloca i32
  %6 = alloca [6 x i8]
  %7 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @i32.checked_mul(ptr addrspace(1) %7, i32 %1, i32 %2)
  %8 = load i8, ptr %6, !tbaa !2, !range !10
  %9 = icmp eq i8 %8, 0
  br i1 %9, label %b4, label %b3

b3:
  ret i8 0

b4:
  %10 = getelementptr inbounds i8, ptr %6, i16 2
  %11 = load i32, ptr %10, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %6, i16 2
  %13 = load i32, ptr %12, !tbaa !2
  store i32 %13, ptr %5, !tbaa !2
  %14 = addrspacecast ptr %4 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %16 = load i32, ptr addrspace(1) %15
  %17 = load i32, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @i32.checked_add(ptr addrspace(1) %14, i32 %16, i32 %17)
  %18 = load i8, ptr %4, !tbaa !2, !range !10
  %19 = icmp eq i8 %18, 0
  br i1 %19, label %b7, label %b6

b6:
  ret i8 0

b7:
  %20 = getelementptr inbounds i8, ptr %4, i16 2
  %21 = load i32, ptr %20, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %4, i16 2
  %23 = load i32, ptr %22, !tbaa !2
  store i32 %23, ptr %3, !tbaa !2
  %24 = load i32, ptr %3, !tbaa !2
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %24, ptr addrspace(1) %25
  ret i8 1
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca [6 x i8]
  store i8 100, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %3, i16 2
  store i32 2000000000, ptr %4, !tbaa !2
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Player.hit(ptr addrspace(1) %5, i8 zeroext 30)
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Player.heal(ptr addrspace(1) %6, i8 zeroext -56)
  %7 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %7)
  %8 = load i8, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %8)
  call cc 1005 addrspace(1) void @N$PN()
  %9 = addrspacecast ptr %3 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Player.hit(ptr addrspace(1) %9, i8 zeroext -1)
  %10 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %10)
  %11 = load i8, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %11)
  call cc 1005 addrspace(1) void @N$PN()
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  %13 = call cc 1005 addrspace(1) i8 @Player.award(ptr addrspace(1) %12, i32 1000, i32 100000)
  store i8 %13, ptr %2, !tbaa !2
  %14 = load i8, ptr %2, !tbaa !2, !range !5
  call cc 1005 addrspace(1) void @N$PB(i8 %14)
  %15 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %15)
  %16 = getelementptr inbounds i8, ptr %3, i16 2
  %17 = load i32, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %17)
  call cc 1005 addrspace(1) void @N$PN()
  %18 = addrspacecast ptr %3 to ptr addrspace(1)
  %19 = call cc 1005 addrspace(1) i8 @Player.award(ptr addrspace(1) %18, i32 100000, i32 100000)
  store i8 %19, ptr %1, !tbaa !2
  %20 = load i8, ptr %1, !tbaa !2, !range !6
  call cc 1005 addrspace(1) void @N$PB(i8 %20)
  %21 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  %22 = getelementptr inbounds i8, ptr %3, i16 2
  %23 = load i32, ptr %22, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %23)
  call cc 1005 addrspace(1) void @N$PN()
  %24 = addrspacecast ptr %3 to ptr addrspace(1)
  %25 = call cc 1005 addrspace(1) i8 @Player.award(ptr addrspace(1) %24, i32 100000000, i32 2)
  store i8 %25, ptr %0, !tbaa !2
  %26 = load i8, ptr %0, !tbaa !2, !range !7
  call cc 1005 addrspace(1) void @N$PB(i8 %26)
  %27 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %27)
  %28 = getelementptr inbounds i8, ptr %3, i16 2
  %29 = load i32, ptr %28, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %29)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

define internal cc 1005 void @i32.checked_add(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i32
  %4 = add i32 %1, %2
  store i32 %4, ptr %3, !tbaa !2
  %5 = load i32, ptr %3, !tbaa !2
  %6 = xor i32 %1, %5
  %7 = load i32, ptr %3, !tbaa !2
  %8 = xor i32 %2, %7
  %9 = and i32 %6, %8
  %10 = icmp slt i32 %9, 0
  br i1 %10, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  br label %b4

b4:
  %11 = load i32, ptr %3, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %11, ptr addrspace(1) %12
  ret void
}

define internal cc 1005 void @i32.checked_mul(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i32
  %6 = mul i32 %1, %2
  store i32 %6, ptr %5, !tbaa !2
  %7 = icmp eq i32 %1, -1
  br i1 %7, label %b5, label %b6

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  br label %b4

b4:
  %8 = load i32, ptr %5, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %8, ptr addrspace(1) %9
  ret void

b5:
  %10 = icmp eq i32 %2, -2147483648
  %11 = zext i1 %10 to i8
  store i8 %11, ptr %4, !tbaa !2
  br label %b7

b6:
  %12 = icmp ne i32 %1, 0
  %13 = zext i1 %12 to i8
  store i8 %13, ptr %3, !tbaa !2
  br i1 %12, label %b8, label %b9

b7:
  %14 = load i8, ptr %4, !tbaa !2, !range !8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b2, label %b3

b8:
  %16 = load i32, ptr %5, !tbaa !2
  %17 = sdiv i32 %16, %1
  %18 = srem i32 %16, %1
  %19 = icmp ne i32 %18, 0
  %20 = zext i1 %19 to i8
  %21 = xor i32 %18, %1
  %22 = icmp slt i32 %21, 0
  %23 = zext i1 %22 to i8
  %24 = and i8 %20, %23
  %25 = zext i8 %24 to i32
  %26 = and i32 %25, 1
  %27 = sub i32 %17, %26
  %28 = icmp ne i32 %27, %2
  %29 = zext i1 %28 to i8
  store i8 %29, ptr %3, !tbaa !2
  br label %b9

b9:
  %30 = load i8, ptr %3, !tbaa !2, !range !9
  store i8 %30, ptr %4, !tbaa !2
  br label %b7
}

define internal cc 1005 i8 @u8.saturating_add(i8 %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = zext i8 %0 to i16
  %4 = zext i8 %1 to i16
  %5 = add i16 %3, %4
  %6 = trunc i16 %5 to i8
  store i8 %6, ptr %2, !tbaa !2
  %7 = load i8, ptr %2, !tbaa !2
  %8 = zext i8 %7 to i16
  %9 = zext i8 %0 to i16
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %b2, label %b3

b2:
  ret i8 -1

b3:
  br label %b4

b4:
  %11 = load i8, ptr %2, !tbaa !2
  ret i8 %11
}

define internal cc 1005 i8 @u8.saturating_sub(i8 %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = zext i8 %0 to i16
  %4 = zext i8 %1 to i16
  %5 = sub i16 %3, %4
  %6 = trunc i16 %5 to i8
  store i8 %6, ptr %2, !tbaa !2
  %7 = zext i8 %0 to i16
  %8 = zext i8 %1 to i16
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %b2, label %b3

b2:
  ret i8 0

b3:
  br label %b4

b4:
  %10 = load i8, ptr %2, !tbaa !2
  ret i8 %10
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
