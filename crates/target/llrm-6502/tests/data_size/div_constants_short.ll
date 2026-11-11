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
  %3 = call i32 @c_short_3() #4
  %4 = load i32, ptr %2, align 2, !tbaa !7
  %5 = add nsw i32 %4, %3
  store i32 %5, ptr %2, align 2, !tbaa !7
  %6 = call i32 @c_short_7() #4
  %7 = load i32, ptr %2, align 2, !tbaa !7
  %8 = add nsw i32 %7, %6
  store i32 %8, ptr %2, align 2, !tbaa !7
  %9 = call i32 @c_short_10() #4
  %10 = load i32, ptr %2, align 2, !tbaa !7
  %11 = add nsw i32 %10, %9
  store i32 %11, ptr %2, align 2, !tbaa !7
  %12 = call i32 @c_short_100() #4
  %13 = load i32, ptr %2, align 2, !tbaa !7
  %14 = add nsw i32 %13, %12
  store i32 %14, ptr %2, align 2, !tbaa !7
  %15 = call i32 @c_short_1000() #4
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add nsw i32 %16, %15
  store i32 %17, ptr %2, align 2, !tbaa !7
  %18 = call i32 @c_short_32767() #4
  %19 = load i32, ptr %2, align 2, !tbaa !7
  %20 = add nsw i32 %19, %18
  store i32 %20, ptr %2, align 2, !tbaa !7
  %21 = call i32 @c_ushort_3() #4
  %22 = load i32, ptr %2, align 2, !tbaa !7
  %23 = add nsw i32 %22, %21
  store i32 %23, ptr %2, align 2, !tbaa !7
  %24 = call i32 @c_ushort_7() #4
  %25 = load i32, ptr %2, align 2, !tbaa !7
  %26 = add nsw i32 %25, %24
  store i32 %26, ptr %2, align 2, !tbaa !7
  %27 = call i32 @c_ushort_10() #4
  %28 = load i32, ptr %2, align 2, !tbaa !7
  %29 = add nsw i32 %28, %27
  store i32 %29, ptr %2, align 2, !tbaa !7
  %30 = call i32 @c_ushort_100() #4
  %31 = load i32, ptr %2, align 2, !tbaa !7
  %32 = add nsw i32 %31, %30
  store i32 %32, ptr %2, align 2, !tbaa !7
  %33 = call i32 @c_ushort_1000() #4
  %34 = load i32, ptr %2, align 2, !tbaa !7
  %35 = add nsw i32 %34, %33
  store i32 %35, ptr %2, align 2, !tbaa !7
  %36 = call i32 @c_ushort_32767() #4
  %37 = load i32, ptr %2, align 2, !tbaa !7
  %38 = add nsw i32 %37, %36
  store i32 %38, ptr %2, align 2, !tbaa !7
  %39 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %39) #4
  %40 = load i32, ptr @sum, align 2, !tbaa !7
  call void @report(i32 noundef %40) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @c_short_3() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 3, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %84, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %87

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = call i32 @next() #4
  %51 = and i32 %50, 1
  %52 = icmp ne i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %47
  %54 = load i16, ptr %2, align 2, !tbaa !9
  %55 = sub nsw i16 0, %54
  store i16 %55, ptr %2, align 2, !tbaa !9
  br label %56

56:                                               ; preds = %53, %47
  %57 = load i16, ptr %2, align 2, !tbaa !9
  %58 = sdiv i16 %57, 3
  store i16 %58, ptr %3, align 2, !tbaa !9
  %59 = load i16, ptr %2, align 2, !tbaa !9
  %60 = srem i16 %59, 3
  store i16 %60, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %3, align 2, !tbaa !9
  %62 = load i16, ptr %2, align 2, !tbaa !9
  %63 = load volatile i16, ptr %1, align 2, !tbaa !9
  %64 = sdiv i16 %62, %63
  %65 = icmp ne i16 %61, %64
  br i1 %65, label %72, label %66

66:                                               ; preds = %56
  %67 = load i16, ptr %4, align 2, !tbaa !9
  %68 = load i16, ptr %2, align 2, !tbaa !9
  %69 = load volatile i16, ptr %1, align 2, !tbaa !9
  %70 = srem i16 %68, %69
  %71 = icmp ne i16 %67, %70
  br i1 %71, label %72, label %75

72:                                               ; preds = %66, %56
  %73 = load i32, ptr %5, align 2, !tbaa !7
  %74 = add nsw i32 %73, 1
  store i32 %74, ptr %5, align 2, !tbaa !7
  br label %75

75:                                               ; preds = %72, %66
  %76 = load i16, ptr %3, align 2, !tbaa !9
  %77 = sext i16 %76 to i32
  %78 = mul i32 %77, 31
  %79 = load i16, ptr %4, align 2, !tbaa !9
  %80 = sext i16 %79 to i32
  %81 = add i32 %78, %80
  %82 = load i32, ptr @sum, align 2, !tbaa !7
  %83 = add i32 %82, %81
  store i32 %83, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %84

84:                                               ; preds = %75
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !11

87:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %88

88:                                               ; preds = %178, %87
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 12
  br i1 %90, label %91, label %181

91:                                               ; preds = %88
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 16383, ptr %9, align 2, !tbaa !9
  %92 = load i32, ptr %6, align 2, !tbaa !7
  %93 = icmp slt i32 %92, 2
  br i1 %93, label %94, label %96

94:                                               ; preds = %91
  %95 = load i32, ptr %6, align 2, !tbaa !7
  br label %148

96:                                               ; preds = %91
  %97 = load i32, ptr %6, align 2, !tbaa !7
  %98 = icmp slt i32 %97, 4
  br i1 %98, label %99, label %105

99:                                               ; preds = %96
  %100 = load i16, ptr %9, align 2, !tbaa !9
  %101 = sext i16 %100 to i32
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 2
  %104 = sub nsw i32 %101, %103
  br label %146

105:                                              ; preds = %96
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 6
  br i1 %107, label %108, label %112

108:                                              ; preds = %105
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = sub nsw i32 %109, 5
  %111 = add nsw i32 3, %110
  br label %144

112:                                              ; preds = %105
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = icmp slt i32 %113, 8
  br i1 %114, label %115, label %123

115:                                              ; preds = %112
  %116 = load i16, ptr %9, align 2, !tbaa !9
  %117 = sdiv i16 %116, 3
  %118 = mul nsw i16 %117, 3
  %119 = sext i16 %118 to i32
  %120 = load i32, ptr %6, align 2, !tbaa !7
  %121 = sub nsw i32 %120, 6
  %122 = sub nsw i32 %119, %121
  br label %142

123:                                              ; preds = %112
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = icmp slt i32 %124, 10
  br i1 %125, label %126, label %134

126:                                              ; preds = %123
  %127 = load i16, ptr %9, align 2, !tbaa !9
  %128 = sub nsw i16 %127, 3
  %129 = sext i16 %128 to i32
  %130 = load i32, ptr %6, align 2, !tbaa !7
  %131 = sub nsw i32 %130, 8
  %132 = add nsw i32 %129, %131
  %133 = trunc i32 %132 to i16
  br label %139

134:                                              ; preds = %123
  %135 = load i32, ptr %6, align 2, !tbaa !7
  %136 = sub nsw i32 %135, 10
  %137 = sub nsw i32 6, %136
  %138 = trunc i32 %137 to i16
  br label %139

139:                                              ; preds = %134, %126
  %140 = phi i16 [ %133, %126 ], [ %138, %134 ]
  %141 = sext i16 %140 to i32
  br label %142

142:                                              ; preds = %139, %115
  %143 = phi i32 [ %122, %115 ], [ %141, %139 ]
  br label %144

144:                                              ; preds = %142, %108
  %145 = phi i32 [ %111, %108 ], [ %143, %142 ]
  br label %146

146:                                              ; preds = %144, %99
  %147 = phi i32 [ %104, %99 ], [ %145, %144 ]
  br label %148

148:                                              ; preds = %146, %94
  %149 = phi i32 [ %95, %94 ], [ %147, %146 ]
  %150 = trunc i32 %149 to i16
  store i16 %150, ptr %2, align 2, !tbaa !9
  %151 = load i16, ptr %2, align 2, !tbaa !9
  %152 = sdiv i16 %151, 3
  store i16 %152, ptr %3, align 2, !tbaa !9
  %153 = load i16, ptr %2, align 2, !tbaa !9
  %154 = srem i16 %153, 3
  store i16 %154, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %3, align 2, !tbaa !9
  %156 = load i16, ptr %2, align 2, !tbaa !9
  %157 = load volatile i16, ptr %1, align 2, !tbaa !9
  %158 = sdiv i16 %156, %157
  %159 = icmp ne i16 %155, %158
  br i1 %159, label %166, label %160

160:                                              ; preds = %148
  %161 = load i16, ptr %4, align 2, !tbaa !9
  %162 = load i16, ptr %2, align 2, !tbaa !9
  %163 = load volatile i16, ptr %1, align 2, !tbaa !9
  %164 = srem i16 %162, %163
  %165 = icmp ne i16 %161, %164
  br i1 %165, label %166, label %169

166:                                              ; preds = %160, %148
  %167 = load i32, ptr %5, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %5, align 2, !tbaa !7
  br label %169

169:                                              ; preds = %166, %160
  %170 = load i16, ptr %3, align 2, !tbaa !9
  %171 = sext i16 %170 to i32
  %172 = mul i32 %171, 31
  %173 = load i16, ptr %4, align 2, !tbaa !9
  %174 = sext i16 %173 to i32
  %175 = add i32 %172, %174
  %176 = load i32, ptr @sum, align 2, !tbaa !7
  %177 = add i32 %176, %175
  store i32 %177, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %178

178:                                              ; preds = %169
  %179 = load i32, ptr %6, align 2, !tbaa !7
  %180 = add nsw i32 %179, 1
  store i32 %180, ptr %6, align 2, !tbaa !7
  br label %88, !llvm.loop !13

181:                                              ; preds = %88
  %182 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %182
}

; Function Attrs: nounwind optsize
define internal i32 @c_short_7() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 7, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %84, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %87

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = call i32 @next() #4
  %51 = and i32 %50, 1
  %52 = icmp ne i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %47
  %54 = load i16, ptr %2, align 2, !tbaa !9
  %55 = sub nsw i16 0, %54
  store i16 %55, ptr %2, align 2, !tbaa !9
  br label %56

56:                                               ; preds = %53, %47
  %57 = load i16, ptr %2, align 2, !tbaa !9
  %58 = sdiv i16 %57, 7
  store i16 %58, ptr %3, align 2, !tbaa !9
  %59 = load i16, ptr %2, align 2, !tbaa !9
  %60 = srem i16 %59, 7
  store i16 %60, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %3, align 2, !tbaa !9
  %62 = load i16, ptr %2, align 2, !tbaa !9
  %63 = load volatile i16, ptr %1, align 2, !tbaa !9
  %64 = sdiv i16 %62, %63
  %65 = icmp ne i16 %61, %64
  br i1 %65, label %72, label %66

66:                                               ; preds = %56
  %67 = load i16, ptr %4, align 2, !tbaa !9
  %68 = load i16, ptr %2, align 2, !tbaa !9
  %69 = load volatile i16, ptr %1, align 2, !tbaa !9
  %70 = srem i16 %68, %69
  %71 = icmp ne i16 %67, %70
  br i1 %71, label %72, label %75

72:                                               ; preds = %66, %56
  %73 = load i32, ptr %5, align 2, !tbaa !7
  %74 = add nsw i32 %73, 1
  store i32 %74, ptr %5, align 2, !tbaa !7
  br label %75

75:                                               ; preds = %72, %66
  %76 = load i16, ptr %3, align 2, !tbaa !9
  %77 = sext i16 %76 to i32
  %78 = mul i32 %77, 31
  %79 = load i16, ptr %4, align 2, !tbaa !9
  %80 = sext i16 %79 to i32
  %81 = add i32 %78, %80
  %82 = load i32, ptr @sum, align 2, !tbaa !7
  %83 = add i32 %82, %81
  store i32 %83, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %84

84:                                               ; preds = %75
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !14

87:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %88

88:                                               ; preds = %178, %87
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 12
  br i1 %90, label %91, label %181

91:                                               ; preds = %88
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 16383, ptr %9, align 2, !tbaa !9
  %92 = load i32, ptr %6, align 2, !tbaa !7
  %93 = icmp slt i32 %92, 2
  br i1 %93, label %94, label %96

94:                                               ; preds = %91
  %95 = load i32, ptr %6, align 2, !tbaa !7
  br label %148

96:                                               ; preds = %91
  %97 = load i32, ptr %6, align 2, !tbaa !7
  %98 = icmp slt i32 %97, 4
  br i1 %98, label %99, label %105

99:                                               ; preds = %96
  %100 = load i16, ptr %9, align 2, !tbaa !9
  %101 = sext i16 %100 to i32
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 2
  %104 = sub nsw i32 %101, %103
  br label %146

105:                                              ; preds = %96
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 6
  br i1 %107, label %108, label %112

108:                                              ; preds = %105
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = sub nsw i32 %109, 5
  %111 = add nsw i32 7, %110
  br label %144

112:                                              ; preds = %105
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = icmp slt i32 %113, 8
  br i1 %114, label %115, label %123

115:                                              ; preds = %112
  %116 = load i16, ptr %9, align 2, !tbaa !9
  %117 = sdiv i16 %116, 7
  %118 = mul nsw i16 %117, 7
  %119 = sext i16 %118 to i32
  %120 = load i32, ptr %6, align 2, !tbaa !7
  %121 = sub nsw i32 %120, 6
  %122 = sub nsw i32 %119, %121
  br label %142

123:                                              ; preds = %112
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = icmp slt i32 %124, 10
  br i1 %125, label %126, label %134

126:                                              ; preds = %123
  %127 = load i16, ptr %9, align 2, !tbaa !9
  %128 = sub nsw i16 %127, 7
  %129 = sext i16 %128 to i32
  %130 = load i32, ptr %6, align 2, !tbaa !7
  %131 = sub nsw i32 %130, 8
  %132 = add nsw i32 %129, %131
  %133 = trunc i32 %132 to i16
  br label %139

134:                                              ; preds = %123
  %135 = load i32, ptr %6, align 2, !tbaa !7
  %136 = sub nsw i32 %135, 10
  %137 = sub nsw i32 14, %136
  %138 = trunc i32 %137 to i16
  br label %139

139:                                              ; preds = %134, %126
  %140 = phi i16 [ %133, %126 ], [ %138, %134 ]
  %141 = sext i16 %140 to i32
  br label %142

142:                                              ; preds = %139, %115
  %143 = phi i32 [ %122, %115 ], [ %141, %139 ]
  br label %144

144:                                              ; preds = %142, %108
  %145 = phi i32 [ %111, %108 ], [ %143, %142 ]
  br label %146

146:                                              ; preds = %144, %99
  %147 = phi i32 [ %104, %99 ], [ %145, %144 ]
  br label %148

148:                                              ; preds = %146, %94
  %149 = phi i32 [ %95, %94 ], [ %147, %146 ]
  %150 = trunc i32 %149 to i16
  store i16 %150, ptr %2, align 2, !tbaa !9
  %151 = load i16, ptr %2, align 2, !tbaa !9
  %152 = sdiv i16 %151, 7
  store i16 %152, ptr %3, align 2, !tbaa !9
  %153 = load i16, ptr %2, align 2, !tbaa !9
  %154 = srem i16 %153, 7
  store i16 %154, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %3, align 2, !tbaa !9
  %156 = load i16, ptr %2, align 2, !tbaa !9
  %157 = load volatile i16, ptr %1, align 2, !tbaa !9
  %158 = sdiv i16 %156, %157
  %159 = icmp ne i16 %155, %158
  br i1 %159, label %166, label %160

160:                                              ; preds = %148
  %161 = load i16, ptr %4, align 2, !tbaa !9
  %162 = load i16, ptr %2, align 2, !tbaa !9
  %163 = load volatile i16, ptr %1, align 2, !tbaa !9
  %164 = srem i16 %162, %163
  %165 = icmp ne i16 %161, %164
  br i1 %165, label %166, label %169

166:                                              ; preds = %160, %148
  %167 = load i32, ptr %5, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %5, align 2, !tbaa !7
  br label %169

169:                                              ; preds = %166, %160
  %170 = load i16, ptr %3, align 2, !tbaa !9
  %171 = sext i16 %170 to i32
  %172 = mul i32 %171, 31
  %173 = load i16, ptr %4, align 2, !tbaa !9
  %174 = sext i16 %173 to i32
  %175 = add i32 %172, %174
  %176 = load i32, ptr @sum, align 2, !tbaa !7
  %177 = add i32 %176, %175
  store i32 %177, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %178

178:                                              ; preds = %169
  %179 = load i32, ptr %6, align 2, !tbaa !7
  %180 = add nsw i32 %179, 1
  store i32 %180, ptr %6, align 2, !tbaa !7
  br label %88, !llvm.loop !15

181:                                              ; preds = %88
  %182 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %182
}

; Function Attrs: nounwind optsize
define internal i32 @c_short_10() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 10, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %84, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %87

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = call i32 @next() #4
  %51 = and i32 %50, 1
  %52 = icmp ne i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %47
  %54 = load i16, ptr %2, align 2, !tbaa !9
  %55 = sub nsw i16 0, %54
  store i16 %55, ptr %2, align 2, !tbaa !9
  br label %56

56:                                               ; preds = %53, %47
  %57 = load i16, ptr %2, align 2, !tbaa !9
  %58 = sdiv i16 %57, 10
  store i16 %58, ptr %3, align 2, !tbaa !9
  %59 = load i16, ptr %2, align 2, !tbaa !9
  %60 = srem i16 %59, 10
  store i16 %60, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %3, align 2, !tbaa !9
  %62 = load i16, ptr %2, align 2, !tbaa !9
  %63 = load volatile i16, ptr %1, align 2, !tbaa !9
  %64 = sdiv i16 %62, %63
  %65 = icmp ne i16 %61, %64
  br i1 %65, label %72, label %66

66:                                               ; preds = %56
  %67 = load i16, ptr %4, align 2, !tbaa !9
  %68 = load i16, ptr %2, align 2, !tbaa !9
  %69 = load volatile i16, ptr %1, align 2, !tbaa !9
  %70 = srem i16 %68, %69
  %71 = icmp ne i16 %67, %70
  br i1 %71, label %72, label %75

72:                                               ; preds = %66, %56
  %73 = load i32, ptr %5, align 2, !tbaa !7
  %74 = add nsw i32 %73, 1
  store i32 %74, ptr %5, align 2, !tbaa !7
  br label %75

75:                                               ; preds = %72, %66
  %76 = load i16, ptr %3, align 2, !tbaa !9
  %77 = sext i16 %76 to i32
  %78 = mul i32 %77, 31
  %79 = load i16, ptr %4, align 2, !tbaa !9
  %80 = sext i16 %79 to i32
  %81 = add i32 %78, %80
  %82 = load i32, ptr @sum, align 2, !tbaa !7
  %83 = add i32 %82, %81
  store i32 %83, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %84

84:                                               ; preds = %75
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !16

87:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %88

88:                                               ; preds = %178, %87
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 12
  br i1 %90, label %91, label %181

91:                                               ; preds = %88
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 16383, ptr %9, align 2, !tbaa !9
  %92 = load i32, ptr %6, align 2, !tbaa !7
  %93 = icmp slt i32 %92, 2
  br i1 %93, label %94, label %96

94:                                               ; preds = %91
  %95 = load i32, ptr %6, align 2, !tbaa !7
  br label %148

96:                                               ; preds = %91
  %97 = load i32, ptr %6, align 2, !tbaa !7
  %98 = icmp slt i32 %97, 4
  br i1 %98, label %99, label %105

99:                                               ; preds = %96
  %100 = load i16, ptr %9, align 2, !tbaa !9
  %101 = sext i16 %100 to i32
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 2
  %104 = sub nsw i32 %101, %103
  br label %146

105:                                              ; preds = %96
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 6
  br i1 %107, label %108, label %112

108:                                              ; preds = %105
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = sub nsw i32 %109, 5
  %111 = add nsw i32 10, %110
  br label %144

112:                                              ; preds = %105
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = icmp slt i32 %113, 8
  br i1 %114, label %115, label %123

115:                                              ; preds = %112
  %116 = load i16, ptr %9, align 2, !tbaa !9
  %117 = sdiv i16 %116, 10
  %118 = mul nsw i16 %117, 10
  %119 = sext i16 %118 to i32
  %120 = load i32, ptr %6, align 2, !tbaa !7
  %121 = sub nsw i32 %120, 6
  %122 = sub nsw i32 %119, %121
  br label %142

123:                                              ; preds = %112
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = icmp slt i32 %124, 10
  br i1 %125, label %126, label %134

126:                                              ; preds = %123
  %127 = load i16, ptr %9, align 2, !tbaa !9
  %128 = sub nsw i16 %127, 10
  %129 = sext i16 %128 to i32
  %130 = load i32, ptr %6, align 2, !tbaa !7
  %131 = sub nsw i32 %130, 8
  %132 = add nsw i32 %129, %131
  %133 = trunc i32 %132 to i16
  br label %139

134:                                              ; preds = %123
  %135 = load i32, ptr %6, align 2, !tbaa !7
  %136 = sub nsw i32 %135, 10
  %137 = sub nsw i32 20, %136
  %138 = trunc i32 %137 to i16
  br label %139

139:                                              ; preds = %134, %126
  %140 = phi i16 [ %133, %126 ], [ %138, %134 ]
  %141 = sext i16 %140 to i32
  br label %142

142:                                              ; preds = %139, %115
  %143 = phi i32 [ %122, %115 ], [ %141, %139 ]
  br label %144

144:                                              ; preds = %142, %108
  %145 = phi i32 [ %111, %108 ], [ %143, %142 ]
  br label %146

146:                                              ; preds = %144, %99
  %147 = phi i32 [ %104, %99 ], [ %145, %144 ]
  br label %148

148:                                              ; preds = %146, %94
  %149 = phi i32 [ %95, %94 ], [ %147, %146 ]
  %150 = trunc i32 %149 to i16
  store i16 %150, ptr %2, align 2, !tbaa !9
  %151 = load i16, ptr %2, align 2, !tbaa !9
  %152 = sdiv i16 %151, 10
  store i16 %152, ptr %3, align 2, !tbaa !9
  %153 = load i16, ptr %2, align 2, !tbaa !9
  %154 = srem i16 %153, 10
  store i16 %154, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %3, align 2, !tbaa !9
  %156 = load i16, ptr %2, align 2, !tbaa !9
  %157 = load volatile i16, ptr %1, align 2, !tbaa !9
  %158 = sdiv i16 %156, %157
  %159 = icmp ne i16 %155, %158
  br i1 %159, label %166, label %160

160:                                              ; preds = %148
  %161 = load i16, ptr %4, align 2, !tbaa !9
  %162 = load i16, ptr %2, align 2, !tbaa !9
  %163 = load volatile i16, ptr %1, align 2, !tbaa !9
  %164 = srem i16 %162, %163
  %165 = icmp ne i16 %161, %164
  br i1 %165, label %166, label %169

166:                                              ; preds = %160, %148
  %167 = load i32, ptr %5, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %5, align 2, !tbaa !7
  br label %169

169:                                              ; preds = %166, %160
  %170 = load i16, ptr %3, align 2, !tbaa !9
  %171 = sext i16 %170 to i32
  %172 = mul i32 %171, 31
  %173 = load i16, ptr %4, align 2, !tbaa !9
  %174 = sext i16 %173 to i32
  %175 = add i32 %172, %174
  %176 = load i32, ptr @sum, align 2, !tbaa !7
  %177 = add i32 %176, %175
  store i32 %177, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %178

178:                                              ; preds = %169
  %179 = load i32, ptr %6, align 2, !tbaa !7
  %180 = add nsw i32 %179, 1
  store i32 %180, ptr %6, align 2, !tbaa !7
  br label %88, !llvm.loop !17

181:                                              ; preds = %88
  %182 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %182
}

; Function Attrs: nounwind optsize
define internal i32 @c_short_100() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 100, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %84, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %87

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = call i32 @next() #4
  %51 = and i32 %50, 1
  %52 = icmp ne i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %47
  %54 = load i16, ptr %2, align 2, !tbaa !9
  %55 = sub nsw i16 0, %54
  store i16 %55, ptr %2, align 2, !tbaa !9
  br label %56

56:                                               ; preds = %53, %47
  %57 = load i16, ptr %2, align 2, !tbaa !9
  %58 = sdiv i16 %57, 100
  store i16 %58, ptr %3, align 2, !tbaa !9
  %59 = load i16, ptr %2, align 2, !tbaa !9
  %60 = srem i16 %59, 100
  store i16 %60, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %3, align 2, !tbaa !9
  %62 = load i16, ptr %2, align 2, !tbaa !9
  %63 = load volatile i16, ptr %1, align 2, !tbaa !9
  %64 = sdiv i16 %62, %63
  %65 = icmp ne i16 %61, %64
  br i1 %65, label %72, label %66

66:                                               ; preds = %56
  %67 = load i16, ptr %4, align 2, !tbaa !9
  %68 = load i16, ptr %2, align 2, !tbaa !9
  %69 = load volatile i16, ptr %1, align 2, !tbaa !9
  %70 = srem i16 %68, %69
  %71 = icmp ne i16 %67, %70
  br i1 %71, label %72, label %75

72:                                               ; preds = %66, %56
  %73 = load i32, ptr %5, align 2, !tbaa !7
  %74 = add nsw i32 %73, 1
  store i32 %74, ptr %5, align 2, !tbaa !7
  br label %75

75:                                               ; preds = %72, %66
  %76 = load i16, ptr %3, align 2, !tbaa !9
  %77 = sext i16 %76 to i32
  %78 = mul i32 %77, 31
  %79 = load i16, ptr %4, align 2, !tbaa !9
  %80 = sext i16 %79 to i32
  %81 = add i32 %78, %80
  %82 = load i32, ptr @sum, align 2, !tbaa !7
  %83 = add i32 %82, %81
  store i32 %83, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %84

84:                                               ; preds = %75
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !18

87:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %88

88:                                               ; preds = %178, %87
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 12
  br i1 %90, label %91, label %181

91:                                               ; preds = %88
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 16383, ptr %9, align 2, !tbaa !9
  %92 = load i32, ptr %6, align 2, !tbaa !7
  %93 = icmp slt i32 %92, 2
  br i1 %93, label %94, label %96

94:                                               ; preds = %91
  %95 = load i32, ptr %6, align 2, !tbaa !7
  br label %148

96:                                               ; preds = %91
  %97 = load i32, ptr %6, align 2, !tbaa !7
  %98 = icmp slt i32 %97, 4
  br i1 %98, label %99, label %105

99:                                               ; preds = %96
  %100 = load i16, ptr %9, align 2, !tbaa !9
  %101 = sext i16 %100 to i32
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 2
  %104 = sub nsw i32 %101, %103
  br label %146

105:                                              ; preds = %96
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 6
  br i1 %107, label %108, label %112

108:                                              ; preds = %105
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = sub nsw i32 %109, 5
  %111 = add nsw i32 100, %110
  br label %144

112:                                              ; preds = %105
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = icmp slt i32 %113, 8
  br i1 %114, label %115, label %123

115:                                              ; preds = %112
  %116 = load i16, ptr %9, align 2, !tbaa !9
  %117 = sdiv i16 %116, 100
  %118 = mul nsw i16 %117, 100
  %119 = sext i16 %118 to i32
  %120 = load i32, ptr %6, align 2, !tbaa !7
  %121 = sub nsw i32 %120, 6
  %122 = sub nsw i32 %119, %121
  br label %142

123:                                              ; preds = %112
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = icmp slt i32 %124, 10
  br i1 %125, label %126, label %134

126:                                              ; preds = %123
  %127 = load i16, ptr %9, align 2, !tbaa !9
  %128 = sub nsw i16 %127, 100
  %129 = sext i16 %128 to i32
  %130 = load i32, ptr %6, align 2, !tbaa !7
  %131 = sub nsw i32 %130, 8
  %132 = add nsw i32 %129, %131
  %133 = trunc i32 %132 to i16
  br label %139

134:                                              ; preds = %123
  %135 = load i32, ptr %6, align 2, !tbaa !7
  %136 = sub nsw i32 %135, 10
  %137 = sub nsw i32 200, %136
  %138 = trunc i32 %137 to i16
  br label %139

139:                                              ; preds = %134, %126
  %140 = phi i16 [ %133, %126 ], [ %138, %134 ]
  %141 = sext i16 %140 to i32
  br label %142

142:                                              ; preds = %139, %115
  %143 = phi i32 [ %122, %115 ], [ %141, %139 ]
  br label %144

144:                                              ; preds = %142, %108
  %145 = phi i32 [ %111, %108 ], [ %143, %142 ]
  br label %146

146:                                              ; preds = %144, %99
  %147 = phi i32 [ %104, %99 ], [ %145, %144 ]
  br label %148

148:                                              ; preds = %146, %94
  %149 = phi i32 [ %95, %94 ], [ %147, %146 ]
  %150 = trunc i32 %149 to i16
  store i16 %150, ptr %2, align 2, !tbaa !9
  %151 = load i16, ptr %2, align 2, !tbaa !9
  %152 = sdiv i16 %151, 100
  store i16 %152, ptr %3, align 2, !tbaa !9
  %153 = load i16, ptr %2, align 2, !tbaa !9
  %154 = srem i16 %153, 100
  store i16 %154, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %3, align 2, !tbaa !9
  %156 = load i16, ptr %2, align 2, !tbaa !9
  %157 = load volatile i16, ptr %1, align 2, !tbaa !9
  %158 = sdiv i16 %156, %157
  %159 = icmp ne i16 %155, %158
  br i1 %159, label %166, label %160

160:                                              ; preds = %148
  %161 = load i16, ptr %4, align 2, !tbaa !9
  %162 = load i16, ptr %2, align 2, !tbaa !9
  %163 = load volatile i16, ptr %1, align 2, !tbaa !9
  %164 = srem i16 %162, %163
  %165 = icmp ne i16 %161, %164
  br i1 %165, label %166, label %169

166:                                              ; preds = %160, %148
  %167 = load i32, ptr %5, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %5, align 2, !tbaa !7
  br label %169

169:                                              ; preds = %166, %160
  %170 = load i16, ptr %3, align 2, !tbaa !9
  %171 = sext i16 %170 to i32
  %172 = mul i32 %171, 31
  %173 = load i16, ptr %4, align 2, !tbaa !9
  %174 = sext i16 %173 to i32
  %175 = add i32 %172, %174
  %176 = load i32, ptr @sum, align 2, !tbaa !7
  %177 = add i32 %176, %175
  store i32 %177, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %178

178:                                              ; preds = %169
  %179 = load i32, ptr %6, align 2, !tbaa !7
  %180 = add nsw i32 %179, 1
  store i32 %180, ptr %6, align 2, !tbaa !7
  br label %88, !llvm.loop !19

181:                                              ; preds = %88
  %182 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %182
}

; Function Attrs: nounwind optsize
define internal i32 @c_short_1000() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 1000, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %84, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %87

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = call i32 @next() #4
  %51 = and i32 %50, 1
  %52 = icmp ne i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %47
  %54 = load i16, ptr %2, align 2, !tbaa !9
  %55 = sub nsw i16 0, %54
  store i16 %55, ptr %2, align 2, !tbaa !9
  br label %56

56:                                               ; preds = %53, %47
  %57 = load i16, ptr %2, align 2, !tbaa !9
  %58 = sdiv i16 %57, 1000
  store i16 %58, ptr %3, align 2, !tbaa !9
  %59 = load i16, ptr %2, align 2, !tbaa !9
  %60 = srem i16 %59, 1000
  store i16 %60, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %3, align 2, !tbaa !9
  %62 = load i16, ptr %2, align 2, !tbaa !9
  %63 = load volatile i16, ptr %1, align 2, !tbaa !9
  %64 = sdiv i16 %62, %63
  %65 = icmp ne i16 %61, %64
  br i1 %65, label %72, label %66

66:                                               ; preds = %56
  %67 = load i16, ptr %4, align 2, !tbaa !9
  %68 = load i16, ptr %2, align 2, !tbaa !9
  %69 = load volatile i16, ptr %1, align 2, !tbaa !9
  %70 = srem i16 %68, %69
  %71 = icmp ne i16 %67, %70
  br i1 %71, label %72, label %75

72:                                               ; preds = %66, %56
  %73 = load i32, ptr %5, align 2, !tbaa !7
  %74 = add nsw i32 %73, 1
  store i32 %74, ptr %5, align 2, !tbaa !7
  br label %75

75:                                               ; preds = %72, %66
  %76 = load i16, ptr %3, align 2, !tbaa !9
  %77 = sext i16 %76 to i32
  %78 = mul i32 %77, 31
  %79 = load i16, ptr %4, align 2, !tbaa !9
  %80 = sext i16 %79 to i32
  %81 = add i32 %78, %80
  %82 = load i32, ptr @sum, align 2, !tbaa !7
  %83 = add i32 %82, %81
  store i32 %83, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %84

84:                                               ; preds = %75
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !20

87:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %88

88:                                               ; preds = %178, %87
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 12
  br i1 %90, label %91, label %181

91:                                               ; preds = %88
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 16383, ptr %9, align 2, !tbaa !9
  %92 = load i32, ptr %6, align 2, !tbaa !7
  %93 = icmp slt i32 %92, 2
  br i1 %93, label %94, label %96

94:                                               ; preds = %91
  %95 = load i32, ptr %6, align 2, !tbaa !7
  br label %148

96:                                               ; preds = %91
  %97 = load i32, ptr %6, align 2, !tbaa !7
  %98 = icmp slt i32 %97, 4
  br i1 %98, label %99, label %105

99:                                               ; preds = %96
  %100 = load i16, ptr %9, align 2, !tbaa !9
  %101 = sext i16 %100 to i32
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 2
  %104 = sub nsw i32 %101, %103
  br label %146

105:                                              ; preds = %96
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 6
  br i1 %107, label %108, label %112

108:                                              ; preds = %105
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = sub nsw i32 %109, 5
  %111 = add nsw i32 1000, %110
  br label %144

112:                                              ; preds = %105
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = icmp slt i32 %113, 8
  br i1 %114, label %115, label %123

115:                                              ; preds = %112
  %116 = load i16, ptr %9, align 2, !tbaa !9
  %117 = sdiv i16 %116, 1000
  %118 = mul nsw i16 %117, 1000
  %119 = sext i16 %118 to i32
  %120 = load i32, ptr %6, align 2, !tbaa !7
  %121 = sub nsw i32 %120, 6
  %122 = sub nsw i32 %119, %121
  br label %142

123:                                              ; preds = %112
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = icmp slt i32 %124, 10
  br i1 %125, label %126, label %134

126:                                              ; preds = %123
  %127 = load i16, ptr %9, align 2, !tbaa !9
  %128 = sub nsw i16 %127, 1000
  %129 = sext i16 %128 to i32
  %130 = load i32, ptr %6, align 2, !tbaa !7
  %131 = sub nsw i32 %130, 8
  %132 = add nsw i32 %129, %131
  %133 = trunc i32 %132 to i16
  br label %139

134:                                              ; preds = %123
  %135 = load i32, ptr %6, align 2, !tbaa !7
  %136 = sub nsw i32 %135, 10
  %137 = sub nsw i32 2000, %136
  %138 = trunc i32 %137 to i16
  br label %139

139:                                              ; preds = %134, %126
  %140 = phi i16 [ %133, %126 ], [ %138, %134 ]
  %141 = sext i16 %140 to i32
  br label %142

142:                                              ; preds = %139, %115
  %143 = phi i32 [ %122, %115 ], [ %141, %139 ]
  br label %144

144:                                              ; preds = %142, %108
  %145 = phi i32 [ %111, %108 ], [ %143, %142 ]
  br label %146

146:                                              ; preds = %144, %99
  %147 = phi i32 [ %104, %99 ], [ %145, %144 ]
  br label %148

148:                                              ; preds = %146, %94
  %149 = phi i32 [ %95, %94 ], [ %147, %146 ]
  %150 = trunc i32 %149 to i16
  store i16 %150, ptr %2, align 2, !tbaa !9
  %151 = load i16, ptr %2, align 2, !tbaa !9
  %152 = sdiv i16 %151, 1000
  store i16 %152, ptr %3, align 2, !tbaa !9
  %153 = load i16, ptr %2, align 2, !tbaa !9
  %154 = srem i16 %153, 1000
  store i16 %154, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %3, align 2, !tbaa !9
  %156 = load i16, ptr %2, align 2, !tbaa !9
  %157 = load volatile i16, ptr %1, align 2, !tbaa !9
  %158 = sdiv i16 %156, %157
  %159 = icmp ne i16 %155, %158
  br i1 %159, label %166, label %160

160:                                              ; preds = %148
  %161 = load i16, ptr %4, align 2, !tbaa !9
  %162 = load i16, ptr %2, align 2, !tbaa !9
  %163 = load volatile i16, ptr %1, align 2, !tbaa !9
  %164 = srem i16 %162, %163
  %165 = icmp ne i16 %161, %164
  br i1 %165, label %166, label %169

166:                                              ; preds = %160, %148
  %167 = load i32, ptr %5, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %5, align 2, !tbaa !7
  br label %169

169:                                              ; preds = %166, %160
  %170 = load i16, ptr %3, align 2, !tbaa !9
  %171 = sext i16 %170 to i32
  %172 = mul i32 %171, 31
  %173 = load i16, ptr %4, align 2, !tbaa !9
  %174 = sext i16 %173 to i32
  %175 = add i32 %172, %174
  %176 = load i32, ptr @sum, align 2, !tbaa !7
  %177 = add i32 %176, %175
  store i32 %177, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %178

178:                                              ; preds = %169
  %179 = load i32, ptr %6, align 2, !tbaa !7
  %180 = add nsw i32 %179, 1
  store i32 %180, ptr %6, align 2, !tbaa !7
  br label %88, !llvm.loop !21

181:                                              ; preds = %88
  %182 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %182
}

; Function Attrs: nounwind optsize
define internal i32 @c_short_32767() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 32767, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %84, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %87

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = call i32 @next() #4
  %51 = and i32 %50, 1
  %52 = icmp ne i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %47
  %54 = load i16, ptr %2, align 2, !tbaa !9
  %55 = sub nsw i16 0, %54
  store i16 %55, ptr %2, align 2, !tbaa !9
  br label %56

56:                                               ; preds = %53, %47
  %57 = load i16, ptr %2, align 2, !tbaa !9
  %58 = sdiv i16 %57, 32767
  store i16 %58, ptr %3, align 2, !tbaa !9
  %59 = load i16, ptr %2, align 2, !tbaa !9
  %60 = srem i16 %59, 32767
  store i16 %60, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %3, align 2, !tbaa !9
  %62 = load i16, ptr %2, align 2, !tbaa !9
  %63 = load volatile i16, ptr %1, align 2, !tbaa !9
  %64 = sdiv i16 %62, %63
  %65 = icmp ne i16 %61, %64
  br i1 %65, label %72, label %66

66:                                               ; preds = %56
  %67 = load i16, ptr %4, align 2, !tbaa !9
  %68 = load i16, ptr %2, align 2, !tbaa !9
  %69 = load volatile i16, ptr %1, align 2, !tbaa !9
  %70 = srem i16 %68, %69
  %71 = icmp ne i16 %67, %70
  br i1 %71, label %72, label %75

72:                                               ; preds = %66, %56
  %73 = load i32, ptr %5, align 2, !tbaa !7
  %74 = add nsw i32 %73, 1
  store i32 %74, ptr %5, align 2, !tbaa !7
  br label %75

75:                                               ; preds = %72, %66
  %76 = load i16, ptr %3, align 2, !tbaa !9
  %77 = sext i16 %76 to i32
  %78 = mul i32 %77, 31
  %79 = load i16, ptr %4, align 2, !tbaa !9
  %80 = sext i16 %79 to i32
  %81 = add i32 %78, %80
  %82 = load i32, ptr @sum, align 2, !tbaa !7
  %83 = add i32 %82, %81
  store i32 %83, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %84

84:                                               ; preds = %75
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !22

87:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %88

88:                                               ; preds = %178, %87
  %89 = load i32, ptr %6, align 2, !tbaa !7
  %90 = icmp slt i32 %89, 12
  br i1 %90, label %91, label %181

91:                                               ; preds = %88
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 16383, ptr %9, align 2, !tbaa !9
  %92 = load i32, ptr %6, align 2, !tbaa !7
  %93 = icmp slt i32 %92, 2
  br i1 %93, label %94, label %96

94:                                               ; preds = %91
  %95 = load i32, ptr %6, align 2, !tbaa !7
  br label %148

96:                                               ; preds = %91
  %97 = load i32, ptr %6, align 2, !tbaa !7
  %98 = icmp slt i32 %97, 4
  br i1 %98, label %99, label %105

99:                                               ; preds = %96
  %100 = load i16, ptr %9, align 2, !tbaa !9
  %101 = sext i16 %100 to i32
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 2
  %104 = sub nsw i32 %101, %103
  br label %146

105:                                              ; preds = %96
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 6
  br i1 %107, label %108, label %112

108:                                              ; preds = %105
  %109 = load i32, ptr %6, align 2, !tbaa !7
  %110 = sub nsw i32 %109, 5
  %111 = add nsw i32 32767, %110
  br label %144

112:                                              ; preds = %105
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = icmp slt i32 %113, 8
  br i1 %114, label %115, label %123

115:                                              ; preds = %112
  %116 = load i16, ptr %9, align 2, !tbaa !9
  %117 = sdiv i16 %116, 32767
  %118 = mul nsw i16 %117, 32767
  %119 = sext i16 %118 to i32
  %120 = load i32, ptr %6, align 2, !tbaa !7
  %121 = sub nsw i32 %120, 6
  %122 = sub nsw i32 %119, %121
  br label %142

123:                                              ; preds = %112
  %124 = load i32, ptr %6, align 2, !tbaa !7
  %125 = icmp slt i32 %124, 10
  br i1 %125, label %126, label %134

126:                                              ; preds = %123
  %127 = load i16, ptr %9, align 2, !tbaa !9
  %128 = sub nsw i16 %127, 32767
  %129 = sext i16 %128 to i32
  %130 = load i32, ptr %6, align 2, !tbaa !7
  %131 = sub nsw i32 %130, 8
  %132 = add nsw i32 %129, %131
  %133 = trunc i32 %132 to i16
  br label %139

134:                                              ; preds = %123
  %135 = load i32, ptr %6, align 2, !tbaa !7
  %136 = sub nsw i32 %135, 10
  %137 = sub nsw i32 -2, %136
  %138 = trunc i32 %137 to i16
  br label %139

139:                                              ; preds = %134, %126
  %140 = phi i16 [ %133, %126 ], [ %138, %134 ]
  %141 = sext i16 %140 to i32
  br label %142

142:                                              ; preds = %139, %115
  %143 = phi i32 [ %122, %115 ], [ %141, %139 ]
  br label %144

144:                                              ; preds = %142, %108
  %145 = phi i32 [ %111, %108 ], [ %143, %142 ]
  br label %146

146:                                              ; preds = %144, %99
  %147 = phi i32 [ %104, %99 ], [ %145, %144 ]
  br label %148

148:                                              ; preds = %146, %94
  %149 = phi i32 [ %95, %94 ], [ %147, %146 ]
  %150 = trunc i32 %149 to i16
  store i16 %150, ptr %2, align 2, !tbaa !9
  %151 = load i16, ptr %2, align 2, !tbaa !9
  %152 = sdiv i16 %151, 32767
  store i16 %152, ptr %3, align 2, !tbaa !9
  %153 = load i16, ptr %2, align 2, !tbaa !9
  %154 = srem i16 %153, 32767
  store i16 %154, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %3, align 2, !tbaa !9
  %156 = load i16, ptr %2, align 2, !tbaa !9
  %157 = load volatile i16, ptr %1, align 2, !tbaa !9
  %158 = sdiv i16 %156, %157
  %159 = icmp ne i16 %155, %158
  br i1 %159, label %166, label %160

160:                                              ; preds = %148
  %161 = load i16, ptr %4, align 2, !tbaa !9
  %162 = load i16, ptr %2, align 2, !tbaa !9
  %163 = load volatile i16, ptr %1, align 2, !tbaa !9
  %164 = srem i16 %162, %163
  %165 = icmp ne i16 %161, %164
  br i1 %165, label %166, label %169

166:                                              ; preds = %160, %148
  %167 = load i32, ptr %5, align 2, !tbaa !7
  %168 = add nsw i32 %167, 1
  store i32 %168, ptr %5, align 2, !tbaa !7
  br label %169

169:                                              ; preds = %166, %160
  %170 = load i16, ptr %3, align 2, !tbaa !9
  %171 = sext i16 %170 to i32
  %172 = mul i32 %171, 31
  %173 = load i16, ptr %4, align 2, !tbaa !9
  %174 = sext i16 %173 to i32
  %175 = add i32 %172, %174
  %176 = load i32, ptr @sum, align 2, !tbaa !7
  %177 = add i32 %176, %175
  store i32 %177, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %178

178:                                              ; preds = %169
  %179 = load i32, ptr %6, align 2, !tbaa !7
  %180 = add nsw i32 %179, 1
  store i32 %180, ptr %6, align 2, !tbaa !7
  br label %88, !llvm.loop !23

181:                                              ; preds = %88
  %182 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %182
}

; Function Attrs: nounwind optsize
define internal i32 @c_ushort_3() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 3, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %77, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %80

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = load i16, ptr %2, align 2, !tbaa !9
  %51 = udiv i16 %50, 3
  store i16 %51, ptr %3, align 2, !tbaa !9
  %52 = load i16, ptr %2, align 2, !tbaa !9
  %53 = urem i16 %52, 3
  store i16 %53, ptr %4, align 2, !tbaa !9
  %54 = load i16, ptr %3, align 2, !tbaa !9
  %55 = load i16, ptr %2, align 2, !tbaa !9
  %56 = load volatile i16, ptr %1, align 2, !tbaa !9
  %57 = udiv i16 %55, %56
  %58 = icmp ne i16 %54, %57
  br i1 %58, label %65, label %59

59:                                               ; preds = %47
  %60 = load i16, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %2, align 2, !tbaa !9
  %62 = load volatile i16, ptr %1, align 2, !tbaa !9
  %63 = urem i16 %61, %62
  %64 = icmp ne i16 %60, %63
  br i1 %64, label %65, label %68

65:                                               ; preds = %59, %47
  %66 = load i32, ptr %5, align 2, !tbaa !7
  %67 = add nsw i32 %66, 1
  store i32 %67, ptr %5, align 2, !tbaa !7
  br label %68

68:                                               ; preds = %65, %59
  %69 = load i16, ptr %3, align 2, !tbaa !9
  %70 = zext i16 %69 to i32
  %71 = mul i32 %70, 31
  %72 = load i16, ptr %4, align 2, !tbaa !9
  %73 = zext i16 %72 to i32
  %74 = add i32 %71, %73
  %75 = load i32, ptr @sum, align 2, !tbaa !7
  %76 = add i32 %75, %74
  store i32 %76, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %77

77:                                               ; preds = %68
  %78 = load i32, ptr %6, align 2, !tbaa !7
  %79 = add nsw i32 %78, 1
  store i32 %79, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !24

80:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %81

81:                                               ; preds = %171, %80
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 12
  br i1 %83, label %84, label %174

84:                                               ; preds = %81
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 -1, ptr %9, align 2, !tbaa !9
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = icmp slt i32 %85, 2
  br i1 %86, label %87, label %89

87:                                               ; preds = %84
  %88 = load i32, ptr %6, align 2, !tbaa !7
  br label %141

89:                                               ; preds = %84
  %90 = load i32, ptr %6, align 2, !tbaa !7
  %91 = icmp slt i32 %90, 4
  br i1 %91, label %92, label %98

92:                                               ; preds = %89
  %93 = load i16, ptr %9, align 2, !tbaa !9
  %94 = zext i16 %93 to i32
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = sub nsw i32 %95, 2
  %97 = sub nsw i32 %94, %96
  br label %139

98:                                               ; preds = %89
  %99 = load i32, ptr %6, align 2, !tbaa !7
  %100 = icmp slt i32 %99, 6
  br i1 %100, label %101, label %105

101:                                              ; preds = %98
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 5
  %104 = add nsw i32 3, %103
  br label %137

105:                                              ; preds = %98
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 8
  br i1 %107, label %108, label %116

108:                                              ; preds = %105
  %109 = load i16, ptr %9, align 2, !tbaa !9
  %110 = udiv i16 %109, 3
  %111 = mul i16 %110, 3
  %112 = zext i16 %111 to i32
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = sub nsw i32 %113, 6
  %115 = sub nsw i32 %112, %114
  br label %135

116:                                              ; preds = %105
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = icmp slt i32 %117, 10
  br i1 %118, label %119, label %127

119:                                              ; preds = %116
  %120 = load i16, ptr %9, align 2, !tbaa !9
  %121 = sub i16 %120, 3
  %122 = zext i16 %121 to i32
  %123 = load i32, ptr %6, align 2, !tbaa !7
  %124 = sub nsw i32 %123, 8
  %125 = add nsw i32 %122, %124
  %126 = trunc i32 %125 to i16
  br label %132

127:                                              ; preds = %116
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 6, %129
  %131 = trunc i32 %130 to i16
  br label %132

132:                                              ; preds = %127, %119
  %133 = phi i16 [ %126, %119 ], [ %131, %127 ]
  %134 = zext i16 %133 to i32
  br label %135

135:                                              ; preds = %132, %108
  %136 = phi i32 [ %115, %108 ], [ %134, %132 ]
  br label %137

137:                                              ; preds = %135, %101
  %138 = phi i32 [ %104, %101 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %92
  %140 = phi i32 [ %97, %92 ], [ %138, %137 ]
  br label %141

141:                                              ; preds = %139, %87
  %142 = phi i32 [ %88, %87 ], [ %140, %139 ]
  %143 = trunc i32 %142 to i16
  store i16 %143, ptr %2, align 2, !tbaa !9
  %144 = load i16, ptr %2, align 2, !tbaa !9
  %145 = udiv i16 %144, 3
  store i16 %145, ptr %3, align 2, !tbaa !9
  %146 = load i16, ptr %2, align 2, !tbaa !9
  %147 = urem i16 %146, 3
  store i16 %147, ptr %4, align 2, !tbaa !9
  %148 = load i16, ptr %3, align 2, !tbaa !9
  %149 = load i16, ptr %2, align 2, !tbaa !9
  %150 = load volatile i16, ptr %1, align 2, !tbaa !9
  %151 = udiv i16 %149, %150
  %152 = icmp ne i16 %148, %151
  br i1 %152, label %159, label %153

153:                                              ; preds = %141
  %154 = load i16, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %2, align 2, !tbaa !9
  %156 = load volatile i16, ptr %1, align 2, !tbaa !9
  %157 = urem i16 %155, %156
  %158 = icmp ne i16 %154, %157
  br i1 %158, label %159, label %162

159:                                              ; preds = %153, %141
  %160 = load i32, ptr %5, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %5, align 2, !tbaa !7
  br label %162

162:                                              ; preds = %159, %153
  %163 = load i16, ptr %3, align 2, !tbaa !9
  %164 = zext i16 %163 to i32
  %165 = mul i32 %164, 31
  %166 = load i16, ptr %4, align 2, !tbaa !9
  %167 = zext i16 %166 to i32
  %168 = add i32 %165, %167
  %169 = load i32, ptr @sum, align 2, !tbaa !7
  %170 = add i32 %169, %168
  store i32 %170, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %171

171:                                              ; preds = %162
  %172 = load i32, ptr %6, align 2, !tbaa !7
  %173 = add nsw i32 %172, 1
  store i32 %173, ptr %6, align 2, !tbaa !7
  br label %81, !llvm.loop !25

174:                                              ; preds = %81
  %175 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %175
}

; Function Attrs: nounwind optsize
define internal i32 @c_ushort_7() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 7, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %77, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %80

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = load i16, ptr %2, align 2, !tbaa !9
  %51 = udiv i16 %50, 7
  store i16 %51, ptr %3, align 2, !tbaa !9
  %52 = load i16, ptr %2, align 2, !tbaa !9
  %53 = urem i16 %52, 7
  store i16 %53, ptr %4, align 2, !tbaa !9
  %54 = load i16, ptr %3, align 2, !tbaa !9
  %55 = load i16, ptr %2, align 2, !tbaa !9
  %56 = load volatile i16, ptr %1, align 2, !tbaa !9
  %57 = udiv i16 %55, %56
  %58 = icmp ne i16 %54, %57
  br i1 %58, label %65, label %59

59:                                               ; preds = %47
  %60 = load i16, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %2, align 2, !tbaa !9
  %62 = load volatile i16, ptr %1, align 2, !tbaa !9
  %63 = urem i16 %61, %62
  %64 = icmp ne i16 %60, %63
  br i1 %64, label %65, label %68

65:                                               ; preds = %59, %47
  %66 = load i32, ptr %5, align 2, !tbaa !7
  %67 = add nsw i32 %66, 1
  store i32 %67, ptr %5, align 2, !tbaa !7
  br label %68

68:                                               ; preds = %65, %59
  %69 = load i16, ptr %3, align 2, !tbaa !9
  %70 = zext i16 %69 to i32
  %71 = mul i32 %70, 31
  %72 = load i16, ptr %4, align 2, !tbaa !9
  %73 = zext i16 %72 to i32
  %74 = add i32 %71, %73
  %75 = load i32, ptr @sum, align 2, !tbaa !7
  %76 = add i32 %75, %74
  store i32 %76, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %77

77:                                               ; preds = %68
  %78 = load i32, ptr %6, align 2, !tbaa !7
  %79 = add nsw i32 %78, 1
  store i32 %79, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !26

80:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %81

81:                                               ; preds = %171, %80
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 12
  br i1 %83, label %84, label %174

84:                                               ; preds = %81
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 -1, ptr %9, align 2, !tbaa !9
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = icmp slt i32 %85, 2
  br i1 %86, label %87, label %89

87:                                               ; preds = %84
  %88 = load i32, ptr %6, align 2, !tbaa !7
  br label %141

89:                                               ; preds = %84
  %90 = load i32, ptr %6, align 2, !tbaa !7
  %91 = icmp slt i32 %90, 4
  br i1 %91, label %92, label %98

92:                                               ; preds = %89
  %93 = load i16, ptr %9, align 2, !tbaa !9
  %94 = zext i16 %93 to i32
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = sub nsw i32 %95, 2
  %97 = sub nsw i32 %94, %96
  br label %139

98:                                               ; preds = %89
  %99 = load i32, ptr %6, align 2, !tbaa !7
  %100 = icmp slt i32 %99, 6
  br i1 %100, label %101, label %105

101:                                              ; preds = %98
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 5
  %104 = add nsw i32 7, %103
  br label %137

105:                                              ; preds = %98
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 8
  br i1 %107, label %108, label %116

108:                                              ; preds = %105
  %109 = load i16, ptr %9, align 2, !tbaa !9
  %110 = udiv i16 %109, 7
  %111 = mul i16 %110, 7
  %112 = zext i16 %111 to i32
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = sub nsw i32 %113, 6
  %115 = sub nsw i32 %112, %114
  br label %135

116:                                              ; preds = %105
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = icmp slt i32 %117, 10
  br i1 %118, label %119, label %127

119:                                              ; preds = %116
  %120 = load i16, ptr %9, align 2, !tbaa !9
  %121 = sub i16 %120, 7
  %122 = zext i16 %121 to i32
  %123 = load i32, ptr %6, align 2, !tbaa !7
  %124 = sub nsw i32 %123, 8
  %125 = add nsw i32 %122, %124
  %126 = trunc i32 %125 to i16
  br label %132

127:                                              ; preds = %116
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 14, %129
  %131 = trunc i32 %130 to i16
  br label %132

132:                                              ; preds = %127, %119
  %133 = phi i16 [ %126, %119 ], [ %131, %127 ]
  %134 = zext i16 %133 to i32
  br label %135

135:                                              ; preds = %132, %108
  %136 = phi i32 [ %115, %108 ], [ %134, %132 ]
  br label %137

137:                                              ; preds = %135, %101
  %138 = phi i32 [ %104, %101 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %92
  %140 = phi i32 [ %97, %92 ], [ %138, %137 ]
  br label %141

141:                                              ; preds = %139, %87
  %142 = phi i32 [ %88, %87 ], [ %140, %139 ]
  %143 = trunc i32 %142 to i16
  store i16 %143, ptr %2, align 2, !tbaa !9
  %144 = load i16, ptr %2, align 2, !tbaa !9
  %145 = udiv i16 %144, 7
  store i16 %145, ptr %3, align 2, !tbaa !9
  %146 = load i16, ptr %2, align 2, !tbaa !9
  %147 = urem i16 %146, 7
  store i16 %147, ptr %4, align 2, !tbaa !9
  %148 = load i16, ptr %3, align 2, !tbaa !9
  %149 = load i16, ptr %2, align 2, !tbaa !9
  %150 = load volatile i16, ptr %1, align 2, !tbaa !9
  %151 = udiv i16 %149, %150
  %152 = icmp ne i16 %148, %151
  br i1 %152, label %159, label %153

153:                                              ; preds = %141
  %154 = load i16, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %2, align 2, !tbaa !9
  %156 = load volatile i16, ptr %1, align 2, !tbaa !9
  %157 = urem i16 %155, %156
  %158 = icmp ne i16 %154, %157
  br i1 %158, label %159, label %162

159:                                              ; preds = %153, %141
  %160 = load i32, ptr %5, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %5, align 2, !tbaa !7
  br label %162

162:                                              ; preds = %159, %153
  %163 = load i16, ptr %3, align 2, !tbaa !9
  %164 = zext i16 %163 to i32
  %165 = mul i32 %164, 31
  %166 = load i16, ptr %4, align 2, !tbaa !9
  %167 = zext i16 %166 to i32
  %168 = add i32 %165, %167
  %169 = load i32, ptr @sum, align 2, !tbaa !7
  %170 = add i32 %169, %168
  store i32 %170, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %171

171:                                              ; preds = %162
  %172 = load i32, ptr %6, align 2, !tbaa !7
  %173 = add nsw i32 %172, 1
  store i32 %173, ptr %6, align 2, !tbaa !7
  br label %81, !llvm.loop !27

174:                                              ; preds = %81
  %175 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %175
}

; Function Attrs: nounwind optsize
define internal i32 @c_ushort_10() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 10, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %77, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %80

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = load i16, ptr %2, align 2, !tbaa !9
  %51 = udiv i16 %50, 10
  store i16 %51, ptr %3, align 2, !tbaa !9
  %52 = load i16, ptr %2, align 2, !tbaa !9
  %53 = urem i16 %52, 10
  store i16 %53, ptr %4, align 2, !tbaa !9
  %54 = load i16, ptr %3, align 2, !tbaa !9
  %55 = load i16, ptr %2, align 2, !tbaa !9
  %56 = load volatile i16, ptr %1, align 2, !tbaa !9
  %57 = udiv i16 %55, %56
  %58 = icmp ne i16 %54, %57
  br i1 %58, label %65, label %59

59:                                               ; preds = %47
  %60 = load i16, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %2, align 2, !tbaa !9
  %62 = load volatile i16, ptr %1, align 2, !tbaa !9
  %63 = urem i16 %61, %62
  %64 = icmp ne i16 %60, %63
  br i1 %64, label %65, label %68

65:                                               ; preds = %59, %47
  %66 = load i32, ptr %5, align 2, !tbaa !7
  %67 = add nsw i32 %66, 1
  store i32 %67, ptr %5, align 2, !tbaa !7
  br label %68

68:                                               ; preds = %65, %59
  %69 = load i16, ptr %3, align 2, !tbaa !9
  %70 = zext i16 %69 to i32
  %71 = mul i32 %70, 31
  %72 = load i16, ptr %4, align 2, !tbaa !9
  %73 = zext i16 %72 to i32
  %74 = add i32 %71, %73
  %75 = load i32, ptr @sum, align 2, !tbaa !7
  %76 = add i32 %75, %74
  store i32 %76, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %77

77:                                               ; preds = %68
  %78 = load i32, ptr %6, align 2, !tbaa !7
  %79 = add nsw i32 %78, 1
  store i32 %79, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !28

80:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %81

81:                                               ; preds = %171, %80
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 12
  br i1 %83, label %84, label %174

84:                                               ; preds = %81
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 -1, ptr %9, align 2, !tbaa !9
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = icmp slt i32 %85, 2
  br i1 %86, label %87, label %89

87:                                               ; preds = %84
  %88 = load i32, ptr %6, align 2, !tbaa !7
  br label %141

89:                                               ; preds = %84
  %90 = load i32, ptr %6, align 2, !tbaa !7
  %91 = icmp slt i32 %90, 4
  br i1 %91, label %92, label %98

92:                                               ; preds = %89
  %93 = load i16, ptr %9, align 2, !tbaa !9
  %94 = zext i16 %93 to i32
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = sub nsw i32 %95, 2
  %97 = sub nsw i32 %94, %96
  br label %139

98:                                               ; preds = %89
  %99 = load i32, ptr %6, align 2, !tbaa !7
  %100 = icmp slt i32 %99, 6
  br i1 %100, label %101, label %105

101:                                              ; preds = %98
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 5
  %104 = add nsw i32 10, %103
  br label %137

105:                                              ; preds = %98
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 8
  br i1 %107, label %108, label %116

108:                                              ; preds = %105
  %109 = load i16, ptr %9, align 2, !tbaa !9
  %110 = udiv i16 %109, 10
  %111 = mul i16 %110, 10
  %112 = zext i16 %111 to i32
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = sub nsw i32 %113, 6
  %115 = sub nsw i32 %112, %114
  br label %135

116:                                              ; preds = %105
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = icmp slt i32 %117, 10
  br i1 %118, label %119, label %127

119:                                              ; preds = %116
  %120 = load i16, ptr %9, align 2, !tbaa !9
  %121 = sub i16 %120, 10
  %122 = zext i16 %121 to i32
  %123 = load i32, ptr %6, align 2, !tbaa !7
  %124 = sub nsw i32 %123, 8
  %125 = add nsw i32 %122, %124
  %126 = trunc i32 %125 to i16
  br label %132

127:                                              ; preds = %116
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 20, %129
  %131 = trunc i32 %130 to i16
  br label %132

132:                                              ; preds = %127, %119
  %133 = phi i16 [ %126, %119 ], [ %131, %127 ]
  %134 = zext i16 %133 to i32
  br label %135

135:                                              ; preds = %132, %108
  %136 = phi i32 [ %115, %108 ], [ %134, %132 ]
  br label %137

137:                                              ; preds = %135, %101
  %138 = phi i32 [ %104, %101 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %92
  %140 = phi i32 [ %97, %92 ], [ %138, %137 ]
  br label %141

141:                                              ; preds = %139, %87
  %142 = phi i32 [ %88, %87 ], [ %140, %139 ]
  %143 = trunc i32 %142 to i16
  store i16 %143, ptr %2, align 2, !tbaa !9
  %144 = load i16, ptr %2, align 2, !tbaa !9
  %145 = udiv i16 %144, 10
  store i16 %145, ptr %3, align 2, !tbaa !9
  %146 = load i16, ptr %2, align 2, !tbaa !9
  %147 = urem i16 %146, 10
  store i16 %147, ptr %4, align 2, !tbaa !9
  %148 = load i16, ptr %3, align 2, !tbaa !9
  %149 = load i16, ptr %2, align 2, !tbaa !9
  %150 = load volatile i16, ptr %1, align 2, !tbaa !9
  %151 = udiv i16 %149, %150
  %152 = icmp ne i16 %148, %151
  br i1 %152, label %159, label %153

153:                                              ; preds = %141
  %154 = load i16, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %2, align 2, !tbaa !9
  %156 = load volatile i16, ptr %1, align 2, !tbaa !9
  %157 = urem i16 %155, %156
  %158 = icmp ne i16 %154, %157
  br i1 %158, label %159, label %162

159:                                              ; preds = %153, %141
  %160 = load i32, ptr %5, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %5, align 2, !tbaa !7
  br label %162

162:                                              ; preds = %159, %153
  %163 = load i16, ptr %3, align 2, !tbaa !9
  %164 = zext i16 %163 to i32
  %165 = mul i32 %164, 31
  %166 = load i16, ptr %4, align 2, !tbaa !9
  %167 = zext i16 %166 to i32
  %168 = add i32 %165, %167
  %169 = load i32, ptr @sum, align 2, !tbaa !7
  %170 = add i32 %169, %168
  store i32 %170, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %171

171:                                              ; preds = %162
  %172 = load i32, ptr %6, align 2, !tbaa !7
  %173 = add nsw i32 %172, 1
  store i32 %173, ptr %6, align 2, !tbaa !7
  br label %81, !llvm.loop !29

174:                                              ; preds = %81
  %175 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %175
}

; Function Attrs: nounwind optsize
define internal i32 @c_ushort_100() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 100, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %77, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %80

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = load i16, ptr %2, align 2, !tbaa !9
  %51 = udiv i16 %50, 100
  store i16 %51, ptr %3, align 2, !tbaa !9
  %52 = load i16, ptr %2, align 2, !tbaa !9
  %53 = urem i16 %52, 100
  store i16 %53, ptr %4, align 2, !tbaa !9
  %54 = load i16, ptr %3, align 2, !tbaa !9
  %55 = load i16, ptr %2, align 2, !tbaa !9
  %56 = load volatile i16, ptr %1, align 2, !tbaa !9
  %57 = udiv i16 %55, %56
  %58 = icmp ne i16 %54, %57
  br i1 %58, label %65, label %59

59:                                               ; preds = %47
  %60 = load i16, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %2, align 2, !tbaa !9
  %62 = load volatile i16, ptr %1, align 2, !tbaa !9
  %63 = urem i16 %61, %62
  %64 = icmp ne i16 %60, %63
  br i1 %64, label %65, label %68

65:                                               ; preds = %59, %47
  %66 = load i32, ptr %5, align 2, !tbaa !7
  %67 = add nsw i32 %66, 1
  store i32 %67, ptr %5, align 2, !tbaa !7
  br label %68

68:                                               ; preds = %65, %59
  %69 = load i16, ptr %3, align 2, !tbaa !9
  %70 = zext i16 %69 to i32
  %71 = mul i32 %70, 31
  %72 = load i16, ptr %4, align 2, !tbaa !9
  %73 = zext i16 %72 to i32
  %74 = add i32 %71, %73
  %75 = load i32, ptr @sum, align 2, !tbaa !7
  %76 = add i32 %75, %74
  store i32 %76, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %77

77:                                               ; preds = %68
  %78 = load i32, ptr %6, align 2, !tbaa !7
  %79 = add nsw i32 %78, 1
  store i32 %79, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !30

80:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %81

81:                                               ; preds = %171, %80
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 12
  br i1 %83, label %84, label %174

84:                                               ; preds = %81
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 -1, ptr %9, align 2, !tbaa !9
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = icmp slt i32 %85, 2
  br i1 %86, label %87, label %89

87:                                               ; preds = %84
  %88 = load i32, ptr %6, align 2, !tbaa !7
  br label %141

89:                                               ; preds = %84
  %90 = load i32, ptr %6, align 2, !tbaa !7
  %91 = icmp slt i32 %90, 4
  br i1 %91, label %92, label %98

92:                                               ; preds = %89
  %93 = load i16, ptr %9, align 2, !tbaa !9
  %94 = zext i16 %93 to i32
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = sub nsw i32 %95, 2
  %97 = sub nsw i32 %94, %96
  br label %139

98:                                               ; preds = %89
  %99 = load i32, ptr %6, align 2, !tbaa !7
  %100 = icmp slt i32 %99, 6
  br i1 %100, label %101, label %105

101:                                              ; preds = %98
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 5
  %104 = add nsw i32 100, %103
  br label %137

105:                                              ; preds = %98
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 8
  br i1 %107, label %108, label %116

108:                                              ; preds = %105
  %109 = load i16, ptr %9, align 2, !tbaa !9
  %110 = udiv i16 %109, 100
  %111 = mul i16 %110, 100
  %112 = zext i16 %111 to i32
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = sub nsw i32 %113, 6
  %115 = sub nsw i32 %112, %114
  br label %135

116:                                              ; preds = %105
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = icmp slt i32 %117, 10
  br i1 %118, label %119, label %127

119:                                              ; preds = %116
  %120 = load i16, ptr %9, align 2, !tbaa !9
  %121 = sub i16 %120, 100
  %122 = zext i16 %121 to i32
  %123 = load i32, ptr %6, align 2, !tbaa !7
  %124 = sub nsw i32 %123, 8
  %125 = add nsw i32 %122, %124
  %126 = trunc i32 %125 to i16
  br label %132

127:                                              ; preds = %116
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 200, %129
  %131 = trunc i32 %130 to i16
  br label %132

132:                                              ; preds = %127, %119
  %133 = phi i16 [ %126, %119 ], [ %131, %127 ]
  %134 = zext i16 %133 to i32
  br label %135

135:                                              ; preds = %132, %108
  %136 = phi i32 [ %115, %108 ], [ %134, %132 ]
  br label %137

137:                                              ; preds = %135, %101
  %138 = phi i32 [ %104, %101 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %92
  %140 = phi i32 [ %97, %92 ], [ %138, %137 ]
  br label %141

141:                                              ; preds = %139, %87
  %142 = phi i32 [ %88, %87 ], [ %140, %139 ]
  %143 = trunc i32 %142 to i16
  store i16 %143, ptr %2, align 2, !tbaa !9
  %144 = load i16, ptr %2, align 2, !tbaa !9
  %145 = udiv i16 %144, 100
  store i16 %145, ptr %3, align 2, !tbaa !9
  %146 = load i16, ptr %2, align 2, !tbaa !9
  %147 = urem i16 %146, 100
  store i16 %147, ptr %4, align 2, !tbaa !9
  %148 = load i16, ptr %3, align 2, !tbaa !9
  %149 = load i16, ptr %2, align 2, !tbaa !9
  %150 = load volatile i16, ptr %1, align 2, !tbaa !9
  %151 = udiv i16 %149, %150
  %152 = icmp ne i16 %148, %151
  br i1 %152, label %159, label %153

153:                                              ; preds = %141
  %154 = load i16, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %2, align 2, !tbaa !9
  %156 = load volatile i16, ptr %1, align 2, !tbaa !9
  %157 = urem i16 %155, %156
  %158 = icmp ne i16 %154, %157
  br i1 %158, label %159, label %162

159:                                              ; preds = %153, %141
  %160 = load i32, ptr %5, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %5, align 2, !tbaa !7
  br label %162

162:                                              ; preds = %159, %153
  %163 = load i16, ptr %3, align 2, !tbaa !9
  %164 = zext i16 %163 to i32
  %165 = mul i32 %164, 31
  %166 = load i16, ptr %4, align 2, !tbaa !9
  %167 = zext i16 %166 to i32
  %168 = add i32 %165, %167
  %169 = load i32, ptr @sum, align 2, !tbaa !7
  %170 = add i32 %169, %168
  store i32 %170, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %171

171:                                              ; preds = %162
  %172 = load i32, ptr %6, align 2, !tbaa !7
  %173 = add nsw i32 %172, 1
  store i32 %173, ptr %6, align 2, !tbaa !7
  br label %81, !llvm.loop !31

174:                                              ; preds = %81
  %175 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %175
}

; Function Attrs: nounwind optsize
define internal i32 @c_ushort_1000() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 1000, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %77, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %80

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = load i16, ptr %2, align 2, !tbaa !9
  %51 = udiv i16 %50, 1000
  store i16 %51, ptr %3, align 2, !tbaa !9
  %52 = load i16, ptr %2, align 2, !tbaa !9
  %53 = urem i16 %52, 1000
  store i16 %53, ptr %4, align 2, !tbaa !9
  %54 = load i16, ptr %3, align 2, !tbaa !9
  %55 = load i16, ptr %2, align 2, !tbaa !9
  %56 = load volatile i16, ptr %1, align 2, !tbaa !9
  %57 = udiv i16 %55, %56
  %58 = icmp ne i16 %54, %57
  br i1 %58, label %65, label %59

59:                                               ; preds = %47
  %60 = load i16, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %2, align 2, !tbaa !9
  %62 = load volatile i16, ptr %1, align 2, !tbaa !9
  %63 = urem i16 %61, %62
  %64 = icmp ne i16 %60, %63
  br i1 %64, label %65, label %68

65:                                               ; preds = %59, %47
  %66 = load i32, ptr %5, align 2, !tbaa !7
  %67 = add nsw i32 %66, 1
  store i32 %67, ptr %5, align 2, !tbaa !7
  br label %68

68:                                               ; preds = %65, %59
  %69 = load i16, ptr %3, align 2, !tbaa !9
  %70 = zext i16 %69 to i32
  %71 = mul i32 %70, 31
  %72 = load i16, ptr %4, align 2, !tbaa !9
  %73 = zext i16 %72 to i32
  %74 = add i32 %71, %73
  %75 = load i32, ptr @sum, align 2, !tbaa !7
  %76 = add i32 %75, %74
  store i32 %76, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %77

77:                                               ; preds = %68
  %78 = load i32, ptr %6, align 2, !tbaa !7
  %79 = add nsw i32 %78, 1
  store i32 %79, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !32

80:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %81

81:                                               ; preds = %171, %80
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 12
  br i1 %83, label %84, label %174

84:                                               ; preds = %81
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 -1, ptr %9, align 2, !tbaa !9
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = icmp slt i32 %85, 2
  br i1 %86, label %87, label %89

87:                                               ; preds = %84
  %88 = load i32, ptr %6, align 2, !tbaa !7
  br label %141

89:                                               ; preds = %84
  %90 = load i32, ptr %6, align 2, !tbaa !7
  %91 = icmp slt i32 %90, 4
  br i1 %91, label %92, label %98

92:                                               ; preds = %89
  %93 = load i16, ptr %9, align 2, !tbaa !9
  %94 = zext i16 %93 to i32
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = sub nsw i32 %95, 2
  %97 = sub nsw i32 %94, %96
  br label %139

98:                                               ; preds = %89
  %99 = load i32, ptr %6, align 2, !tbaa !7
  %100 = icmp slt i32 %99, 6
  br i1 %100, label %101, label %105

101:                                              ; preds = %98
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 5
  %104 = add nsw i32 1000, %103
  br label %137

105:                                              ; preds = %98
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 8
  br i1 %107, label %108, label %116

108:                                              ; preds = %105
  %109 = load i16, ptr %9, align 2, !tbaa !9
  %110 = udiv i16 %109, 1000
  %111 = mul i16 %110, 1000
  %112 = zext i16 %111 to i32
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = sub nsw i32 %113, 6
  %115 = sub nsw i32 %112, %114
  br label %135

116:                                              ; preds = %105
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = icmp slt i32 %117, 10
  br i1 %118, label %119, label %127

119:                                              ; preds = %116
  %120 = load i16, ptr %9, align 2, !tbaa !9
  %121 = sub i16 %120, 1000
  %122 = zext i16 %121 to i32
  %123 = load i32, ptr %6, align 2, !tbaa !7
  %124 = sub nsw i32 %123, 8
  %125 = add nsw i32 %122, %124
  %126 = trunc i32 %125 to i16
  br label %132

127:                                              ; preds = %116
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 2000, %129
  %131 = trunc i32 %130 to i16
  br label %132

132:                                              ; preds = %127, %119
  %133 = phi i16 [ %126, %119 ], [ %131, %127 ]
  %134 = zext i16 %133 to i32
  br label %135

135:                                              ; preds = %132, %108
  %136 = phi i32 [ %115, %108 ], [ %134, %132 ]
  br label %137

137:                                              ; preds = %135, %101
  %138 = phi i32 [ %104, %101 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %92
  %140 = phi i32 [ %97, %92 ], [ %138, %137 ]
  br label %141

141:                                              ; preds = %139, %87
  %142 = phi i32 [ %88, %87 ], [ %140, %139 ]
  %143 = trunc i32 %142 to i16
  store i16 %143, ptr %2, align 2, !tbaa !9
  %144 = load i16, ptr %2, align 2, !tbaa !9
  %145 = udiv i16 %144, 1000
  store i16 %145, ptr %3, align 2, !tbaa !9
  %146 = load i16, ptr %2, align 2, !tbaa !9
  %147 = urem i16 %146, 1000
  store i16 %147, ptr %4, align 2, !tbaa !9
  %148 = load i16, ptr %3, align 2, !tbaa !9
  %149 = load i16, ptr %2, align 2, !tbaa !9
  %150 = load volatile i16, ptr %1, align 2, !tbaa !9
  %151 = udiv i16 %149, %150
  %152 = icmp ne i16 %148, %151
  br i1 %152, label %159, label %153

153:                                              ; preds = %141
  %154 = load i16, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %2, align 2, !tbaa !9
  %156 = load volatile i16, ptr %1, align 2, !tbaa !9
  %157 = urem i16 %155, %156
  %158 = icmp ne i16 %154, %157
  br i1 %158, label %159, label %162

159:                                              ; preds = %153, %141
  %160 = load i32, ptr %5, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %5, align 2, !tbaa !7
  br label %162

162:                                              ; preds = %159, %153
  %163 = load i16, ptr %3, align 2, !tbaa !9
  %164 = zext i16 %163 to i32
  %165 = mul i32 %164, 31
  %166 = load i16, ptr %4, align 2, !tbaa !9
  %167 = zext i16 %166 to i32
  %168 = add i32 %165, %167
  %169 = load i32, ptr @sum, align 2, !tbaa !7
  %170 = add i32 %169, %168
  store i32 %170, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %171

171:                                              ; preds = %162
  %172 = load i32, ptr %6, align 2, !tbaa !7
  %173 = add nsw i32 %172, 1
  store i32 %173, ptr %6, align 2, !tbaa !7
  br label %81, !llvm.loop !33

174:                                              ; preds = %81
  %175 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %175
}

; Function Attrs: nounwind optsize
define internal i32 @c_ushort_32767() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store volatile i16 32767, ptr %1, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %77, %0
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %80

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
  %49 = trunc i32 %48 to i16
  store i16 %49, ptr %2, align 2, !tbaa !9
  %50 = load i16, ptr %2, align 2, !tbaa !9
  %51 = udiv i16 %50, 32767
  store i16 %51, ptr %3, align 2, !tbaa !9
  %52 = load i16, ptr %2, align 2, !tbaa !9
  %53 = urem i16 %52, 32767
  store i16 %53, ptr %4, align 2, !tbaa !9
  %54 = load i16, ptr %3, align 2, !tbaa !9
  %55 = load i16, ptr %2, align 2, !tbaa !9
  %56 = load volatile i16, ptr %1, align 2, !tbaa !9
  %57 = udiv i16 %55, %56
  %58 = icmp ne i16 %54, %57
  br i1 %58, label %65, label %59

59:                                               ; preds = %47
  %60 = load i16, ptr %4, align 2, !tbaa !9
  %61 = load i16, ptr %2, align 2, !tbaa !9
  %62 = load volatile i16, ptr %1, align 2, !tbaa !9
  %63 = urem i16 %61, %62
  %64 = icmp ne i16 %60, %63
  br i1 %64, label %65, label %68

65:                                               ; preds = %59, %47
  %66 = load i32, ptr %5, align 2, !tbaa !7
  %67 = add nsw i32 %66, 1
  store i32 %67, ptr %5, align 2, !tbaa !7
  br label %68

68:                                               ; preds = %65, %59
  %69 = load i16, ptr %3, align 2, !tbaa !9
  %70 = zext i16 %69 to i32
  %71 = mul i32 %70, 31
  %72 = load i16, ptr %4, align 2, !tbaa !9
  %73 = zext i16 %72 to i32
  %74 = add i32 %71, %73
  %75 = load i32, ptr @sum, align 2, !tbaa !7
  %76 = add i32 %75, %74
  store i32 %76, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %77

77:                                               ; preds = %68
  %78 = load i32, ptr %6, align 2, !tbaa !7
  %79 = add nsw i32 %78, 1
  store i32 %79, ptr %6, align 2, !tbaa !7
  br label %10, !llvm.loop !34

80:                                               ; preds = %10
  store i32 0, ptr %6, align 2, !tbaa !7
  br label %81

81:                                               ; preds = %171, %80
  %82 = load i32, ptr %6, align 2, !tbaa !7
  %83 = icmp slt i32 %82, 12
  br i1 %83, label %84, label %174

84:                                               ; preds = %81
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 -1, ptr %9, align 2, !tbaa !9
  %85 = load i32, ptr %6, align 2, !tbaa !7
  %86 = icmp slt i32 %85, 2
  br i1 %86, label %87, label %89

87:                                               ; preds = %84
  %88 = load i32, ptr %6, align 2, !tbaa !7
  br label %141

89:                                               ; preds = %84
  %90 = load i32, ptr %6, align 2, !tbaa !7
  %91 = icmp slt i32 %90, 4
  br i1 %91, label %92, label %98

92:                                               ; preds = %89
  %93 = load i16, ptr %9, align 2, !tbaa !9
  %94 = zext i16 %93 to i32
  %95 = load i32, ptr %6, align 2, !tbaa !7
  %96 = sub nsw i32 %95, 2
  %97 = sub nsw i32 %94, %96
  br label %139

98:                                               ; preds = %89
  %99 = load i32, ptr %6, align 2, !tbaa !7
  %100 = icmp slt i32 %99, 6
  br i1 %100, label %101, label %105

101:                                              ; preds = %98
  %102 = load i32, ptr %6, align 2, !tbaa !7
  %103 = sub nsw i32 %102, 5
  %104 = add nsw i32 32767, %103
  br label %137

105:                                              ; preds = %98
  %106 = load i32, ptr %6, align 2, !tbaa !7
  %107 = icmp slt i32 %106, 8
  br i1 %107, label %108, label %116

108:                                              ; preds = %105
  %109 = load i16, ptr %9, align 2, !tbaa !9
  %110 = udiv i16 %109, 32767
  %111 = mul i16 %110, 32767
  %112 = zext i16 %111 to i32
  %113 = load i32, ptr %6, align 2, !tbaa !7
  %114 = sub nsw i32 %113, 6
  %115 = sub nsw i32 %112, %114
  br label %135

116:                                              ; preds = %105
  %117 = load i32, ptr %6, align 2, !tbaa !7
  %118 = icmp slt i32 %117, 10
  br i1 %118, label %119, label %127

119:                                              ; preds = %116
  %120 = load i16, ptr %9, align 2, !tbaa !9
  %121 = sub i16 %120, 32767
  %122 = zext i16 %121 to i32
  %123 = load i32, ptr %6, align 2, !tbaa !7
  %124 = sub nsw i32 %123, 8
  %125 = add nsw i32 %122, %124
  %126 = trunc i32 %125 to i16
  br label %132

127:                                              ; preds = %116
  %128 = load i32, ptr %6, align 2, !tbaa !7
  %129 = sub nsw i32 %128, 10
  %130 = sub nsw i32 -2, %129
  %131 = trunc i32 %130 to i16
  br label %132

132:                                              ; preds = %127, %119
  %133 = phi i16 [ %126, %119 ], [ %131, %127 ]
  %134 = zext i16 %133 to i32
  br label %135

135:                                              ; preds = %132, %108
  %136 = phi i32 [ %115, %108 ], [ %134, %132 ]
  br label %137

137:                                              ; preds = %135, %101
  %138 = phi i32 [ %104, %101 ], [ %136, %135 ]
  br label %139

139:                                              ; preds = %137, %92
  %140 = phi i32 [ %97, %92 ], [ %138, %137 ]
  br label %141

141:                                              ; preds = %139, %87
  %142 = phi i32 [ %88, %87 ], [ %140, %139 ]
  %143 = trunc i32 %142 to i16
  store i16 %143, ptr %2, align 2, !tbaa !9
  %144 = load i16, ptr %2, align 2, !tbaa !9
  %145 = udiv i16 %144, 32767
  store i16 %145, ptr %3, align 2, !tbaa !9
  %146 = load i16, ptr %2, align 2, !tbaa !9
  %147 = urem i16 %146, 32767
  store i16 %147, ptr %4, align 2, !tbaa !9
  %148 = load i16, ptr %3, align 2, !tbaa !9
  %149 = load i16, ptr %2, align 2, !tbaa !9
  %150 = load volatile i16, ptr %1, align 2, !tbaa !9
  %151 = udiv i16 %149, %150
  %152 = icmp ne i16 %148, %151
  br i1 %152, label %159, label %153

153:                                              ; preds = %141
  %154 = load i16, ptr %4, align 2, !tbaa !9
  %155 = load i16, ptr %2, align 2, !tbaa !9
  %156 = load volatile i16, ptr %1, align 2, !tbaa !9
  %157 = urem i16 %155, %156
  %158 = icmp ne i16 %154, %157
  br i1 %158, label %159, label %162

159:                                              ; preds = %153, %141
  %160 = load i32, ptr %5, align 2, !tbaa !7
  %161 = add nsw i32 %160, 1
  store i32 %161, ptr %5, align 2, !tbaa !7
  br label %162

162:                                              ; preds = %159, %153
  %163 = load i16, ptr %3, align 2, !tbaa !9
  %164 = zext i16 %163 to i32
  %165 = mul i32 %164, 31
  %166 = load i16, ptr %4, align 2, !tbaa !9
  %167 = zext i16 %166 to i32
  %168 = add i32 %165, %167
  %169 = load i32, ptr @sum, align 2, !tbaa !7
  %170 = add i32 %169, %168
  store i32 %170, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %171

171:                                              ; preds = %162
  %172 = load i32, ptr %6, align 2, !tbaa !7
  %173 = add nsw i32 %172, 1
  store i32 %173, ptr %6, align 2, !tbaa !7
  br label %81, !llvm.loop !35

174:                                              ; preds = %81
  %175 = load i32, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %175
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
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
!19 = distinct !{!19, !12}
!20 = distinct !{!20, !12}
!21 = distinct !{!21, !12}
!22 = distinct !{!22, !12}
!23 = distinct !{!23, !12}
!24 = distinct !{!24, !12}
!25 = distinct !{!25, !12}
!26 = distinct !{!26, !12}
!27 = distinct !{!27, !12}
!28 = distinct !{!28, !12}
!29 = distinct !{!29, !12}
!30 = distinct !{!30, !12}
!31 = distinct !{!31, !12}
!32 = distinct !{!32, !12}
!33 = distinct !{!33, !12}
!34 = distinct !{!34, !12}
!35 = distinct !{!35, !12}
