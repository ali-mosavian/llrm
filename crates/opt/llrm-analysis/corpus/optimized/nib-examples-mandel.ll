target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 .:-=+*#%@\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00step \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00, limit \00"

define internal i16 @escape(i32 %0, i32 %1) addrspace(1) memory(none) willreturn {
b1:
  br label %b2

b2:
  %2 = phi i32 [ 0, %b1 ], [ %25, %b8 ]
  %3 = phi i32 [ 0, %b1 ], [ %23, %b8 ]
  %4 = phi i16 [ 0, %b1 ], [ %26, %b8 ]
  %5 = icmp ult i16 %4, 40
  br i1 %5, label %b3, label %b5

b3:
  %6 = sext i32 %2 to i64
  %7 = mul i64 %6, %6
  %8 = ashr i64 %7, 12
  %9 = trunc i64 %8 to i32
  %10 = sext i32 %3 to i64
  %11 = mul i64 %10, %10
  %12 = ashr i64 %11, 12
  %13 = trunc i64 %12 to i32
  %14 = add i32 %9, %13
  %15 = icmp sgt i32 %14, 16384
  br i1 %15, label %b6, label %b8

b5:
  ret i16 40

b6:
  ret i16 %4

b8:
  %16 = shl i64 %6, 13
  %17 = ashr i64 %16, 12
  %18 = trunc i64 %17 to i32
  %19 = sext i32 %18 to i64
  %20 = mul i64 %19, %10
  %21 = ashr i64 %20, 12
  %22 = trunc i64 %21 to i32
  %23 = add i32 %22, %1
  %24 = sub i32 %9, %13
  %25 = add i32 %24, %0
  %26 = add i16 %4, 1
  br label %b2
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = getelementptr i8, ptr @$str1, i16 6
  %1 = getelementptr i8, ptr @$str2, i16 6
  %2 = getelementptr i8, ptr %0, i16 -4
  br label %b2

b2:
  %3 = phi i16 [ 0, %b1 ], [ %61, %b9 ]
  %4 = icmp slt i16 %3, 22
  br i1 %4, label %b3, label %b5

b3:
  %5 = sext i16 %3 to i32
  %6 = shl i32 %5, 12
  %7 = sext i32 %6 to i64
  %8 = mul i64 %7, 446
  %9 = ashr i64 %8, 12
  %10 = trunc i64 %9 to i32
  %11 = sub i32 4915, %10
  br label %b6

b5:
  %12 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %12)
  call addrspace(1) void @N$PQ4(i32 204, i8 12)
  %13 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %13)
  call addrspace(1) void @N$PU2(i16 40)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %0)
  ret i16 0

b6:
  %14 = phi ptr [ %1, %b3 ], [ %58, %b12 ]
  %15 = phi i16 [ 0, %b3 ], [ %69, %b12 ]
  %16 = icmp slt i16 %15, 64
  br i1 %16, label %b7, label %b9

b7:
  %17 = sext i16 %15 to i32
  %18 = shl i32 %17, 12
  %19 = sext i32 %18 to i64
  %20 = mul i64 %19, 204
  %21 = ashr i64 %20, 12
  %22 = trunc i64 %21 to i32
  %23 = add i32 %22, -9011
  br label %24

24:
  %25 = phi i32 [ 0, %b7 ], [ %52, %42 ]
  %26 = phi i32 [ 0, %b7 ], [ %50, %42 ]
  %27 = phi i16 [ 0, %b7 ], [ %53, %42 ]
  %28 = icmp ult i16 %27, 40
  br i1 %28, label %29, label %40

29:
  %30 = sext i32 %25 to i64
  %31 = mul i64 %30, %30
  %32 = ashr i64 %31, 12
  %33 = trunc i64 %32 to i32
  %34 = sext i32 %26 to i64
  %35 = mul i64 %34, %34
  %36 = ashr i64 %35, 12
  %37 = trunc i64 %36 to i32
  %38 = add i32 %33, %37
  %39 = icmp sgt i32 %38, 16384
  br i1 %39, label %41, label %42

40:
  br label %54

41:
  br label %54

42:
  %43 = shl i64 %30, 13
  %44 = ashr i64 %43, 12
  %45 = trunc i64 %44 to i32
  %46 = sext i32 %45 to i64
  %47 = mul i64 %46, %34
  %48 = ashr i64 %47, 12
  %49 = trunc i64 %48 to i32
  %50 = add i32 %49, %11
  %51 = sub i32 %33, %37
  %52 = add i32 %51, %23
  %53 = add i16 %27, 1
  br label %24

54:
  %55 = phi i16 [ 40, %40 ], [ %27, %41 ]
  %56 = getelementptr i8, ptr %14, i16 -4
  %57 = load i16, ptr %56
  %58 = call addrspace(1) ptr @N$BGRW(ptr %14, i16 1, i16 1)
  %59 = getelementptr i8, ptr %58, i16 %57
  %60 = icmp eq i16 %55, 40
  br i1 %60, label %b12, label %b11

b9:
  call addrspace(1) void @N$PS(ptr %14)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %14)
  %61 = add i16 %3, 1
  br label %b2

b11:
  %62 = urem i16 %55, 9
  %63 = load i16, ptr %2
  %64 = icmp ult i16 %62, %63
  br i1 %64, label %b13, label %b14

b12:
  %65 = phi i8 [ 64, %54 ], [ %71, %b13 ]
  store i8 %65, ptr %59
  %66 = getelementptr i8, ptr %58, i16 -4
  %67 = load i16, ptr %66
  %68 = getelementptr i8, ptr %58, i16 %67
  store i8 0, ptr %68
  %69 = add i16 %15, 1
  br label %b6

b13:
  %70 = getelementptr i8, ptr %0, i16 %62
  %71 = load i8, ptr %70
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
