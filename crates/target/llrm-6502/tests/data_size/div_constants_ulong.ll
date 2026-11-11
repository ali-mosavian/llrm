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
  %3 = call i32 @c_ulong_3() #4
  %4 = load i32, ptr %2, align 2, !tbaa !7
  %5 = add nsw i32 %4, %3
  store i32 %5, ptr %2, align 2, !tbaa !7
  %6 = call i32 @c_ulong_5() #4
  %7 = load i32, ptr %2, align 2, !tbaa !7
  %8 = add nsw i32 %7, %6
  store i32 %8, ptr %2, align 2, !tbaa !7
  %9 = call i32 @c_ulong_7() #4
  %10 = load i32, ptr %2, align 2, !tbaa !7
  %11 = add nsw i32 %10, %9
  store i32 %11, ptr %2, align 2, !tbaa !7
  %12 = call i32 @c_ulong_10() #4
  %13 = load i32, ptr %2, align 2, !tbaa !7
  %14 = add nsw i32 %13, %12
  store i32 %14, ptr %2, align 2, !tbaa !7
  %15 = call i32 @c_ulong_100() #4
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add nsw i32 %16, %15
  store i32 %17, ptr %2, align 2, !tbaa !7
  %18 = call i32 @c_ulong_641() #4
  %19 = load i32, ptr %2, align 2, !tbaa !7
  %20 = add nsw i32 %19, %18
  store i32 %20, ptr %2, align 2, !tbaa !7
  %21 = call i32 @c_ulong_1000() #4
  %22 = load i32, ptr %2, align 2, !tbaa !7
  %23 = add nsw i32 %22, %21
  store i32 %23, ptr %2, align 2, !tbaa !7
  %24 = call i32 @c_ulong_65537() #4
  %25 = load i32, ptr %2, align 2, !tbaa !7
  %26 = add nsw i32 %25, %24
  store i32 %26, ptr %2, align 2, !tbaa !7
  %27 = call i32 @c_ulong_6700417() #4
  %28 = load i32, ptr %2, align 2, !tbaa !7
  %29 = add nsw i32 %28, %27
  store i32 %29, ptr %2, align 2, !tbaa !7
  %30 = call i32 @c_ulong_2147483647() #4
  %31 = load i32, ptr %2, align 2, !tbaa !7
  %32 = add nsw i32 %31, %30
  store i32 %32, ptr %2, align 2, !tbaa !7
  %33 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %33) #4
  %34 = load i32, ptr @sum, align 2, !tbaa !7
  call void @report(i32 noundef %34) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_3() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 3
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 3
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !9

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 3, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 3
  %107 = mul i32 %106, 3
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 3
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 6, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 3
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 3
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !11

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_5() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 5
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 5
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !12

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 5, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 5
  %107 = mul i32 %106, 5
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 5
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 10, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 5
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 5
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !13

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_7() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 7
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 7
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !14

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 7, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 7
  %107 = mul i32 %106, 7
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 7
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 14, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 7
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 7
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !15

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_10() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 10
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 10
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !16

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 10, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 10
  %107 = mul i32 %106, 10
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 10
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 20, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 10
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 10
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !17

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_100() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 100
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 100
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !18

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 100, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 100
  %107 = mul i32 %106, 100
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 100
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 200, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 100
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 100
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !19

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_641() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 641
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 641
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !20

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 641, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 641
  %107 = mul i32 %106, 641
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 641
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 1282, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 641
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 641
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !21

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_1000() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 1000
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 1000
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !22

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 1000, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 1000
  %107 = mul i32 %106, 1000
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 1000
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 2000, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 1000
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 1000
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !23

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_65537() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 65537
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 65537
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !24

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 65537, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 65537
  %107 = mul i32 %106, 65537
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 65537
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 131074, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 65537
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 65537
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !25

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_6700417() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 6700417
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 6700417
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !26

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 6700417, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 6700417
  %107 = mul i32 %106, 6700417
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 6700417
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 13400834, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 6700417
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 6700417
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !27

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
}

; Function Attrs: nounwind optsize
define internal i32 @c_ulong_2147483647() #0 {
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

10:                                               ; preds = %74, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %77

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
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = udiv i32 %49, 2147483647
  store i32 %50, ptr %3, align 2, !tbaa !7
  %51 = load i32, ptr %2, align 2, !tbaa !7
  %52 = urem i32 %51, 2147483647
  store i32 %52, ptr %4, align 2, !tbaa !7
  %53 = load i32, ptr %3, align 2, !tbaa !7
  %54 = load i32, ptr %2, align 2, !tbaa !7
  %55 = load volatile i32, ptr %1, align 2, !tbaa !7
  %56 = udiv i32 %54, %55
  %57 = icmp ne i32 %53, %56
  br i1 %57, label %64, label %58

58:                                               ; preds = %47
  %59 = load i32, ptr %4, align 2, !tbaa !7
  %60 = load i32, ptr %2, align 2, !tbaa !7
  %61 = load volatile i32, ptr %1, align 2, !tbaa !7
  %62 = urem i32 %60, %61
  %63 = icmp ne i32 %59, %62
  br i1 %63, label %64, label %67

64:                                               ; preds = %58, %47
  %65 = load i32, ptr %5, align 2, !tbaa !7
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %5, align 2, !tbaa !7
  br label %67

67:                                               ; preds = %64, %58
  %68 = load i32, ptr %3, align 2, !tbaa !7
  %69 = mul i32 %68, 31
  %70 = load i32, ptr %4, align 2, !tbaa !7
  %71 = add i32 %69, %70
  %72 = load i32, ptr @sum, align 2, !tbaa !7
  %73 = add i32 %72, %71
  store i32 %73, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %74

74:                                               ; preds = %67
  %75 = load i32, ptr %6, align 2, !tbaa !7
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !28

77:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %78

78:                                               ; preds = %159, %77
  %79 = load i32, ptr %6, align 2, !tbaa !7
  %80 = icmp slt i32 %79, 12
  br i1 %80, label %81, label %162

81:                                               ; preds = %78
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 -1, ptr %9, align 2, !tbaa !7
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 2
  br i1 %83, label %84, label %86

84:                                               ; preds = %81
  %85 = load i32, ptr %6, align 2, !tbaa !7
  br label %132

86:                                               ; preds = %81
  %87 = load i32, ptr %6, align 2, !tbaa !7
  %88 = icmp slt i32 %87, 4
  br i1 %88, label %89, label %94

89:                                               ; preds = %86
  %90 = load i32, ptr %9, align 2, !tbaa !7
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = sub nsw i32 %91, 2
  %93 = sub i32 %90, %92
  br label %130

94:                                               ; preds = %86
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = icmp slt i32 %95, 6
  br i1 %96, label %97, label %101

97:                                               ; preds = %94
  %98 = load i32, ptr %6, align 2, !tbaa !7
  %99 = sub nsw i32 %98, 5
  %100 = add i32 2147483647, %99
  br label %128

101:                                              ; preds = %94
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = icmp slt i32 %102, 8
  br i1 %103, label %104, label %111

104:                                              ; preds = %101
  %105 = load i32, ptr %9, align 2, !tbaa !7
  %106 = udiv i32 %105, 2147483647
  %107 = mul i32 %106, 2147483647
  %108 = load i32, ptr %6, align 2, !tbaa !7
  %109 = sub nsw i32 %108, 6
  %110 = sub i32 %107, %109
  br label %126

111:                                              ; preds = %101
  %112 = load i32, ptr %6, align 2, !tbaa !7
  %113 = icmp slt i32 %112, 10
  br i1 %113, label %114, label %120

114:                                              ; preds = %111
  %115 = load i32, ptr %9, align 2, !tbaa !7
  %116 = sub i32 %115, 2147483647
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = sub nsw i32 %117, 8
  %119 = add i32 %116, %118
  br label %124

120:                                              ; preds = %111
  %121 = load i32, ptr %6, align 2, !tbaa !7
  %122 = sub nsw i32 %121, 10
  %123 = sub nsw i32 -2, %122
  br label %124

124:                                              ; preds = %120, %114
  %125 = phi i32 [ %119, %114 ], [ %123, %120 ]
  br label %126

126:                                              ; preds = %124, %104
  %127 = phi i32 [ %110, %104 ], [ %125, %124 ]
  br label %128

128:                                              ; preds = %126, %97
  %129 = phi i32 [ %100, %97 ], [ %127, %126 ]
  br label %130

130:                                              ; preds = %128, %89
  %131 = phi i32 [ %93, %89 ], [ %129, %128 ]
  br label %132

132:                                              ; preds = %130, %84
  %133 = phi i32 [ %85, %84 ], [ %131, %130 ]
  store i32 %133, ptr %2, align 2, !tbaa !7
  %134 = load i32, ptr %2, align 2, !tbaa !7
  %135 = udiv i32 %134, 2147483647
  store i32 %135, ptr %3, align 2, !tbaa !7
  %136 = load i32, ptr %2, align 2, !tbaa !7
  %137 = urem i32 %136, 2147483647
  store i32 %137, ptr %4, align 2, !tbaa !7
  %138 = load i32, ptr %3, align 2, !tbaa !7
  %139 = load i32, ptr %2, align 2, !tbaa !7
  %140 = load volatile i32, ptr %1, align 2, !tbaa !7
  %141 = udiv i32 %139, %140
  %142 = icmp ne i32 %138, %141
  br i1 %142, label %149, label %143

143:                                              ; preds = %132
  %144 = load i32, ptr %4, align 2, !tbaa !7
  %145 = load i32, ptr %2, align 2, !tbaa !7
  %146 = load volatile i32, ptr %1, align 2, !tbaa !7
  %147 = urem i32 %145, %146
  %148 = icmp ne i32 %144, %147
  br i1 %148, label %149, label %152

149:                                              ; preds = %143, %132
  %150 = load i32, ptr %5, align 2, !tbaa !7
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %5, align 2, !tbaa !7
  br label %152

152:                                              ; preds = %149, %143
  %153 = load i32, ptr %3, align 2, !tbaa !7
  %154 = mul i32 %153, 31
  %155 = load i32, ptr %4, align 2, !tbaa !7
  %156 = add i32 %154, %155
  %157 = load i32, ptr @sum, align 2, !tbaa !7
  %158 = add i32 %157, %156
  store i32 %158, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %159

159:                                              ; preds = %152
  %160 = load i32, ptr %6, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %6, align 2, !tbaa !7
  br label %78, !llvm.loop !29

162:                                              ; preds = %78
  %163 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %163
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
