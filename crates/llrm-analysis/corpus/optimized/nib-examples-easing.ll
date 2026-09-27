target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00linear\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00in\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00out\00"
@$str5 = internal constant [13 x i8] c"\08\00\06\00\06\00smooth\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\00step\00"
@$str7 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @linear(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  ret i16 %0
}

define internal i16 @ease_in(i16 %0) addrspace(1) memory(none) willreturn {
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

define internal i16 @ease_out(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = sub i16 100, %0
  %2 = mul i16 %1, %1
  %3 = sdiv i16 %2, 100
  %4 = srem i16 %2, 100
  %5 = icmp ne i16 %4, 0
  %6 = sext i1 %5 to i8
  %7 = xor i16 %4, 100
  %8 = icmp slt i16 %7, 0
  %9 = sext i1 %8 to i8
  %10 = and i8 %6, %9
  %11 = sext i8 %10 to i16
  %12 = and i16 %11, 1
  %13 = sub i16 %3, %12
  %14 = sub i16 100, %13
  ret i16 %14
}

define internal i16 @Slide.at(ptr addrspace(1) %0, i16 %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load i16, ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %0
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = sub i16 %6, %4
  %8 = call addrspace(1) i16 @$call14(i16 %3, i16 %1)
  %9 = mul i16 %7, %8
  %10 = sdiv i16 %9, 100
  %11 = srem i16 %9, 100
  %12 = icmp ne i16 %11, 0
  %13 = sext i1 %12 to i8
  %14 = xor i16 %11, 100
  %15 = icmp slt i16 %14, 0
  %16 = sext i1 %15 to i8
  %17 = and i8 %13, %16
  %18 = sext i8 %17 to i16
  %19 = and i16 %18, 1
  %20 = sub i16 %10, %19
  %21 = add i16 %4, %20
  ret i16 %21
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 6, i1 false)
  %1 = getelementptr i8, ptr @$str1, i16 6
  %2 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 5, i16 2)
  %3 = getelementptr i8, ptr %2, i16 0
  %4 = getelementptr i8, ptr @$str2, i16 6
  store ptr %4, ptr %3
  %5 = getelementptr i8, ptr %2, i16 2
  %6 = getelementptr i8, ptr @$str3, i16 6
  store ptr %6, ptr %5
  %7 = getelementptr i8, ptr %2, i16 4
  %8 = getelementptr i8, ptr @$str4, i16 6
  store ptr %8, ptr %7
  %9 = getelementptr i8, ptr %2, i16 6
  %10 = getelementptr i8, ptr @$str5, i16 6
  store ptr %10, ptr %9
  %11 = getelementptr i8, ptr %2, i16 8
  %12 = getelementptr i8, ptr @$str6, i16 6
  store ptr %12, ptr %11
  %13 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 5, i16 2)
  %14 = getelementptr i8, ptr %13, i16 0
  store i16 0, ptr %14
  %15 = getelementptr i8, ptr %13, i16 2
  store i16 1, ptr %15
  %16 = getelementptr i8, ptr %13, i16 4
  store i16 2, ptr %16
  %17 = getelementptr i8, ptr %13, i16 6
  store i16 3, ptr %17
  %18 = getelementptr i8, ptr %13, i16 8
  store i16 4, ptr %18
  %19 = getelementptr i8, ptr %13, i16 -4
  %20 = load i16, ptr %19
  %21 = getelementptr inbounds i8, ptr %0, i16 2
  %22 = getelementptr inbounds i8, ptr %0, i16 4
  %23 = getelementptr i8, ptr %2, i16 -4
  %24 = getelementptr i8, ptr @$str7, i16 6
  %25 = addrspacecast ptr %0 to ptr addrspace(1)
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 4
  %27 = getelementptr i8, ptr addrspace(1) %25, i16 2
  br label %b2

b2:
  %28 = phi i16 [ 0, %b1 ], [ %77, %b13 ]
  %29 = icmp ult i16 %28, %20
  br i1 %29, label %b3, label %b5

b3:
  %30 = load i16, ptr %19
  %31 = icmp ult i16 %28, %30
  br i1 %31, label %b6, label %b7

b5:
  call addrspace(1) void @N$BDRP(ptr %13)
  %32 = icmp ne ptr %2, null
  br i1 %32, label %b15, label %b14

b6:
  %33 = shl i16 %28, 1
  %34 = getelementptr i8, ptr %13, i16 %33
  %35 = load i16, ptr %34
  store i16 0, ptr %0, !tbaa !2
  store i16 40, ptr %21, !tbaa !2
  store i16 %35, ptr %22, !tbaa !2
  call addrspace(1) void @N$PBEG()
  %36 = load i16, ptr %23
  %37 = icmp ult i16 %28, %36
  br i1 %37, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %38 = getelementptr i8, ptr %2, i16 %33
  %39 = load ptr, ptr %38
  call addrspace(1) void @N$PFLD(i8 6, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PS(ptr %39)
  call addrspace(1) void @N$PS(ptr %24)
  %40 = call addrspace(1) ptr @N$PEND()
  br label %b10

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  %41 = phi ptr [ %40, %b8 ], [ %75, %b11 ]
  %42 = phi i16 [ 0, %b8 ], [ %76, %b11 ]
  %43 = icmp slt i16 %42, 6
  br i1 %43, label %b11, label %b13

b11:
  %44 = mul i16 %42, 100
  %45 = sdiv i16 %44, 5
  %46 = srem i16 %44, 5
  %47 = icmp ne i16 %46, 0
  %48 = sext i1 %47 to i8
  %49 = xor i16 %46, 5
  %50 = icmp slt i16 %49, 0
  %51 = sext i1 %50 to i8
  %52 = and i8 %48, %51
  %53 = sext i8 %52 to i16
  %54 = and i16 %53, 1
  %55 = sub i16 %45, %54
  %56 = load i16, ptr addrspace(1) %26
  %57 = load i16, ptr addrspace(1) %25
  %58 = load i16, ptr addrspace(1) %27
  %59 = sub i16 %58, %57
  %60 = call addrspace(1) i16 @$call14(i16 %56, i16 %55)
  %61 = mul i16 %59, %60
  %62 = sdiv i16 %61, 100
  %63 = srem i16 %61, 100
  %64 = icmp ne i16 %63, 0
  %65 = sext i1 %64 to i8
  %66 = xor i16 %63, 100
  %67 = icmp slt i16 %66, 0
  %68 = sext i1 %67 to i8
  %69 = and i8 %65, %68
  %70 = sext i8 %69 to i16
  %71 = and i16 %70, 1
  %72 = sub i16 %62, %71
  %73 = add i16 %57, %72
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PS(ptr %24)
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %73)
  %74 = call addrspace(1) ptr @N$PEND()
  %75 = call addrspace(1) ptr @N$TAPP(ptr %41, ptr %74)
  call addrspace(1) void @N$BDRP(ptr %74)
  %76 = add i16 %42, 1
  br label %b10

b13:
  call addrspace(1) void @N$PS(ptr %41)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %41)
  %77 = add i16 %28, 1
  br label %b2

b14:
  call addrspace(1) void @N$BDRP(ptr %2)
  ret i16 0

b15:
  %78 = load i16, ptr %23
  br label %b16

b16:
  %79 = phi i16 [ 0, %b15 ], [ %84, %b18 ]
  %80 = icmp ult i16 %79, %78
  br i1 %80, label %b18, label %b14

b18:
  %81 = shl i16 %79, 1
  %82 = getelementptr i8, ptr %2, i16 %81
  %83 = load ptr, ptr %82
  call addrspace(1) void @N$BDRP(ptr %83)
  %84 = add i16 %79, 1
  br label %b16
}

define internal i16 @main$smooth(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = icmp slt i16 %0, 50
  br i1 %1, label %b2, label %b3

b2:
  %2 = shl i16 %0, 1
  %3 = mul i16 %2, %2
  %4 = sdiv i16 %3, 100
  %5 = srem i16 %3, 100
  %6 = icmp ne i16 %5, 0
  %7 = sext i1 %6 to i8
  %8 = xor i16 %5, 100
  %9 = icmp slt i16 %8, 0
  %10 = sext i1 %9 to i8
  %11 = and i8 %7, %10
  %12 = sext i8 %11 to i16
  %13 = and i16 %12, 1
  %14 = sub i16 %4, %13
  %15 = sdiv i16 %14, 2
  %16 = srem i16 %14, 2
  %17 = icmp ne i16 %16, 0
  %18 = sext i1 %17 to i8
  %19 = xor i16 %16, 2
  %20 = icmp slt i16 %19, 0
  %21 = sext i1 %20 to i8
  %22 = and i8 %18, %21
  %23 = sext i8 %22 to i16
  %24 = and i16 %23, 1
  %25 = sub i16 %15, %24
  ret i16 %25

b3:
  %26 = shl i16 %0, 1
  %27 = add i16 %26, -100
  %28 = sub i16 100, %27
  %29 = mul i16 %28, %28
  %30 = sdiv i16 %29, 100
  %31 = srem i16 %29, 100
  %32 = icmp ne i16 %31, 0
  %33 = sext i1 %32 to i8
  %34 = xor i16 %31, 100
  %35 = icmp slt i16 %34, 0
  %36 = sext i1 %35 to i8
  %37 = and i8 %33, %36
  %38 = sext i8 %37 to i16
  %39 = and i16 %38, 1
  %40 = sub i16 %30, %39
  %41 = sub i16 100, %40
  %42 = sdiv i16 %41, 2
  %43 = srem i16 %41, 2
  %44 = icmp ne i16 %43, 0
  %45 = sext i1 %44 to i8
  %46 = xor i16 %43, 2
  %47 = icmp slt i16 %46, 0
  %48 = sext i1 %47 to i8
  %49 = and i8 %45, %48
  %50 = sext i8 %49 to i16
  %51 = and i16 %50, 1
  %52 = sub i16 %42, %51
  %53 = add i16 %52, 50
  ret i16 %53
}

define internal i16 @main.$lambda2(i16 %0) addrspace(1) memory(none) willreturn {
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

define internal i16 @$call14(i16 %0, i16 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = icmp eq i16 %0, 0
  br i1 %2, label %4, label %b3

b3:
  %3 = icmp eq i16 %0, 1
  br i1 %3, label %b6, label %b5

4:
  ret i16 %1

b5:
  %5 = icmp eq i16 %0, 2
  br i1 %5, label %19, label %b7

b6:
  %6 = mul i16 %1, %1
  %7 = sdiv i16 %6, 100
  %8 = srem i16 %6, 100
  %9 = icmp ne i16 %8, 0
  %10 = sext i1 %9 to i8
  %11 = xor i16 %8, 100
  %12 = icmp slt i16 %11, 0
  %13 = sext i1 %12 to i8
  %14 = and i8 %10, %13
  %15 = sext i8 %14 to i16
  %16 = and i16 %15, 1
  %17 = sub i16 %7, %16
  ret i16 %17

b7:
  %18 = icmp eq i16 %0, 3
  br i1 %18, label %b10, label %34

19:
  %20 = sub i16 100, %1
  %21 = mul i16 %20, %20
  %22 = sdiv i16 %21, 100
  %23 = srem i16 %21, 100
  %24 = icmp ne i16 %23, 0
  %25 = sext i1 %24 to i8
  %26 = xor i16 %23, 100
  %27 = icmp slt i16 %26, 0
  %28 = sext i1 %27 to i8
  %29 = and i8 %25, %28
  %30 = sext i8 %29 to i16
  %31 = and i16 %30, 1
  %32 = sub i16 %22, %31
  %33 = sub i16 100, %32
  ret i16 %33

34:
  %35 = sdiv i16 %1, 50
  %36 = srem i16 %1, 50
  %37 = icmp ne i16 %36, 0
  %38 = sext i1 %37 to i8
  %39 = xor i16 %36, 50
  %40 = icmp slt i16 %39, 0
  %41 = sext i1 %40 to i8
  %42 = and i8 %38, %41
  %43 = sext i8 %42 to i16
  %44 = and i16 %43, 1
  %45 = sub i16 %35, %44
  %46 = mul i16 %45, 50
  ret i16 %46

b10:
  %47 = call addrspace(1) i16 @main$smooth(i16 %1)
  ret i16 %47
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
