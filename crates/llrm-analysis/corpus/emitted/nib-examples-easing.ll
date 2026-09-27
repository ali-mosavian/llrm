target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00linear\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00in\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00out\00"
@$str5 = internal constant [13 x i8] c"\08\00\06\00\06\00smooth\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\00step\00"
@$str7 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @linear(i16 %0) addrspace(1) {
b1:
  ret i16 %0
}

define internal i16 @ease_in(i16 %0) addrspace(1) {
b1:
  %1 = mul i16 %0, %0
  %2 = sdiv i16 %1, 100
  %3 = srem i16 %1, 100
  %4 = icmp ne i16 %3, 0
  %5 = sext i1 %4 to i8
  %6 = xor i16 %3, 100
  %7 = icmp slt i16 %6, 0
  %8 = sext i1 %7 to i8
  %9 = and i8 %5, %8
  %10 = sext i8 %9 to i16
  %11 = and i16 %10, 1
  %12 = sub i16 %2, %11
  ret i16 %12
}

define internal i16 @ease_out(i16 %0) addrspace(1) {
b1:
  %1 = sub i16 100, %0
  %2 = call addrspace(1) i16 @ease_in(i16 %1)
  %3 = sub i16 100, %2
  ret i16 %3
}

define internal i16 @Slide.at(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  store i16 0, ptr %2
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load i16, ptr addrspace(1) %3
  store i16 %4, ptr %2, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = load i16, ptr addrspace(1) %0
  %9 = sub i16 %7, %8
  %10 = load i16, ptr %2, !tbaa !2
  %11 = call addrspace(1) i16 @$call14(i16 %10, i16 %1)
  %12 = mul i16 %9, %11
  %13 = sdiv i16 %12, 100
  %14 = srem i16 %12, 100
  %15 = icmp ne i16 %14, 0
  %16 = sext i1 %15 to i8
  %17 = xor i16 %14, 100
  %18 = icmp slt i16 %17, 0
  %19 = sext i1 %18 to i8
  %20 = and i8 %16, %19
  %21 = sext i8 %20 to i16
  %22 = and i16 %21, 1
  %23 = sub i16 %13, %22
  %24 = add i16 %5, %23
  ret i16 %24
}

define internal i16 @main() addrspace(1) {
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
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store ptr null, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 6, i1 false)
  store i16 0, ptr %6
  store i16 0, ptr %7
  store ptr null, ptr %8
  store ptr null, ptr %9
  %10 = getelementptr i8, ptr @$str1, i16 6
  %11 = call addrspace(1) ptr @N$BGRW(ptr %10, i16 5, i16 2)
  %12 = getelementptr i8, ptr %11, i16 0
  %13 = getelementptr i8, ptr @$str2, i16 6
  store ptr %13, ptr %12
  %14 = getelementptr i8, ptr %11, i16 2
  %15 = getelementptr i8, ptr @$str3, i16 6
  store ptr %15, ptr %14
  %16 = getelementptr i8, ptr %11, i16 4
  %17 = getelementptr i8, ptr @$str4, i16 6
  store ptr %17, ptr %16
  %18 = getelementptr i8, ptr %11, i16 6
  %19 = getelementptr i8, ptr @$str5, i16 6
  store ptr %19, ptr %18
  %20 = getelementptr i8, ptr %11, i16 8
  %21 = getelementptr i8, ptr @$str6, i16 6
  store ptr %21, ptr %20
  store ptr %11, ptr %9, !tbaa !2
  %22 = getelementptr i8, ptr @$str1, i16 6
  %23 = call addrspace(1) ptr @N$BGRW(ptr %22, i16 5, i16 2)
  %24 = getelementptr i8, ptr %23, i16 0
  store i16 0, ptr %24
  %25 = getelementptr i8, ptr %23, i16 2
  store i16 1, ptr %25
  %26 = getelementptr i8, ptr %23, i16 4
  store i16 2, ptr %26
  %27 = getelementptr i8, ptr %23, i16 6
  store i16 3, ptr %27
  %28 = getelementptr i8, ptr %23, i16 8
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
  %35 = sext i1 %34 to i8
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b3, label %b5

b3:
  %37 = load ptr, ptr %8, !tbaa !2
  %38 = load i16, ptr %7, !tbaa !2
  %39 = getelementptr i8, ptr %37, i16 -4
  %40 = load i16, ptr %39
  %41 = icmp ult i16 %38, %40
  %42 = sext i1 %41 to i8
  %43 = icmp ne i8 %42, 0
  br i1 %43, label %b6, label %b7

b4:
  %44 = load i16, ptr %7, !tbaa !2
  %45 = add i16 %44, 1
  store i16 %45, ptr %7, !tbaa !2
  br label %b2

b5:
  %46 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %46)
  %47 = load ptr, ptr %9, !tbaa !2
  %48 = icmp ne ptr %47, null
  %49 = sext i1 %48 to i8
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b15, label %b14

b6:
  %51 = mul i16 %38, 2
  %52 = getelementptr i8, ptr %37, i16 %51
  %53 = load i16, ptr %52
  store i16 0, ptr %5, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 40, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 %53, ptr %55, !tbaa !2
  call addrspace(1) void @N$PBEG()
  %56 = load ptr, ptr %9, !tbaa !2
  %57 = load i16, ptr %7, !tbaa !2
  %58 = getelementptr i8, ptr %56, i16 -4
  %59 = load i16, ptr %58
  %60 = icmp ult i16 %57, %59
  %61 = sext i1 %60 to i8
  %62 = icmp ne i8 %61, 0
  br i1 %62, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %63 = mul i16 %57, 2
  %64 = getelementptr i8, ptr %56, i16 %63
  %65 = load ptr, ptr %64
  call addrspace(1) void @N$PFLD(i8 6, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PS(ptr %65)
  %66 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %66)
  %67 = call addrspace(1) ptr @N$PEND()
  store ptr %67, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 6, ptr %2, !tbaa !2
  br label %b10

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  %68 = load i16, ptr %3, !tbaa !2
  %69 = load i16, ptr %2, !tbaa !2
  %70 = icmp slt i16 %68, %69
  %71 = sext i1 %70 to i8
  %72 = icmp ne i8 %71, 0
  br i1 %72, label %b11, label %b13

b11:
  %73 = addrspacecast ptr %5 to ptr addrspace(1)
  %74 = load i16, ptr %3, !tbaa !2
  %75 = mul i16 %74, 100
  %76 = sdiv i16 %75, 5
  %77 = srem i16 %75, 5
  %78 = icmp ne i16 %77, 0
  %79 = sext i1 %78 to i8
  %80 = xor i16 %77, 5
  %81 = icmp slt i16 %80, 0
  %82 = sext i1 %81 to i8
  %83 = and i8 %79, %82
  %84 = sext i8 %83 to i16
  %85 = and i16 %84, 1
  %86 = sub i16 %76, %85
  %87 = call addrspace(1) i16 @Slide.at(ptr addrspace(1) %73, i16 %86)
  store i16 %87, ptr %1, !tbaa !2
  call addrspace(1) void @N$PBEG()
  %88 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %88)
  %89 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %89)
  %90 = call addrspace(1) ptr @N$PEND()
  %91 = load ptr, ptr %4, !tbaa !2
  %92 = call addrspace(1) ptr @N$TAPP(ptr %91, ptr %90)
  store ptr %92, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %90)
  br label %b12

b12:
  %93 = load i16, ptr %3, !tbaa !2
  %94 = add i16 %93, 1
  store i16 %94, ptr %3, !tbaa !2
  br label %b10

b13:
  %95 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$PS(ptr %95)
  call addrspace(1) void @N$PN()
  %96 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %96)
  br label %b4

b14:
  call addrspace(1) void @N$BDRP(ptr %47)
  ret i16 0

b15:
  %97 = getelementptr i8, ptr %47, i16 -4
  %98 = load i16, ptr %97
  store i16 0, ptr %0, !tbaa !2
  br label %b16

b16:
  %99 = load i16, ptr %0, !tbaa !2
  %100 = icmp ult i16 %99, %98
  %101 = sext i1 %100 to i8
  %102 = icmp ne i8 %101, 0
  br i1 %102, label %b18, label %b17

b17:
  br label %b14

b18:
  %103 = mul i16 %99, 2
  %104 = getelementptr i8, ptr %47, i16 %103
  %105 = load ptr, ptr %104
  call addrspace(1) void @N$BDRP(ptr %105)
  %106 = add i16 %99, 1
  store i16 %106, ptr %0, !tbaa !2
  br label %b16
}

define internal i16 @main$smooth(i16 %0) addrspace(1) {
b1:
  %1 = icmp slt i16 %0, 50
  %2 = sext i1 %1 to i8
  %3 = icmp ne i8 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = mul i16 %0, 2
  %5 = call addrspace(1) i16 @ease_in(i16 %4)
  %6 = sdiv i16 %5, 2
  %7 = srem i16 %5, 2
  %8 = icmp ne i16 %7, 0
  %9 = sext i1 %8 to i8
  %10 = xor i16 %7, 2
  %11 = icmp slt i16 %10, 0
  %12 = sext i1 %11 to i8
  %13 = and i8 %9, %12
  %14 = sext i8 %13 to i16
  %15 = and i16 %14, 1
  %16 = sub i16 %6, %15
  ret i16 %16

b3:
  br label %b4

b4:
  %17 = mul i16 %0, 2
  %18 = sub i16 %17, 100
  %19 = call addrspace(1) i16 @ease_out(i16 %18)
  %20 = sdiv i16 %19, 2
  %21 = srem i16 %19, 2
  %22 = icmp ne i16 %21, 0
  %23 = sext i1 %22 to i8
  %24 = xor i16 %21, 2
  %25 = icmp slt i16 %24, 0
  %26 = sext i1 %25 to i8
  %27 = and i8 %23, %26
  %28 = sext i8 %27 to i16
  %29 = and i16 %28, 1
  %30 = sub i16 %20, %29
  %31 = add i16 50, %30
  ret i16 %31
}

define internal i16 @main.$lambda2(i16 %0) addrspace(1) {
b1:
  %1 = sdiv i16 %0, 50
  %2 = srem i16 %0, 50
  %3 = icmp ne i16 %2, 0
  %4 = sext i1 %3 to i8
  %5 = xor i16 %2, 50
  %6 = icmp slt i16 %5, 0
  %7 = sext i1 %6 to i8
  %8 = and i8 %4, %7
  %9 = sext i8 %8 to i16
  %10 = and i16 %9, 1
  %11 = sub i16 %1, %10
  %12 = mul i16 %11, 50
  ret i16 %12
}

define internal i16 @$call14(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = icmp eq i16 %0, 0
  %3 = sext i1 %2 to i8
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b4, label %b3

b3:
  %5 = icmp eq i16 %0, 1
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b6, label %b5

b4:
  %8 = call addrspace(1) i16 @linear(i16 %1)
  ret i16 %8

b5:
  %9 = icmp eq i16 %0, 2
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b8, label %b7

b6:
  %12 = call addrspace(1) i16 @ease_in(i16 %1)
  ret i16 %12

b7:
  %13 = icmp eq i16 %0, 3
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b10, label %b9

b8:
  %16 = call addrspace(1) i16 @ease_out(i16 %1)
  ret i16 %16

b9:
  %17 = call addrspace(1) i16 @main.$lambda2(i16 %1)
  ret i16 %17

b10:
  %18 = call addrspace(1) i16 @main$smooth(i16 %1)
  ret i16 %18
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$TAPP(ptr, ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
