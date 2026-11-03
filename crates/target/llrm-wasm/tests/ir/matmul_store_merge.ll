; ModuleID = 'bench/matmul/matmul.c'
source_filename = "bench/matmul/matmul.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nofree norecurse nosync nounwind memory(none)
define hidden i32 @bench_matmul(i16 noundef zeroext %0) local_unnamed_addr #0 {
  %2 = alloca [8 x [8 x i16]], align 16
  %3 = alloca [8 x [8 x i16]], align 16
  %4 = alloca [8 x [8 x i32]], align 16
  call void @llvm.lifetime.start.p0(ptr nonnull %2) #4
  call void @llvm.lifetime.start.p0(ptr nonnull %3) #4
  call void @llvm.lifetime.start.p0(ptr nonnull %4) #4
  %5 = add i16 %0, 1
  store i16 %5, ptr %2, align 16, !tbaa !6
  store i16 2, ptr %3, align 16, !tbaa !6
  %6 = add i16 %0, 2
  %7 = getelementptr inbounds nuw i8, ptr %2, i32 2
  store i16 %6, ptr %7, align 2, !tbaa !6
  %8 = getelementptr inbounds nuw i8, ptr %3, i32 2
  store i16 1, ptr %8, align 2, !tbaa !6
  %9 = add i16 %0, 3
  %10 = getelementptr inbounds nuw i8, ptr %2, i32 4
  store i16 %9, ptr %10, align 4, !tbaa !6
  %11 = getelementptr inbounds nuw i8, ptr %3, i32 4
  store i16 2, ptr %11, align 4, !tbaa !6
  %12 = add i16 %0, 4
  %13 = getelementptr inbounds nuw i8, ptr %2, i32 6
  store i16 %12, ptr %13, align 2, !tbaa !6
  %14 = getelementptr inbounds nuw i8, ptr %3, i32 6
  store i16 0, ptr %14, align 2, !tbaa !6
  %15 = add i16 %0, 5
  %16 = getelementptr inbounds nuw i8, ptr %2, i32 8
  store i16 %15, ptr %16, align 8, !tbaa !6
  %17 = getelementptr inbounds nuw i8, ptr %3, i32 8
  store i16 1, ptr %17, align 8, !tbaa !6
  %18 = add i16 %0, 6
  %19 = getelementptr inbounds nuw i8, ptr %2, i32 10
  store i16 %18, ptr %19, align 2, !tbaa !6
  %20 = getelementptr inbounds nuw i8, ptr %3, i32 10
  store i16 2, ptr %20, align 2, !tbaa !6
  %21 = add i16 %0, 7
  %22 = getelementptr inbounds nuw i8, ptr %2, i32 12
  store i16 %21, ptr %22, align 4, !tbaa !6
  %23 = getelementptr inbounds nuw i8, ptr %3, i32 12
  store i16 0, ptr %23, align 4, !tbaa !6
  %24 = add i16 %0, 8
  %25 = getelementptr inbounds nuw i8, ptr %2, i32 14
  store i16 %24, ptr %25, align 2, !tbaa !6
  %26 = getelementptr inbounds nuw i8, ptr %3, i32 14
  store i16 1, ptr %26, align 2, !tbaa !6
  %27 = getelementptr inbounds nuw i8, ptr %2, i32 16
  %28 = getelementptr inbounds nuw i8, ptr %3, i32 16
  store i16 %12, ptr %27, align 16, !tbaa !6
  store i16 1, ptr %28, align 16, !tbaa !6
  %29 = getelementptr inbounds nuw i8, ptr %2, i32 18
  store i16 %15, ptr %29, align 2, !tbaa !6
  %30 = getelementptr inbounds nuw i8, ptr %3, i32 18
  store i16 2, ptr %30, align 2, !tbaa !6
  %31 = getelementptr inbounds nuw i8, ptr %2, i32 20
  store i16 %18, ptr %31, align 4, !tbaa !6
  %32 = getelementptr inbounds nuw i8, ptr %3, i32 20
  store i16 0, ptr %32, align 4, !tbaa !6
  %33 = getelementptr inbounds nuw i8, ptr %2, i32 22
  store i16 %21, ptr %33, align 2, !tbaa !6
  %34 = getelementptr inbounds nuw i8, ptr %3, i32 22
  store i16 1, ptr %34, align 2, !tbaa !6
  %35 = getelementptr inbounds nuw i8, ptr %2, i32 24
  store i16 %24, ptr %35, align 8, !tbaa !6
  %36 = getelementptr inbounds nuw i8, ptr %3, i32 24
  store i16 2, ptr %36, align 8, !tbaa !6
  %37 = add i16 %0, 9
  %38 = getelementptr inbounds nuw i8, ptr %2, i32 26
  store i16 %37, ptr %38, align 2, !tbaa !6
  %39 = getelementptr inbounds nuw i8, ptr %3, i32 26
  store i16 0, ptr %39, align 2, !tbaa !6
  %40 = add i16 %0, 10
  %41 = getelementptr inbounds nuw i8, ptr %2, i32 28
  store i16 %40, ptr %41, align 4, !tbaa !6
  %42 = getelementptr inbounds nuw i8, ptr %3, i32 28
  store i16 1, ptr %42, align 4, !tbaa !6
  %43 = add i16 %0, 11
  %44 = getelementptr inbounds nuw i8, ptr %2, i32 30
  store i16 %43, ptr %44, align 2, !tbaa !6
  %45 = getelementptr inbounds nuw i8, ptr %3, i32 30
  store i16 2, ptr %45, align 2, !tbaa !6
  %46 = getelementptr inbounds nuw i8, ptr %2, i32 32
  %47 = getelementptr inbounds nuw i8, ptr %3, i32 32
  store i16 %21, ptr %46, align 16, !tbaa !6
  store i16 2, ptr %47, align 16, !tbaa !6
  %48 = getelementptr inbounds nuw i8, ptr %2, i32 34
  store i16 %24, ptr %48, align 2, !tbaa !6
  %49 = getelementptr inbounds nuw i8, ptr %3, i32 34
  store i16 0, ptr %49, align 2, !tbaa !6
  %50 = getelementptr inbounds nuw i8, ptr %2, i32 36
  store i16 %37, ptr %50, align 4, !tbaa !6
  %51 = getelementptr inbounds nuw i8, ptr %3, i32 36
  store i16 2, ptr %51, align 4, !tbaa !6
  %52 = getelementptr inbounds nuw i8, ptr %2, i32 38
  store i16 %40, ptr %52, align 2, !tbaa !6
  %53 = getelementptr inbounds nuw i8, ptr %3, i32 38
  store i16 2, ptr %53, align 2, !tbaa !6
  %54 = getelementptr inbounds nuw i8, ptr %2, i32 40
  store i16 %43, ptr %54, align 8, !tbaa !6
  %55 = getelementptr inbounds nuw i8, ptr %3, i32 40
  store i16 0, ptr %55, align 8, !tbaa !6
  %56 = add i16 %0, 12
  %57 = getelementptr inbounds nuw i8, ptr %2, i32 42
  store i16 %56, ptr %57, align 2, !tbaa !6
  %58 = getelementptr inbounds nuw i8, ptr %3, i32 42
  store i16 1, ptr %58, align 2, !tbaa !6
  %59 = add i16 %0, 13
  %60 = getelementptr inbounds nuw i8, ptr %2, i32 44
  store i16 %59, ptr %60, align 4, !tbaa !6
  %61 = getelementptr inbounds nuw i8, ptr %3, i32 44
  store i16 2, ptr %61, align 4, !tbaa !6
  %62 = add i16 %0, 14
  %63 = getelementptr inbounds nuw i8, ptr %2, i32 46
  store i16 %62, ptr %63, align 2, !tbaa !6
  %64 = getelementptr inbounds nuw i8, ptr %3, i32 46
  store i16 0, ptr %64, align 2, !tbaa !6
  %65 = getelementptr inbounds nuw i8, ptr %2, i32 48
  %66 = getelementptr inbounds nuw i8, ptr %3, i32 48
  store i16 %40, ptr %65, align 16, !tbaa !6
  store i16 0, ptr %66, align 16, !tbaa !6
  %67 = getelementptr inbounds nuw i8, ptr %2, i32 50
  store i16 %43, ptr %67, align 2, !tbaa !6
  %68 = getelementptr inbounds nuw i8, ptr %3, i32 50
  store i16 1, ptr %68, align 2, !tbaa !6
  %69 = getelementptr inbounds nuw i8, ptr %2, i32 52
  store i16 %56, ptr %69, align 4, !tbaa !6
  %70 = getelementptr inbounds nuw i8, ptr %3, i32 52
  store i16 2, ptr %70, align 4, !tbaa !6
  %71 = getelementptr inbounds nuw i8, ptr %2, i32 54
  store i16 %59, ptr %71, align 2, !tbaa !6
  %72 = getelementptr inbounds nuw i8, ptr %3, i32 54
  store i16 2, ptr %72, align 2, !tbaa !6
  %73 = getelementptr inbounds nuw i8, ptr %2, i32 56
  store i16 %62, ptr %73, align 8, !tbaa !6
  %74 = getelementptr inbounds nuw i8, ptr %3, i32 56
  store i16 1, ptr %74, align 8, !tbaa !6
  %75 = add i16 %0, 15
  %76 = getelementptr inbounds nuw i8, ptr %2, i32 58
  store i16 %75, ptr %76, align 2, !tbaa !6
  %77 = getelementptr inbounds nuw i8, ptr %3, i32 58
  store i16 2, ptr %77, align 2, !tbaa !6
  %78 = add i16 %0, 16
  %79 = getelementptr inbounds nuw i8, ptr %2, i32 60
  store i16 %78, ptr %79, align 4, !tbaa !6
  %80 = getelementptr inbounds nuw i8, ptr %3, i32 60
  store i16 0, ptr %80, align 4, !tbaa !6
  %81 = add i16 %0, 17
  %82 = getelementptr inbounds nuw i8, ptr %2, i32 62
  store i16 %81, ptr %82, align 2, !tbaa !6
  %83 = getelementptr inbounds nuw i8, ptr %3, i32 62
  store i16 1, ptr %83, align 2, !tbaa !6
  %84 = getelementptr inbounds nuw i8, ptr %2, i32 64
  %85 = getelementptr inbounds nuw i8, ptr %3, i32 64
  store i16 %59, ptr %84, align 16, !tbaa !6
  store i16 1, ptr %85, align 16, !tbaa !6
  %86 = getelementptr inbounds nuw i8, ptr %2, i32 66
  store i16 %62, ptr %86, align 2, !tbaa !6
  %87 = getelementptr inbounds nuw i8, ptr %3, i32 66
  store i16 2, ptr %87, align 2, !tbaa !6
  %88 = getelementptr inbounds nuw i8, ptr %2, i32 68
  store i16 %75, ptr %88, align 4, !tbaa !6
  %89 = getelementptr inbounds nuw i8, ptr %3, i32 68
  store i16 0, ptr %89, align 4, !tbaa !6
  %90 = getelementptr inbounds nuw i8, ptr %2, i32 70
  store i16 %78, ptr %90, align 2, !tbaa !6
  %91 = getelementptr inbounds nuw i8, ptr %3, i32 70
  store i16 1, ptr %91, align 2, !tbaa !6
  %92 = getelementptr inbounds nuw i8, ptr %2, i32 72
  store i16 %81, ptr %92, align 8, !tbaa !6
  %93 = getelementptr inbounds nuw i8, ptr %3, i32 72
  store i16 2, ptr %93, align 8, !tbaa !6
  %94 = add i16 %0, 18
  %95 = getelementptr inbounds nuw i8, ptr %2, i32 74
  store i16 %94, ptr %95, align 2, !tbaa !6
  %96 = getelementptr inbounds nuw i8, ptr %3, i32 74
  store i16 0, ptr %96, align 2, !tbaa !6
  %97 = add i16 %0, 19
  %98 = getelementptr inbounds nuw i8, ptr %2, i32 76
  store i16 %97, ptr %98, align 4, !tbaa !6
  %99 = getelementptr inbounds nuw i8, ptr %3, i32 76
  store i16 1, ptr %99, align 4, !tbaa !6
  %100 = add i16 %0, 20
  %101 = getelementptr inbounds nuw i8, ptr %2, i32 78
  store i16 %100, ptr %101, align 2, !tbaa !6
  %102 = getelementptr inbounds nuw i8, ptr %3, i32 78
  store i16 2, ptr %102, align 2, !tbaa !6
  %103 = getelementptr inbounds nuw i8, ptr %2, i32 80
  %104 = getelementptr inbounds nuw i8, ptr %3, i32 80
  store i16 %78, ptr %103, align 16, !tbaa !6
  store i16 2, ptr %104, align 16, !tbaa !6
  %105 = getelementptr inbounds nuw i8, ptr %2, i32 82
  store i16 %81, ptr %105, align 2, !tbaa !6
  %106 = getelementptr inbounds nuw i8, ptr %3, i32 82
  store i16 0, ptr %106, align 2, !tbaa !6
  %107 = getelementptr inbounds nuw i8, ptr %2, i32 84
  store i16 %94, ptr %107, align 4, !tbaa !6
  %108 = getelementptr inbounds nuw i8, ptr %3, i32 84
  store i16 1, ptr %108, align 4, !tbaa !6
  %109 = getelementptr inbounds nuw i8, ptr %2, i32 86
  store i16 %97, ptr %109, align 2, !tbaa !6
  %110 = getelementptr inbounds nuw i8, ptr %3, i32 86
  store i16 2, ptr %110, align 2, !tbaa !6
  %111 = getelementptr inbounds nuw i8, ptr %2, i32 88
  store i16 %100, ptr %111, align 8, !tbaa !6
  %112 = getelementptr inbounds nuw i8, ptr %3, i32 88
  store i16 0, ptr %112, align 8, !tbaa !6
  %113 = add i16 %0, 21
  %114 = getelementptr inbounds nuw i8, ptr %2, i32 90
  store i16 %113, ptr %114, align 2, !tbaa !6
  %115 = getelementptr inbounds nuw i8, ptr %3, i32 90
  store i16 2, ptr %115, align 2, !tbaa !6
  %116 = add i16 %0, 22
  %117 = getelementptr inbounds nuw i8, ptr %2, i32 92
  store i16 %116, ptr %117, align 4, !tbaa !6
  %118 = getelementptr inbounds nuw i8, ptr %3, i32 92
  store i16 2, ptr %118, align 4, !tbaa !6
  %119 = add i16 %0, 23
  %120 = getelementptr inbounds nuw i8, ptr %2, i32 94
  store i16 %119, ptr %120, align 2, !tbaa !6
  %121 = getelementptr inbounds nuw i8, ptr %3, i32 94
  store i16 0, ptr %121, align 2, !tbaa !6
  %122 = getelementptr inbounds nuw i8, ptr %2, i32 96
  %123 = getelementptr inbounds nuw i8, ptr %3, i32 96
  store i16 %97, ptr %122, align 16, !tbaa !6
  store i16 0, ptr %123, align 16, !tbaa !6
  %124 = getelementptr inbounds nuw i8, ptr %2, i32 98
  store i16 %100, ptr %124, align 2, !tbaa !6
  %125 = getelementptr inbounds nuw i8, ptr %3, i32 98
  store i16 1, ptr %125, align 2, !tbaa !6
  %126 = getelementptr inbounds nuw i8, ptr %2, i32 100
  store i16 %113, ptr %126, align 4, !tbaa !6
  %127 = getelementptr inbounds nuw i8, ptr %3, i32 100
  store i16 2, ptr %127, align 4, !tbaa !6
  %128 = getelementptr inbounds nuw i8, ptr %2, i32 102
  store i16 %116, ptr %128, align 2, !tbaa !6
  %129 = getelementptr inbounds nuw i8, ptr %3, i32 102
  store i16 0, ptr %129, align 2, !tbaa !6
  %130 = getelementptr inbounds nuw i8, ptr %2, i32 104
  store i16 %119, ptr %130, align 8, !tbaa !6
  %131 = getelementptr inbounds nuw i8, ptr %3, i32 104
  store i16 1, ptr %131, align 8, !tbaa !6
  %132 = add i16 %0, 24
  %133 = getelementptr inbounds nuw i8, ptr %2, i32 106
  store i16 %132, ptr %133, align 2, !tbaa !6
  %134 = getelementptr inbounds nuw i8, ptr %3, i32 106
  store i16 2, ptr %134, align 2, !tbaa !6
  %135 = add i16 %0, 25
  %136 = getelementptr inbounds nuw i8, ptr %2, i32 108
  store i16 %135, ptr %136, align 4, !tbaa !6
  %137 = getelementptr inbounds nuw i8, ptr %3, i32 108
  store i16 2, ptr %137, align 4, !tbaa !6
  %138 = add i16 %0, 26
  %139 = getelementptr inbounds nuw i8, ptr %2, i32 110
  store i16 %138, ptr %139, align 2, !tbaa !6
  %140 = getelementptr inbounds nuw i8, ptr %3, i32 110
  store i16 1, ptr %140, align 2, !tbaa !6
  %141 = getelementptr inbounds nuw i8, ptr %2, i32 112
  %142 = getelementptr inbounds nuw i8, ptr %3, i32 112
  store i16 %116, ptr %141, align 16, !tbaa !6
  store i16 1, ptr %142, align 16, !tbaa !6
  %143 = getelementptr inbounds nuw i8, ptr %2, i32 114
  store i16 %119, ptr %143, align 2, !tbaa !6
  %144 = getelementptr inbounds nuw i8, ptr %3, i32 114
  store i16 2, ptr %144, align 2, !tbaa !6
  %145 = getelementptr inbounds nuw i8, ptr %2, i32 116
  store i16 %132, ptr %145, align 4, !tbaa !6
  %146 = getelementptr inbounds nuw i8, ptr %3, i32 116
  store i16 0, ptr %146, align 4, !tbaa !6
  %147 = getelementptr inbounds nuw i8, ptr %2, i32 118
  store i16 %135, ptr %147, align 2, !tbaa !6
  %148 = getelementptr inbounds nuw i8, ptr %3, i32 118
  store i16 1, ptr %148, align 2, !tbaa !6
  %149 = getelementptr inbounds nuw i8, ptr %2, i32 120
  store i16 %138, ptr %149, align 8, !tbaa !6
  %150 = getelementptr inbounds nuw i8, ptr %3, i32 120
  store i16 2, ptr %150, align 8, !tbaa !6
  %151 = add i16 %0, 27
  %152 = getelementptr inbounds nuw i8, ptr %2, i32 122
  store i16 %151, ptr %152, align 2, !tbaa !6
  %153 = getelementptr inbounds nuw i8, ptr %3, i32 122
  store i16 0, ptr %153, align 2, !tbaa !6
  %154 = add i16 %0, 28
  %155 = getelementptr inbounds nuw i8, ptr %2, i32 124
  store i16 %154, ptr %155, align 4, !tbaa !6
  %156 = getelementptr inbounds nuw i8, ptr %3, i32 124
  store i16 1, ptr %156, align 4, !tbaa !6
  %157 = add i16 %0, 29
  %158 = getelementptr inbounds nuw i8, ptr %2, i32 126
  store i16 %157, ptr %158, align 2, !tbaa !6
  %159 = getelementptr inbounds nuw i8, ptr %3, i32 126
  store i16 2, ptr %159, align 2, !tbaa !6
  br label %160

160:                                              ; preds = %1, %231
  %161 = phi i32 [ 0, %1 ], [ %232, %231 ]
  %162 = getelementptr inbounds nuw [16 x i8], ptr %2, i32 %161
  %163 = getelementptr inbounds nuw [32 x i8], ptr %4, i32 %161
  %164 = load i16, ptr %162, align 16, !tbaa !6
  %165 = getelementptr inbounds nuw i8, ptr %162, i32 2
  %166 = load i16, ptr %165, align 2, !tbaa !6
  %167 = getelementptr inbounds nuw i8, ptr %162, i32 4
  %168 = load i16, ptr %167, align 4, !tbaa !6
  %169 = getelementptr inbounds nuw i8, ptr %162, i32 6
  %170 = load i16, ptr %169, align 2, !tbaa !6
  %171 = getelementptr inbounds nuw i8, ptr %162, i32 8
  %172 = load i16, ptr %171, align 8, !tbaa !6
  %173 = getelementptr inbounds nuw i8, ptr %162, i32 10
  %174 = load i16, ptr %173, align 2, !tbaa !6
  %175 = getelementptr inbounds nuw i8, ptr %162, i32 12
  %176 = load i16, ptr %175, align 4, !tbaa !6
  %177 = getelementptr inbounds nuw i8, ptr %162, i32 14
  %178 = load i16, ptr %177, align 2, !tbaa !6
  %179 = sext i16 %164 to i32
  %180 = sext i16 %166 to i32
  %181 = sext i16 %168 to i32
  %182 = sext i16 %170 to i32
  %183 = sext i16 %172 to i32
  %184 = sext i16 %174 to i32
  %185 = sext i16 %176 to i32
  %186 = sext i16 %178 to i32
  br label %187

187:                                              ; preds = %160, %187
  %188 = phi i32 [ 0, %160 ], [ %229, %187 ]
  %189 = getelementptr inbounds nuw [2 x i8], ptr %3, i32 %188
  %190 = load i16, ptr %189, align 2, !tbaa !6
  %191 = sext i16 %190 to i32
  %192 = mul nsw i32 %191, %179
  %193 = getelementptr inbounds nuw i8, ptr %189, i32 16
  %194 = load i16, ptr %193, align 2, !tbaa !6
  %195 = sext i16 %194 to i32
  %196 = mul nsw i32 %195, %180
  %197 = add nsw i32 %196, %192
  %198 = getelementptr inbounds nuw i8, ptr %189, i32 32
  %199 = load i16, ptr %198, align 2, !tbaa !6
  %200 = sext i16 %199 to i32
  %201 = mul nsw i32 %200, %181
  %202 = add nsw i32 %201, %197
  %203 = getelementptr inbounds nuw i8, ptr %189, i32 48
  %204 = load i16, ptr %203, align 2, !tbaa !6
  %205 = sext i16 %204 to i32
  %206 = mul nsw i32 %205, %182
  %207 = add nsw i32 %206, %202
  %208 = getelementptr inbounds nuw i8, ptr %189, i32 64
  %209 = load i16, ptr %208, align 2, !tbaa !6
  %210 = sext i16 %209 to i32
  %211 = mul nsw i32 %210, %183
  %212 = add nsw i32 %211, %207
  %213 = getelementptr inbounds nuw i8, ptr %189, i32 80
  %214 = load i16, ptr %213, align 2, !tbaa !6
  %215 = sext i16 %214 to i32
  %216 = mul nsw i32 %215, %184
  %217 = add nsw i32 %216, %212
  %218 = getelementptr inbounds nuw i8, ptr %189, i32 96
  %219 = load i16, ptr %218, align 2, !tbaa !6
  %220 = sext i16 %219 to i32
  %221 = mul nsw i32 %220, %185
  %222 = add nsw i32 %221, %217
  %223 = getelementptr inbounds nuw i8, ptr %189, i32 112
  %224 = load i16, ptr %223, align 2, !tbaa !6
  %225 = sext i16 %224 to i32
  %226 = mul nsw i32 %225, %186
  %227 = add nsw i32 %226, %222
  %228 = getelementptr inbounds nuw [4 x i8], ptr %163, i32 %188
  store i32 %227, ptr %228, align 4, !tbaa !8
  %229 = add nuw nsw i32 %188, 1
  %230 = icmp eq i32 %229, 8
  br i1 %230, label %231, label %187, !llvm.loop !10

231:                                              ; preds = %187
  %232 = add nuw nsw i32 %161, 1
  %233 = icmp eq i32 %232, 8
  br i1 %233, label %234, label %160, !llvm.loop !12

234:                                              ; preds = %231
  %235 = load i32, ptr %4, align 16, !tbaa !8
  %236 = getelementptr inbounds nuw i8, ptr %4, i32 4
  %237 = load i32, ptr %236, align 4, !tbaa !8
  %238 = shl i32 %237, 1
  %239 = add i32 %238, %235
  %240 = getelementptr inbounds nuw i8, ptr %4, i32 8
  %241 = load i32, ptr %240, align 8, !tbaa !8
  %242 = mul i32 %241, 3
  %243 = add i32 %242, %239
  %244 = getelementptr inbounds nuw i8, ptr %4, i32 12
  %245 = load i32, ptr %244, align 4, !tbaa !8
  %246 = shl i32 %245, 2
  %247 = add i32 %246, %243
  %248 = getelementptr inbounds nuw i8, ptr %4, i32 16
  %249 = load i32, ptr %248, align 16, !tbaa !8
  %250 = mul i32 %249, 5
  %251 = add i32 %250, %247
  %252 = getelementptr inbounds nuw i8, ptr %4, i32 20
  %253 = load i32, ptr %252, align 4, !tbaa !8
  %254 = mul i32 %253, 6
  %255 = add i32 %254, %251
  %256 = getelementptr inbounds nuw i8, ptr %4, i32 24
  %257 = load i32, ptr %256, align 8, !tbaa !8
  %258 = mul i32 %257, 7
  %259 = add i32 %258, %255
  %260 = getelementptr inbounds nuw i8, ptr %4, i32 28
  %261 = load i32, ptr %260, align 4, !tbaa !8
  %262 = shl i32 %261, 3
  %263 = add i32 %262, %259
  %264 = getelementptr inbounds nuw i8, ptr %4, i32 32
  %265 = load i32, ptr %264, align 16, !tbaa !8
  %266 = mul i32 %265, 9
  %267 = add i32 %266, %263
  %268 = getelementptr inbounds nuw i8, ptr %4, i32 36
  %269 = load i32, ptr %268, align 4, !tbaa !8
  %270 = mul i32 %269, 10
  %271 = add i32 %270, %267
  %272 = getelementptr inbounds nuw i8, ptr %4, i32 40
  %273 = load i32, ptr %272, align 8, !tbaa !8
  %274 = mul i32 %273, 11
  %275 = add i32 %274, %271
  %276 = getelementptr inbounds nuw i8, ptr %4, i32 44
  %277 = load i32, ptr %276, align 4, !tbaa !8
  %278 = mul i32 %277, 12
  %279 = add i32 %278, %275
  %280 = getelementptr inbounds nuw i8, ptr %4, i32 48
  %281 = load i32, ptr %280, align 16, !tbaa !8
  %282 = mul i32 %281, 13
  %283 = add i32 %282, %279
  %284 = getelementptr inbounds nuw i8, ptr %4, i32 52
  %285 = load i32, ptr %284, align 4, !tbaa !8
  %286 = mul i32 %285, 14
  %287 = add i32 %286, %283
  %288 = getelementptr inbounds nuw i8, ptr %4, i32 56
  %289 = load i32, ptr %288, align 8, !tbaa !8
  %290 = mul i32 %289, 15
  %291 = add i32 %290, %287
  %292 = getelementptr inbounds nuw i8, ptr %4, i32 60
  %293 = load i32, ptr %292, align 4, !tbaa !8
  %294 = shl i32 %293, 4
  %295 = add i32 %294, %291
  %296 = getelementptr inbounds nuw i8, ptr %4, i32 64
  %297 = load i32, ptr %296, align 16, !tbaa !8
  %298 = mul i32 %297, 17
  %299 = add i32 %298, %295
  %300 = getelementptr inbounds nuw i8, ptr %4, i32 68
  %301 = load i32, ptr %300, align 4, !tbaa !8
  %302 = mul i32 %301, 18
  %303 = add i32 %302, %299
  %304 = getelementptr inbounds nuw i8, ptr %4, i32 72
  %305 = load i32, ptr %304, align 8, !tbaa !8
  %306 = mul i32 %305, 19
  %307 = add i32 %306, %303
  %308 = getelementptr inbounds nuw i8, ptr %4, i32 76
  %309 = load i32, ptr %308, align 4, !tbaa !8
  %310 = mul i32 %309, 20
  %311 = add i32 %310, %307
  %312 = getelementptr inbounds nuw i8, ptr %4, i32 80
  %313 = load i32, ptr %312, align 16, !tbaa !8
  %314 = mul i32 %313, 21
  %315 = add i32 %314, %311
  %316 = getelementptr inbounds nuw i8, ptr %4, i32 84
  %317 = load i32, ptr %316, align 4, !tbaa !8
  %318 = mul i32 %317, 22
  %319 = add i32 %318, %315
  %320 = getelementptr inbounds nuw i8, ptr %4, i32 88
  %321 = load i32, ptr %320, align 8, !tbaa !8
  %322 = mul i32 %321, 23
  %323 = add i32 %322, %319
  %324 = getelementptr inbounds nuw i8, ptr %4, i32 92
  %325 = load i32, ptr %324, align 4, !tbaa !8
  %326 = mul i32 %325, 24
  %327 = add i32 %326, %323
  %328 = getelementptr inbounds nuw i8, ptr %4, i32 96
  %329 = load i32, ptr %328, align 16, !tbaa !8
  %330 = mul i32 %329, 25
  %331 = add i32 %330, %327
  %332 = getelementptr inbounds nuw i8, ptr %4, i32 100
  %333 = load i32, ptr %332, align 4, !tbaa !8
  %334 = mul i32 %333, 26
  %335 = add i32 %334, %331
  %336 = getelementptr inbounds nuw i8, ptr %4, i32 104
  %337 = load i32, ptr %336, align 8, !tbaa !8
  %338 = mul i32 %337, 27
  %339 = add i32 %338, %335
  %340 = getelementptr inbounds nuw i8, ptr %4, i32 108
  %341 = load i32, ptr %340, align 4, !tbaa !8
  %342 = mul i32 %341, 28
  %343 = add i32 %342, %339
  %344 = getelementptr inbounds nuw i8, ptr %4, i32 112
  %345 = load i32, ptr %344, align 16, !tbaa !8
  %346 = mul i32 %345, 29
  %347 = add i32 %346, %343
  %348 = getelementptr inbounds nuw i8, ptr %4, i32 116
  %349 = load i32, ptr %348, align 4, !tbaa !8
  %350 = mul i32 %349, 30
  %351 = add i32 %350, %347
  %352 = getelementptr inbounds nuw i8, ptr %4, i32 120
  %353 = load i32, ptr %352, align 8, !tbaa !8
  %354 = mul i32 %353, 31
  %355 = add i32 %354, %351
  %356 = getelementptr inbounds nuw i8, ptr %4, i32 124
  %357 = load i32, ptr %356, align 4, !tbaa !8
  %358 = shl i32 %357, 5
  %359 = add i32 %358, %355
  %360 = getelementptr inbounds nuw i8, ptr %4, i32 128
  %361 = load i32, ptr %360, align 16, !tbaa !8
  %362 = mul i32 %361, 33
  %363 = add i32 %362, %359
  %364 = getelementptr inbounds nuw i8, ptr %4, i32 132
  %365 = load i32, ptr %364, align 4, !tbaa !8
  %366 = mul i32 %365, 34
  %367 = add i32 %366, %363
  %368 = getelementptr inbounds nuw i8, ptr %4, i32 136
  %369 = load i32, ptr %368, align 8, !tbaa !8
  %370 = mul i32 %369, 35
  %371 = add i32 %370, %367
  %372 = getelementptr inbounds nuw i8, ptr %4, i32 140
  %373 = load i32, ptr %372, align 4, !tbaa !8
  %374 = mul i32 %373, 36
  %375 = add i32 %374, %371
  %376 = getelementptr inbounds nuw i8, ptr %4, i32 144
  %377 = load i32, ptr %376, align 16, !tbaa !8
  %378 = mul i32 %377, 37
  %379 = add i32 %378, %375
  %380 = getelementptr inbounds nuw i8, ptr %4, i32 148
  %381 = load i32, ptr %380, align 4, !tbaa !8
  %382 = mul i32 %381, 38
  %383 = add i32 %382, %379
  %384 = getelementptr inbounds nuw i8, ptr %4, i32 152
  %385 = load i32, ptr %384, align 8, !tbaa !8
  %386 = mul i32 %385, 39
  %387 = add i32 %386, %383
  %388 = getelementptr inbounds nuw i8, ptr %4, i32 156
  %389 = load i32, ptr %388, align 4, !tbaa !8
  %390 = mul i32 %389, 40
  %391 = add i32 %390, %387
  %392 = getelementptr inbounds nuw i8, ptr %4, i32 160
  %393 = load i32, ptr %392, align 16, !tbaa !8
  %394 = mul i32 %393, 41
  %395 = add i32 %394, %391
  %396 = getelementptr inbounds nuw i8, ptr %4, i32 164
  %397 = load i32, ptr %396, align 4, !tbaa !8
  %398 = mul i32 %397, 42
  %399 = add i32 %398, %395
  %400 = getelementptr inbounds nuw i8, ptr %4, i32 168
  %401 = load i32, ptr %400, align 8, !tbaa !8
  %402 = mul i32 %401, 43
  %403 = add i32 %402, %399
  %404 = getelementptr inbounds nuw i8, ptr %4, i32 172
  %405 = load i32, ptr %404, align 4, !tbaa !8
  %406 = mul i32 %405, 44
  %407 = add i32 %406, %403
  %408 = getelementptr inbounds nuw i8, ptr %4, i32 176
  %409 = load i32, ptr %408, align 16, !tbaa !8
  %410 = mul i32 %409, 45
  %411 = add i32 %410, %407
  %412 = getelementptr inbounds nuw i8, ptr %4, i32 180
  %413 = load i32, ptr %412, align 4, !tbaa !8
  %414 = mul i32 %413, 46
  %415 = add i32 %414, %411
  %416 = getelementptr inbounds nuw i8, ptr %4, i32 184
  %417 = load i32, ptr %416, align 8, !tbaa !8
  %418 = mul i32 %417, 47
  %419 = add i32 %418, %415
  %420 = getelementptr inbounds nuw i8, ptr %4, i32 188
  %421 = load i32, ptr %420, align 4, !tbaa !8
  %422 = mul i32 %421, 48
  %423 = add i32 %422, %419
  %424 = getelementptr inbounds nuw i8, ptr %4, i32 192
  %425 = load i32, ptr %424, align 16, !tbaa !8
  %426 = mul i32 %425, 49
  %427 = add i32 %426, %423
  %428 = getelementptr inbounds nuw i8, ptr %4, i32 196
  %429 = load i32, ptr %428, align 4, !tbaa !8
  %430 = mul i32 %429, 50
  %431 = add i32 %430, %427
  %432 = getelementptr inbounds nuw i8, ptr %4, i32 200
  %433 = load i32, ptr %432, align 8, !tbaa !8
  %434 = mul i32 %433, 51
  %435 = add i32 %434, %431
  %436 = getelementptr inbounds nuw i8, ptr %4, i32 204
  %437 = load i32, ptr %436, align 4, !tbaa !8
  %438 = mul i32 %437, 52
  %439 = add i32 %438, %435
  %440 = getelementptr inbounds nuw i8, ptr %4, i32 208
  %441 = load i32, ptr %440, align 16, !tbaa !8
  %442 = mul i32 %441, 53
  %443 = add i32 %442, %439
  %444 = getelementptr inbounds nuw i8, ptr %4, i32 212
  %445 = load i32, ptr %444, align 4, !tbaa !8
  %446 = mul i32 %445, 54
  %447 = add i32 %446, %443
  %448 = getelementptr inbounds nuw i8, ptr %4, i32 216
  %449 = load i32, ptr %448, align 8, !tbaa !8
  %450 = mul i32 %449, 55
  %451 = add i32 %450, %447
  %452 = getelementptr inbounds nuw i8, ptr %4, i32 220
  %453 = load i32, ptr %452, align 4, !tbaa !8
  %454 = mul i32 %453, 56
  %455 = add i32 %454, %451
  %456 = getelementptr inbounds nuw i8, ptr %4, i32 224
  %457 = load i32, ptr %456, align 16, !tbaa !8
  %458 = mul i32 %457, 57
  %459 = add i32 %458, %455
  %460 = getelementptr inbounds nuw i8, ptr %4, i32 228
  %461 = load i32, ptr %460, align 4, !tbaa !8
  %462 = mul i32 %461, 58
  %463 = add i32 %462, %459
  %464 = getelementptr inbounds nuw i8, ptr %4, i32 232
  %465 = load i32, ptr %464, align 8, !tbaa !8
  %466 = mul i32 %465, 59
  %467 = add i32 %466, %463
  %468 = getelementptr inbounds nuw i8, ptr %4, i32 236
  %469 = load i32, ptr %468, align 4, !tbaa !8
  %470 = mul i32 %469, 60
  %471 = add i32 %470, %467
  %472 = getelementptr inbounds nuw i8, ptr %4, i32 240
  %473 = load i32, ptr %472, align 16, !tbaa !8
  %474 = mul i32 %473, 61
  %475 = add i32 %474, %471
  %476 = getelementptr inbounds nuw i8, ptr %4, i32 244
  %477 = load i32, ptr %476, align 4, !tbaa !8
  %478 = mul i32 %477, 62
  %479 = add i32 %478, %475
  %480 = getelementptr inbounds nuw i8, ptr %4, i32 248
  %481 = load i32, ptr %480, align 8, !tbaa !8
  %482 = mul i32 %481, 63
  %483 = add i32 %482, %479
  %484 = getelementptr inbounds nuw i8, ptr %4, i32 252
  %485 = load i32, ptr %484, align 4, !tbaa !8
  %486 = shl i32 %485, 6
  %487 = add i32 %486, %483
  call void @llvm.lifetime.end.p0(ptr nonnull %4) #4
  call void @llvm.lifetime.end.p0(ptr nonnull %3) #4
  call void @llvm.lifetime.end.p0(ptr nonnull %2) #4
  ret i32 %487
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define hidden noundef i32 @main() #2 {
  %1 = tail call i32 @bench_matmul(i16 noundef zeroext 0) #5
  tail call void @report(i32 noundef %1) #6
  ret i32 0
}

declare void @report(i32 noundef) local_unnamed_addr #3

attributes #0 = { nofree norecurse nosync nounwind memory(none) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #3 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }
attributes #6 = { nobuiltin nounwind "no-builtins" }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"short", !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
