target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 .:-=+*#%@\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00step \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00, limit \00"

define internal i16 @escape(i32 %0, i32 %1) addrspace(1) {
b1:
  %2 = alloca i32
  %3 = alloca i32
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i32
  %7 = alloca i32
  store i32 0, ptr %2
  store i32 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i32 0, ptr %6
  store i32 0, ptr %7
  store i32 0, ptr %7, !tbaa !2
  store i32 0, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 40, ptr %4, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %5, !tbaa !2
  %9 = load i16, ptr %4, !tbaa !2
  %10 = icmp ult i16 %8, %9
  %11 = sext i1 %10 to i8
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i32, ptr %7, !tbaa !2
  %14 = load i32, ptr %7, !tbaa !2
  %15 = sext i32 %13 to i64
  %16 = sext i32 %14 to i64
  %17 = mul i64 %15, %16
  %18 = ashr i64 %17, 12
  %19 = trunc i64 %18 to i32
  store i32 %19, ptr %3, !tbaa !2
  %20 = load i32, ptr %6, !tbaa !2
  %21 = load i32, ptr %6, !tbaa !2
  %22 = sext i32 %20 to i64
  %23 = sext i32 %21 to i64
  %24 = mul i64 %22, %23
  %25 = ashr i64 %24, 12
  %26 = trunc i64 %25 to i32
  store i32 %26, ptr %2, !tbaa !2
  %27 = load i32, ptr %3, !tbaa !2
  %28 = load i32, ptr %2, !tbaa !2
  %29 = add i32 %27, %28
  %30 = icmp sgt i32 %29, 16384
  %31 = sext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b6, label %b7

b4:
  %33 = load i16, ptr %5, !tbaa !2
  %34 = add i16 %33, 1
  store i16 %34, ptr %5, !tbaa !2
  br label %b2

b5:
  ret i16 40

b6:
  %35 = load i16, ptr %5, !tbaa !2
  ret i16 %35

b7:
  br label %b8

b8:
  %36 = load i32, ptr %7, !tbaa !2
  %37 = sext i32 8192 to i64
  %38 = sext i32 %36 to i64
  %39 = mul i64 %37, %38
  %40 = ashr i64 %39, 12
  %41 = trunc i64 %40 to i32
  %42 = load i32, ptr %6, !tbaa !2
  %43 = sext i32 %41 to i64
  %44 = sext i32 %42 to i64
  %45 = mul i64 %43, %44
  %46 = ashr i64 %45, 12
  %47 = trunc i64 %46 to i32
  %48 = add i32 %47, %1
  store i32 %48, ptr %6, !tbaa !2
  %49 = load i32, ptr %3, !tbaa !2
  %50 = load i32, ptr %2, !tbaa !2
  %51 = sub i32 %49, %50
  %52 = add i32 %51, %0
  store i32 %52, ptr %7, !tbaa !2
  br label %b4
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i8
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  %5 = alloca ptr
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca i32
  %12 = alloca ptr
  store i8 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i32 0, ptr %4
  store ptr null, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i32 0, ptr %8
  store i32 0, ptr %9
  store i32 0, ptr %10
  store i32 0, ptr %11
  store ptr null, ptr %12
  %13 = getelementptr i8, ptr @$str1, i16 6
  store ptr %13, ptr %12, !tbaa !2
  %14 = sub i32 0, 9011
  store i32 %14, ptr %11, !tbaa !2
  store i32 4915, ptr %10, !tbaa !2
  %15 = zext i8 12 to i32
  %16 = shl i32 64, %15
  %17 = sext i32 13107 to i64
  %18 = sext i32 %16 to i64
  %19 = shl i64 %17, 12
  %20 = sdiv i64 %19, %18
  %21 = trunc i64 %20 to i32
  store i32 %21, ptr %9, !tbaa !2
  %22 = zext i8 12 to i32
  %23 = shl i32 22, %22
  %24 = sext i32 9830 to i64
  %25 = sext i32 %23 to i64
  %26 = shl i64 %24, 12
  %27 = sdiv i64 %26, %25
  %28 = trunc i64 %27 to i32
  store i32 %28, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 22, ptr %6, !tbaa !2
  br label %b2

b2:
  %29 = load i16, ptr %7, !tbaa !2
  %30 = load i16, ptr %6, !tbaa !2
  %31 = icmp slt i16 %29, %30
  %32 = sext i1 %31 to i8
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %b3, label %b5

b3:
  %34 = getelementptr i8, ptr @$str2, i16 6
  store ptr %34, ptr %5, !tbaa !2
  %35 = load i32, ptr %10, !tbaa !2
  %36 = load i16, ptr %7, !tbaa !2
  %37 = sext i16 %36 to i32
  %38 = zext i8 12 to i32
  %39 = shl i32 %37, %38
  %40 = load i32, ptr %8, !tbaa !2
  %41 = sext i32 %39 to i64
  %42 = sext i32 %40 to i64
  %43 = mul i64 %41, %42
  %44 = ashr i64 %43, 12
  %45 = trunc i64 %44 to i32
  %46 = sub i32 %35, %45
  store i32 %46, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 64, ptr %2, !tbaa !2
  br label %b6

b4:
  %47 = load i16, ptr %7, !tbaa !2
  %48 = add i16 %47, 1
  store i16 %48, ptr %7, !tbaa !2
  br label %b2

b5:
  %49 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %49)
  %50 = load i32, ptr %9, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %50, i8 12)
  %51 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %51)
  call addrspace(1) void @N$PU2(i16 40)
  call addrspace(1) void @N$PN()
  %52 = load ptr, ptr %12, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %52)
  ret i16 0

b6:
  %53 = load i16, ptr %3, !tbaa !2
  %54 = load i16, ptr %2, !tbaa !2
  %55 = icmp slt i16 %53, %54
  %56 = sext i1 %55 to i8
  %57 = icmp ne i8 %56, 0
  br i1 %57, label %b7, label %b9

b7:
  %58 = load i32, ptr %11, !tbaa !2
  %59 = load i16, ptr %3, !tbaa !2
  %60 = sext i16 %59 to i32
  %61 = zext i8 12 to i32
  %62 = shl i32 %60, %61
  %63 = load i32, ptr %9, !tbaa !2
  %64 = sext i32 %62 to i64
  %65 = sext i32 %63 to i64
  %66 = mul i64 %64, %65
  %67 = ashr i64 %66, 12
  %68 = trunc i64 %67 to i32
  %69 = add i32 %58, %68
  %70 = load i32, ptr %4, !tbaa !2
  %71 = call addrspace(1) i16 @escape(i32 %69, i32 %70)
  store i16 %71, ptr %1, !tbaa !2
  %72 = load ptr, ptr %5, !tbaa !2
  %73 = getelementptr i8, ptr %72, i16 -4
  %74 = load i16, ptr %73
  %75 = call addrspace(1) ptr @N$BGRW(ptr %72, i16 1, i16 1)
  store ptr %75, ptr %5, !tbaa !2
  %76 = getelementptr i8, ptr %75, i16 %74
  %77 = load i16, ptr %1, !tbaa !2
  %78 = icmp eq i16 %77, 40
  %79 = sext i1 %78 to i8
  %80 = icmp ne i8 %79, 0
  br i1 %80, label %b10, label %b11

b8:
  %81 = load i16, ptr %3, !tbaa !2
  %82 = add i16 %81, 1
  store i16 %82, ptr %3, !tbaa !2
  br label %b6

b9:
  %83 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$PS(ptr %83)
  call addrspace(1) void @N$PN()
  %84 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %84)
  br label %b4

b10:
  store i8 64, ptr %0, !tbaa !2
  br label %b12

b11:
  %85 = load ptr, ptr %12, !tbaa !2
  %86 = load i16, ptr %1, !tbaa !2
  %87 = urem i16 %86, 9
  %88 = getelementptr i8, ptr %85, i16 -4
  %89 = load i16, ptr %88
  %90 = icmp ult i16 %87, %89
  %91 = sext i1 %90 to i8
  %92 = icmp ne i8 %91, 0
  br i1 %92, label %b13, label %b14

b12:
  %93 = load i8, ptr %0, !tbaa !2
  store i8 %93, ptr %76
  %94 = getelementptr i8, ptr %75, i16 -4
  %95 = load i16, ptr %94
  %96 = getelementptr i8, ptr %75, i16 %95
  store i8 0, ptr %96
  br label %b8

b13:
  %97 = getelementptr i8, ptr %85, i16 %87
  %98 = load i8, ptr %97
  store i8 %98, ptr %0, !tbaa !2
  br label %b12

b14:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @N$PS(ptr) addrspace(1)

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
