target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

define i64 @bench_matmul(i16 noundef zeroext %0) nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" memory(none) nofree nosync norecurse willreturn {
  %2 = alloca [8 x [8 x i16]], align 16
  %3 = alloca [8 x [8 x i16]], align 16
  %4 = alloca [8 x [8 x i64]], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %4) nounwind
  %5 = add i16 %0, 1
  store i16 %5, ptr %2, align 16, !tbaa !9
  store i16 2, ptr %3, align 16, !tbaa !9
  %6 = add i16 %0, 2
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 2
  store i16 %6, ptr %7, align 2, !tbaa !9
  %8 = getelementptr inbounds nuw i8, ptr %3, i64 2
  store i16 1, ptr %8, align 2, !tbaa !9
  %9 = add i16 %0, 3
  %10 = getelementptr inbounds nuw i8, ptr %2, i64 4
  store i16 %9, ptr %10, align 4, !tbaa !9
  %11 = getelementptr inbounds nuw i8, ptr %3, i64 4
  store i16 2, ptr %11, align 4, !tbaa !9
  %12 = add i16 %0, 4
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 6
  store i16 %12, ptr %13, align 2, !tbaa !9
  %14 = getelementptr inbounds nuw i8, ptr %3, i64 6
  store i16 0, ptr %14, align 2, !tbaa !9
  %15 = add i16 %0, 5
  %16 = getelementptr inbounds nuw i8, ptr %2, i64 8
  store i16 %15, ptr %16, align 8, !tbaa !9
  %17 = getelementptr inbounds nuw i8, ptr %3, i64 8
  store i16 1, ptr %17, align 8, !tbaa !9
  %18 = add i16 %0, 6
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 10
  store i16 %18, ptr %19, align 2, !tbaa !9
  %20 = getelementptr inbounds nuw i8, ptr %3, i64 10
  store i16 2, ptr %20, align 2, !tbaa !9
  %21 = add i16 %0, 7
  %22 = getelementptr inbounds nuw i8, ptr %2, i64 12
  store i16 %21, ptr %22, align 4, !tbaa !9
  %23 = getelementptr inbounds nuw i8, ptr %3, i64 12
  store i16 0, ptr %23, align 4, !tbaa !9
  %24 = add i16 %0, 8
  %25 = getelementptr inbounds nuw i8, ptr %2, i64 14
  store i16 %24, ptr %25, align 2, !tbaa !9
  %26 = getelementptr inbounds nuw i8, ptr %3, i64 14
  store i16 1, ptr %26, align 2, !tbaa !9
  %27 = getelementptr inbounds nuw i8, ptr %2, i64 16
  %28 = getelementptr inbounds nuw i8, ptr %3, i64 16
  store i16 %12, ptr %27, align 16, !tbaa !9
  store i16 1, ptr %28, align 16, !tbaa !9
  %29 = getelementptr inbounds nuw i8, ptr %2, i64 18
  store i16 %15, ptr %29, align 2, !tbaa !9
  %30 = getelementptr inbounds nuw i8, ptr %3, i64 18
  store i16 2, ptr %30, align 2, !tbaa !9
  %31 = getelementptr inbounds nuw i8, ptr %2, i64 20
  store i16 %18, ptr %31, align 4, !tbaa !9
  %32 = getelementptr inbounds nuw i8, ptr %3, i64 20
  store i16 0, ptr %32, align 4, !tbaa !9
  %33 = getelementptr inbounds nuw i8, ptr %2, i64 22
  store i16 %21, ptr %33, align 2, !tbaa !9
  %34 = getelementptr inbounds nuw i8, ptr %3, i64 22
  store i16 1, ptr %34, align 2, !tbaa !9
  %35 = getelementptr inbounds nuw i8, ptr %2, i64 24
  store i16 %24, ptr %35, align 8, !tbaa !9
  %36 = getelementptr inbounds nuw i8, ptr %3, i64 24
  store i16 2, ptr %36, align 8, !tbaa !9
  %37 = add i16 %0, 9
  %38 = getelementptr inbounds nuw i8, ptr %2, i64 26
  store i16 %37, ptr %38, align 2, !tbaa !9
  %39 = getelementptr inbounds nuw i8, ptr %3, i64 26
  store i16 0, ptr %39, align 2, !tbaa !9
  %40 = add i16 %0, 10
  %41 = getelementptr inbounds nuw i8, ptr %2, i64 28
  store i16 %40, ptr %41, align 4, !tbaa !9
  %42 = getelementptr inbounds nuw i8, ptr %3, i64 28
  store i16 1, ptr %42, align 4, !tbaa !9
  %43 = add i16 %0, 11
  %44 = getelementptr inbounds nuw i8, ptr %2, i64 30
  store i16 %43, ptr %44, align 2, !tbaa !9
  %45 = getelementptr inbounds nuw i8, ptr %3, i64 30
  store i16 2, ptr %45, align 2, !tbaa !9
  %46 = getelementptr inbounds nuw i8, ptr %2, i64 32
  %47 = getelementptr inbounds nuw i8, ptr %3, i64 32
  store i16 %21, ptr %46, align 16, !tbaa !9
  store i16 2, ptr %47, align 16, !tbaa !9
  %48 = getelementptr inbounds nuw i8, ptr %2, i64 34
  store i16 %24, ptr %48, align 2, !tbaa !9
  %49 = getelementptr inbounds nuw i8, ptr %3, i64 34
  store i16 0, ptr %49, align 2, !tbaa !9
  %50 = getelementptr inbounds nuw i8, ptr %2, i64 36
  store i16 %37, ptr %50, align 4, !tbaa !9
  %51 = getelementptr inbounds nuw i8, ptr %3, i64 36
  store i16 2, ptr %51, align 4, !tbaa !9
  %52 = getelementptr inbounds nuw i8, ptr %2, i64 38
  store i16 %40, ptr %52, align 2, !tbaa !9
  %53 = getelementptr inbounds nuw i8, ptr %3, i64 38
  store i16 2, ptr %53, align 2, !tbaa !9
  %54 = getelementptr inbounds nuw i8, ptr %2, i64 40
  store i16 %43, ptr %54, align 8, !tbaa !9
  %55 = getelementptr inbounds nuw i8, ptr %3, i64 40
  store i16 0, ptr %55, align 8, !tbaa !9
  %56 = add i16 %0, 12
  %57 = getelementptr inbounds nuw i8, ptr %2, i64 42
  store i16 %56, ptr %57, align 2, !tbaa !9
  %58 = getelementptr inbounds nuw i8, ptr %3, i64 42
  store i16 1, ptr %58, align 2, !tbaa !9
  %59 = add i16 %0, 13
  %60 = getelementptr inbounds nuw i8, ptr %2, i64 44
  store i16 %59, ptr %60, align 4, !tbaa !9
  %61 = getelementptr inbounds nuw i8, ptr %3, i64 44
  store i16 2, ptr %61, align 4, !tbaa !9
  %62 = add i16 %0, 14
  %63 = getelementptr inbounds nuw i8, ptr %2, i64 46
  store i16 %62, ptr %63, align 2, !tbaa !9
  %64 = getelementptr inbounds nuw i8, ptr %3, i64 46
  store i16 0, ptr %64, align 2, !tbaa !9
  %65 = getelementptr inbounds nuw i8, ptr %2, i64 48
  %66 = getelementptr inbounds nuw i8, ptr %3, i64 48
  store i16 %40, ptr %65, align 16, !tbaa !9
  store i16 0, ptr %66, align 16, !tbaa !9
  %67 = getelementptr inbounds nuw i8, ptr %2, i64 50
  store i16 %43, ptr %67, align 2, !tbaa !9
  %68 = getelementptr inbounds nuw i8, ptr %3, i64 50
  store i16 1, ptr %68, align 2, !tbaa !9
  %69 = getelementptr inbounds nuw i8, ptr %2, i64 52
  store i16 %56, ptr %69, align 4, !tbaa !9
  %70 = getelementptr inbounds nuw i8, ptr %3, i64 52
  store i16 2, ptr %70, align 4, !tbaa !9
  %71 = getelementptr inbounds nuw i8, ptr %2, i64 54
  store i16 %59, ptr %71, align 2, !tbaa !9
  %72 = getelementptr inbounds nuw i8, ptr %3, i64 54
  store i16 2, ptr %72, align 2, !tbaa !9
  %73 = getelementptr inbounds nuw i8, ptr %2, i64 56
  store i16 %62, ptr %73, align 8, !tbaa !9
  %74 = getelementptr inbounds nuw i8, ptr %3, i64 56
  store i16 1, ptr %74, align 8, !tbaa !9
  %75 = add i16 %0, 15
  %76 = getelementptr inbounds nuw i8, ptr %2, i64 58
  store i16 %75, ptr %76, align 2, !tbaa !9
  %77 = getelementptr inbounds nuw i8, ptr %3, i64 58
  store i16 2, ptr %77, align 2, !tbaa !9
  %78 = add i16 %0, 16
  %79 = getelementptr inbounds nuw i8, ptr %2, i64 60
  store i16 %78, ptr %79, align 4, !tbaa !9
  %80 = getelementptr inbounds nuw i8, ptr %3, i64 60
  store i16 0, ptr %80, align 4, !tbaa !9
  %81 = add i16 %0, 17
  %82 = getelementptr inbounds nuw i8, ptr %2, i64 62
  store i16 %81, ptr %82, align 2, !tbaa !9
  %83 = getelementptr inbounds nuw i8, ptr %3, i64 62
  store i16 1, ptr %83, align 2, !tbaa !9
  %84 = getelementptr inbounds nuw i8, ptr %2, i64 64
  %85 = getelementptr inbounds nuw i8, ptr %3, i64 64
  store i16 %59, ptr %84, align 16, !tbaa !9
  store i16 1, ptr %85, align 16, !tbaa !9
  %86 = getelementptr inbounds nuw i8, ptr %2, i64 66
  store i16 %62, ptr %86, align 2, !tbaa !9
  %87 = getelementptr inbounds nuw i8, ptr %3, i64 66
  store i16 2, ptr %87, align 2, !tbaa !9
  %88 = getelementptr inbounds nuw i8, ptr %2, i64 68
  store i16 %75, ptr %88, align 4, !tbaa !9
  %89 = getelementptr inbounds nuw i8, ptr %3, i64 68
  store i16 0, ptr %89, align 4, !tbaa !9
  %90 = getelementptr inbounds nuw i8, ptr %2, i64 70
  store i16 %78, ptr %90, align 2, !tbaa !9
  %91 = getelementptr inbounds nuw i8, ptr %3, i64 70
  store i16 1, ptr %91, align 2, !tbaa !9
  %92 = getelementptr inbounds nuw i8, ptr %2, i64 72
  store i16 %81, ptr %92, align 8, !tbaa !9
  %93 = getelementptr inbounds nuw i8, ptr %3, i64 72
  store i16 2, ptr %93, align 8, !tbaa !9
  %94 = add i16 %0, 18
  %95 = getelementptr inbounds nuw i8, ptr %2, i64 74
  store i16 %94, ptr %95, align 2, !tbaa !9
  %96 = getelementptr inbounds nuw i8, ptr %3, i64 74
  store i16 0, ptr %96, align 2, !tbaa !9
  %97 = add i16 %0, 19
  %98 = getelementptr inbounds nuw i8, ptr %2, i64 76
  store i16 %97, ptr %98, align 4, !tbaa !9
  %99 = getelementptr inbounds nuw i8, ptr %3, i64 76
  store i16 1, ptr %99, align 4, !tbaa !9
  %100 = add i16 %0, 20
  %101 = getelementptr inbounds nuw i8, ptr %2, i64 78
  store i16 %100, ptr %101, align 2, !tbaa !9
  %102 = getelementptr inbounds nuw i8, ptr %3, i64 78
  store i16 2, ptr %102, align 2, !tbaa !9
  %103 = getelementptr inbounds nuw i8, ptr %2, i64 80
  %104 = getelementptr inbounds nuw i8, ptr %3, i64 80
  store i16 %78, ptr %103, align 16, !tbaa !9
  store i16 2, ptr %104, align 16, !tbaa !9
  %105 = getelementptr inbounds nuw i8, ptr %2, i64 82
  store i16 %81, ptr %105, align 2, !tbaa !9
  %106 = getelementptr inbounds nuw i8, ptr %3, i64 82
  store i16 0, ptr %106, align 2, !tbaa !9
  %107 = getelementptr inbounds nuw i8, ptr %2, i64 84
  store i16 %94, ptr %107, align 4, !tbaa !9
  %108 = getelementptr inbounds nuw i8, ptr %3, i64 84
  store i16 1, ptr %108, align 4, !tbaa !9
  %109 = getelementptr inbounds nuw i8, ptr %2, i64 86
  store i16 %97, ptr %109, align 2, !tbaa !9
  %110 = getelementptr inbounds nuw i8, ptr %3, i64 86
  store i16 2, ptr %110, align 2, !tbaa !9
  %111 = getelementptr inbounds nuw i8, ptr %2, i64 88
  store i16 %100, ptr %111, align 8, !tbaa !9
  %112 = getelementptr inbounds nuw i8, ptr %3, i64 88
  store i16 0, ptr %112, align 8, !tbaa !9
  %113 = add i16 %0, 21
  %114 = getelementptr inbounds nuw i8, ptr %2, i64 90
  store i16 %113, ptr %114, align 2, !tbaa !9
  %115 = getelementptr inbounds nuw i8, ptr %3, i64 90
  store i16 2, ptr %115, align 2, !tbaa !9
  %116 = add i16 %0, 22
  %117 = getelementptr inbounds nuw i8, ptr %2, i64 92
  store i16 %116, ptr %117, align 4, !tbaa !9
  %118 = getelementptr inbounds nuw i8, ptr %3, i64 92
  store i16 2, ptr %118, align 4, !tbaa !9
  %119 = add i16 %0, 23
  %120 = getelementptr inbounds nuw i8, ptr %2, i64 94
  store i16 %119, ptr %120, align 2, !tbaa !9
  %121 = getelementptr inbounds nuw i8, ptr %3, i64 94
  store i16 0, ptr %121, align 2, !tbaa !9
  %122 = getelementptr inbounds nuw i8, ptr %2, i64 96
  %123 = getelementptr inbounds nuw i8, ptr %3, i64 96
  store i16 %97, ptr %122, align 16, !tbaa !9
  store i16 0, ptr %123, align 16, !tbaa !9
  %124 = getelementptr inbounds nuw i8, ptr %2, i64 98
  store i16 %100, ptr %124, align 2, !tbaa !9
  %125 = getelementptr inbounds nuw i8, ptr %3, i64 98
  store i16 1, ptr %125, align 2, !tbaa !9
  %126 = getelementptr inbounds nuw i8, ptr %2, i64 100
  store i16 %113, ptr %126, align 4, !tbaa !9
  %127 = getelementptr inbounds nuw i8, ptr %3, i64 100
  store i16 2, ptr %127, align 4, !tbaa !9
  %128 = getelementptr inbounds nuw i8, ptr %2, i64 102
  store i16 %116, ptr %128, align 2, !tbaa !9
  %129 = getelementptr inbounds nuw i8, ptr %3, i64 102
  store i16 0, ptr %129, align 2, !tbaa !9
  %130 = getelementptr inbounds nuw i8, ptr %2, i64 104
  store i16 %119, ptr %130, align 8, !tbaa !9
  %131 = getelementptr inbounds nuw i8, ptr %3, i64 104
  store i16 1, ptr %131, align 8, !tbaa !9
  %132 = add i16 %0, 24
  %133 = getelementptr inbounds nuw i8, ptr %2, i64 106
  store i16 %132, ptr %133, align 2, !tbaa !9
  %134 = getelementptr inbounds nuw i8, ptr %3, i64 106
  store i16 2, ptr %134, align 2, !tbaa !9
  %135 = add i16 %0, 25
  %136 = getelementptr inbounds nuw i8, ptr %2, i64 108
  store i16 %135, ptr %136, align 4, !tbaa !9
  %137 = getelementptr inbounds nuw i8, ptr %3, i64 108
  store i16 2, ptr %137, align 4, !tbaa !9
  %138 = add i16 %0, 26
  %139 = getelementptr inbounds nuw i8, ptr %2, i64 110
  store i16 %138, ptr %139, align 2, !tbaa !9
  %140 = getelementptr inbounds nuw i8, ptr %3, i64 110
  store i16 1, ptr %140, align 2, !tbaa !9
  %141 = getelementptr inbounds nuw i8, ptr %2, i64 112
  %142 = getelementptr inbounds nuw i8, ptr %3, i64 112
  store i16 %116, ptr %141, align 16, !tbaa !9
  store i16 1, ptr %142, align 16, !tbaa !9
  %143 = getelementptr inbounds nuw i8, ptr %2, i64 114
  store i16 %119, ptr %143, align 2, !tbaa !9
  %144 = getelementptr inbounds nuw i8, ptr %3, i64 114
  store i16 2, ptr %144, align 2, !tbaa !9
  %145 = getelementptr inbounds nuw i8, ptr %2, i64 116
  store i16 %132, ptr %145, align 4, !tbaa !9
  %146 = getelementptr inbounds nuw i8, ptr %3, i64 116
  store i16 0, ptr %146, align 4, !tbaa !9
  %147 = getelementptr inbounds nuw i8, ptr %2, i64 118
  store i16 %135, ptr %147, align 2, !tbaa !9
  %148 = getelementptr inbounds nuw i8, ptr %3, i64 118
  store i16 1, ptr %148, align 2, !tbaa !9
  %149 = getelementptr inbounds nuw i8, ptr %2, i64 120
  store i16 %138, ptr %149, align 8, !tbaa !9
  %150 = getelementptr inbounds nuw i8, ptr %3, i64 120
  store i16 2, ptr %150, align 8, !tbaa !9
  %151 = add i16 %0, 27
  %152 = getelementptr inbounds nuw i8, ptr %2, i64 122
  store i16 %151, ptr %152, align 2, !tbaa !9
  %153 = getelementptr inbounds nuw i8, ptr %3, i64 122
  store i16 0, ptr %153, align 2, !tbaa !9
  %154 = add i16 %0, 28
  %155 = getelementptr inbounds nuw i8, ptr %2, i64 124
  store i16 %154, ptr %155, align 4, !tbaa !9
  %156 = getelementptr inbounds nuw i8, ptr %3, i64 124
  store i16 1, ptr %156, align 4, !tbaa !9
  %157 = add i16 %0, 29
  %158 = getelementptr inbounds nuw i8, ptr %2, i64 126
  store i16 %157, ptr %158, align 2, !tbaa !9
  %159 = getelementptr inbounds nuw i8, ptr %3, i64 126
  store i16 2, ptr %159, align 2, !tbaa !9
  br label %160

160:
  %161 = phi ptr [ %248, %246 ], [ %4, %1 ]
  %162 = phi i64 [ 0, %1 ], [ %247, %246 ]
  %163 = getelementptr inbounds nuw [16 x i8], ptr %2, i64 %162
  %164 = load i16, ptr %163, align 16, !tbaa !9
  %165 = getelementptr inbounds nuw i8, ptr %163, i64 2
  %166 = load i16, ptr %165, align 2, !tbaa !9
  %167 = getelementptr inbounds nuw i8, ptr %163, i64 4
  %168 = load i16, ptr %167, align 4, !tbaa !9
  %169 = getelementptr inbounds nuw i8, ptr %163, i64 6
  %170 = load i16, ptr %169, align 2, !tbaa !9
  %171 = getelementptr inbounds nuw i8, ptr %163, i64 8
  %172 = load i16, ptr %171, align 8, !tbaa !9
  %173 = getelementptr inbounds nuw i8, ptr %163, i64 10
  %174 = load i16, ptr %173, align 2, !tbaa !9
  %175 = getelementptr inbounds nuw i8, ptr %163, i64 12
  %176 = load i16, ptr %175, align 4, !tbaa !9
  %177 = getelementptr inbounds nuw i8, ptr %163, i64 14
  %178 = load i16, ptr %177, align 2, !tbaa !9
  %179 = sext i16 %164 to i64
  %180 = sext i16 %166 to i64
  %181 = sext i16 %168 to i64
  %182 = sext i16 %170 to i64
  %183 = sext i16 %172 to i64
  %184 = sext i16 %174 to i64
  %185 = sext i16 %176 to i64
  %186 = sext i16 %178 to i64
  br label %187

187:
  %188 = phi i64 [ 0, %160 ], [ %245, %187 ]
  %189 = shl nuw nsw i64 %188, 1
  %190 = getelementptr i8, ptr %3, i64 %189
  %191 = load i16, ptr %190, align 2, !tbaa !9
  %192 = sext i16 %191 to i64
  %193 = mul nsw i64 %192, %179
  %194 = shl nuw nsw i64 %188, 1
  %195 = getelementptr i8, ptr %3, i64 %194
  %196 = getelementptr i8, ptr %195, i64 16
  %197 = load i16, ptr %196, align 2, !tbaa !9
  %198 = sext i16 %197 to i64
  %199 = mul nsw i64 %198, %180
  %200 = add nsw i64 %199, %193
  %201 = shl nuw nsw i64 %188, 1
  %202 = getelementptr i8, ptr %3, i64 %201
  %203 = getelementptr i8, ptr %202, i64 32
  %204 = load i16, ptr %203, align 2, !tbaa !9
  %205 = sext i16 %204 to i64
  %206 = mul nsw i64 %205, %181
  %207 = add nsw i64 %206, %200
  %208 = shl nuw nsw i64 %188, 1
  %209 = getelementptr i8, ptr %3, i64 %208
  %210 = getelementptr i8, ptr %209, i64 48
  %211 = load i16, ptr %210, align 2, !tbaa !9
  %212 = sext i16 %211 to i64
  %213 = mul nsw i64 %212, %182
  %214 = add nsw i64 %213, %207
  %215 = shl nuw nsw i64 %188, 1
  %216 = getelementptr i8, ptr %3, i64 %215
  %217 = getelementptr i8, ptr %216, i64 64
  %218 = load i16, ptr %217, align 2, !tbaa !9
  %219 = sext i16 %218 to i64
  %220 = mul nsw i64 %219, %183
  %221 = add nsw i64 %220, %214
  %222 = shl nuw nsw i64 %188, 1
  %223 = getelementptr i8, ptr %3, i64 %222
  %224 = getelementptr i8, ptr %223, i64 80
  %225 = load i16, ptr %224, align 2, !tbaa !9
  %226 = sext i16 %225 to i64
  %227 = mul nsw i64 %226, %184
  %228 = add nsw i64 %227, %221
  %229 = shl nuw nsw i64 %188, 1
  %230 = getelementptr i8, ptr %3, i64 %229
  %231 = getelementptr i8, ptr %230, i64 96
  %232 = load i16, ptr %231, align 2, !tbaa !9
  %233 = sext i16 %232 to i64
  %234 = mul nsw i64 %233, %185
  %235 = add nsw i64 %234, %228
  %236 = shl nuw nsw i64 %188, 1
  %237 = getelementptr i8, ptr %3, i64 %236
  %238 = getelementptr i8, ptr %237, i64 112
  %239 = load i16, ptr %238, align 2, !tbaa !9
  %240 = sext i16 %239 to i64
  %241 = mul nsw i64 %240, %186
  %242 = add nsw i64 %241, %235
  %243 = shl nuw nsw i64 %188, 3
  %244 = getelementptr i8, ptr %161, i64 %243
  store i64 %242, ptr %244, align 8, !tbaa !11
  %245 = add nuw nsw i64 %188, 1
  %exitcond3 = icmp eq i64 %245, 8
  br i1 %exitcond3, label %246, label %187, !llvm.loop !17

246:
  %247 = add nuw nsw i64 %162, 1
  %248 = getelementptr i8, ptr %161, i64 64
  %exitcond4 = icmp eq i64 %247, 8
  br i1 %exitcond4, label %249, label %160, !llvm.loop !18

249:
  br label %250

250:
  %251 = phi i64 [ 8, %249 ], [ %308, %250 ]
  %252 = phi i64 [ %307, %250 ], [ 0, %249 ]
  %253 = shl nuw nsw i64 %251, 3
  %254 = getelementptr i8, ptr %4, i64 %253
  %255 = getelementptr i8, ptr %254, i64 -64
  %256 = add i64 %251, -7
  %257 = load i64, ptr %255, align 16, !tbaa !11
  %258 = mul i64 %257, %256
  %259 = add i64 %258, %252
  %260 = shl nuw nsw i64 %251, 3
  %261 = getelementptr i8, ptr %4, i64 %260
  %262 = getelementptr i8, ptr %261, i64 -56
  %263 = load i64, ptr %262, align 8, !tbaa !11
  %264 = add i64 %251, -6
  %265 = mul i64 %263, %264
  %266 = add i64 %265, %259
  %267 = shl nuw nsw i64 %251, 3
  %268 = getelementptr i8, ptr %4, i64 %267
  %269 = getelementptr i8, ptr %268, i64 -48
  %270 = load i64, ptr %269, align 16, !tbaa !11
  %271 = add i64 %251, -5
  %272 = mul i64 %270, %271
  %273 = add i64 %272, %266
  %274 = shl nuw nsw i64 %251, 3
  %275 = getelementptr i8, ptr %4, i64 %274
  %276 = getelementptr i8, ptr %275, i64 -40
  %277 = load i64, ptr %276, align 8, !tbaa !11
  %278 = add i64 %251, -4
  %279 = mul i64 %277, %278
  %280 = add i64 %279, %273
  %281 = shl nuw nsw i64 %251, 3
  %282 = getelementptr i8, ptr %4, i64 %281
  %283 = getelementptr i8, ptr %282, i64 -32
  %284 = load i64, ptr %283, align 16, !tbaa !11
  %285 = add i64 %251, -3
  %286 = mul i64 %284, %285
  %287 = add i64 %286, %280
  %288 = shl nuw nsw i64 %251, 3
  %289 = getelementptr i8, ptr %4, i64 %288
  %290 = getelementptr i8, ptr %289, i64 -24
  %291 = load i64, ptr %290, align 8, !tbaa !11
  %292 = add i64 %251, -2
  %293 = mul i64 %291, %292
  %294 = add i64 %293, %287
  %295 = shl nuw nsw i64 %251, 3
  %296 = getelementptr i8, ptr %4, i64 %295
  %297 = getelementptr i8, ptr %296, i64 -16
  %298 = load i64, ptr %297, align 16, !tbaa !11
  %299 = add i64 %251, -1
  %300 = mul i64 %298, %299
  %301 = add i64 %300, %294
  %302 = shl nuw nsw i64 %251, 3
  %303 = getelementptr i8, ptr %4, i64 %302
  %304 = getelementptr i8, ptr %303, i64 -8
  %305 = load i64, ptr %304, align 8, !tbaa !11
  %306 = mul i64 %305, %251
  %307 = add i64 %306, %301
  %308 = add nuw nsw i64 %251, 8
  %exitcond6 = icmp eq i64 %308, 72
  br i1 %exitcond6, label %309, label %250, !llvm.loop !20

309:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret i64 %307
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" {
  %1 = tail call i64 @bench_matmul(i16 noundef zeroext 0)
  tail call void @report(i64 noundef %1)
  ret i32 0
}

declare void @report(i64 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic"

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"uwtable", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !7, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !7, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = distinct !{!17, !14}
!18 = distinct !{!18, !14}
!19 = distinct !{!19, !14}
!20 = distinct !{!20, !14}
