target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00normal \00"
@$str2 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, warning \00"
@$str3 = internal constant [18 x i8] c"\08\00\0B\00\0B\00, inverted \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00byte waiting, ready to send\00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00errors: \00"
@$str6 = internal constant [15 x i8] c"\08\00\08\00\08\00control \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00: parity \00"
@$str8 = internal constant [16 x i8] c"\08\00\09\00\09\00, offset \00"

define internal cc 1005 i8 @inverted(i8 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i8
  store i8 %0, ptr %1, !tbaa !2
  %2 = load i8, ptr %1, !tbaa !2
  %3 = lshr i8 %0, 4
  %4 = and i8 %3, 7
  %5 = and i8 %4, 15
  %6 = and i8 %2, -16
  %7 = or i8 %6, %5
  store i8 %7, ptr %1, !tbaa !2
  %8 = load i8, ptr %1, !tbaa !2
  %9 = and i8 %0, 15
  %10 = zext i8 %9 to i16
  %11 = and i16 %10, 7
  %12 = trunc i16 %11 to i8
  %13 = and i8 %12, 7
  %14 = shl i8 %13, 4
  %15 = and i8 %8, -113
  %16 = or i8 %15, %14
  store i8 %16, ptr %1, !tbaa !2
  %17 = load i8, ptr %1, !tbaa !2
  ret i8 %17
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i8
  %6 = alloca i8
  %7 = alloca i8
  %8 = alloca i8
  store i8 7, ptr %8, !tbaa !2
  store i8 -50, ptr %7, !tbaa !2
  %9 = load i8, ptr %8, !tbaa !2
  store i8 %9, ptr %6, !tbaa !2
  %10 = load i8, ptr %7, !tbaa !2
  store i8 %10, ptr %5, !tbaa !2
  %11 = load i8, ptr %8, !tbaa !2
  %12 = call cc 1005 addrspace(1) i8 @inverted(i8 zeroext %11)
  store i8 %12, ptr %4, !tbaa !2
  %13 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %13)
  %14 = load i8, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %14)
  %15 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %15)
  %16 = load i8, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %16)
  %17 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %17)
  %18 = load i8, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %18)
  call cc 1005 addrspace(1) void @N$PN()
  store i8 97, ptr %3, !tbaa !2
  %19 = load i8, ptr %3, !tbaa !2
  %20 = and i8 %19, 1
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b5, label %b3

b2:
  %22 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b3:
  br label %b4

b4:
  %23 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %23)
  %24 = load i8, ptr %3, !tbaa !2
  %25 = lshr i8 %24, 1
  %26 = and i8 %25, 1
  %27 = icmp ne i8 %26, 0
  %28 = zext i1 %27 to i8
  store i8 %28, ptr %1, !tbaa !2
  br i1 %27, label %b7, label %b6

b5:
  %29 = load i8, ptr %3, !tbaa !2
  %30 = lshr i8 %29, 5
  %31 = and i8 %30, 1
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b2, label %b3

b6:
  %33 = load i8, ptr %3, !tbaa !2
  %34 = lshr i8 %33, 2
  %35 = and i8 %34, 1
  %36 = icmp ne i8 %35, 0
  %37 = zext i1 %36 to i8
  store i8 %37, ptr %1, !tbaa !2
  br label %b7

b7:
  %38 = load i8, ptr %1, !tbaa !2, !range !5
  store i8 %38, ptr %2, !tbaa !2
  %39 = icmp ne i8 %38, 0
  br i1 %39, label %b9, label %b8

b8:
  %40 = load i8, ptr %3, !tbaa !2
  %41 = lshr i8 %40, 3
  %42 = and i8 %41, 1
  %43 = icmp ne i8 %42, 0
  %44 = zext i1 %43 to i8
  store i8 %44, ptr %2, !tbaa !2
  br label %b9

b9:
  %45 = load i8, ptr %2, !tbaa !2, !range !6
  call cc 1005 addrspace(1) void @N$PB(i8 %45)
  call cc 1005 addrspace(1) void @N$PN()
  store i8 -45, ptr %0, !tbaa !2
  %46 = load i8, ptr %0, !tbaa !2
  %47 = and i8 %46, -5
  %48 = or i8 %47, 4
  store i8 %48, ptr %0, !tbaa !2
  %49 = load i8, ptr %0, !tbaa !2
  %50 = ashr i8 %49, 5
  %51 = sext i8 %50 to i16
  %52 = add i16 %51, 3
  %53 = trunc i16 %52 to i8
  %54 = and i8 %53, 7
  %55 = shl i8 %54, 5
  %56 = and i8 %49, 31
  %57 = or i8 %56, %55
  store i8 %57, ptr %0, !tbaa !2
  %58 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %58)
  %59 = load i8, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %59)
  %60 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %60)
  %61 = load i8, ptr %0, !tbaa !2
  %62 = lshr i8 %61, 3
  %63 = and i8 %62, 3
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %63)
  %64 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %64)
  %65 = load i8, ptr %0, !tbaa !2
  %66 = ashr i8 %65, 5
  call cc 1005 addrspace(1) void @N$PI1(i8 signext %66)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
