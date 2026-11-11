target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@sum = internal global i32 0, align 2
@state = internal global i32 -1831433054, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  %3 = call i32 @c_long_3() #4
  %4 = load i32, ptr %2, align 2, !tbaa !7
  %5 = add nsw i32 %4, %3
  store i32 %5, ptr %2, align 2, !tbaa !7
  %6 = call i32 @c_long_5() #4
  %7 = load i32, ptr %2, align 2, !tbaa !7
  %8 = add nsw i32 %7, %6
  store i32 %8, ptr %2, align 2, !tbaa !7
  %9 = call i32 @c_long_7() #4
  %10 = load i32, ptr %2, align 2, !tbaa !7
  %11 = add nsw i32 %10, %9
  store i32 %11, ptr %2, align 2, !tbaa !7
  %12 = call i32 @c_long_10() #4
  %13 = load i32, ptr %2, align 2, !tbaa !7
  %14 = add nsw i32 %13, %12
  store i32 %14, ptr %2, align 2, !tbaa !7
  %15 = call i32 @c_long_100() #4
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add nsw i32 %16, %15
  store i32 %17, ptr %2, align 2, !tbaa !7
  %18 = call i32 @c_long_641() #4
  %19 = load i32, ptr %2, align 2, !tbaa !7
  %20 = add nsw i32 %19, %18
  store i32 %20, ptr %2, align 2, !tbaa !7
  %21 = call i32 @c_long_1000() #4
  %22 = load i32, ptr %2, align 2, !tbaa !7
  %23 = add nsw i32 %22, %21
  store i32 %23, ptr %2, align 2, !tbaa !7
  %24 = call i32 @c_long_65537() #4
  %25 = load i32, ptr %2, align 2, !tbaa !7
  %26 = add nsw i32 %25, %24
  store i32 %26, ptr %2, align 2, !tbaa !7
  %27 = call i32 @c_long_6700417() #4
  %28 = load i32, ptr %2, align 2, !tbaa !7
  %29 = add nsw i32 %28, %27
  store i32 %29, ptr %2, align 2, !tbaa !7
  %30 = call i32 @c_long_2147483647() #4
  %31 = load i32, ptr %2, align 2, !tbaa !7
  %32 = add nsw i32 %31, %30
  store i32 %32, ptr %2, align 2, !tbaa !7
  %33 = call i32 @c_long_m3() #4
  %34 = load i32, ptr %2, align 2, !tbaa !7
  %35 = add nsw i32 %34, %33
  store i32 %35, ptr %2, align 2, !tbaa !7
  %36 = call i32 @c_long_m10() #4
  %37 = load i32, ptr %2, align 2, !tbaa !7
  %38 = add nsw i32 %37, %36
  store i32 %38, ptr %2, align 2, !tbaa !7
  %39 = call i32 @c_long_m641() #4
  %40 = load i32, ptr %2, align 2, !tbaa !7
  %41 = add nsw i32 %40, %39
  store i32 %41, ptr %2, align 2, !tbaa !7
  %42 = call i32 @c_long_m2147483647() #4
  %43 = load i32, ptr %2, align 2, !tbaa !7
  %44 = add nsw i32 %43, %42
  store i32 %44, ptr %2, align 2, !tbaa !7
  %45 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %45) #4
  %46 = load i32, ptr @sum, align 2, !tbaa !7
  call void @report(i32 noundef %46) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @c_long_3() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 3, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 3
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 3
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !9

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 3, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 3
  %114 = mul nsw i32 %113, 3
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 3
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 6, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 3
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 3
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !11

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_5() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 5, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 5
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 5
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !12

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 5, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 5
  %114 = mul nsw i32 %113, 5
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 5
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 10, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 5
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 5
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !13

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_7() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 7, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 7
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 7
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !14

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 7, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 7
  %114 = mul nsw i32 %113, 7
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 7
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 14, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 7
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 7
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !15

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_10() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 10, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 10
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 10
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !16

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 10, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 10
  %114 = mul nsw i32 %113, 10
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 10
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 20, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 10
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 10
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !17

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_100() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 100, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 100
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 100
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !18

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 100, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 100
  %114 = mul nsw i32 %113, 100
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 100
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 200, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 100
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 100
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !19

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_641() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 641, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 641
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 641
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !20

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 641, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 641
  %114 = mul nsw i32 %113, 641
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 641
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 1282, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 641
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 641
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !21

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_1000() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 1000, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 1000
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 1000
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !22

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 1000, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 1000
  %114 = mul nsw i32 %113, 1000
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 1000
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 2000, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 1000
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 1000
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !23

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_65537() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 65537, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 65537
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 65537
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !24

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 65537, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 65537
  %114 = mul nsw i32 %113, 65537
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 65537
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 131074, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 65537
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 65537
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !25

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_6700417() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 6700417, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 6700417
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 6700417
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !26

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 6700417, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 6700417
  %114 = mul nsw i32 %113, 6700417
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 6700417
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 13400834, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 6700417
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 6700417
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !27

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_2147483647() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 2147483647, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, 2147483647
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, 2147483647
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !28

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 2147483647, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, 2147483647
  %114 = mul nsw i32 %113, 2147483647
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, 2147483647
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 -2, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, 2147483647
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, 2147483647
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !29

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_m3() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 -3, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, -3
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, -3
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !30

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 -3, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, -3
  %114 = mul nsw i32 %113, -3
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, -3
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 -6, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, -3
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, -3
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !31

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_m10() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 -10, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, -10
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, -10
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !32

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 -10, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, -10
  %114 = mul nsw i32 %113, -10
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, -10
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 -20, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, -10
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, -10
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !33

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_m641() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 -641, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, -641
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, -641
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !34

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 -641, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, -641
  %114 = mul nsw i32 %113, -641
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, -641
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 -1282, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, -641
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, -641
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !35

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: nounwind optsize
define internal i32 @c_long_m2147483647() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i32 -2147483647, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %81, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %84

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = call i32 @next() #4
  store i32 %14, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %15 = call i32 @next() #4
  %16 = and i32 %15, 7
  store i32 %16, ptr %8, align 2, !tbaa !7
  %17 = load i32, ptr %8, align 2, !tbaa !7
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %21

19:                                               ; preds = %13
  %20 = load i32, ptr %7, align 2, !tbaa !7
  br label %47

21:                                               ; preds = %13
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = icmp eq i32 %22, 1
  br i1 %23, label %24, label %27

24:                                               ; preds = %21
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = lshr i32 %25, 16
  br label %45

27:                                               ; preds = %21
  %28 = load i32, ptr %8, align 2, !tbaa !7
  %29 = icmp eq i32 %28, 2
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i32, ptr %7, align 2, !tbaa !7
  %32 = lshr i32 %31, 24
  br label %43

33:                                               ; preds = %27
  %34 = load i32, ptr %8, align 2, !tbaa !7
  %35 = icmp eq i32 %34, 3
  br i1 %35, label %36, label %39

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = lshr i32 %37, 29
  br label %41

39:                                               ; preds = %33
  %40 = load i32, ptr %7, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %39, %36
  %42 = phi i32 [ %38, %36 ], [ %40, %39 ]
  br label %43

43:                                               ; preds = %41, %30
  %44 = phi i32 [ %32, %30 ], [ %42, %41 ]
  br label %45

45:                                               ; preds = %43, %24
  %46 = phi i32 [ %26, %24 ], [ %44, %43 ]
  br label %47

47:                                               ; preds = %45, %19
  %48 = phi i32 [ %20, %19 ], [ %46, %45 ]
  store i32 %48, ptr %2, align 2, !tbaa !7
  %49 = call i32 @next() #4
  %50 = and i32 %49, 1
  %51 = icmp ne i32 %50, 0
  br i1 %51, label %52, label %55

52:                                               ; preds = %47
  %53 = load i32, ptr %2, align 2, !tbaa !7
  %54 = sub nsw i32 0, %53
  store i32 %54, ptr %2, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %52, %47
  %56 = load i32, ptr %2, align 2, !tbaa !7
  %57 = sdiv i32 %56, -2147483647
  store i32 %57, ptr %3, align 2, !tbaa !7
  %58 = load i32, ptr %2, align 2, !tbaa !7
  %59 = srem i32 %58, -2147483647
  store i32 %59, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %3, align 2, !tbaa !7
  %61 = load i32, ptr %2, align 2, !tbaa !7
  %62 = load volatile i32, ptr %1, align 2, !tbaa !7
  %63 = sdiv i32 %61, %62
  %64 = icmp ne i32 %60, %63
  br i1 %64, label %71, label %65

65:                                               ; preds = %55
  %66 = load i32, ptr %4, align 2, !tbaa !7
  %67 = load i32, ptr %2, align 2, !tbaa !7
  %68 = load volatile i32, ptr %1, align 2, !tbaa !7
  %69 = srem i32 %67, %68
  %70 = icmp ne i32 %66, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65, %55
  %72 = load i32, ptr %5, align 2, !tbaa !7
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 2, !tbaa !7
  br label %74

74:                                               ; preds = %71, %65
  %75 = load i32, ptr %3, align 2, !tbaa !7
  %76 = mul i32 %75, 31
  %77 = load i32, ptr %4, align 2, !tbaa !7
  %78 = add i32 %76, %77
  %79 = load i32, ptr @sum, align 2, !tbaa !7
  %80 = add i32 %79, %78
  store i32 %80, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !36

84:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %85

85:                                               ; preds = %166, %84
  %86 = load i32, ptr %6, align 2, !tbaa !7
  %87 = icmp slt i32 %86, 12
  br i1 %87, label %88, label %169

88:                                               ; preds = %85
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 1073741823, ptr %9, align 2, !tbaa !7
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 2
  br i1 %90, label %91, label %93

91:                                               ; preds = %88
  %92 = load i32, ptr %6, align 2, !tbaa !7
  br label %139

93:                                               ; preds = %88
  %94 = load i32, ptr %6, align 2, !tbaa !7
  %95 = icmp slt i32 %94, 4
  br i1 %95, label %96, label %101

96:                                               ; preds = %93
  %97 = load i32, ptr %9, align 2, !tbaa !7
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 2
  %100 = sub nsw i32 %97, %99
  br label %137

101:                                              ; preds = %93
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 6
  br i1 %103, label %104, label %108

104:                                              ; preds = %101
  %105 = load i32, ptr %6, align 2, !tbaa !7
  %106 = sub nsw i32 %105, 5
  %107 = add nsw i32 -2147483647, %106
  br label %135

108:                                              ; preds = %101
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = icmp slt i32 %109, 8
  br i1 %110, label %111, label %118

111:                                              ; preds = %108
  %112 = load i32, ptr %9, align 2, !tbaa !7
  %113 = sdiv i32 %112, -2147483647
  %114 = mul nsw i32 %113, -2147483647
  %115 = load i32, ptr %6, align 2, !tbaa !7
  %116 = sub nsw i32 %115, 6
  %117 = sub nsw i32 %114, %116
  br label %133

118:                                              ; preds = %108
  %119 = load i32, ptr %6, align 2, !tbaa !7
  %120 = icmp slt i32 %119, 10
  br i1 %120, label %121, label %127

121:                                              ; preds = %118
  %122 = load i32, ptr %9, align 2, !tbaa !7
  %123 = sub nsw i32 %122, -2147483647
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = sub nsw i32 %124, 8
  %126 = add nsw i32 %123, %125
  br label %131

127:                                              ; preds = %118
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 2, %129
  br label %131

131:                                              ; preds = %127, %121
  %132 = phi i32 [ %126, %121 ], [ %130, %127 ]
  br label %133

133:                                              ; preds = %131, %111
  %134 = phi i32 [ %117, %111 ], [ %132, %131 ]
  br label %135

135:                                              ; preds = %133, %104
  %136 = phi i32 [ %107, %104 ], [ %134, %133 ]
  br label %137

137:                                              ; preds = %135, %96
  %138 = phi i32 [ %100, %96 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %91
  %140 = phi i32 [ %92, %91 ], [ %138, %137 ]
  store i32 %140, ptr %2, align 2, !tbaa !7
  %141 = load i32, ptr %2, align 2, !tbaa !7
  %142 = sdiv i32 %141, -2147483647
  store i32 %142, ptr %3, align 2, !tbaa !7
  %143 = load i32, ptr %2, align 2, !tbaa !7
  %144 = srem i32 %143, -2147483647
  store i32 %144, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %3, align 2, !tbaa !7
  %146 = load i32, ptr %2, align 2, !tbaa !7
  %147 = load volatile i32, ptr %1, align 2, !tbaa !7
  %148 = sdiv i32 %146, %147
  %149 = icmp ne i32 %145, %148
  br i1 %149, label %156, label %150

150:                                              ; preds = %139
  %151 = load i32, ptr %4, align 2, !tbaa !7
  %152 = load i32, ptr %2, align 2, !tbaa !7
  %153 = load volatile i32, ptr %1, align 2, !tbaa !7
  %154 = srem i32 %152, %153
  %155 = icmp ne i32 %151, %154
  br i1 %155, label %156, label %159

156:                                              ; preds = %150, %139
  %157 = load i32, ptr %5, align 2, !tbaa !7
  %158 = add nsw i32 %157, 1
  store i32 %158, ptr %5, align 2, !tbaa !7
  br label %159

159:                                              ; preds = %156, %150
  %160 = load i32, ptr %3, align 2, !tbaa !7
  %161 = mul i32 %160, 31
  %162 = load i32, ptr %4, align 2, !tbaa !7
  %163 = add i32 %161, %162
  %164 = load i32, ptr @sum, align 2, !tbaa !7
  %165 = add i32 %164, %163
  store i32 %165, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %166

166:                                              ; preds = %159
  %167 = load i32, ptr %6, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %6, align 2, !tbaa !7
  br label %85, !llvm.loop !37

169:                                              ; preds = %85
  %170 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %170
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @next() #0 {
  %1 = load i32, ptr @state, align 2, !tbaa !7
  %2 = shl i32 %1, 13
  %3 = load i32, ptr @state, align 2, !tbaa !7
  %4 = xor i32 %3, %2
  store i32 %4, ptr @state, align 2, !tbaa !7
  %5 = load i32, ptr @state, align 2, !tbaa !7
  %6 = lshr i32 %5, 17
  %7 = load i32, ptr @state, align 2, !tbaa !7
  %8 = xor i32 %7, %6
  store i32 %8, ptr @state, align 2, !tbaa !7
  %9 = load i32, ptr @state, align 2, !tbaa !7
  %10 = shl i32 %9, 5
  %11 = load i32, ptr @state, align 2, !tbaa !7
  %12 = xor i32 %11, %10
  store i32 %12, ptr @state, align 2, !tbaa !7
  %13 = load i32, ptr @state, align 2, !tbaa !7
  ret i32 %13
}

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = distinct !{!12, !10}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
!16 = distinct !{!16, !10}
!17 = distinct !{!17, !10}
!18 = distinct !{!18, !10}
!19 = distinct !{!19, !10}
!20 = distinct !{!20, !10}
!21 = distinct !{!21, !10}
!22 = distinct !{!22, !10}
!23 = distinct !{!23, !10}
!24 = distinct !{!24, !10}
!25 = distinct !{!25, !10}
!26 = distinct !{!26, !10}
!27 = distinct !{!27, !10}
!28 = distinct !{!28, !10}
!29 = distinct !{!29, !10}
!30 = distinct !{!30, !10}
!31 = distinct !{!31, !10}
!32 = distinct !{!32, !10}
!33 = distinct !{!33, !10}
!34 = distinct !{!34, !10}
!35 = distinct !{!35, !10}
!36 = distinct !{!36, !10}
!37 = distinct !{!37, !10}
