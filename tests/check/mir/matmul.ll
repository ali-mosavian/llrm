target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i32 @_bench_matmul(i16 %0) addrspace(1) memory(none) nounwind {
b1:
  %1 = alloca [128 x i8]
  %2 = alloca [128 x i8]
  %3 = alloca [256 x i8]
  %4 = getelementptr inbounds i8, ptr %1, i16 0
  %5 = getelementptr inbounds i8, ptr %2, i16 0
  %6 = add i16 %0, 1
  %7 = getelementptr inbounds i8, ptr %4, i16 0
  store i16 %6, ptr %7, !tbaa !9
  %8 = getelementptr inbounds i8, ptr %5, i16 0
  store i16 2, ptr %8, !tbaa !9
  %9 = add i16 %0, 2
  %10 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %9, ptr %10, !tbaa !9
  %11 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 1, ptr %11, !tbaa !9
  %12 = add i16 %0, 3
  %13 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 %12, ptr %13, !tbaa !9
  %14 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 2, ptr %14, !tbaa !9
  %15 = add i16 %0, 4
  %16 = getelementptr inbounds i8, ptr %4, i16 6
  store i16 %15, ptr %16, !tbaa !9
  %17 = getelementptr inbounds i8, ptr %5, i16 6
  store i16 0, ptr %17, !tbaa !9
  %18 = add i16 %0, 5
  %19 = getelementptr inbounds i8, ptr %4, i16 8
  store i16 %18, ptr %19, !tbaa !9
  %20 = getelementptr inbounds i8, ptr %5, i16 8
  store i16 1, ptr %20, !tbaa !9
  %21 = add i16 %0, 6
  %22 = getelementptr inbounds i8, ptr %4, i16 10
  store i16 %21, ptr %22, !tbaa !9
  %23 = getelementptr inbounds i8, ptr %5, i16 10
  store i16 2, ptr %23, !tbaa !9
  %24 = add i16 %0, 7
  %25 = getelementptr inbounds i8, ptr %4, i16 12
  store i16 %24, ptr %25, !tbaa !9
  %26 = getelementptr inbounds i8, ptr %5, i16 12
  store i16 0, ptr %26, !tbaa !9
  %27 = add i16 %0, 8
  %28 = getelementptr inbounds i8, ptr %4, i16 14
  store i16 %27, ptr %28, !tbaa !9
  %29 = getelementptr inbounds i8, ptr %5, i16 14
  store i16 1, ptr %29, !tbaa !9
  %30 = getelementptr inbounds i8, ptr %1, i16 16
  %31 = getelementptr inbounds i8, ptr %2, i16 16
  %32 = getelementptr inbounds i8, ptr %30, i16 0
  store i16 %15, ptr %32, !tbaa !9
  %33 = getelementptr inbounds i8, ptr %31, i16 0
  store i16 1, ptr %33, !tbaa !9
  %34 = getelementptr inbounds i8, ptr %30, i16 2
  store i16 %18, ptr %34, !tbaa !9
  %35 = getelementptr inbounds i8, ptr %31, i16 2
  store i16 2, ptr %35, !tbaa !9
  %36 = getelementptr inbounds i8, ptr %30, i16 4
  store i16 %21, ptr %36, !tbaa !9
  %37 = getelementptr inbounds i8, ptr %31, i16 4
  store i16 0, ptr %37, !tbaa !9
  %38 = getelementptr inbounds i8, ptr %30, i16 6
  store i16 %24, ptr %38, !tbaa !9
  %39 = getelementptr inbounds i8, ptr %31, i16 6
  store i16 1, ptr %39, !tbaa !9
  %40 = getelementptr inbounds i8, ptr %30, i16 8
  store i16 %27, ptr %40, !tbaa !9
  %41 = getelementptr inbounds i8, ptr %31, i16 8
  store i16 2, ptr %41, !tbaa !9
  %42 = add i16 %0, 9
  %43 = getelementptr inbounds i8, ptr %30, i16 10
  store i16 %42, ptr %43, !tbaa !9
  %44 = getelementptr inbounds i8, ptr %31, i16 10
  store i16 0, ptr %44, !tbaa !9
  %45 = add i16 %0, 10
  %46 = getelementptr inbounds i8, ptr %30, i16 12
  store i16 %45, ptr %46, !tbaa !9
  %47 = getelementptr inbounds i8, ptr %31, i16 12
  store i16 1, ptr %47, !tbaa !9
  %48 = add i16 %0, 11
  %49 = getelementptr inbounds i8, ptr %30, i16 14
  store i16 %48, ptr %49, !tbaa !9
  %50 = getelementptr inbounds i8, ptr %31, i16 14
  store i16 2, ptr %50, !tbaa !9
  %51 = getelementptr inbounds i8, ptr %1, i16 32
  %52 = getelementptr inbounds i8, ptr %2, i16 32
  %53 = getelementptr inbounds i8, ptr %51, i16 0
  store i16 %24, ptr %53, !tbaa !9
  %54 = getelementptr inbounds i8, ptr %52, i16 0
  store i16 2, ptr %54, !tbaa !9
  %55 = getelementptr inbounds i8, ptr %51, i16 2
  store i16 %27, ptr %55, !tbaa !9
  %56 = getelementptr inbounds i8, ptr %52, i16 2
  store i16 0, ptr %56, !tbaa !9
  %57 = getelementptr inbounds i8, ptr %51, i16 4
  store i16 %42, ptr %57, !tbaa !9
  %58 = getelementptr inbounds i8, ptr %52, i16 4
  store i16 2, ptr %58, !tbaa !9
  %59 = getelementptr inbounds i8, ptr %51, i16 6
  store i16 %45, ptr %59, !tbaa !9
  %60 = getelementptr inbounds i8, ptr %52, i16 6
  store i16 2, ptr %60, !tbaa !9
  %61 = getelementptr inbounds i8, ptr %51, i16 8
  store i16 %48, ptr %61, !tbaa !9
  %62 = getelementptr inbounds i8, ptr %52, i16 8
  store i16 0, ptr %62, !tbaa !9
  %63 = add i16 %0, 12
  %64 = getelementptr inbounds i8, ptr %51, i16 10
  store i16 %63, ptr %64, !tbaa !9
  %65 = getelementptr inbounds i8, ptr %52, i16 10
  store i16 1, ptr %65, !tbaa !9
  %66 = add i16 %0, 13
  %67 = getelementptr inbounds i8, ptr %51, i16 12
  store i16 %66, ptr %67, !tbaa !9
  %68 = getelementptr inbounds i8, ptr %52, i16 12
  store i16 2, ptr %68, !tbaa !9
  %69 = add i16 %0, 14
  %70 = getelementptr inbounds i8, ptr %51, i16 14
  store i16 %69, ptr %70, !tbaa !9
  %71 = getelementptr inbounds i8, ptr %52, i16 14
  store i16 0, ptr %71, !tbaa !9
  %72 = getelementptr inbounds i8, ptr %1, i16 48
  %73 = getelementptr inbounds i8, ptr %2, i16 48
  %74 = getelementptr inbounds i8, ptr %72, i16 0
  store i16 %45, ptr %74, !tbaa !9
  %75 = getelementptr inbounds i8, ptr %73, i16 0
  store i16 0, ptr %75, !tbaa !9
  %76 = getelementptr inbounds i8, ptr %72, i16 2
  store i16 %48, ptr %76, !tbaa !9
  %77 = getelementptr inbounds i8, ptr %73, i16 2
  store i16 1, ptr %77, !tbaa !9
  %78 = getelementptr inbounds i8, ptr %72, i16 4
  store i16 %63, ptr %78, !tbaa !9
  %79 = getelementptr inbounds i8, ptr %73, i16 4
  store i16 2, ptr %79, !tbaa !9
  %80 = getelementptr inbounds i8, ptr %72, i16 6
  store i16 %66, ptr %80, !tbaa !9
  %81 = getelementptr inbounds i8, ptr %73, i16 6
  store i16 2, ptr %81, !tbaa !9
  %82 = getelementptr inbounds i8, ptr %72, i16 8
  store i16 %69, ptr %82, !tbaa !9
  %83 = getelementptr inbounds i8, ptr %73, i16 8
  store i16 1, ptr %83, !tbaa !9
  %84 = add i16 %0, 15
  %85 = getelementptr inbounds i8, ptr %72, i16 10
  store i16 %84, ptr %85, !tbaa !9
  %86 = getelementptr inbounds i8, ptr %73, i16 10
  store i16 2, ptr %86, !tbaa !9
  %87 = add i16 %0, 16
  %88 = getelementptr inbounds i8, ptr %72, i16 12
  store i16 %87, ptr %88, !tbaa !9
  %89 = getelementptr inbounds i8, ptr %73, i16 12
  store i16 0, ptr %89, !tbaa !9
  %90 = add i16 %0, 17
  %91 = getelementptr inbounds i8, ptr %72, i16 14
  store i16 %90, ptr %91, !tbaa !9
  %92 = getelementptr inbounds i8, ptr %73, i16 14
  store i16 1, ptr %92, !tbaa !9
  %93 = getelementptr inbounds i8, ptr %1, i16 64
  %94 = getelementptr inbounds i8, ptr %2, i16 64
  %95 = getelementptr inbounds i8, ptr %93, i16 0
  store i16 %66, ptr %95, !tbaa !9
  %96 = getelementptr inbounds i8, ptr %94, i16 0
  store i16 1, ptr %96, !tbaa !9
  %97 = getelementptr inbounds i8, ptr %93, i16 2
  store i16 %69, ptr %97, !tbaa !9
  %98 = getelementptr inbounds i8, ptr %94, i16 2
  store i16 2, ptr %98, !tbaa !9
  %99 = getelementptr inbounds i8, ptr %93, i16 4
  store i16 %84, ptr %99, !tbaa !9
  %100 = getelementptr inbounds i8, ptr %94, i16 4
  store i16 0, ptr %100, !tbaa !9
  %101 = getelementptr inbounds i8, ptr %93, i16 6
  store i16 %87, ptr %101, !tbaa !9
  %102 = getelementptr inbounds i8, ptr %94, i16 6
  store i16 1, ptr %102, !tbaa !9
  %103 = getelementptr inbounds i8, ptr %93, i16 8
  store i16 %90, ptr %103, !tbaa !9
  %104 = getelementptr inbounds i8, ptr %94, i16 8
  store i16 2, ptr %104, !tbaa !9
  %105 = add i16 %0, 18
  %106 = getelementptr inbounds i8, ptr %93, i16 10
  store i16 %105, ptr %106, !tbaa !9
  %107 = getelementptr inbounds i8, ptr %94, i16 10
  store i16 0, ptr %107, !tbaa !9
  %108 = add i16 %0, 19
  %109 = getelementptr inbounds i8, ptr %93, i16 12
  store i16 %108, ptr %109, !tbaa !9
  %110 = getelementptr inbounds i8, ptr %94, i16 12
  store i16 1, ptr %110, !tbaa !9
  %111 = add i16 %0, 20
  %112 = getelementptr inbounds i8, ptr %93, i16 14
  store i16 %111, ptr %112, !tbaa !9
  %113 = getelementptr inbounds i8, ptr %94, i16 14
  store i16 2, ptr %113, !tbaa !9
  %114 = getelementptr inbounds i8, ptr %1, i16 80
  %115 = getelementptr inbounds i8, ptr %2, i16 80
  %116 = getelementptr inbounds i8, ptr %114, i16 0
  store i16 %87, ptr %116, !tbaa !9
  %117 = getelementptr inbounds i8, ptr %115, i16 0
  store i16 2, ptr %117, !tbaa !9
  %118 = getelementptr inbounds i8, ptr %114, i16 2
  store i16 %90, ptr %118, !tbaa !9
  %119 = getelementptr inbounds i8, ptr %115, i16 2
  store i16 0, ptr %119, !tbaa !9
  %120 = getelementptr inbounds i8, ptr %114, i16 4
  store i16 %105, ptr %120, !tbaa !9
  %121 = getelementptr inbounds i8, ptr %115, i16 4
  store i16 1, ptr %121, !tbaa !9
  %122 = getelementptr inbounds i8, ptr %114, i16 6
  store i16 %108, ptr %122, !tbaa !9
  %123 = getelementptr inbounds i8, ptr %115, i16 6
  store i16 2, ptr %123, !tbaa !9
  %124 = getelementptr inbounds i8, ptr %114, i16 8
  store i16 %111, ptr %124, !tbaa !9
  %125 = getelementptr inbounds i8, ptr %115, i16 8
  store i16 0, ptr %125, !tbaa !9
  %126 = add i16 %0, 21
  %127 = getelementptr inbounds i8, ptr %114, i16 10
  store i16 %126, ptr %127, !tbaa !9
  %128 = getelementptr inbounds i8, ptr %115, i16 10
  store i16 2, ptr %128, !tbaa !9
  %129 = add i16 %0, 22
  %130 = getelementptr inbounds i8, ptr %114, i16 12
  store i16 %129, ptr %130, !tbaa !9
  %131 = getelementptr inbounds i8, ptr %115, i16 12
  store i16 2, ptr %131, !tbaa !9
  %132 = add i16 %0, 23
  %133 = getelementptr inbounds i8, ptr %114, i16 14
  store i16 %132, ptr %133, !tbaa !9
  %134 = getelementptr inbounds i8, ptr %115, i16 14
  store i16 0, ptr %134, !tbaa !9
  %135 = getelementptr inbounds i8, ptr %1, i16 96
  %136 = getelementptr inbounds i8, ptr %2, i16 96
  %137 = getelementptr inbounds i8, ptr %135, i16 0
  store i16 %108, ptr %137, !tbaa !9
  %138 = getelementptr inbounds i8, ptr %136, i16 0
  store i16 0, ptr %138, !tbaa !9
  %139 = getelementptr inbounds i8, ptr %135, i16 2
  store i16 %111, ptr %139, !tbaa !9
  %140 = getelementptr inbounds i8, ptr %136, i16 2
  store i16 1, ptr %140, !tbaa !9
  %141 = getelementptr inbounds i8, ptr %135, i16 4
  store i16 %126, ptr %141, !tbaa !9
  %142 = getelementptr inbounds i8, ptr %136, i16 4
  store i16 2, ptr %142, !tbaa !9
  %143 = getelementptr inbounds i8, ptr %135, i16 6
  store i16 %129, ptr %143, !tbaa !9
  %144 = getelementptr inbounds i8, ptr %136, i16 6
  store i16 0, ptr %144, !tbaa !9
  %145 = getelementptr inbounds i8, ptr %135, i16 8
  store i16 %132, ptr %145, !tbaa !9
  %146 = getelementptr inbounds i8, ptr %136, i16 8
  store i16 1, ptr %146, !tbaa !9
  %147 = add i16 %0, 24
  %148 = getelementptr inbounds i8, ptr %135, i16 10
  store i16 %147, ptr %148, !tbaa !9
  %149 = getelementptr inbounds i8, ptr %136, i16 10
  store i16 2, ptr %149, !tbaa !9
  %150 = add i16 %0, 25
  %151 = getelementptr inbounds i8, ptr %135, i16 12
  store i16 %150, ptr %151, !tbaa !9
  %152 = getelementptr inbounds i8, ptr %136, i16 12
  store i16 2, ptr %152, !tbaa !9
  %153 = add i16 %0, 26
  %154 = getelementptr inbounds i8, ptr %135, i16 14
  store i16 %153, ptr %154, !tbaa !9
  %155 = getelementptr inbounds i8, ptr %136, i16 14
  store i16 1, ptr %155, !tbaa !9
  %156 = getelementptr inbounds i8, ptr %1, i16 112
  %157 = getelementptr inbounds i8, ptr %2, i16 112
  %158 = getelementptr inbounds i8, ptr %156, i16 0
  store i16 %129, ptr %158, !tbaa !9
  %159 = getelementptr inbounds i8, ptr %157, i16 0
  store i16 1, ptr %159, !tbaa !9
  %160 = getelementptr inbounds i8, ptr %156, i16 2
  store i16 %132, ptr %160, !tbaa !9
  %161 = getelementptr inbounds i8, ptr %157, i16 2
  store i16 2, ptr %161, !tbaa !9
  %162 = getelementptr inbounds i8, ptr %156, i16 4
  store i16 %147, ptr %162, !tbaa !9
  %163 = getelementptr inbounds i8, ptr %157, i16 4
  store i16 0, ptr %163, !tbaa !9
  %164 = getelementptr inbounds i8, ptr %156, i16 6
  store i16 %150, ptr %164, !tbaa !9
  %165 = getelementptr inbounds i8, ptr %157, i16 6
  store i16 1, ptr %165, !tbaa !9
  %166 = getelementptr inbounds i8, ptr %156, i16 8
  store i16 %153, ptr %166, !tbaa !9
  %167 = getelementptr inbounds i8, ptr %157, i16 8
  store i16 2, ptr %167, !tbaa !9
  %168 = add i16 %0, 27
  %169 = getelementptr inbounds i8, ptr %156, i16 10
  store i16 %168, ptr %169, !tbaa !9
  %170 = getelementptr inbounds i8, ptr %157, i16 10
  store i16 0, ptr %170, !tbaa !9
  %171 = add i16 %0, 28
  %172 = getelementptr inbounds i8, ptr %156, i16 12
  store i16 %171, ptr %172, !tbaa !9
  %173 = getelementptr inbounds i8, ptr %157, i16 12
  store i16 1, ptr %173, !tbaa !9
  %174 = add i16 %0, 29
  %175 = getelementptr inbounds i8, ptr %156, i16 14
  store i16 %174, ptr %175, !tbaa !9
  %176 = getelementptr inbounds i8, ptr %157, i16 14
  store i16 2, ptr %176, !tbaa !9
  br label %b13

b12:
  %177 = getelementptr inbounds i8, ptr %3, i16 0
  %178 = getelementptr inbounds i8, ptr %177, i16 0
  %179 = load i32, ptr %178, !tbaa !11
  %180 = getelementptr inbounds i8, ptr %177, i16 4
  %181 = load i32, ptr %180, !tbaa !11
  %182 = mul i32 %181, 2
  %183 = add i32 %179, %182
  %184 = getelementptr inbounds i8, ptr %177, i16 8
  %185 = load i32, ptr %184, !tbaa !11
  %186 = mul i32 %185, 3
  %187 = add i32 %183, %186
  %188 = getelementptr inbounds i8, ptr %177, i16 12
  %189 = load i32, ptr %188, !tbaa !11
  %190 = mul i32 %189, 4
  %191 = add i32 %187, %190
  %192 = getelementptr inbounds i8, ptr %177, i16 16
  %193 = load i32, ptr %192, !tbaa !11
  %194 = mul i32 %193, 5
  %195 = add i32 %191, %194
  %196 = getelementptr inbounds i8, ptr %177, i16 20
  %197 = load i32, ptr %196, !tbaa !11
  %198 = mul i32 %197, 6
  %199 = add i32 %195, %198
  %200 = getelementptr inbounds i8, ptr %177, i16 24
  %201 = load i32, ptr %200, !tbaa !11
  %202 = mul i32 %201, 7
  %203 = add i32 %199, %202
  %204 = getelementptr inbounds i8, ptr %177, i16 28
  %205 = load i32, ptr %204, !tbaa !11
  %206 = mul i32 %205, 8
  %207 = add i32 %203, %206
  %208 = getelementptr inbounds i8, ptr %3, i16 32
  %209 = getelementptr inbounds i8, ptr %208, i16 0
  %210 = load i32, ptr %209, !tbaa !11
  %211 = mul i32 %210, 9
  %212 = add i32 %207, %211
  %213 = getelementptr inbounds i8, ptr %208, i16 4
  %214 = load i32, ptr %213, !tbaa !11
  %215 = mul i32 %214, 10
  %216 = add i32 %212, %215
  %217 = getelementptr inbounds i8, ptr %208, i16 8
  %218 = load i32, ptr %217, !tbaa !11
  %219 = mul i32 %218, 11
  %220 = add i32 %216, %219
  %221 = getelementptr inbounds i8, ptr %208, i16 12
  %222 = load i32, ptr %221, !tbaa !11
  %223 = mul i32 %222, 12
  %224 = add i32 %220, %223
  %225 = getelementptr inbounds i8, ptr %208, i16 16
  %226 = load i32, ptr %225, !tbaa !11
  %227 = mul i32 %226, 13
  %228 = add i32 %224, %227
  %229 = getelementptr inbounds i8, ptr %208, i16 20
  %230 = load i32, ptr %229, !tbaa !11
  %231 = mul i32 %230, 14
  %232 = add i32 %228, %231
  %233 = getelementptr inbounds i8, ptr %208, i16 24
  %234 = load i32, ptr %233, !tbaa !11
  %235 = mul i32 %234, 15
  %236 = add i32 %232, %235
  %237 = getelementptr inbounds i8, ptr %208, i16 28
  %238 = load i32, ptr %237, !tbaa !11
  %239 = mul i32 %238, 16
  %240 = add i32 %236, %239
  %241 = getelementptr inbounds i8, ptr %3, i16 64
  %242 = getelementptr inbounds i8, ptr %241, i16 0
  %243 = load i32, ptr %242, !tbaa !11
  %244 = mul i32 %243, 17
  %245 = add i32 %240, %244
  %246 = getelementptr inbounds i8, ptr %241, i16 4
  %247 = load i32, ptr %246, !tbaa !11
  %248 = mul i32 %247, 18
  %249 = add i32 %245, %248
  %250 = getelementptr inbounds i8, ptr %241, i16 8
  %251 = load i32, ptr %250, !tbaa !11
  %252 = mul i32 %251, 19
  %253 = add i32 %249, %252
  %254 = getelementptr inbounds i8, ptr %241, i16 12
  %255 = load i32, ptr %254, !tbaa !11
  %256 = mul i32 %255, 20
  %257 = add i32 %253, %256
  %258 = getelementptr inbounds i8, ptr %241, i16 16
  %259 = load i32, ptr %258, !tbaa !11
  %260 = mul i32 %259, 21
  %261 = add i32 %257, %260
  %262 = getelementptr inbounds i8, ptr %241, i16 20
  %263 = load i32, ptr %262, !tbaa !11
  %264 = mul i32 %263, 22
  %265 = add i32 %261, %264
  %266 = getelementptr inbounds i8, ptr %241, i16 24
  %267 = load i32, ptr %266, !tbaa !11
  %268 = mul i32 %267, 23
  %269 = add i32 %265, %268
  %270 = getelementptr inbounds i8, ptr %241, i16 28
  %271 = load i32, ptr %270, !tbaa !11
  %272 = mul i32 %271, 24
  %273 = add i32 %269, %272
  %274 = getelementptr inbounds i8, ptr %3, i16 96
  %275 = getelementptr inbounds i8, ptr %274, i16 0
  %276 = load i32, ptr %275, !tbaa !11
  %277 = mul i32 %276, 25
  %278 = add i32 %273, %277
  %279 = getelementptr inbounds i8, ptr %274, i16 4
  %280 = load i32, ptr %279, !tbaa !11
  %281 = mul i32 %280, 26
  %282 = add i32 %278, %281
  %283 = getelementptr inbounds i8, ptr %274, i16 8
  %284 = load i32, ptr %283, !tbaa !11
  %285 = mul i32 %284, 27
  %286 = add i32 %282, %285
  %287 = getelementptr inbounds i8, ptr %274, i16 12
  %288 = load i32, ptr %287, !tbaa !11
  %289 = mul i32 %288, 28
  %290 = add i32 %286, %289
  %291 = getelementptr inbounds i8, ptr %274, i16 16
  %292 = load i32, ptr %291, !tbaa !11
  %293 = mul i32 %292, 29
  %294 = add i32 %290, %293
  %295 = getelementptr inbounds i8, ptr %274, i16 20
  %296 = load i32, ptr %295, !tbaa !11
  %297 = mul i32 %296, 30
  %298 = add i32 %294, %297
  %299 = getelementptr inbounds i8, ptr %274, i16 24
  %300 = load i32, ptr %299, !tbaa !11
  %301 = mul i32 %300, 31
  %302 = add i32 %298, %301
  %303 = getelementptr inbounds i8, ptr %274, i16 28
  %304 = load i32, ptr %303, !tbaa !11
  %305 = mul i32 %304, 32
  %306 = add i32 %302, %305
  %307 = getelementptr inbounds i8, ptr %3, i16 128
  %308 = getelementptr inbounds i8, ptr %307, i16 0
  %309 = load i32, ptr %308, !tbaa !11
  %310 = mul i32 %309, 33
  %311 = add i32 %306, %310
  %312 = getelementptr inbounds i8, ptr %307, i16 4
  %313 = load i32, ptr %312, !tbaa !11
  %314 = mul i32 %313, 34
  %315 = add i32 %311, %314
  %316 = getelementptr inbounds i8, ptr %307, i16 8
  %317 = load i32, ptr %316, !tbaa !11
  %318 = mul i32 %317, 35
  %319 = add i32 %315, %318
  %320 = getelementptr inbounds i8, ptr %307, i16 12
  %321 = load i32, ptr %320, !tbaa !11
  %322 = mul i32 %321, 36
  %323 = add i32 %319, %322
  %324 = getelementptr inbounds i8, ptr %307, i16 16
  %325 = load i32, ptr %324, !tbaa !11
  %326 = mul i32 %325, 37
  %327 = add i32 %323, %326
  %328 = getelementptr inbounds i8, ptr %307, i16 20
  %329 = load i32, ptr %328, !tbaa !11
  %330 = mul i32 %329, 38
  %331 = add i32 %327, %330
  %332 = getelementptr inbounds i8, ptr %307, i16 24
  %333 = load i32, ptr %332, !tbaa !11
  %334 = mul i32 %333, 39
  %335 = add i32 %331, %334
  %336 = getelementptr inbounds i8, ptr %307, i16 28
  %337 = load i32, ptr %336, !tbaa !11
  %338 = mul i32 %337, 40
  %339 = add i32 %335, %338
  %340 = getelementptr inbounds i8, ptr %3, i16 160
  %341 = getelementptr inbounds i8, ptr %340, i16 0
  %342 = load i32, ptr %341, !tbaa !11
  %343 = mul i32 %342, 41
  %344 = add i32 %339, %343
  %345 = getelementptr inbounds i8, ptr %340, i16 4
  %346 = load i32, ptr %345, !tbaa !11
  %347 = mul i32 %346, 42
  %348 = add i32 %344, %347
  %349 = getelementptr inbounds i8, ptr %340, i16 8
  %350 = load i32, ptr %349, !tbaa !11
  %351 = mul i32 %350, 43
  %352 = add i32 %348, %351
  %353 = getelementptr inbounds i8, ptr %340, i16 12
  %354 = load i32, ptr %353, !tbaa !11
  %355 = mul i32 %354, 44
  %356 = add i32 %352, %355
  %357 = getelementptr inbounds i8, ptr %340, i16 16
  %358 = load i32, ptr %357, !tbaa !11
  %359 = mul i32 %358, 45
  %360 = add i32 %356, %359
  %361 = getelementptr inbounds i8, ptr %340, i16 20
  %362 = load i32, ptr %361, !tbaa !11
  %363 = mul i32 %362, 46
  %364 = add i32 %360, %363
  %365 = getelementptr inbounds i8, ptr %340, i16 24
  %366 = load i32, ptr %365, !tbaa !11
  %367 = mul i32 %366, 47
  %368 = add i32 %364, %367
  %369 = getelementptr inbounds i8, ptr %340, i16 28
  %370 = load i32, ptr %369, !tbaa !11
  %371 = mul i32 %370, 48
  %372 = add i32 %368, %371
  %373 = getelementptr inbounds i8, ptr %3, i16 192
  %374 = getelementptr inbounds i8, ptr %373, i16 0
  %375 = load i32, ptr %374, !tbaa !11
  %376 = mul i32 %375, 49
  %377 = add i32 %372, %376
  %378 = getelementptr inbounds i8, ptr %373, i16 4
  %379 = load i32, ptr %378, !tbaa !11
  %380 = mul i32 %379, 50
  %381 = add i32 %377, %380
  %382 = getelementptr inbounds i8, ptr %373, i16 8
  %383 = load i32, ptr %382, !tbaa !11
  %384 = mul i32 %383, 51
  %385 = add i32 %381, %384
  %386 = getelementptr inbounds i8, ptr %373, i16 12
  %387 = load i32, ptr %386, !tbaa !11
  %388 = mul i32 %387, 52
  %389 = add i32 %385, %388
  %390 = getelementptr inbounds i8, ptr %373, i16 16
  %391 = load i32, ptr %390, !tbaa !11
  %392 = mul i32 %391, 53
  %393 = add i32 %389, %392
  %394 = getelementptr inbounds i8, ptr %373, i16 20
  %395 = load i32, ptr %394, !tbaa !11
  %396 = mul i32 %395, 54
  %397 = add i32 %393, %396
  %398 = getelementptr inbounds i8, ptr %373, i16 24
  %399 = load i32, ptr %398, !tbaa !11
  %400 = mul i32 %399, 55
  %401 = add i32 %397, %400
  %402 = getelementptr inbounds i8, ptr %373, i16 28
  %403 = load i32, ptr %402, !tbaa !11
  %404 = mul i32 %403, 56
  %405 = add i32 %401, %404
  %406 = getelementptr inbounds i8, ptr %3, i16 224
  %407 = getelementptr inbounds i8, ptr %406, i16 0
  %408 = load i32, ptr %407, !tbaa !11
  %409 = mul i32 %408, 57
  %410 = add i32 %405, %409
  %411 = getelementptr inbounds i8, ptr %406, i16 4
  %412 = load i32, ptr %411, !tbaa !11
  %413 = mul i32 %412, 58
  %414 = add i32 %410, %413
  %415 = getelementptr inbounds i8, ptr %406, i16 8
  %416 = load i32, ptr %415, !tbaa !11
  %417 = mul i32 %416, 59
  %418 = add i32 %414, %417
  %419 = getelementptr inbounds i8, ptr %406, i16 12
  %420 = load i32, ptr %419, !tbaa !11
  %421 = mul i32 %420, 60
  %422 = add i32 %418, %421
  %423 = getelementptr inbounds i8, ptr %406, i16 16
  %424 = load i32, ptr %423, !tbaa !11
  %425 = mul i32 %424, 61
  %426 = add i32 %422, %425
  %427 = getelementptr inbounds i8, ptr %406, i16 20
  %428 = load i32, ptr %427, !tbaa !11
  %429 = mul i32 %428, 62
  %430 = add i32 %426, %429
  %431 = getelementptr inbounds i8, ptr %406, i16 24
  %432 = load i32, ptr %431, !tbaa !11
  %433 = mul i32 %432, 63
  %434 = add i32 %430, %433
  %435 = getelementptr inbounds i8, ptr %406, i16 28
  %436 = load i32, ptr %435, !tbaa !11
  %437 = mul i32 %436, 64
  %438 = add i32 %434, %437
  ret i32 %438

b13:
  %lsr.iv21 = phi i16 [ 0, %b1 ], [ %lsr.iv.next2, %b15 ]
  %439 = shl i16 %lsr.iv21, 1
  %440 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %441 = load i16, ptr %440, !tbaa !9
  %442 = sext i16 %441 to i32
  %443 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %444 = getelementptr i8, ptr %443, i16 2
  %445 = load i16, ptr %444, !tbaa !9
  %446 = sext i16 %445 to i32
  %447 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %448 = getelementptr i8, ptr %447, i16 4
  %449 = load i16, ptr %448, !tbaa !9
  %450 = sext i16 %449 to i32
  %451 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %452 = getelementptr i8, ptr %451, i16 6
  %453 = load i16, ptr %452, !tbaa !9
  %454 = sext i16 %453 to i32
  %455 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %456 = getelementptr i8, ptr %455, i16 8
  %457 = load i16, ptr %456, !tbaa !9
  %458 = sext i16 %457 to i32
  %459 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %460 = getelementptr i8, ptr %459, i16 10
  %461 = load i16, ptr %460, !tbaa !9
  %462 = sext i16 %461 to i32
  %463 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %464 = getelementptr i8, ptr %463, i16 12
  %465 = load i16, ptr %464, !tbaa !9
  %466 = sext i16 %465 to i32
  %467 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %468 = getelementptr i8, ptr %467, i16 14
  %469 = load i16, ptr %468, !tbaa !9
  %470 = sext i16 %469 to i32
  br label %b16

b15:
  %lsr.iv.next2 = add i16 %lsr.iv21, 16
  %471 = icmp ne i16 %lsr.iv.next2, 128
  br i1 %471, label %b13, label %b12

b16:
  %lsr.iv3 = phi i16 [ %439, %b13 ], [ %lsr.iv.next, %b16 ]
  %lsr.iv11 = phi i16 [ -16, %b13 ], [ %lsr.iv.next1, %b16 ]
  %472 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %473 = getelementptr i8, ptr %472, i16 16
  %474 = load i16, ptr %473, !tbaa !9
  %475 = sext i16 %474 to i32
  %476 = mul nsw i32 %442, %475
  %477 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %478 = getelementptr i8, ptr %477, i16 32
  %479 = load i16, ptr %478, !tbaa !9
  %480 = sext i16 %479 to i32
  %481 = mul nsw i32 %446, %480
  %482 = add nsw i32 %476, %481
  %483 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %484 = getelementptr i8, ptr %483, i16 48
  %485 = load i16, ptr %484, !tbaa !9
  %486 = sext i16 %485 to i32
  %487 = mul nsw i32 %450, %486
  %488 = add nsw i32 %482, %487
  %489 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %490 = getelementptr i8, ptr %489, i16 64
  %491 = load i16, ptr %490, !tbaa !9
  %492 = sext i16 %491 to i32
  %493 = mul nsw i32 %454, %492
  %494 = add nsw i32 %488, %493
  %495 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %496 = getelementptr i8, ptr %495, i16 80
  %497 = load i16, ptr %496, !tbaa !9
  %498 = sext i16 %497 to i32
  %499 = mul nsw i32 %458, %498
  %500 = add nsw i32 %494, %499
  %501 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %502 = getelementptr i8, ptr %501, i16 96
  %503 = load i16, ptr %502, !tbaa !9
  %504 = sext i16 %503 to i32
  %505 = mul nsw i32 %462, %504
  %506 = add nsw i32 %500, %505
  %507 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %508 = getelementptr i8, ptr %507, i16 112
  %509 = load i16, ptr %508, !tbaa !9
  %510 = sext i16 %509 to i32
  %511 = mul nsw i32 %466, %510
  %512 = add nsw i32 %506, %511
  %513 = getelementptr i8, ptr %2, i16 %lsr.iv11
  %514 = getelementptr i8, ptr %513, i16 128
  %515 = load i16, ptr %514, !tbaa !9
  %516 = sext i16 %515 to i32
  %517 = mul nsw i32 %470, %516
  %518 = add nsw i32 %512, %517
  %519 = getelementptr i8, ptr %3, i16 %lsr.iv3
  store i32 %518, ptr %519, !tbaa !11
  %lsr.iv.next = add i16 %lsr.iv3, 4
  %lsr.iv.next1 = add i16 %lsr.iv11, 2
  %520 = icmp ne i16 %lsr.iv.next1, 0
  br i1 %520, label %b16, label %b15
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"int4", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"int8", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"float4", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!"float8", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!"float10", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!"pointer2", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!"pointer4", !6, i64 0}
!23 = !{!22, !22, i64 0}
