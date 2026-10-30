target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00linear\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00in\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00out\00"
@$str5 = internal constant [13 x i8] c"\08\00\06\00\06\00smooth\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\00step\00"
@$str7 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 i16 @linear(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  ret i16 %0
}

define internal cc 1005 i16 @ease_in(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = mul i16 %0, %0
  %2 = sdiv i16 %1, 100
  %3 = srem i16 %1, 100
  %4 = icmp ne i16 %3, 0
  %5 = zext i1 %4 to i8
  %6 = xor i16 %3, 100
  %7 = icmp slt i16 %6, 0
  %8 = zext i1 %7 to i8
  %9 = and i8 %5, %8
  %10 = zext i8 %9 to i16
  %11 = and i16 %10, 1
  %12 = sub i16 %2, %11
  ret i16 %12
}

define internal cc 1005 i16 @ease_out(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = sub i16 100, %0
  %2 = call cc 1005 addrspace(1) i16 @ease_in(i16 %1)
  %3 = sub i16 100, %2
  ret i16 %3
}

define internal cc 1005 i16 @Slide.at(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load i16, ptr addrspace(1) %3
  store i16 %4, ptr %2, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = load i16, ptr addrspace(1) %0
  %9 = sub i16 %7, %8
  %10 = load i16, ptr %2, !tbaa !2
  %11 = call cc 1005 addrspace(1) i16 @$call14(i16 %10, i16 %1)
  %12 = mul i16 %9, %11
  %13 = sdiv i16 %12, 100
  %14 = srem i16 %12, 100
  %15 = icmp ne i16 %14, 0
  %16 = zext i1 %15 to i8
  %17 = xor i16 %14, 100
  %18 = icmp slt i16 %17, 0
  %19 = zext i1 %18 to i8
  %20 = and i8 %16, %19
  %21 = zext i8 %20 to i16
  %22 = and i16 %21, 1
  %23 = sub i16 %13, %22
  %24 = add i16 %5, %23
  ret i16 %24
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca ptr
  %5 = alloca [6 x i8]
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca ptr
  %9 = alloca ptr
  %10 = getelementptr i8, ptr @$str1, i16 6
  %11 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %10, i16 5, i16 2)
  %12 = getelementptr inbounds i8, ptr %11, i16 0
  %13 = getelementptr i8, ptr @$str2, i16 6
  store ptr %13, ptr %12
  %14 = getelementptr inbounds i8, ptr %11, i16 2
  %15 = getelementptr i8, ptr @$str3, i16 6
  store ptr %15, ptr %14
  %16 = getelementptr inbounds i8, ptr %11, i16 4
  %17 = getelementptr i8, ptr @$str4, i16 6
  store ptr %17, ptr %16
  %18 = getelementptr inbounds i8, ptr %11, i16 6
  %19 = getelementptr i8, ptr @$str5, i16 6
  store ptr %19, ptr %18
  %20 = getelementptr inbounds i8, ptr %11, i16 8
  %21 = getelementptr i8, ptr @$str6, i16 6
  store ptr %21, ptr %20
  store ptr %11, ptr %9, !tbaa !2
  %22 = getelementptr i8, ptr @$str1, i16 6
  %23 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %22, i16 5, i16 2)
  %24 = getelementptr inbounds i8, ptr %23, i16 0
  store i16 0, ptr %24
  %25 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 1, ptr %25
  %26 = getelementptr inbounds i8, ptr %23, i16 4
  store i16 2, ptr %26
  %27 = getelementptr inbounds i8, ptr %23, i16 6
  store i16 3, ptr %27
  %28 = getelementptr inbounds i8, ptr %23, i16 8
  store i16 4, ptr %28
  store ptr %23, ptr %8, !tbaa !2
  %29 = load ptr, ptr %8, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  store i16 0, ptr %7, !tbaa !2
  store i16 %31, ptr %6, !tbaa !2
  br label %b2

b2:
  %32 = load i16, ptr %7, !tbaa !2
  %33 = load i16, ptr %6, !tbaa !2
  %34 = icmp ult i16 %32, %33
  br i1 %34, label %b3, label %b5

b3:
  %35 = load ptr, ptr %8, !tbaa !2
  %36 = load i16, ptr %7, !tbaa !2
  %37 = getelementptr i8, ptr %35, i16 -4
  %38 = load i16, ptr %37
  %39 = icmp ult i16 %36, %38
  br i1 %39, label %b6, label %b7

b4:
  %40 = load i16, ptr %7, !tbaa !2
  %41 = add nuw i16 %40, 1
  store i16 %41, ptr %7, !tbaa !2
  br label %b2

b5:
  %42 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %42)
  %43 = load ptr, ptr %9, !tbaa !2
  %44 = icmp ne ptr %43, null
  br i1 %44, label %b15, label %b14

b6:
  %45 = mul i16 %36, 2
  %46 = getelementptr inbounds i8, ptr %35, i16 %45
  %47 = load i16, ptr %46
  store i16 0, ptr %5, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 40, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 %47, ptr %49, !tbaa !2
  call cc 1005 addrspace(1) void @N$PBEG()
  %50 = load ptr, ptr %9, !tbaa !2
  %51 = load i16, ptr %7, !tbaa !2
  %52 = getelementptr i8, ptr %50, i16 -4
  %53 = load i16, ptr %52
  %54 = icmp ult i16 %51, %53
  br i1 %54, label %b8, label %b9

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %55 = mul i16 %51, 2
  %56 = getelementptr inbounds i8, ptr %50, i16 %55
  %57 = load ptr, ptr %56
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 6, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PS(ptr %57)
  %58 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %58)
  %59 = call cc 1005 addrspace(1) ptr @N$PEND()
  store ptr %59, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 6, ptr %2, !tbaa !2
  br label %b10

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b10:
  %60 = load i16, ptr %3, !tbaa !2
  %61 = load i16, ptr %2, !tbaa !2
  %62 = icmp slt i16 %60, %61
  br i1 %62, label %b11, label %b13

b11:
  %63 = addrspacecast ptr %5 to ptr addrspace(1)
  %64 = load i16, ptr %3, !tbaa !2
  %65 = mul i16 %64, 100
  %66 = sdiv i16 %65, 5
  %67 = srem i16 %65, 5
  %68 = icmp ne i16 %67, 0
  %69 = zext i1 %68 to i8
  %70 = xor i16 %67, 5
  %71 = icmp slt i16 %70, 0
  %72 = zext i1 %71 to i8
  %73 = and i8 %69, %72
  %74 = zext i8 %73 to i16
  %75 = and i16 %74, 1
  %76 = sub i16 %66, %75
  %77 = call cc 1005 addrspace(1) i16 @Slide.at(ptr addrspace(1) %63, i16 %76)
  store i16 %77, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PBEG()
  %78 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %78)
  %79 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %79)
  %80 = call cc 1005 addrspace(1) ptr @N$PEND()
  %81 = load ptr, ptr %4, !tbaa !2
  %82 = call cc 1005 addrspace(1) ptr @N$TAPP(ptr %81, ptr %80)
  store ptr %82, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %80)
  br label %b12

b12:
  %83 = load i16, ptr %3, !tbaa !2
  %84 = add nsw i16 %83, 1
  store i16 %84, ptr %3, !tbaa !2
  br label %b10

b13:
  %85 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %85)
  call cc 1005 addrspace(1) void @N$PN()
  %86 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %86)
  br label %b4

b14:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %43)
  ret i16 0

b15:
  %87 = getelementptr i8, ptr %43, i16 -4
  %88 = load i16, ptr %87
  store i16 0, ptr %0, !tbaa !2
  br label %b16

b16:
  %89 = load i16, ptr %0, !tbaa !2
  %90 = icmp ult i16 %89, %88
  br i1 %90, label %b18, label %b17

b17:
  br label %b14

b18:
  %91 = mul i16 %89, 2
  %92 = getelementptr inbounds i8, ptr %43, i16 %91
  %93 = load ptr, ptr %92
  call cc 1005 addrspace(1) void @N$BDRP(ptr %93)
  %94 = add i16 %89, 1
  store i16 %94, ptr %0, !tbaa !2
  br label %b16
}

define internal cc 1005 i16 @main$smooth(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = icmp slt i16 %0, 50
  br i1 %1, label %b2, label %b3

b2:
  %2 = mul i16 %0, 2
  %3 = call cc 1005 addrspace(1) i16 @ease_in(i16 %2)
  %4 = sdiv i16 %3, 2
  %5 = srem i16 %3, 2
  %6 = icmp ne i16 %5, 0
  %7 = zext i1 %6 to i8
  %8 = xor i16 %5, 2
  %9 = icmp slt i16 %8, 0
  %10 = zext i1 %9 to i8
  %11 = and i8 %7, %10
  %12 = zext i8 %11 to i16
  %13 = and i16 %12, 1
  %14 = sub i16 %4, %13
  ret i16 %14

b3:
  br label %b4

b4:
  %15 = mul i16 %0, 2
  %16 = sub i16 %15, 100
  %17 = call cc 1005 addrspace(1) i16 @ease_out(i16 %16)
  %18 = sdiv i16 %17, 2
  %19 = srem i16 %17, 2
  %20 = icmp ne i16 %19, 0
  %21 = zext i1 %20 to i8
  %22 = xor i16 %19, 2
  %23 = icmp slt i16 %22, 0
  %24 = zext i1 %23 to i8
  %25 = and i8 %21, %24
  %26 = zext i8 %25 to i16
  %27 = and i16 %26, 1
  %28 = sub i16 %18, %27
  %29 = add i16 50, %28
  ret i16 %29
}

define internal cc 1005 i16 @main.$lambda2(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = sdiv i16 %0, 50
  %2 = srem i16 %0, 50
  %3 = icmp ne i16 %2, 0
  %4 = zext i1 %3 to i8
  %5 = xor i16 %2, 50
  %6 = icmp slt i16 %5, 0
  %7 = zext i1 %6 to i8
  %8 = and i8 %4, %7
  %9 = zext i8 %8 to i16
  %10 = and i16 %9, 1
  %11 = sub i16 %1, %10
  %12 = mul i16 %11, 50
  ret i16 %12
}

define internal cc 1005 i16 @$call14(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = icmp eq i16 %0, 0
  br i1 %2, label %b4, label %b3

b3:
  %3 = icmp eq i16 %0, 1
  br i1 %3, label %b6, label %b5

b4:
  %4 = call cc 1005 addrspace(1) i16 @linear(i16 %1)
  ret i16 %4

b5:
  %5 = icmp eq i16 %0, 2
  br i1 %5, label %b8, label %b7

b6:
  %6 = call cc 1005 addrspace(1) i16 @ease_in(i16 %1)
  ret i16 %6

b7:
  %7 = icmp eq i16 %0, 3
  br i1 %7, label %b10, label %b9

b8:
  %8 = call cc 1005 addrspace(1) i16 @ease_out(i16 %1)
  ret i16 %8

b9:
  %9 = call cc 1005 addrspace(1) i16 @main.$lambda2(i16 %1)
  ret i16 %9

b10:
  %10 = call cc 1005 addrspace(1) i16 @main$smooth(i16 %1)
  ret i16 %10
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$TAPP(ptr, ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
