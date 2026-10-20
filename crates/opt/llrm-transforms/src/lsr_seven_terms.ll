@"YY%" = global [8 x i16] zeroinitializer
@"XP%" = global [8 x i16] zeroinitializer
@"F%" = global [701 x i16] zeroinitializer
@"SP%" = global [8001 x i16] zeroinitializer
@"CD%" = global [2001 x i16] zeroinitializer
define void @f() {
b1:
    %0 = load i16, ptr getelementptr (i8, ptr @"YY%", i16 2)
  %1 = load i16, ptr getelementptr (i8, ptr @"XP%", i16 2)
  %2 = load i16, ptr getelementptr (i8, ptr @"YY%", i16 4)
  %3 = load i16, ptr getelementptr (i8, ptr @"XP%", i16 4)
  %4 = load i16, ptr getelementptr (i8, ptr @"YY%", i16 6)
  %5 = load i16, ptr getelementptr (i8, ptr @"XP%", i16 6)
  %6 = load i16, ptr getelementptr (i8, ptr @"YY%", i16 8)
  %7 = load i16, ptr getelementptr (i8, ptr @"XP%", i16 8)
  %8 = load i16, ptr getelementptr (i8, ptr @"YY%", i16 10)
  %9 = load i16, ptr getelementptr (i8, ptr @"XP%", i16 10)
  %10 = load i16, ptr getelementptr (i8, ptr @"YY%", i16 12)
  %11 = load i16, ptr getelementptr (i8, ptr @"XP%", i16 12)
  %12 = load i16, ptr getelementptr (i8, ptr @"YY%", i16 14)
  %13 = load i16, ptr getelementptr (i8, ptr @"XP%", i16 14)
  br label %b5

b5:
  %14 = phi i16 [ 24, %b1 ], [ %67, %b5 ]
  %15 = sub i16 %14, %1
  %16 = getelementptr i16, ptr @"F%", i16 %15
  %17 = load i16, ptr %16
  %18 = add i16 %0, %17
  %19 = getelementptr i16, ptr @"SP%", i16 %18
  %20 = load i16, ptr %19
  %21 = sub i16 %14, %3
  %22 = getelementptr i16, ptr @"F%", i16 %21
  %23 = load i16, ptr %22
  %24 = add i16 %2, %23
  %25 = getelementptr i16, ptr @"SP%", i16 %24
  %26 = load i16, ptr %25
  %27 = add i16 %20, %26
  %28 = sub i16 %14, %5
  %29 = getelementptr i16, ptr @"F%", i16 %28
  %30 = load i16, ptr %29
  %31 = add i16 %4, %30
  %32 = getelementptr i16, ptr @"SP%", i16 %31
  %33 = load i16, ptr %32
  %34 = add i16 %27, %33
  %35 = sub i16 %14, %7
  %36 = getelementptr i16, ptr @"F%", i16 %35
  %37 = load i16, ptr %36
  %38 = add i16 %6, %37
  %39 = getelementptr i16, ptr @"SP%", i16 %38
  %40 = load i16, ptr %39
  %41 = add i16 %34, %40
  %42 = sub i16 %14, %9
  %43 = getelementptr i16, ptr @"F%", i16 %42
  %44 = load i16, ptr %43
  %45 = add i16 %8, %44
  %46 = getelementptr i16, ptr @"SP%", i16 %45
  %47 = load i16, ptr %46
  %48 = add i16 %41, %47
  %49 = sub i16 %14, %11
  %50 = getelementptr i16, ptr @"F%", i16 %49
  %51 = load i16, ptr %50
  %52 = add i16 %10, %51
  %53 = getelementptr i16, ptr @"SP%", i16 %52
  %54 = load i16, ptr %53
  %55 = add i16 %48, %54
  %56 = sub i16 %14, %13
  %57 = getelementptr i16, ptr @"F%", i16 %56
  %58 = load i16, ptr %57
  %59 = add i16 %12, %58
  %60 = getelementptr i16, ptr @"SP%", i16 %59
  %61 = load i16, ptr %60
  %62 = add i16 %55, %61
  %63 = getelementptr i16, ptr @"CD%", i16 %62
  %64 = load i16, ptr %63
  %65 = trunc i16 %64 to i8
  %66 = getelementptr i8, ptr inttoptr (i16 -24576 to ptr), i16 %14
  store volatile i8 %65, ptr %66
  %67 = add nuw nsw i16 %14, 1
  %68 = icmp ule i16 %67, 295
  br i1 %68, label %b5, label %b6

b6:
  ret void
}
