; ModuleID = '../../../../tests/run/c/long_compare_chain.c'
source_filename = "../../../../tests/run/c/long_compare_chain.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k"

@v = internal constant [15 x i32] [i32 0, i32 1, i32 -1, i32 32767, i32 32768, i32 65535, i32 65536, i32 -65536, i32 2147483647, i32 -2147483648, i32 65537, i32 -65536, i32 -70000, i32 70000, i32 -2147418112], align 2

; Function Attrs: nounwind optsize
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca [8 x i32], align 2
  %5 = alloca [4 x i32], align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  store i32 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store i32 0, ptr %2, align 2, !tbaa !6
  br label %10

10:                                               ; preds = %207, %0
  %11 = load i32, ptr %2, align 2, !tbaa !6
  %12 = icmp ult i32 %11, 15
  br i1 %12, label %13, label %210

13:                                               ; preds = %10
  store i32 0, ptr %3, align 2, !tbaa !6
  br label %14

14:                                               ; preds = %20, %13
  %15 = load i32, ptr %3, align 2, !tbaa !6
  %16 = icmp ult i32 %15, 8
  br i1 %16, label %17, label %23

17:                                               ; preds = %14
  %18 = load i32, ptr %3, align 2, !tbaa !6
  %19 = getelementptr inbounds nuw [8 x i32], ptr %4, i32 0, i32 %18
  store i32 0, ptr %19, align 2, !tbaa !7
  br label %20

20:                                               ; preds = %17
  %21 = load i32, ptr %3, align 2, !tbaa !6
  %22 = add i32 %21, 1
  store i32 %22, ptr %3, align 2, !tbaa !6
  br label %14, !llvm.loop !9

23:                                               ; preds = %14
  store i32 0, ptr %3, align 2, !tbaa !6
  br label %24

24:                                               ; preds = %30, %23
  %25 = load i32, ptr %3, align 2, !tbaa !6
  %26 = icmp ult i32 %25, 4
  br i1 %26, label %27, label %33

27:                                               ; preds = %24
  %28 = load i32, ptr %3, align 2, !tbaa !6
  %29 = getelementptr inbounds nuw [4 x i32], ptr %5, i32 0, i32 %28
  store i32 0, ptr %29, align 2, !tbaa !7
  br label %30

30:                                               ; preds = %27
  %31 = load i32, ptr %3, align 2, !tbaa !6
  %32 = add i32 %31, 1
  store i32 %32, ptr %3, align 2, !tbaa !6
  br label %24, !llvm.loop !11

33:                                               ; preds = %24
  store i32 0, ptr %3, align 2, !tbaa !6
  br label %34

34:                                               ; preds = %158, %33
  %35 = load i32, ptr %3, align 2, !tbaa !6
  %36 = icmp ult i32 %35, 15
  br i1 %36, label %37, label %161

37:                                               ; preds = %34
  call void @llvm.lifetime.start.p0(ptr %6) #4
  %38 = load i32, ptr %2, align 2, !tbaa !6
  %39 = getelementptr inbounds nuw [15 x i32], ptr @v, i32 0, i32 %38
  %40 = load i32, ptr %39, align 2, !tbaa !7
  store i32 %40, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #4
  %41 = load i32, ptr %3, align 2, !tbaa !6
  %42 = getelementptr inbounds nuw [15 x i32], ptr @v, i32 0, i32 %41
  %43 = load i32, ptr %42, align 2, !tbaa !7
  store i32 %43, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #4
  %44 = load i32, ptr %6, align 2, !tbaa !7
  store i32 %44, ptr %8, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %9) #4
  %45 = load i32, ptr %7, align 2, !tbaa !7
  store i32 %45, ptr %9, align 2, !tbaa !7
  %46 = load i32, ptr %6, align 2, !tbaa !7
  %47 = load i32, ptr %7, align 2, !tbaa !7
  %48 = call i32 @slt(i32 noundef %46, i32 noundef %47) #5
  %49 = load i32, ptr %6, align 2, !tbaa !7
  %50 = load i32, ptr %7, align 2, !tbaa !7
  %51 = call i32 @slt_if(i32 noundef %49, i32 noundef %50) #5
  %52 = shl i32 %51, 1
  %53 = or i32 %48, %52
  %54 = load i32, ptr %3, align 2, !tbaa !6
  %55 = mul i32 2, %54
  %56 = shl i32 %53, %55
  %57 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 0
  %58 = load i32, ptr %57, align 2, !tbaa !7
  %59 = or i32 %58, %56
  store i32 %59, ptr %57, align 2, !tbaa !7
  %60 = load i32, ptr %6, align 2, !tbaa !7
  %61 = load i32, ptr %7, align 2, !tbaa !7
  %62 = call i32 @sge(i32 noundef %60, i32 noundef %61) #5
  %63 = load i32, ptr %6, align 2, !tbaa !7
  %64 = load i32, ptr %7, align 2, !tbaa !7
  %65 = call i32 @sge_if(i32 noundef %63, i32 noundef %64) #5
  %66 = shl i32 %65, 1
  %67 = or i32 %62, %66
  %68 = load i32, ptr %3, align 2, !tbaa !6
  %69 = mul i32 2, %68
  %70 = shl i32 %67, %69
  %71 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 1
  %72 = load i32, ptr %71, align 2, !tbaa !7
  %73 = or i32 %72, %70
  store i32 %73, ptr %71, align 2, !tbaa !7
  %74 = load i32, ptr %6, align 2, !tbaa !7
  %75 = load i32, ptr %7, align 2, !tbaa !7
  %76 = call i32 @sle(i32 noundef %74, i32 noundef %75) #5
  %77 = load i32, ptr %6, align 2, !tbaa !7
  %78 = load i32, ptr %7, align 2, !tbaa !7
  %79 = call i32 @sle_if(i32 noundef %77, i32 noundef %78) #5
  %80 = shl i32 %79, 1
  %81 = or i32 %76, %80
  %82 = load i32, ptr %3, align 2, !tbaa !6
  %83 = mul i32 2, %82
  %84 = shl i32 %81, %83
  %85 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 2
  %86 = load i32, ptr %85, align 2, !tbaa !7
  %87 = or i32 %86, %84
  store i32 %87, ptr %85, align 2, !tbaa !7
  %88 = load i32, ptr %6, align 2, !tbaa !7
  %89 = load i32, ptr %7, align 2, !tbaa !7
  %90 = call i32 @sgt(i32 noundef %88, i32 noundef %89) #5
  %91 = load i32, ptr %6, align 2, !tbaa !7
  %92 = load i32, ptr %7, align 2, !tbaa !7
  %93 = call i32 @sgt_if(i32 noundef %91, i32 noundef %92) #5
  %94 = shl i32 %93, 1
  %95 = or i32 %90, %94
  %96 = load i32, ptr %3, align 2, !tbaa !6
  %97 = mul i32 2, %96
  %98 = shl i32 %95, %97
  %99 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 3
  %100 = load i32, ptr %99, align 2, !tbaa !7
  %101 = or i32 %100, %98
  store i32 %101, ptr %99, align 2, !tbaa !7
  %102 = load i32, ptr %8, align 2, !tbaa !7
  %103 = load i32, ptr %9, align 2, !tbaa !7
  %104 = call i32 @ult(i32 noundef %102, i32 noundef %103) #5
  %105 = load i32, ptr %8, align 2, !tbaa !7
  %106 = load i32, ptr %9, align 2, !tbaa !7
  %107 = call i32 @ult_if(i32 noundef %105, i32 noundef %106) #5
  %108 = shl i32 %107, 1
  %109 = or i32 %104, %108
  %110 = load i32, ptr %3, align 2, !tbaa !6
  %111 = mul i32 2, %110
  %112 = shl i32 %109, %111
  %113 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 4
  %114 = load i32, ptr %113, align 2, !tbaa !7
  %115 = or i32 %114, %112
  store i32 %115, ptr %113, align 2, !tbaa !7
  %116 = load i32, ptr %8, align 2, !tbaa !7
  %117 = load i32, ptr %9, align 2, !tbaa !7
  %118 = call i32 @uge(i32 noundef %116, i32 noundef %117) #5
  %119 = load i32, ptr %8, align 2, !tbaa !7
  %120 = load i32, ptr %9, align 2, !tbaa !7
  %121 = call i32 @uge_if(i32 noundef %119, i32 noundef %120) #5
  %122 = shl i32 %121, 1
  %123 = or i32 %118, %122
  %124 = load i32, ptr %3, align 2, !tbaa !6
  %125 = mul i32 2, %124
  %126 = shl i32 %123, %125
  %127 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 5
  %128 = load i32, ptr %127, align 2, !tbaa !7
  %129 = or i32 %128, %126
  store i32 %129, ptr %127, align 2, !tbaa !7
  %130 = load i32, ptr %8, align 2, !tbaa !7
  %131 = load i32, ptr %9, align 2, !tbaa !7
  %132 = call i32 @ule(i32 noundef %130, i32 noundef %131) #5
  %133 = load i32, ptr %8, align 2, !tbaa !7
  %134 = load i32, ptr %9, align 2, !tbaa !7
  %135 = call i32 @ule_if(i32 noundef %133, i32 noundef %134) #5
  %136 = shl i32 %135, 1
  %137 = or i32 %132, %136
  %138 = load i32, ptr %3, align 2, !tbaa !6
  %139 = mul i32 2, %138
  %140 = shl i32 %137, %139
  %141 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 6
  %142 = load i32, ptr %141, align 2, !tbaa !7
  %143 = or i32 %142, %140
  store i32 %143, ptr %141, align 2, !tbaa !7
  %144 = load i32, ptr %8, align 2, !tbaa !7
  %145 = load i32, ptr %9, align 2, !tbaa !7
  %146 = call i32 @ugt(i32 noundef %144, i32 noundef %145) #5
  %147 = load i32, ptr %8, align 2, !tbaa !7
  %148 = load i32, ptr %9, align 2, !tbaa !7
  %149 = call i32 @ugt_if(i32 noundef %147, i32 noundef %148) #5
  %150 = shl i32 %149, 1
  %151 = or i32 %146, %150
  %152 = load i32, ptr %3, align 2, !tbaa !6
  %153 = mul i32 2, %152
  %154 = shl i32 %151, %153
  %155 = getelementptr inbounds [8 x i32], ptr %4, i32 0, i32 7
  %156 = load i32, ptr %155, align 2, !tbaa !7
  %157 = or i32 %156, %154
  store i32 %157, ptr %155, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #4
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  br label %158

158:                                              ; preds = %37
  %159 = load i32, ptr %3, align 2, !tbaa !6
  %160 = add i32 %159, 1
  store i32 %160, ptr %3, align 2, !tbaa !6
  br label %34, !llvm.loop !12

161:                                              ; preds = %34
  %162 = load i32, ptr %2, align 2, !tbaa !6
  %163 = getelementptr inbounds nuw [15 x i32], ptr @v, i32 0, i32 %162
  %164 = load i32, ptr %163, align 2, !tbaa !7
  %165 = call i32 @slt_const(i32 noundef %164) #5
  %166 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 0
  store i32 %165, ptr %166, align 2, !tbaa !7
  %167 = load i32, ptr %2, align 2, !tbaa !6
  %168 = getelementptr inbounds nuw [15 x i32], ptr @v, i32 0, i32 %167
  %169 = load i32, ptr %168, align 2, !tbaa !7
  %170 = call i32 @sge_const(i32 noundef %169) #5
  %171 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 1
  store i32 %170, ptr %171, align 2, !tbaa !7
  %172 = load i32, ptr %2, align 2, !tbaa !6
  %173 = getelementptr inbounds nuw [15 x i32], ptr @v, i32 0, i32 %172
  %174 = load i32, ptr %173, align 2, !tbaa !7
  %175 = call i32 @ult_const(i32 noundef %174) #5
  %176 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 2
  store i32 %175, ptr %176, align 2, !tbaa !7
  %177 = load i32, ptr %2, align 2, !tbaa !6
  %178 = getelementptr inbounds nuw [15 x i32], ptr @v, i32 0, i32 %177
  %179 = load i32, ptr %178, align 2, !tbaa !7
  %180 = call i32 @uge_const(i32 noundef %179) #5
  %181 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 3
  store i32 %180, ptr %181, align 2, !tbaa !7
  store i32 0, ptr %3, align 2, !tbaa !6
  br label %182

182:                                              ; preds = %189, %161
  %183 = load i32, ptr %3, align 2, !tbaa !6
  %184 = icmp ult i32 %183, 8
  br i1 %184, label %185, label %192

185:                                              ; preds = %182
  %186 = load i32, ptr %3, align 2, !tbaa !6
  %187 = getelementptr inbounds nuw [8 x i32], ptr %4, i32 0, i32 %186
  %188 = load i32, ptr %187, align 2, !tbaa !7
  call void @report(i32 noundef %188) #5
  br label %189

189:                                              ; preds = %185
  %190 = load i32, ptr %3, align 2, !tbaa !6
  %191 = add i32 %190, 1
  store i32 %191, ptr %3, align 2, !tbaa !6
  br label %182, !llvm.loop !13

192:                                              ; preds = %182
  %193 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 0
  %194 = load i32, ptr %193, align 2, !tbaa !7
  %195 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 1
  %196 = load i32, ptr %195, align 2, !tbaa !7
  %197 = shl i32 %196, 1
  %198 = or i32 %194, %197
  %199 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 2
  %200 = load i32, ptr %199, align 2, !tbaa !7
  %201 = shl i32 %200, 2
  %202 = or i32 %198, %201
  %203 = getelementptr inbounds [4 x i32], ptr %5, i32 0, i32 3
  %204 = load i32, ptr %203, align 2, !tbaa !7
  %205 = shl i32 %204, 3
  %206 = or i32 %202, %205
  call void @report(i32 noundef %206) #5
  br label %207

207:                                              ; preds = %192
  %208 = load i32, ptr %2, align 2, !tbaa !6
  %209 = add i32 %208, 1
  store i32 %209, ptr %2, align 2, !tbaa !6
  br label %10, !llvm.loop !14

210:                                              ; preds = %10
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind optsize
define internal i32 @slt(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp slt i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @slt_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp slt i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: noinline nounwind optsize
define internal i32 @sge(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp sge i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @sge_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp sge i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: noinline nounwind optsize
define internal i32 @sle(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp sle i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @sle_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp sle i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: noinline nounwind optsize
define internal i32 @sgt(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp sgt i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @sgt_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp sgt i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: noinline nounwind optsize
define internal i32 @ult(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp ult i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @ult_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp ult i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: noinline nounwind optsize
define internal i32 @uge(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp uge i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @uge_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp uge i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: noinline nounwind optsize
define internal i32 @ule(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp ule i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @ule_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp ule i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: noinline nounwind optsize
define internal i32 @ugt(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp ugt i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind optsize
define internal i32 @ugt_if(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !7
  %8 = icmp ugt i32 %6, %7
  br i1 %8, label %9, label %10

9:                                                ; preds = %2
  store i32 1, ptr %3, align 2
  br label %11

10:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %11

11:                                               ; preds = %10, %9
  %12 = load i32, ptr %3, align 2
  ret i32 %12
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind optsize
define internal i32 @slt_const(i32 noundef %0) #2 {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  %3 = load i32, ptr %2, align 2, !tbaa !7
  %4 = icmp slt i32 %3, 70000
  %5 = zext i1 %4 to i32
  ret i32 %5
}

; Function Attrs: noinline nounwind optsize
define internal i32 @sge_const(i32 noundef %0) #2 {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  %3 = load i32, ptr %2, align 2, !tbaa !7
  %4 = icmp sge i32 %3, -70000
  %5 = zext i1 %4 to i32
  ret i32 %5
}

; Function Attrs: noinline nounwind optsize
define internal i32 @ult_const(i32 noundef %0) #2 {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  %3 = load i32, ptr %2, align 2, !tbaa !7
  %4 = icmp ult i32 %3, -2147418112
  %5 = zext i1 %4 to i32
  ret i32 %5
}

; Function Attrs: noinline nounwind optsize
define internal i32 @uge_const(i32 noundef %0) #2 {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  %3 = load i32, ptr %2, align 2, !tbaa !7
  %4 = icmp uge i32 %3, 65536
  %5 = zext i1 %4 to i32
  ret i32 %5
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

attributes #0 = { nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { noinline nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #3 = { optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin optsize "no-builtins" }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !4, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = distinct !{!12, !10}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
