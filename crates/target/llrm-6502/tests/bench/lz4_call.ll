declare void @touch()
define dso_local i16 @lz4(ptr noundef %0, ptr noundef %1, i16 noundef %2)  {
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  %12 = alloca i16, align 2
  %13 = alloca i8, align 1
  %14 = alloca i8, align 1
  store ptr %0, ptr %4, align 2
  store ptr %1, ptr %5, align 2
  store i16 %2, ptr %6, align 2
  store i16 0, ptr %7, align 2
  store i16 0, ptr %8, align 2
  br label %15
15:                                               ; preds = %133, %3
  %16 = load i16, ptr %8, align 2
  %17 = load i16, ptr %6, align 2
  %18 = icmp ult i16 %16, %17
  br i1 %18, label %19, label %134
19:                                               ; preds = %15
  %20 = load ptr, ptr %4, align 2
  %21 = load i16, ptr %7, align 2
  %22 = getelementptr inbounds nuw i8, ptr %20, i16 %21
  %23 = load i8, ptr %22, align 1
  store i8 %23, ptr %13, align 1
  %24 = load i16, ptr %7, align 2
  %25 = add i16 %24, 1
  store i16 %25, ptr %7, align 2
  %26 = load i8, ptr %13, align 1
  %27 = zext i8 %26 to i16
  %28 = ashr i16 %27, 4
  store i16 %28, ptr %9, align 2
  %29 = load i8, ptr %13, align 1
  %30 = zext i8 %29 to i16
  %31 = and i16 %30, 15
  %32 = add nsw i16 %31, 4
  store i16 %32, ptr %10, align 2
  %33 = load i16, ptr %9, align 2
  %34 = icmp eq i16 %33, 15
  br i1 %34, label %35, label %52
35:                                               ; preds = %19
  store i8 -1, ptr %14, align 1
  br label %36
36:                                               ; preds = %40, %35
  %37 = load i8, ptr %14, align 1
  %38 = zext i8 %37 to i16
  %39 = icmp eq i16 %38, 255
  br i1 %39, label %40, label %51
40:                                               ; preds = %36
  %41 = load ptr, ptr %4, align 2
  %42 = load i16, ptr %7, align 2
  %43 = getelementptr inbounds nuw i8, ptr %41, i16 %42
  %44 = load i8, ptr %43, align 1
  store i8 %44, ptr %14, align 1
  %45 = load i16, ptr %7, align 2
  %46 = add i16 %45, 1
  store i16 %46, ptr %7, align 2
  %47 = load i8, ptr %14, align 1
  %48 = zext i8 %47 to i16
  %49 = load i16, ptr %9, align 2
  %50 = add i16 %49, %48
  store i16 %50, ptr %9, align 2
  br label %36
51:                                               ; preds = %36
  br label %52
52:                                               ; preds = %51, %19
  br label %53
53:                                               ; preds = %56, %52
  %54 = load i16, ptr %9, align 2
  %55 = icmp ne i16 %54, 0
  br i1 %55, label %56, label %70
56:                                               ; preds = %53
  %57 = load ptr, ptr %4, align 2
  %58 = load i16, ptr %7, align 2
  %59 = getelementptr inbounds nuw i8, ptr %57, i16 %58
  %60 = load i8, ptr %59, align 1
  %61 = load ptr, ptr %5, align 2
  %62 = load i16, ptr %8, align 2
  %63 = getelementptr inbounds nuw i8, ptr %61, i16 %62
  store i8 %60, ptr %63, align 1
  %64 = load i16, ptr %8, align 2
  %65 = add i16 %64, 1
  store i16 %65, ptr %8, align 2
  %66 = load i16, ptr %7, align 2
  %67 = add i16 %66, 1
  store i16 %67, ptr %7, align 2
  %68 = load i16, ptr %9, align 2
  %69 = add i16 %68, -1
  store i16 %69, ptr %9, align 2
  br label %53
70:                                               ; preds = %53
  %71 = load i16, ptr %8, align 2
  %72 = load i16, ptr %6, align 2
  %73 = icmp uge i16 %71, %72
  br i1 %73, label %74, label %75
74:                                               ; preds = %70
  br label %134
75:                                               ; preds = %70
  %76 = load ptr, ptr %4, align 2
  %77 = load i16, ptr %7, align 2
  %78 = getelementptr inbounds nuw i8, ptr %76, i16 %77
  %79 = load i8, ptr %78, align 1
  %80 = zext i8 %79 to i16
  store i16 %80, ptr %11, align 2
  %81 = load i16, ptr %7, align 2
  %82 = add i16 %81, 1
  store i16 %82, ptr %7, align 2
  %83 = load ptr, ptr %4, align 2
  %84 = load i16, ptr %7, align 2
  %85 = getelementptr inbounds nuw i8, ptr %83, i16 %84
  %86 = load i8, ptr %85, align 1
  %87 = zext i8 %86 to i16
  %88 = shl i16 %87, 8
  %89 = load i16, ptr %11, align 2
  %90 = add i16 %89, %88
  store i16 %90, ptr %11, align 2
  %91 = load i16, ptr %7, align 2
  %92 = add i16 %91, 1
  store i16 %92, ptr %7, align 2
  %93 = load i16, ptr %10, align 2
  %94 = icmp eq i16 %93, 19
  br i1 %94, label %95, label %112
95:                                               ; preds = %75
  store i8 -1, ptr %14, align 1
  br label %96
96:                                               ; preds = %100, %95
  %97 = load i8, ptr %14, align 1
  %98 = zext i8 %97 to i16
  %99 = icmp eq i16 %98, 255
  br i1 %99, label %100, label %111
100:                                              ; preds = %96
  %101 = load ptr, ptr %4, align 2
  %102 = load i16, ptr %7, align 2
  %103 = getelementptr inbounds nuw i8, ptr %101, i16 %102
  %104 = load i8, ptr %103, align 1
  store i8 %104, ptr %14, align 1
  %105 = load i16, ptr %7, align 2
  %106 = add i16 %105, 1
  store i16 %106, ptr %7, align 2
  %107 = load i8, ptr %14, align 1
  %108 = zext i8 %107 to i16
  %109 = load i16, ptr %10, align 2
  %110 = add i16 %109, %108
  store i16 %110, ptr %10, align 2
  br label %96
111:                                              ; preds = %96
  br label %112
112:                                              ; preds = %111, %75
  call void @touch()
  %113 = load i16, ptr %8, align 2
  %114 = load i16, ptr %11, align 2
  %115 = sub i16 %113, %114
  store i16 %115, ptr %12, align 2
  br label %116
116:                                              ; preds = %119, %112
  %117 = load i16, ptr %10, align 2
  %118 = icmp ne i16 %117, 0
  br i1 %118, label %119, label %133
119:                                              ; preds = %116
  %120 = load ptr, ptr %5, align 2
  %121 = load i16, ptr %12, align 2
  %122 = getelementptr inbounds nuw i8, ptr %120, i16 %121
  %123 = load i8, ptr %122, align 1
  %124 = load ptr, ptr %5, align 2
  %125 = load i16, ptr %8, align 2
  %126 = getelementptr inbounds nuw i8, ptr %124, i16 %125
  store i8 %123, ptr %126, align 1
  %127 = load i16, ptr %8, align 2
  %128 = add i16 %127, 1
  store i16 %128, ptr %8, align 2
  %129 = load i16, ptr %12, align 2
  %130 = add i16 %129, 1
  store i16 %130, ptr %12, align 2
  %131 = load i16, ptr %10, align 2
  %132 = add i16 %131, -1
  store i16 %132, ptr %10, align 2
  br label %116
133:                                              ; preds = %116
  br label %15
134:                                              ; preds = %74, %15
  %135 = load i16, ptr %8, align 2
  ret i16 %135
}
