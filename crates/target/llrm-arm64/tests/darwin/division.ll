; ModuleID = 'division.c'
source_filename = "division.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [8 x i8] c"i8 %lx\0A\00", align 1
@.str.1 = private unnamed_addr constant [8 x i8] c"u8 %lx\0A\00", align 1
@.str.2 = private unnamed_addr constant [9 x i8] c"i16 %lx\0A\00", align 1
@.str.3 = private unnamed_addr constant [9 x i8] c"u16 %lx\0A\00", align 1
@.str.4 = private unnamed_addr constant [9 x i8] c"i32 %lx\0A\00", align 1
@.str.5 = private unnamed_addr constant [9 x i8] c"u32 %lx\0A\00", align 1
@.str.6 = private unnamed_addr constant [9 x i8] c"i64 %lx\0A\00", align 1
@.str.7 = private unnamed_addr constant [9 x i8] c"u64 %lx\0A\00", align 1
@i8_values = internal constant [13 x i8] c"\00\01\FF\02\FE\07\F9d\9C\7F\807\C9", align 1
@u8_values = internal constant [10 x i8] c"\00\01\02\07d\7F\80\C8\FE\FF", align 1
@i16_values = internal constant [13 x i16] [i16 0, i16 1, i16 -1, i16 2, i16 -2, i16 7, i16 -7, i16 1000, i16 -1000, i16 32767, i16 -32768, i16 12345, i16 -12345], align 2
@u16_values = internal constant [9 x i16] [i16 0, i16 1, i16 2, i16 7, i16 1000, i16 32767, i16 -32768, i16 -1, i16 -25536], align 2
@i32_values = internal constant [14 x i32] [i32 0, i32 1, i32 -1, i32 2, i32 -2, i32 7, i32 -7, i32 1000, i32 -1000, i32 2147483647, i32 -2147483648, i32 123456789, i32 -123456789, i32 65536], align 4
@u32_values = internal constant [11 x i32] [i32 0, i32 1, i32 2, i32 7, i32 1000, i32 2147483647, i32 -2147483648, i32 -1, i32 -2, i32 123456789, i32 -1294967296], align 4
@i64_values = internal constant [14 x i64] [i64 0, i64 1, i64 -1, i64 2, i64 -2, i64 7, i64 -7, i64 1000, i64 -1000, i64 9223372036854775807, i64 -9223372036854775808, i64 1234567890123456789, i64 -1234567890123456789, i64 4294967296], align 8
@u64_values = internal constant [10 x i64] [i64 0, i64 1, i64 2, i64 7, i64 1000, i64 9223372036854775807, i64 -9223372036854775808, i64 -1, i64 -6101065172474983726, i64 4294967296], align 8

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i64 @i8_run()
  %3 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %2)
  %4 = call i64 @u8_run()
  %5 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i64 noundef %4)
  %6 = call i64 @i16_run()
  %7 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i64 noundef %6)
  %8 = call i64 @u16_run()
  %9 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, i64 noundef %8)
  %10 = call i64 @i32_run()
  %11 = call i32 (ptr, ...) @printf(ptr noundef @.str.4, i64 noundef %10)
  %12 = call i64 @u32_run()
  %13 = call i32 (ptr, ...) @printf(ptr noundef @.str.5, i64 noundef %12)
  %14 = call i64 @i64_run()
  %15 = call i32 (ptr, ...) @printf(ptr noundef @.str.6, i64 noundef %14)
  %16 = call i64 @u64_run()
  %17 = call i32 (ptr, ...) @printf(ptr noundef @.str.7, i64 noundef %16)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @i8_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i8, align 1
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %377, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 13
  br i1 %7, label %8, label %380

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [13 x i8], ptr @i8_values, i64 0, i64 %10
  %12 = load i8, ptr %11, align 1, !tbaa !13
  store i8 %12, ptr %3, align 1, !tbaa !13
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i8, ptr %3, align 1, !tbaa !13
  %16 = sext i8 %15 to i32
  %17 = sdiv i32 %16, 2
  %18 = sext i32 %17 to i64
  %19 = add i64 %14, %18
  store i64 %19, ptr %1, align 8, !tbaa !10
  %20 = load i64, ptr %1, align 8, !tbaa !10
  %21 = mul i64 %20, 31
  %22 = load i8, ptr %3, align 1, !tbaa !13
  %23 = sext i8 %22 to i32
  %24 = srem i32 %23, 2
  %25 = sext i32 %24 to i64
  %26 = add i64 %21, %25
  store i64 %26, ptr %1, align 8, !tbaa !10
  %27 = load i64, ptr %1, align 8, !tbaa !10
  %28 = mul i64 %27, 31
  %29 = load i8, ptr %3, align 1, !tbaa !13
  %30 = sext i8 %29 to i32
  %31 = sdiv i32 %30, 3
  %32 = sext i32 %31 to i64
  %33 = add i64 %28, %32
  store i64 %33, ptr %1, align 8, !tbaa !10
  %34 = load i64, ptr %1, align 8, !tbaa !10
  %35 = mul i64 %34, 31
  %36 = load i8, ptr %3, align 1, !tbaa !13
  %37 = sext i8 %36 to i32
  %38 = srem i32 %37, 3
  %39 = sext i32 %38 to i64
  %40 = add i64 %35, %39
  store i64 %40, ptr %1, align 8, !tbaa !10
  %41 = load i64, ptr %1, align 8, !tbaa !10
  %42 = mul i64 %41, 31
  %43 = load i8, ptr %3, align 1, !tbaa !13
  %44 = sext i8 %43 to i32
  %45 = sdiv i32 %44, -3
  %46 = sext i32 %45 to i64
  %47 = add i64 %42, %46
  store i64 %47, ptr %1, align 8, !tbaa !10
  %48 = load i64, ptr %1, align 8, !tbaa !10
  %49 = mul i64 %48, 31
  %50 = load i8, ptr %3, align 1, !tbaa !13
  %51 = sext i8 %50 to i32
  %52 = srem i32 %51, -3
  %53 = sext i32 %52 to i64
  %54 = add i64 %49, %53
  store i64 %54, ptr %1, align 8, !tbaa !10
  %55 = load i64, ptr %1, align 8, !tbaa !10
  %56 = mul i64 %55, 31
  %57 = load i8, ptr %3, align 1, !tbaa !13
  %58 = sext i8 %57 to i32
  %59 = sdiv i32 %58, 5
  %60 = sext i32 %59 to i64
  %61 = add i64 %56, %60
  store i64 %61, ptr %1, align 8, !tbaa !10
  %62 = load i64, ptr %1, align 8, !tbaa !10
  %63 = mul i64 %62, 31
  %64 = load i8, ptr %3, align 1, !tbaa !13
  %65 = sext i8 %64 to i32
  %66 = srem i32 %65, 5
  %67 = sext i32 %66 to i64
  %68 = add i64 %63, %67
  store i64 %68, ptr %1, align 8, !tbaa !10
  %69 = load i64, ptr %1, align 8, !tbaa !10
  %70 = mul i64 %69, 31
  %71 = load i8, ptr %3, align 1, !tbaa !13
  %72 = sext i8 %71 to i32
  %73 = sdiv i32 %72, 6
  %74 = sext i32 %73 to i64
  %75 = add i64 %70, %74
  store i64 %75, ptr %1, align 8, !tbaa !10
  %76 = load i64, ptr %1, align 8, !tbaa !10
  %77 = mul i64 %76, 31
  %78 = load i8, ptr %3, align 1, !tbaa !13
  %79 = sext i8 %78 to i32
  %80 = srem i32 %79, 6
  %81 = sext i32 %80 to i64
  %82 = add i64 %77, %81
  store i64 %82, ptr %1, align 8, !tbaa !10
  %83 = load i64, ptr %1, align 8, !tbaa !10
  %84 = mul i64 %83, 31
  %85 = load i8, ptr %3, align 1, !tbaa !13
  %86 = sext i8 %85 to i32
  %87 = sdiv i32 %86, 7
  %88 = sext i32 %87 to i64
  %89 = add i64 %84, %88
  store i64 %89, ptr %1, align 8, !tbaa !10
  %90 = load i64, ptr %1, align 8, !tbaa !10
  %91 = mul i64 %90, 31
  %92 = load i8, ptr %3, align 1, !tbaa !13
  %93 = sext i8 %92 to i32
  %94 = srem i32 %93, 7
  %95 = sext i32 %94 to i64
  %96 = add i64 %91, %95
  store i64 %96, ptr %1, align 8, !tbaa !10
  %97 = load i64, ptr %1, align 8, !tbaa !10
  %98 = mul i64 %97, 31
  %99 = load i8, ptr %3, align 1, !tbaa !13
  %100 = sext i8 %99 to i32
  %101 = sdiv i32 %100, -7
  %102 = sext i32 %101 to i64
  %103 = add i64 %98, %102
  store i64 %103, ptr %1, align 8, !tbaa !10
  %104 = load i64, ptr %1, align 8, !tbaa !10
  %105 = mul i64 %104, 31
  %106 = load i8, ptr %3, align 1, !tbaa !13
  %107 = sext i8 %106 to i32
  %108 = srem i32 %107, -7
  %109 = sext i32 %108 to i64
  %110 = add i64 %105, %109
  store i64 %110, ptr %1, align 8, !tbaa !10
  %111 = load i64, ptr %1, align 8, !tbaa !10
  %112 = mul i64 %111, 31
  %113 = load i8, ptr %3, align 1, !tbaa !13
  %114 = sext i8 %113 to i32
  %115 = sdiv i32 %114, 9
  %116 = sext i32 %115 to i64
  %117 = add i64 %112, %116
  store i64 %117, ptr %1, align 8, !tbaa !10
  %118 = load i64, ptr %1, align 8, !tbaa !10
  %119 = mul i64 %118, 31
  %120 = load i8, ptr %3, align 1, !tbaa !13
  %121 = sext i8 %120 to i32
  %122 = srem i32 %121, 9
  %123 = sext i32 %122 to i64
  %124 = add i64 %119, %123
  store i64 %124, ptr %1, align 8, !tbaa !10
  %125 = load i64, ptr %1, align 8, !tbaa !10
  %126 = mul i64 %125, 31
  %127 = load i8, ptr %3, align 1, !tbaa !13
  %128 = sext i8 %127 to i32
  %129 = sdiv i32 %128, 10
  %130 = sext i32 %129 to i64
  %131 = add i64 %126, %130
  store i64 %131, ptr %1, align 8, !tbaa !10
  %132 = load i64, ptr %1, align 8, !tbaa !10
  %133 = mul i64 %132, 31
  %134 = load i8, ptr %3, align 1, !tbaa !13
  %135 = sext i8 %134 to i32
  %136 = srem i32 %135, 10
  %137 = sext i32 %136 to i64
  %138 = add i64 %133, %137
  store i64 %138, ptr %1, align 8, !tbaa !10
  %139 = load i64, ptr %1, align 8, !tbaa !10
  %140 = mul i64 %139, 31
  %141 = load i8, ptr %3, align 1, !tbaa !13
  %142 = sext i8 %141 to i32
  %143 = sdiv i32 %142, -10
  %144 = sext i32 %143 to i64
  %145 = add i64 %140, %144
  store i64 %145, ptr %1, align 8, !tbaa !10
  %146 = load i64, ptr %1, align 8, !tbaa !10
  %147 = mul i64 %146, 31
  %148 = load i8, ptr %3, align 1, !tbaa !13
  %149 = sext i8 %148 to i32
  %150 = srem i32 %149, -10
  %151 = sext i32 %150 to i64
  %152 = add i64 %147, %151
  store i64 %152, ptr %1, align 8, !tbaa !10
  %153 = load i64, ptr %1, align 8, !tbaa !10
  %154 = mul i64 %153, 31
  %155 = load i8, ptr %3, align 1, !tbaa !13
  %156 = sext i8 %155 to i32
  %157 = sdiv i32 %156, 11
  %158 = sext i32 %157 to i64
  %159 = add i64 %154, %158
  store i64 %159, ptr %1, align 8, !tbaa !10
  %160 = load i64, ptr %1, align 8, !tbaa !10
  %161 = mul i64 %160, 31
  %162 = load i8, ptr %3, align 1, !tbaa !13
  %163 = sext i8 %162 to i32
  %164 = srem i32 %163, 11
  %165 = sext i32 %164 to i64
  %166 = add i64 %161, %165
  store i64 %166, ptr %1, align 8, !tbaa !10
  %167 = load i64, ptr %1, align 8, !tbaa !10
  %168 = mul i64 %167, 31
  %169 = load i8, ptr %3, align 1, !tbaa !13
  %170 = sext i8 %169 to i32
  %171 = sdiv i32 %170, 12
  %172 = sext i32 %171 to i64
  %173 = add i64 %168, %172
  store i64 %173, ptr %1, align 8, !tbaa !10
  %174 = load i64, ptr %1, align 8, !tbaa !10
  %175 = mul i64 %174, 31
  %176 = load i8, ptr %3, align 1, !tbaa !13
  %177 = sext i8 %176 to i32
  %178 = srem i32 %177, 12
  %179 = sext i32 %178 to i64
  %180 = add i64 %175, %179
  store i64 %180, ptr %1, align 8, !tbaa !10
  %181 = load i64, ptr %1, align 8, !tbaa !10
  %182 = mul i64 %181, 31
  %183 = load i8, ptr %3, align 1, !tbaa !13
  %184 = sext i8 %183 to i32
  %185 = sdiv i32 %184, 13
  %186 = sext i32 %185 to i64
  %187 = add i64 %182, %186
  store i64 %187, ptr %1, align 8, !tbaa !10
  %188 = load i64, ptr %1, align 8, !tbaa !10
  %189 = mul i64 %188, 31
  %190 = load i8, ptr %3, align 1, !tbaa !13
  %191 = sext i8 %190 to i32
  %192 = srem i32 %191, 13
  %193 = sext i32 %192 to i64
  %194 = add i64 %189, %193
  store i64 %194, ptr %1, align 8, !tbaa !10
  %195 = load i64, ptr %1, align 8, !tbaa !10
  %196 = mul i64 %195, 31
  %197 = load i8, ptr %3, align 1, !tbaa !13
  %198 = sext i8 %197 to i32
  %199 = sdiv i32 %198, 16
  %200 = sext i32 %199 to i64
  %201 = add i64 %196, %200
  store i64 %201, ptr %1, align 8, !tbaa !10
  %202 = load i64, ptr %1, align 8, !tbaa !10
  %203 = mul i64 %202, 31
  %204 = load i8, ptr %3, align 1, !tbaa !13
  %205 = sext i8 %204 to i32
  %206 = srem i32 %205, 16
  %207 = sext i32 %206 to i64
  %208 = add i64 %203, %207
  store i64 %208, ptr %1, align 8, !tbaa !10
  %209 = load i64, ptr %1, align 8, !tbaa !10
  %210 = mul i64 %209, 31
  %211 = load i8, ptr %3, align 1, !tbaa !13
  %212 = sext i8 %211 to i32
  %213 = sdiv i32 %212, -16
  %214 = sext i32 %213 to i64
  %215 = add i64 %210, %214
  store i64 %215, ptr %1, align 8, !tbaa !10
  %216 = load i64, ptr %1, align 8, !tbaa !10
  %217 = mul i64 %216, 31
  %218 = load i8, ptr %3, align 1, !tbaa !13
  %219 = sext i8 %218 to i32
  %220 = srem i32 %219, -16
  %221 = sext i32 %220 to i64
  %222 = add i64 %217, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i8, ptr %3, align 1, !tbaa !13
  %226 = sext i8 %225 to i32
  %227 = sdiv i32 %226, 25
  %228 = sext i32 %227 to i64
  %229 = add i64 %224, %228
  store i64 %229, ptr %1, align 8, !tbaa !10
  %230 = load i64, ptr %1, align 8, !tbaa !10
  %231 = mul i64 %230, 31
  %232 = load i8, ptr %3, align 1, !tbaa !13
  %233 = sext i8 %232 to i32
  %234 = srem i32 %233, 25
  %235 = sext i32 %234 to i64
  %236 = add i64 %231, %235
  store i64 %236, ptr %1, align 8, !tbaa !10
  %237 = load i64, ptr %1, align 8, !tbaa !10
  %238 = mul i64 %237, 31
  %239 = load i8, ptr %3, align 1, !tbaa !13
  %240 = sext i8 %239 to i32
  %241 = sdiv i32 %240, 100
  %242 = sext i32 %241 to i64
  %243 = add i64 %238, %242
  store i64 %243, ptr %1, align 8, !tbaa !10
  %244 = load i64, ptr %1, align 8, !tbaa !10
  %245 = mul i64 %244, 31
  %246 = load i8, ptr %3, align 1, !tbaa !13
  %247 = sext i8 %246 to i32
  %248 = srem i32 %247, 100
  %249 = sext i32 %248 to i64
  %250 = add i64 %245, %249
  store i64 %250, ptr %1, align 8, !tbaa !10
  %251 = load i64, ptr %1, align 8, !tbaa !10
  %252 = mul i64 %251, 31
  %253 = load i8, ptr %3, align 1, !tbaa !13
  %254 = sext i8 %253 to i32
  %255 = sdiv i32 %254, -100
  %256 = sext i32 %255 to i64
  %257 = add i64 %252, %256
  store i64 %257, ptr %1, align 8, !tbaa !10
  %258 = load i64, ptr %1, align 8, !tbaa !10
  %259 = mul i64 %258, 31
  %260 = load i8, ptr %3, align 1, !tbaa !13
  %261 = sext i8 %260 to i32
  %262 = srem i32 %261, -100
  %263 = sext i32 %262 to i64
  %264 = add i64 %259, %263
  store i64 %264, ptr %1, align 8, !tbaa !10
  %265 = load i64, ptr %1, align 8, !tbaa !10
  %266 = mul i64 %265, 31
  %267 = load i8, ptr %3, align 1, !tbaa !13
  %268 = sext i8 %267 to i32
  %269 = sdiv i32 %268, 125
  %270 = sext i32 %269 to i64
  %271 = add i64 %266, %270
  store i64 %271, ptr %1, align 8, !tbaa !10
  %272 = load i64, ptr %1, align 8, !tbaa !10
  %273 = mul i64 %272, 31
  %274 = load i8, ptr %3, align 1, !tbaa !13
  %275 = sext i8 %274 to i32
  %276 = srem i32 %275, 125
  %277 = sext i32 %276 to i64
  %278 = add i64 %273, %277
  store i64 %278, ptr %1, align 8, !tbaa !10
  %279 = load i64, ptr %1, align 8, !tbaa !10
  %280 = mul i64 %279, 31
  %281 = load i8, ptr %3, align 1, !tbaa !13
  %282 = sext i8 %281 to i32
  %283 = sdiv i32 %282, -1
  %284 = sext i32 %283 to i64
  %285 = add i64 %280, %284
  store i64 %285, ptr %1, align 8, !tbaa !10
  %286 = load i64, ptr %1, align 8, !tbaa !10
  %287 = mul i64 %286, 31
  %288 = load i8, ptr %3, align 1, !tbaa !13
  %289 = sext i8 %288 to i32
  %290 = srem i32 %289, -1
  %291 = sext i32 %290 to i64
  %292 = add i64 %287, %291
  store i64 %292, ptr %1, align 8, !tbaa !10
  %293 = load i64, ptr %1, align 8, !tbaa !10
  %294 = mul i64 %293, 31
  %295 = load i8, ptr %3, align 1, !tbaa !13
  %296 = sext i8 %295 to i32
  %297 = sdiv i32 %296, -128
  %298 = sext i32 %297 to i64
  %299 = add i64 %294, %298
  store i64 %299, ptr %1, align 8, !tbaa !10
  %300 = load i64, ptr %1, align 8, !tbaa !10
  %301 = mul i64 %300, 31
  %302 = load i8, ptr %3, align 1, !tbaa !13
  %303 = sext i8 %302 to i32
  %304 = srem i32 %303, -128
  %305 = sext i32 %304 to i64
  %306 = add i64 %301, %305
  store i64 %306, ptr %1, align 8, !tbaa !10
  %307 = load i64, ptr %1, align 8, !tbaa !10
  %308 = mul i64 %307, 31
  %309 = load i8, ptr %3, align 1, !tbaa !13
  %310 = sext i8 %309 to i32
  %311 = sdiv i32 %310, -127
  %312 = sext i32 %311 to i64
  %313 = add i64 %308, %312
  store i64 %313, ptr %1, align 8, !tbaa !10
  %314 = load i64, ptr %1, align 8, !tbaa !10
  %315 = mul i64 %314, 31
  %316 = load i8, ptr %3, align 1, !tbaa !13
  %317 = sext i8 %316 to i32
  %318 = srem i32 %317, -127
  %319 = sext i32 %318 to i64
  %320 = add i64 %315, %319
  store i64 %320, ptr %1, align 8, !tbaa !10
  %321 = load i64, ptr %1, align 8, !tbaa !10
  %322 = mul i64 %321, 31
  %323 = load i8, ptr %3, align 1, !tbaa !13
  %324 = sext i8 %323 to i32
  %325 = sdiv i32 %324, -24
  %326 = sext i32 %325 to i64
  %327 = add i64 %322, %326
  store i64 %327, ptr %1, align 8, !tbaa !10
  %328 = load i64, ptr %1, align 8, !tbaa !10
  %329 = mul i64 %328, 31
  %330 = load i8, ptr %3, align 1, !tbaa !13
  %331 = sext i8 %330 to i32
  %332 = srem i32 %331, -24
  %333 = sext i32 %332 to i64
  %334 = add i64 %329, %333
  store i64 %334, ptr %1, align 8, !tbaa !10
  %335 = load i64, ptr %1, align 8, !tbaa !10
  %336 = mul i64 %335, 31
  %337 = load i8, ptr %3, align 1, !tbaa !13
  %338 = sext i8 %337 to i32
  %339 = sdiv i32 %338, 24
  %340 = sext i32 %339 to i64
  %341 = add i64 %336, %340
  store i64 %341, ptr %1, align 8, !tbaa !10
  %342 = load i64, ptr %1, align 8, !tbaa !10
  %343 = mul i64 %342, 31
  %344 = load i8, ptr %3, align 1, !tbaa !13
  %345 = sext i8 %344 to i32
  %346 = srem i32 %345, 24
  %347 = sext i32 %346 to i64
  %348 = add i64 %343, %347
  store i64 %348, ptr %1, align 8, !tbaa !10
  %349 = load i64, ptr %1, align 8, !tbaa !10
  %350 = mul i64 %349, 31
  %351 = load i8, ptr %3, align 1, !tbaa !13
  %352 = sext i8 %351 to i32
  %353 = sdiv i32 %352, 16
  %354 = sext i32 %353 to i64
  %355 = add i64 %350, %354
  store i64 %355, ptr %1, align 8, !tbaa !10
  %356 = load i64, ptr %1, align 8, !tbaa !10
  %357 = mul i64 %356, 31
  %358 = load i8, ptr %3, align 1, !tbaa !13
  %359 = sext i8 %358 to i32
  %360 = srem i32 %359, 16
  %361 = sext i32 %360 to i64
  %362 = add i64 %357, %361
  store i64 %362, ptr %1, align 8, !tbaa !10
  %363 = load i64, ptr %1, align 8, !tbaa !10
  %364 = mul i64 %363, 31
  %365 = load i8, ptr %3, align 1, !tbaa !13
  %366 = sext i8 %365 to i32
  %367 = sdiv i32 %366, 1
  %368 = sext i32 %367 to i64
  %369 = add i64 %364, %368
  store i64 %369, ptr %1, align 8, !tbaa !10
  %370 = load i64, ptr %1, align 8, !tbaa !10
  %371 = mul i64 %370, 31
  %372 = load i8, ptr %3, align 1, !tbaa !13
  %373 = sext i8 %372 to i32
  %374 = srem i32 %373, 1
  %375 = sext i32 %374 to i64
  %376 = add i64 %371, %375
  store i64 %376, ptr %1, align 8, !tbaa !10
  br label %377

377:                                              ; preds = %8
  %378 = load i32, ptr %2, align 4, !tbaa !12
  %379 = add i32 %378, 1
  store i32 %379, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !14

380:                                              ; preds = %4
  %381 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %381
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @u8_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i8, align 1
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %251, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 10
  br i1 %7, label %8, label %254

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [10 x i8], ptr @u8_values, i64 0, i64 %10
  %12 = load i8, ptr %11, align 1, !tbaa !13
  store i8 %12, ptr %3, align 1, !tbaa !13
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i8, ptr %3, align 1, !tbaa !13
  %16 = zext i8 %15 to i32
  %17 = sdiv i32 %16, 2
  %18 = sext i32 %17 to i64
  %19 = add i64 %14, %18
  store i64 %19, ptr %1, align 8, !tbaa !10
  %20 = load i64, ptr %1, align 8, !tbaa !10
  %21 = mul i64 %20, 31
  %22 = load i8, ptr %3, align 1, !tbaa !13
  %23 = zext i8 %22 to i32
  %24 = srem i32 %23, 2
  %25 = sext i32 %24 to i64
  %26 = add i64 %21, %25
  store i64 %26, ptr %1, align 8, !tbaa !10
  %27 = load i64, ptr %1, align 8, !tbaa !10
  %28 = mul i64 %27, 31
  %29 = load i8, ptr %3, align 1, !tbaa !13
  %30 = zext i8 %29 to i32
  %31 = sdiv i32 %30, 3
  %32 = sext i32 %31 to i64
  %33 = add i64 %28, %32
  store i64 %33, ptr %1, align 8, !tbaa !10
  %34 = load i64, ptr %1, align 8, !tbaa !10
  %35 = mul i64 %34, 31
  %36 = load i8, ptr %3, align 1, !tbaa !13
  %37 = zext i8 %36 to i32
  %38 = srem i32 %37, 3
  %39 = sext i32 %38 to i64
  %40 = add i64 %35, %39
  store i64 %40, ptr %1, align 8, !tbaa !10
  %41 = load i64, ptr %1, align 8, !tbaa !10
  %42 = mul i64 %41, 31
  %43 = load i8, ptr %3, align 1, !tbaa !13
  %44 = zext i8 %43 to i32
  %45 = sdiv i32 %44, 5
  %46 = sext i32 %45 to i64
  %47 = add i64 %42, %46
  store i64 %47, ptr %1, align 8, !tbaa !10
  %48 = load i64, ptr %1, align 8, !tbaa !10
  %49 = mul i64 %48, 31
  %50 = load i8, ptr %3, align 1, !tbaa !13
  %51 = zext i8 %50 to i32
  %52 = srem i32 %51, 5
  %53 = sext i32 %52 to i64
  %54 = add i64 %49, %53
  store i64 %54, ptr %1, align 8, !tbaa !10
  %55 = load i64, ptr %1, align 8, !tbaa !10
  %56 = mul i64 %55, 31
  %57 = load i8, ptr %3, align 1, !tbaa !13
  %58 = zext i8 %57 to i32
  %59 = sdiv i32 %58, 6
  %60 = sext i32 %59 to i64
  %61 = add i64 %56, %60
  store i64 %61, ptr %1, align 8, !tbaa !10
  %62 = load i64, ptr %1, align 8, !tbaa !10
  %63 = mul i64 %62, 31
  %64 = load i8, ptr %3, align 1, !tbaa !13
  %65 = zext i8 %64 to i32
  %66 = srem i32 %65, 6
  %67 = sext i32 %66 to i64
  %68 = add i64 %63, %67
  store i64 %68, ptr %1, align 8, !tbaa !10
  %69 = load i64, ptr %1, align 8, !tbaa !10
  %70 = mul i64 %69, 31
  %71 = load i8, ptr %3, align 1, !tbaa !13
  %72 = zext i8 %71 to i32
  %73 = sdiv i32 %72, 7
  %74 = sext i32 %73 to i64
  %75 = add i64 %70, %74
  store i64 %75, ptr %1, align 8, !tbaa !10
  %76 = load i64, ptr %1, align 8, !tbaa !10
  %77 = mul i64 %76, 31
  %78 = load i8, ptr %3, align 1, !tbaa !13
  %79 = zext i8 %78 to i32
  %80 = srem i32 %79, 7
  %81 = sext i32 %80 to i64
  %82 = add i64 %77, %81
  store i64 %82, ptr %1, align 8, !tbaa !10
  %83 = load i64, ptr %1, align 8, !tbaa !10
  %84 = mul i64 %83, 31
  %85 = load i8, ptr %3, align 1, !tbaa !13
  %86 = zext i8 %85 to i32
  %87 = sdiv i32 %86, 9
  %88 = sext i32 %87 to i64
  %89 = add i64 %84, %88
  store i64 %89, ptr %1, align 8, !tbaa !10
  %90 = load i64, ptr %1, align 8, !tbaa !10
  %91 = mul i64 %90, 31
  %92 = load i8, ptr %3, align 1, !tbaa !13
  %93 = zext i8 %92 to i32
  %94 = srem i32 %93, 9
  %95 = sext i32 %94 to i64
  %96 = add i64 %91, %95
  store i64 %96, ptr %1, align 8, !tbaa !10
  %97 = load i64, ptr %1, align 8, !tbaa !10
  %98 = mul i64 %97, 31
  %99 = load i8, ptr %3, align 1, !tbaa !13
  %100 = zext i8 %99 to i32
  %101 = sdiv i32 %100, 10
  %102 = sext i32 %101 to i64
  %103 = add i64 %98, %102
  store i64 %103, ptr %1, align 8, !tbaa !10
  %104 = load i64, ptr %1, align 8, !tbaa !10
  %105 = mul i64 %104, 31
  %106 = load i8, ptr %3, align 1, !tbaa !13
  %107 = zext i8 %106 to i32
  %108 = srem i32 %107, 10
  %109 = sext i32 %108 to i64
  %110 = add i64 %105, %109
  store i64 %110, ptr %1, align 8, !tbaa !10
  %111 = load i64, ptr %1, align 8, !tbaa !10
  %112 = mul i64 %111, 31
  %113 = load i8, ptr %3, align 1, !tbaa !13
  %114 = zext i8 %113 to i32
  %115 = sdiv i32 %114, 11
  %116 = sext i32 %115 to i64
  %117 = add i64 %112, %116
  store i64 %117, ptr %1, align 8, !tbaa !10
  %118 = load i64, ptr %1, align 8, !tbaa !10
  %119 = mul i64 %118, 31
  %120 = load i8, ptr %3, align 1, !tbaa !13
  %121 = zext i8 %120 to i32
  %122 = srem i32 %121, 11
  %123 = sext i32 %122 to i64
  %124 = add i64 %119, %123
  store i64 %124, ptr %1, align 8, !tbaa !10
  %125 = load i64, ptr %1, align 8, !tbaa !10
  %126 = mul i64 %125, 31
  %127 = load i8, ptr %3, align 1, !tbaa !13
  %128 = zext i8 %127 to i32
  %129 = sdiv i32 %128, 12
  %130 = sext i32 %129 to i64
  %131 = add i64 %126, %130
  store i64 %131, ptr %1, align 8, !tbaa !10
  %132 = load i64, ptr %1, align 8, !tbaa !10
  %133 = mul i64 %132, 31
  %134 = load i8, ptr %3, align 1, !tbaa !13
  %135 = zext i8 %134 to i32
  %136 = srem i32 %135, 12
  %137 = sext i32 %136 to i64
  %138 = add i64 %133, %137
  store i64 %138, ptr %1, align 8, !tbaa !10
  %139 = load i64, ptr %1, align 8, !tbaa !10
  %140 = mul i64 %139, 31
  %141 = load i8, ptr %3, align 1, !tbaa !13
  %142 = zext i8 %141 to i32
  %143 = sdiv i32 %142, 13
  %144 = sext i32 %143 to i64
  %145 = add i64 %140, %144
  store i64 %145, ptr %1, align 8, !tbaa !10
  %146 = load i64, ptr %1, align 8, !tbaa !10
  %147 = mul i64 %146, 31
  %148 = load i8, ptr %3, align 1, !tbaa !13
  %149 = zext i8 %148 to i32
  %150 = srem i32 %149, 13
  %151 = sext i32 %150 to i64
  %152 = add i64 %147, %151
  store i64 %152, ptr %1, align 8, !tbaa !10
  %153 = load i64, ptr %1, align 8, !tbaa !10
  %154 = mul i64 %153, 31
  %155 = load i8, ptr %3, align 1, !tbaa !13
  %156 = zext i8 %155 to i32
  %157 = sdiv i32 %156, 15
  %158 = sext i32 %157 to i64
  %159 = add i64 %154, %158
  store i64 %159, ptr %1, align 8, !tbaa !10
  %160 = load i64, ptr %1, align 8, !tbaa !10
  %161 = mul i64 %160, 31
  %162 = load i8, ptr %3, align 1, !tbaa !13
  %163 = zext i8 %162 to i32
  %164 = srem i32 %163, 15
  %165 = sext i32 %164 to i64
  %166 = add i64 %161, %165
  store i64 %166, ptr %1, align 8, !tbaa !10
  %167 = load i64, ptr %1, align 8, !tbaa !10
  %168 = mul i64 %167, 31
  %169 = load i8, ptr %3, align 1, !tbaa !13
  %170 = zext i8 %169 to i32
  %171 = sdiv i32 %170, 16
  %172 = sext i32 %171 to i64
  %173 = add i64 %168, %172
  store i64 %173, ptr %1, align 8, !tbaa !10
  %174 = load i64, ptr %1, align 8, !tbaa !10
  %175 = mul i64 %174, 31
  %176 = load i8, ptr %3, align 1, !tbaa !13
  %177 = zext i8 %176 to i32
  %178 = srem i32 %177, 16
  %179 = sext i32 %178 to i64
  %180 = add i64 %175, %179
  store i64 %180, ptr %1, align 8, !tbaa !10
  %181 = load i64, ptr %1, align 8, !tbaa !10
  %182 = mul i64 %181, 31
  %183 = load i8, ptr %3, align 1, !tbaa !13
  %184 = zext i8 %183 to i32
  %185 = sdiv i32 %184, 25
  %186 = sext i32 %185 to i64
  %187 = add i64 %182, %186
  store i64 %187, ptr %1, align 8, !tbaa !10
  %188 = load i64, ptr %1, align 8, !tbaa !10
  %189 = mul i64 %188, 31
  %190 = load i8, ptr %3, align 1, !tbaa !13
  %191 = zext i8 %190 to i32
  %192 = srem i32 %191, 25
  %193 = sext i32 %192 to i64
  %194 = add i64 %189, %193
  store i64 %194, ptr %1, align 8, !tbaa !10
  %195 = load i64, ptr %1, align 8, !tbaa !10
  %196 = mul i64 %195, 31
  %197 = load i8, ptr %3, align 1, !tbaa !13
  %198 = zext i8 %197 to i32
  %199 = sdiv i32 %198, 60
  %200 = sext i32 %199 to i64
  %201 = add i64 %196, %200
  store i64 %201, ptr %1, align 8, !tbaa !10
  %202 = load i64, ptr %1, align 8, !tbaa !10
  %203 = mul i64 %202, 31
  %204 = load i8, ptr %3, align 1, !tbaa !13
  %205 = zext i8 %204 to i32
  %206 = srem i32 %205, 60
  %207 = sext i32 %206 to i64
  %208 = add i64 %203, %207
  store i64 %208, ptr %1, align 8, !tbaa !10
  %209 = load i64, ptr %1, align 8, !tbaa !10
  %210 = mul i64 %209, 31
  %211 = load i8, ptr %3, align 1, !tbaa !13
  %212 = zext i8 %211 to i32
  %213 = sdiv i32 %212, 100
  %214 = sext i32 %213 to i64
  %215 = add i64 %210, %214
  store i64 %215, ptr %1, align 8, !tbaa !10
  %216 = load i64, ptr %1, align 8, !tbaa !10
  %217 = mul i64 %216, 31
  %218 = load i8, ptr %3, align 1, !tbaa !13
  %219 = zext i8 %218 to i32
  %220 = srem i32 %219, 100
  %221 = sext i32 %220 to i64
  %222 = add i64 %217, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i8, ptr %3, align 1, !tbaa !13
  %226 = zext i8 %225 to i32
  %227 = sdiv i32 %226, 125
  %228 = sext i32 %227 to i64
  %229 = add i64 %224, %228
  store i64 %229, ptr %1, align 8, !tbaa !10
  %230 = load i64, ptr %1, align 8, !tbaa !10
  %231 = mul i64 %230, 31
  %232 = load i8, ptr %3, align 1, !tbaa !13
  %233 = zext i8 %232 to i32
  %234 = srem i32 %233, 125
  %235 = sext i32 %234 to i64
  %236 = add i64 %231, %235
  store i64 %236, ptr %1, align 8, !tbaa !10
  %237 = load i64, ptr %1, align 8, !tbaa !10
  %238 = mul i64 %237, 31
  %239 = load i8, ptr %3, align 1, !tbaa !13
  %240 = zext i8 %239 to i32
  %241 = sdiv i32 %240, 255
  %242 = sext i32 %241 to i64
  %243 = add i64 %238, %242
  store i64 %243, ptr %1, align 8, !tbaa !10
  %244 = load i64, ptr %1, align 8, !tbaa !10
  %245 = mul i64 %244, 31
  %246 = load i8, ptr %3, align 1, !tbaa !13
  %247 = zext i8 %246 to i32
  %248 = srem i32 %247, 255
  %249 = sext i32 %248 to i64
  %250 = add i64 %245, %249
  store i64 %250, ptr %1, align 8, !tbaa !10
  br label %251

251:                                              ; preds = %8
  %252 = load i32, ptr %2, align 4, !tbaa !12
  %253 = add i32 %252, 1
  store i32 %253, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !16

254:                                              ; preds = %4
  %255 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %255
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @i16_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i16, align 2
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %363, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 13
  br i1 %7, label %8, label %366

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [13 x i16], ptr @i16_values, i64 0, i64 %10
  %12 = load i16, ptr %11, align 2, !tbaa !17
  store i16 %12, ptr %3, align 2, !tbaa !17
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i16, ptr %3, align 2, !tbaa !17
  %16 = sext i16 %15 to i32
  %17 = sdiv i32 %16, 2
  %18 = sext i32 %17 to i64
  %19 = add i64 %14, %18
  store i64 %19, ptr %1, align 8, !tbaa !10
  %20 = load i64, ptr %1, align 8, !tbaa !10
  %21 = mul i64 %20, 31
  %22 = load i16, ptr %3, align 2, !tbaa !17
  %23 = sext i16 %22 to i32
  %24 = srem i32 %23, 2
  %25 = sext i32 %24 to i64
  %26 = add i64 %21, %25
  store i64 %26, ptr %1, align 8, !tbaa !10
  %27 = load i64, ptr %1, align 8, !tbaa !10
  %28 = mul i64 %27, 31
  %29 = load i16, ptr %3, align 2, !tbaa !17
  %30 = sext i16 %29 to i32
  %31 = sdiv i32 %30, 3
  %32 = sext i32 %31 to i64
  %33 = add i64 %28, %32
  store i64 %33, ptr %1, align 8, !tbaa !10
  %34 = load i64, ptr %1, align 8, !tbaa !10
  %35 = mul i64 %34, 31
  %36 = load i16, ptr %3, align 2, !tbaa !17
  %37 = sext i16 %36 to i32
  %38 = srem i32 %37, 3
  %39 = sext i32 %38 to i64
  %40 = add i64 %35, %39
  store i64 %40, ptr %1, align 8, !tbaa !10
  %41 = load i64, ptr %1, align 8, !tbaa !10
  %42 = mul i64 %41, 31
  %43 = load i16, ptr %3, align 2, !tbaa !17
  %44 = sext i16 %43 to i32
  %45 = sdiv i32 %44, -3
  %46 = sext i32 %45 to i64
  %47 = add i64 %42, %46
  store i64 %47, ptr %1, align 8, !tbaa !10
  %48 = load i64, ptr %1, align 8, !tbaa !10
  %49 = mul i64 %48, 31
  %50 = load i16, ptr %3, align 2, !tbaa !17
  %51 = sext i16 %50 to i32
  %52 = srem i32 %51, -3
  %53 = sext i32 %52 to i64
  %54 = add i64 %49, %53
  store i64 %54, ptr %1, align 8, !tbaa !10
  %55 = load i64, ptr %1, align 8, !tbaa !10
  %56 = mul i64 %55, 31
  %57 = load i16, ptr %3, align 2, !tbaa !17
  %58 = sext i16 %57 to i32
  %59 = sdiv i32 %58, 5
  %60 = sext i32 %59 to i64
  %61 = add i64 %56, %60
  store i64 %61, ptr %1, align 8, !tbaa !10
  %62 = load i64, ptr %1, align 8, !tbaa !10
  %63 = mul i64 %62, 31
  %64 = load i16, ptr %3, align 2, !tbaa !17
  %65 = sext i16 %64 to i32
  %66 = srem i32 %65, 5
  %67 = sext i32 %66 to i64
  %68 = add i64 %63, %67
  store i64 %68, ptr %1, align 8, !tbaa !10
  %69 = load i64, ptr %1, align 8, !tbaa !10
  %70 = mul i64 %69, 31
  %71 = load i16, ptr %3, align 2, !tbaa !17
  %72 = sext i16 %71 to i32
  %73 = sdiv i32 %72, 6
  %74 = sext i32 %73 to i64
  %75 = add i64 %70, %74
  store i64 %75, ptr %1, align 8, !tbaa !10
  %76 = load i64, ptr %1, align 8, !tbaa !10
  %77 = mul i64 %76, 31
  %78 = load i16, ptr %3, align 2, !tbaa !17
  %79 = sext i16 %78 to i32
  %80 = srem i32 %79, 6
  %81 = sext i32 %80 to i64
  %82 = add i64 %77, %81
  store i64 %82, ptr %1, align 8, !tbaa !10
  %83 = load i64, ptr %1, align 8, !tbaa !10
  %84 = mul i64 %83, 31
  %85 = load i16, ptr %3, align 2, !tbaa !17
  %86 = sext i16 %85 to i32
  %87 = sdiv i32 %86, 7
  %88 = sext i32 %87 to i64
  %89 = add i64 %84, %88
  store i64 %89, ptr %1, align 8, !tbaa !10
  %90 = load i64, ptr %1, align 8, !tbaa !10
  %91 = mul i64 %90, 31
  %92 = load i16, ptr %3, align 2, !tbaa !17
  %93 = sext i16 %92 to i32
  %94 = srem i32 %93, 7
  %95 = sext i32 %94 to i64
  %96 = add i64 %91, %95
  store i64 %96, ptr %1, align 8, !tbaa !10
  %97 = load i64, ptr %1, align 8, !tbaa !10
  %98 = mul i64 %97, 31
  %99 = load i16, ptr %3, align 2, !tbaa !17
  %100 = sext i16 %99 to i32
  %101 = sdiv i32 %100, -7
  %102 = sext i32 %101 to i64
  %103 = add i64 %98, %102
  store i64 %103, ptr %1, align 8, !tbaa !10
  %104 = load i64, ptr %1, align 8, !tbaa !10
  %105 = mul i64 %104, 31
  %106 = load i16, ptr %3, align 2, !tbaa !17
  %107 = sext i16 %106 to i32
  %108 = srem i32 %107, -7
  %109 = sext i32 %108 to i64
  %110 = add i64 %105, %109
  store i64 %110, ptr %1, align 8, !tbaa !10
  %111 = load i64, ptr %1, align 8, !tbaa !10
  %112 = mul i64 %111, 31
  %113 = load i16, ptr %3, align 2, !tbaa !17
  %114 = sext i16 %113 to i32
  %115 = sdiv i32 %114, 9
  %116 = sext i32 %115 to i64
  %117 = add i64 %112, %116
  store i64 %117, ptr %1, align 8, !tbaa !10
  %118 = load i64, ptr %1, align 8, !tbaa !10
  %119 = mul i64 %118, 31
  %120 = load i16, ptr %3, align 2, !tbaa !17
  %121 = sext i16 %120 to i32
  %122 = srem i32 %121, 9
  %123 = sext i32 %122 to i64
  %124 = add i64 %119, %123
  store i64 %124, ptr %1, align 8, !tbaa !10
  %125 = load i64, ptr %1, align 8, !tbaa !10
  %126 = mul i64 %125, 31
  %127 = load i16, ptr %3, align 2, !tbaa !17
  %128 = sext i16 %127 to i32
  %129 = sdiv i32 %128, 10
  %130 = sext i32 %129 to i64
  %131 = add i64 %126, %130
  store i64 %131, ptr %1, align 8, !tbaa !10
  %132 = load i64, ptr %1, align 8, !tbaa !10
  %133 = mul i64 %132, 31
  %134 = load i16, ptr %3, align 2, !tbaa !17
  %135 = sext i16 %134 to i32
  %136 = srem i32 %135, 10
  %137 = sext i32 %136 to i64
  %138 = add i64 %133, %137
  store i64 %138, ptr %1, align 8, !tbaa !10
  %139 = load i64, ptr %1, align 8, !tbaa !10
  %140 = mul i64 %139, 31
  %141 = load i16, ptr %3, align 2, !tbaa !17
  %142 = sext i16 %141 to i32
  %143 = sdiv i32 %142, -10
  %144 = sext i32 %143 to i64
  %145 = add i64 %140, %144
  store i64 %145, ptr %1, align 8, !tbaa !10
  %146 = load i64, ptr %1, align 8, !tbaa !10
  %147 = mul i64 %146, 31
  %148 = load i16, ptr %3, align 2, !tbaa !17
  %149 = sext i16 %148 to i32
  %150 = srem i32 %149, -10
  %151 = sext i32 %150 to i64
  %152 = add i64 %147, %151
  store i64 %152, ptr %1, align 8, !tbaa !10
  %153 = load i64, ptr %1, align 8, !tbaa !10
  %154 = mul i64 %153, 31
  %155 = load i16, ptr %3, align 2, !tbaa !17
  %156 = sext i16 %155 to i32
  %157 = sdiv i32 %156, 11
  %158 = sext i32 %157 to i64
  %159 = add i64 %154, %158
  store i64 %159, ptr %1, align 8, !tbaa !10
  %160 = load i64, ptr %1, align 8, !tbaa !10
  %161 = mul i64 %160, 31
  %162 = load i16, ptr %3, align 2, !tbaa !17
  %163 = sext i16 %162 to i32
  %164 = srem i32 %163, 11
  %165 = sext i32 %164 to i64
  %166 = add i64 %161, %165
  store i64 %166, ptr %1, align 8, !tbaa !10
  %167 = load i64, ptr %1, align 8, !tbaa !10
  %168 = mul i64 %167, 31
  %169 = load i16, ptr %3, align 2, !tbaa !17
  %170 = sext i16 %169 to i32
  %171 = sdiv i32 %170, 12
  %172 = sext i32 %171 to i64
  %173 = add i64 %168, %172
  store i64 %173, ptr %1, align 8, !tbaa !10
  %174 = load i64, ptr %1, align 8, !tbaa !10
  %175 = mul i64 %174, 31
  %176 = load i16, ptr %3, align 2, !tbaa !17
  %177 = sext i16 %176 to i32
  %178 = srem i32 %177, 12
  %179 = sext i32 %178 to i64
  %180 = add i64 %175, %179
  store i64 %180, ptr %1, align 8, !tbaa !10
  %181 = load i64, ptr %1, align 8, !tbaa !10
  %182 = mul i64 %181, 31
  %183 = load i16, ptr %3, align 2, !tbaa !17
  %184 = sext i16 %183 to i32
  %185 = sdiv i32 %184, 13
  %186 = sext i32 %185 to i64
  %187 = add i64 %182, %186
  store i64 %187, ptr %1, align 8, !tbaa !10
  %188 = load i64, ptr %1, align 8, !tbaa !10
  %189 = mul i64 %188, 31
  %190 = load i16, ptr %3, align 2, !tbaa !17
  %191 = sext i16 %190 to i32
  %192 = srem i32 %191, 13
  %193 = sext i32 %192 to i64
  %194 = add i64 %189, %193
  store i64 %194, ptr %1, align 8, !tbaa !10
  %195 = load i64, ptr %1, align 8, !tbaa !10
  %196 = mul i64 %195, 31
  %197 = load i16, ptr %3, align 2, !tbaa !17
  %198 = sext i16 %197 to i32
  %199 = sdiv i32 %198, 16
  %200 = sext i32 %199 to i64
  %201 = add i64 %196, %200
  store i64 %201, ptr %1, align 8, !tbaa !10
  %202 = load i64, ptr %1, align 8, !tbaa !10
  %203 = mul i64 %202, 31
  %204 = load i16, ptr %3, align 2, !tbaa !17
  %205 = sext i16 %204 to i32
  %206 = srem i32 %205, 16
  %207 = sext i32 %206 to i64
  %208 = add i64 %203, %207
  store i64 %208, ptr %1, align 8, !tbaa !10
  %209 = load i64, ptr %1, align 8, !tbaa !10
  %210 = mul i64 %209, 31
  %211 = load i16, ptr %3, align 2, !tbaa !17
  %212 = sext i16 %211 to i32
  %213 = sdiv i32 %212, -16
  %214 = sext i32 %213 to i64
  %215 = add i64 %210, %214
  store i64 %215, ptr %1, align 8, !tbaa !10
  %216 = load i64, ptr %1, align 8, !tbaa !10
  %217 = mul i64 %216, 31
  %218 = load i16, ptr %3, align 2, !tbaa !17
  %219 = sext i16 %218 to i32
  %220 = srem i32 %219, -16
  %221 = sext i32 %220 to i64
  %222 = add i64 %217, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i16, ptr %3, align 2, !tbaa !17
  %226 = sext i16 %225 to i32
  %227 = sdiv i32 %226, 25
  %228 = sext i32 %227 to i64
  %229 = add i64 %224, %228
  store i64 %229, ptr %1, align 8, !tbaa !10
  %230 = load i64, ptr %1, align 8, !tbaa !10
  %231 = mul i64 %230, 31
  %232 = load i16, ptr %3, align 2, !tbaa !17
  %233 = sext i16 %232 to i32
  %234 = srem i32 %233, 25
  %235 = sext i32 %234 to i64
  %236 = add i64 %231, %235
  store i64 %236, ptr %1, align 8, !tbaa !10
  %237 = load i64, ptr %1, align 8, !tbaa !10
  %238 = mul i64 %237, 31
  %239 = load i16, ptr %3, align 2, !tbaa !17
  %240 = sext i16 %239 to i32
  %241 = sdiv i32 %240, 100
  %242 = sext i32 %241 to i64
  %243 = add i64 %238, %242
  store i64 %243, ptr %1, align 8, !tbaa !10
  %244 = load i64, ptr %1, align 8, !tbaa !10
  %245 = mul i64 %244, 31
  %246 = load i16, ptr %3, align 2, !tbaa !17
  %247 = sext i16 %246 to i32
  %248 = srem i32 %247, 100
  %249 = sext i32 %248 to i64
  %250 = add i64 %245, %249
  store i64 %250, ptr %1, align 8, !tbaa !10
  %251 = load i64, ptr %1, align 8, !tbaa !10
  %252 = mul i64 %251, 31
  %253 = load i16, ptr %3, align 2, !tbaa !17
  %254 = sext i16 %253 to i32
  %255 = sdiv i32 %254, -100
  %256 = sext i32 %255 to i64
  %257 = add i64 %252, %256
  store i64 %257, ptr %1, align 8, !tbaa !10
  %258 = load i64, ptr %1, align 8, !tbaa !10
  %259 = mul i64 %258, 31
  %260 = load i16, ptr %3, align 2, !tbaa !17
  %261 = sext i16 %260 to i32
  %262 = srem i32 %261, -100
  %263 = sext i32 %262 to i64
  %264 = add i64 %259, %263
  store i64 %264, ptr %1, align 8, !tbaa !10
  %265 = load i64, ptr %1, align 8, !tbaa !10
  %266 = mul i64 %265, 31
  %267 = load i16, ptr %3, align 2, !tbaa !17
  %268 = sext i16 %267 to i32
  %269 = sdiv i32 %268, 125
  %270 = sext i32 %269 to i64
  %271 = add i64 %266, %270
  store i64 %271, ptr %1, align 8, !tbaa !10
  %272 = load i64, ptr %1, align 8, !tbaa !10
  %273 = mul i64 %272, 31
  %274 = load i16, ptr %3, align 2, !tbaa !17
  %275 = sext i16 %274 to i32
  %276 = srem i32 %275, 125
  %277 = sext i32 %276 to i64
  %278 = add i64 %273, %277
  store i64 %278, ptr %1, align 8, !tbaa !10
  %279 = load i64, ptr %1, align 8, !tbaa !10
  %280 = mul i64 %279, 31
  %281 = load i16, ptr %3, align 2, !tbaa !17
  %282 = sext i16 %281 to i32
  %283 = sdiv i32 %282, 255
  %284 = sext i32 %283 to i64
  %285 = add i64 %280, %284
  store i64 %285, ptr %1, align 8, !tbaa !10
  %286 = load i64, ptr %1, align 8, !tbaa !10
  %287 = mul i64 %286, 31
  %288 = load i16, ptr %3, align 2, !tbaa !17
  %289 = sext i16 %288 to i32
  %290 = srem i32 %289, 255
  %291 = sext i32 %290 to i64
  %292 = add i64 %287, %291
  store i64 %292, ptr %1, align 8, !tbaa !10
  %293 = load i64, ptr %1, align 8, !tbaa !10
  %294 = mul i64 %293, 31
  %295 = load i16, ptr %3, align 2, !tbaa !17
  %296 = sext i16 %295 to i32
  %297 = sdiv i32 %296, -256
  %298 = sext i32 %297 to i64
  %299 = add i64 %294, %298
  store i64 %299, ptr %1, align 8, !tbaa !10
  %300 = load i64, ptr %1, align 8, !tbaa !10
  %301 = mul i64 %300, 31
  %302 = load i16, ptr %3, align 2, !tbaa !17
  %303 = sext i16 %302 to i32
  %304 = srem i32 %303, -256
  %305 = sext i32 %304 to i64
  %306 = add i64 %301, %305
  store i64 %306, ptr %1, align 8, !tbaa !10
  %307 = load i64, ptr %1, align 8, !tbaa !10
  %308 = mul i64 %307, 31
  %309 = load i16, ptr %3, align 2, !tbaa !17
  %310 = sext i16 %309 to i32
  %311 = sdiv i32 %310, 641
  %312 = sext i32 %311 to i64
  %313 = add i64 %308, %312
  store i64 %313, ptr %1, align 8, !tbaa !10
  %314 = load i64, ptr %1, align 8, !tbaa !10
  %315 = mul i64 %314, 31
  %316 = load i16, ptr %3, align 2, !tbaa !17
  %317 = sext i16 %316 to i32
  %318 = srem i32 %317, 641
  %319 = sext i32 %318 to i64
  %320 = add i64 %315, %319
  store i64 %320, ptr %1, align 8, !tbaa !10
  %321 = load i64, ptr %1, align 8, !tbaa !10
  %322 = mul i64 %321, 31
  %323 = load i16, ptr %3, align 2, !tbaa !17
  %324 = sext i16 %323 to i32
  %325 = sdiv i32 %324, 1000
  %326 = sext i32 %325 to i64
  %327 = add i64 %322, %326
  store i64 %327, ptr %1, align 8, !tbaa !10
  %328 = load i64, ptr %1, align 8, !tbaa !10
  %329 = mul i64 %328, 31
  %330 = load i16, ptr %3, align 2, !tbaa !17
  %331 = sext i16 %330 to i32
  %332 = srem i32 %331, 1000
  %333 = sext i32 %332 to i64
  %334 = add i64 %329, %333
  store i64 %334, ptr %1, align 8, !tbaa !10
  %335 = load i64, ptr %1, align 8, !tbaa !10
  %336 = mul i64 %335, 31
  %337 = load i16, ptr %3, align 2, !tbaa !17
  %338 = sext i16 %337 to i32
  %339 = sdiv i32 %338, -1000
  %340 = sext i32 %339 to i64
  %341 = add i64 %336, %340
  store i64 %341, ptr %1, align 8, !tbaa !10
  %342 = load i64, ptr %1, align 8, !tbaa !10
  %343 = mul i64 %342, 31
  %344 = load i16, ptr %3, align 2, !tbaa !17
  %345 = sext i16 %344 to i32
  %346 = srem i32 %345, -1000
  %347 = sext i32 %346 to i64
  %348 = add i64 %343, %347
  store i64 %348, ptr %1, align 8, !tbaa !10
  %349 = load i64, ptr %1, align 8, !tbaa !10
  %350 = mul i64 %349, 31
  %351 = load i16, ptr %3, align 2, !tbaa !17
  %352 = sext i16 %351 to i32
  %353 = sdiv i32 %352, 3600
  %354 = sext i32 %353 to i64
  %355 = add i64 %350, %354
  store i64 %355, ptr %1, align 8, !tbaa !10
  %356 = load i64, ptr %1, align 8, !tbaa !10
  %357 = mul i64 %356, 31
  %358 = load i16, ptr %3, align 2, !tbaa !17
  %359 = sext i16 %358 to i32
  %360 = srem i32 %359, 3600
  %361 = sext i32 %360 to i64
  %362 = add i64 %357, %361
  store i64 %362, ptr %1, align 8, !tbaa !10
  br label %363

363:                                              ; preds = %8
  %364 = load i32, ptr %2, align 4, !tbaa !12
  %365 = add i32 %364, 1
  store i32 %365, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !19

366:                                              ; preds = %4
  %367 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %367
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @u16_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i16, align 2
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %321, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 9
  br i1 %7, label %8, label %324

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [9 x i16], ptr @u16_values, i64 0, i64 %10
  %12 = load i16, ptr %11, align 2, !tbaa !17
  store i16 %12, ptr %3, align 2, !tbaa !17
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i16, ptr %3, align 2, !tbaa !17
  %16 = zext i16 %15 to i32
  %17 = sdiv i32 %16, 2
  %18 = sext i32 %17 to i64
  %19 = add i64 %14, %18
  store i64 %19, ptr %1, align 8, !tbaa !10
  %20 = load i64, ptr %1, align 8, !tbaa !10
  %21 = mul i64 %20, 31
  %22 = load i16, ptr %3, align 2, !tbaa !17
  %23 = zext i16 %22 to i32
  %24 = srem i32 %23, 2
  %25 = sext i32 %24 to i64
  %26 = add i64 %21, %25
  store i64 %26, ptr %1, align 8, !tbaa !10
  %27 = load i64, ptr %1, align 8, !tbaa !10
  %28 = mul i64 %27, 31
  %29 = load i16, ptr %3, align 2, !tbaa !17
  %30 = zext i16 %29 to i32
  %31 = sdiv i32 %30, 3
  %32 = sext i32 %31 to i64
  %33 = add i64 %28, %32
  store i64 %33, ptr %1, align 8, !tbaa !10
  %34 = load i64, ptr %1, align 8, !tbaa !10
  %35 = mul i64 %34, 31
  %36 = load i16, ptr %3, align 2, !tbaa !17
  %37 = zext i16 %36 to i32
  %38 = srem i32 %37, 3
  %39 = sext i32 %38 to i64
  %40 = add i64 %35, %39
  store i64 %40, ptr %1, align 8, !tbaa !10
  %41 = load i64, ptr %1, align 8, !tbaa !10
  %42 = mul i64 %41, 31
  %43 = load i16, ptr %3, align 2, !tbaa !17
  %44 = zext i16 %43 to i32
  %45 = sdiv i32 %44, 5
  %46 = sext i32 %45 to i64
  %47 = add i64 %42, %46
  store i64 %47, ptr %1, align 8, !tbaa !10
  %48 = load i64, ptr %1, align 8, !tbaa !10
  %49 = mul i64 %48, 31
  %50 = load i16, ptr %3, align 2, !tbaa !17
  %51 = zext i16 %50 to i32
  %52 = srem i32 %51, 5
  %53 = sext i32 %52 to i64
  %54 = add i64 %49, %53
  store i64 %54, ptr %1, align 8, !tbaa !10
  %55 = load i64, ptr %1, align 8, !tbaa !10
  %56 = mul i64 %55, 31
  %57 = load i16, ptr %3, align 2, !tbaa !17
  %58 = zext i16 %57 to i32
  %59 = sdiv i32 %58, 6
  %60 = sext i32 %59 to i64
  %61 = add i64 %56, %60
  store i64 %61, ptr %1, align 8, !tbaa !10
  %62 = load i64, ptr %1, align 8, !tbaa !10
  %63 = mul i64 %62, 31
  %64 = load i16, ptr %3, align 2, !tbaa !17
  %65 = zext i16 %64 to i32
  %66 = srem i32 %65, 6
  %67 = sext i32 %66 to i64
  %68 = add i64 %63, %67
  store i64 %68, ptr %1, align 8, !tbaa !10
  %69 = load i64, ptr %1, align 8, !tbaa !10
  %70 = mul i64 %69, 31
  %71 = load i16, ptr %3, align 2, !tbaa !17
  %72 = zext i16 %71 to i32
  %73 = sdiv i32 %72, 7
  %74 = sext i32 %73 to i64
  %75 = add i64 %70, %74
  store i64 %75, ptr %1, align 8, !tbaa !10
  %76 = load i64, ptr %1, align 8, !tbaa !10
  %77 = mul i64 %76, 31
  %78 = load i16, ptr %3, align 2, !tbaa !17
  %79 = zext i16 %78 to i32
  %80 = srem i32 %79, 7
  %81 = sext i32 %80 to i64
  %82 = add i64 %77, %81
  store i64 %82, ptr %1, align 8, !tbaa !10
  %83 = load i64, ptr %1, align 8, !tbaa !10
  %84 = mul i64 %83, 31
  %85 = load i16, ptr %3, align 2, !tbaa !17
  %86 = zext i16 %85 to i32
  %87 = sdiv i32 %86, 9
  %88 = sext i32 %87 to i64
  %89 = add i64 %84, %88
  store i64 %89, ptr %1, align 8, !tbaa !10
  %90 = load i64, ptr %1, align 8, !tbaa !10
  %91 = mul i64 %90, 31
  %92 = load i16, ptr %3, align 2, !tbaa !17
  %93 = zext i16 %92 to i32
  %94 = srem i32 %93, 9
  %95 = sext i32 %94 to i64
  %96 = add i64 %91, %95
  store i64 %96, ptr %1, align 8, !tbaa !10
  %97 = load i64, ptr %1, align 8, !tbaa !10
  %98 = mul i64 %97, 31
  %99 = load i16, ptr %3, align 2, !tbaa !17
  %100 = zext i16 %99 to i32
  %101 = sdiv i32 %100, 10
  %102 = sext i32 %101 to i64
  %103 = add i64 %98, %102
  store i64 %103, ptr %1, align 8, !tbaa !10
  %104 = load i64, ptr %1, align 8, !tbaa !10
  %105 = mul i64 %104, 31
  %106 = load i16, ptr %3, align 2, !tbaa !17
  %107 = zext i16 %106 to i32
  %108 = srem i32 %107, 10
  %109 = sext i32 %108 to i64
  %110 = add i64 %105, %109
  store i64 %110, ptr %1, align 8, !tbaa !10
  %111 = load i64, ptr %1, align 8, !tbaa !10
  %112 = mul i64 %111, 31
  %113 = load i16, ptr %3, align 2, !tbaa !17
  %114 = zext i16 %113 to i32
  %115 = sdiv i32 %114, 11
  %116 = sext i32 %115 to i64
  %117 = add i64 %112, %116
  store i64 %117, ptr %1, align 8, !tbaa !10
  %118 = load i64, ptr %1, align 8, !tbaa !10
  %119 = mul i64 %118, 31
  %120 = load i16, ptr %3, align 2, !tbaa !17
  %121 = zext i16 %120 to i32
  %122 = srem i32 %121, 11
  %123 = sext i32 %122 to i64
  %124 = add i64 %119, %123
  store i64 %124, ptr %1, align 8, !tbaa !10
  %125 = load i64, ptr %1, align 8, !tbaa !10
  %126 = mul i64 %125, 31
  %127 = load i16, ptr %3, align 2, !tbaa !17
  %128 = zext i16 %127 to i32
  %129 = sdiv i32 %128, 12
  %130 = sext i32 %129 to i64
  %131 = add i64 %126, %130
  store i64 %131, ptr %1, align 8, !tbaa !10
  %132 = load i64, ptr %1, align 8, !tbaa !10
  %133 = mul i64 %132, 31
  %134 = load i16, ptr %3, align 2, !tbaa !17
  %135 = zext i16 %134 to i32
  %136 = srem i32 %135, 12
  %137 = sext i32 %136 to i64
  %138 = add i64 %133, %137
  store i64 %138, ptr %1, align 8, !tbaa !10
  %139 = load i64, ptr %1, align 8, !tbaa !10
  %140 = mul i64 %139, 31
  %141 = load i16, ptr %3, align 2, !tbaa !17
  %142 = zext i16 %141 to i32
  %143 = sdiv i32 %142, 13
  %144 = sext i32 %143 to i64
  %145 = add i64 %140, %144
  store i64 %145, ptr %1, align 8, !tbaa !10
  %146 = load i64, ptr %1, align 8, !tbaa !10
  %147 = mul i64 %146, 31
  %148 = load i16, ptr %3, align 2, !tbaa !17
  %149 = zext i16 %148 to i32
  %150 = srem i32 %149, 13
  %151 = sext i32 %150 to i64
  %152 = add i64 %147, %151
  store i64 %152, ptr %1, align 8, !tbaa !10
  %153 = load i64, ptr %1, align 8, !tbaa !10
  %154 = mul i64 %153, 31
  %155 = load i16, ptr %3, align 2, !tbaa !17
  %156 = zext i16 %155 to i32
  %157 = sdiv i32 %156, 15
  %158 = sext i32 %157 to i64
  %159 = add i64 %154, %158
  store i64 %159, ptr %1, align 8, !tbaa !10
  %160 = load i64, ptr %1, align 8, !tbaa !10
  %161 = mul i64 %160, 31
  %162 = load i16, ptr %3, align 2, !tbaa !17
  %163 = zext i16 %162 to i32
  %164 = srem i32 %163, 15
  %165 = sext i32 %164 to i64
  %166 = add i64 %161, %165
  store i64 %166, ptr %1, align 8, !tbaa !10
  %167 = load i64, ptr %1, align 8, !tbaa !10
  %168 = mul i64 %167, 31
  %169 = load i16, ptr %3, align 2, !tbaa !17
  %170 = zext i16 %169 to i32
  %171 = sdiv i32 %170, 16
  %172 = sext i32 %171 to i64
  %173 = add i64 %168, %172
  store i64 %173, ptr %1, align 8, !tbaa !10
  %174 = load i64, ptr %1, align 8, !tbaa !10
  %175 = mul i64 %174, 31
  %176 = load i16, ptr %3, align 2, !tbaa !17
  %177 = zext i16 %176 to i32
  %178 = srem i32 %177, 16
  %179 = sext i32 %178 to i64
  %180 = add i64 %175, %179
  store i64 %180, ptr %1, align 8, !tbaa !10
  %181 = load i64, ptr %1, align 8, !tbaa !10
  %182 = mul i64 %181, 31
  %183 = load i16, ptr %3, align 2, !tbaa !17
  %184 = zext i16 %183 to i32
  %185 = sdiv i32 %184, 25
  %186 = sext i32 %185 to i64
  %187 = add i64 %182, %186
  store i64 %187, ptr %1, align 8, !tbaa !10
  %188 = load i64, ptr %1, align 8, !tbaa !10
  %189 = mul i64 %188, 31
  %190 = load i16, ptr %3, align 2, !tbaa !17
  %191 = zext i16 %190 to i32
  %192 = srem i32 %191, 25
  %193 = sext i32 %192 to i64
  %194 = add i64 %189, %193
  store i64 %194, ptr %1, align 8, !tbaa !10
  %195 = load i64, ptr %1, align 8, !tbaa !10
  %196 = mul i64 %195, 31
  %197 = load i16, ptr %3, align 2, !tbaa !17
  %198 = zext i16 %197 to i32
  %199 = sdiv i32 %198, 60
  %200 = sext i32 %199 to i64
  %201 = add i64 %196, %200
  store i64 %201, ptr %1, align 8, !tbaa !10
  %202 = load i64, ptr %1, align 8, !tbaa !10
  %203 = mul i64 %202, 31
  %204 = load i16, ptr %3, align 2, !tbaa !17
  %205 = zext i16 %204 to i32
  %206 = srem i32 %205, 60
  %207 = sext i32 %206 to i64
  %208 = add i64 %203, %207
  store i64 %208, ptr %1, align 8, !tbaa !10
  %209 = load i64, ptr %1, align 8, !tbaa !10
  %210 = mul i64 %209, 31
  %211 = load i16, ptr %3, align 2, !tbaa !17
  %212 = zext i16 %211 to i32
  %213 = sdiv i32 %212, 100
  %214 = sext i32 %213 to i64
  %215 = add i64 %210, %214
  store i64 %215, ptr %1, align 8, !tbaa !10
  %216 = load i64, ptr %1, align 8, !tbaa !10
  %217 = mul i64 %216, 31
  %218 = load i16, ptr %3, align 2, !tbaa !17
  %219 = zext i16 %218 to i32
  %220 = srem i32 %219, 100
  %221 = sext i32 %220 to i64
  %222 = add i64 %217, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i16, ptr %3, align 2, !tbaa !17
  %226 = zext i16 %225 to i32
  %227 = sdiv i32 %226, 125
  %228 = sext i32 %227 to i64
  %229 = add i64 %224, %228
  store i64 %229, ptr %1, align 8, !tbaa !10
  %230 = load i64, ptr %1, align 8, !tbaa !10
  %231 = mul i64 %230, 31
  %232 = load i16, ptr %3, align 2, !tbaa !17
  %233 = zext i16 %232 to i32
  %234 = srem i32 %233, 125
  %235 = sext i32 %234 to i64
  %236 = add i64 %231, %235
  store i64 %236, ptr %1, align 8, !tbaa !10
  %237 = load i64, ptr %1, align 8, !tbaa !10
  %238 = mul i64 %237, 31
  %239 = load i16, ptr %3, align 2, !tbaa !17
  %240 = zext i16 %239 to i32
  %241 = sdiv i32 %240, 255
  %242 = sext i32 %241 to i64
  %243 = add i64 %238, %242
  store i64 %243, ptr %1, align 8, !tbaa !10
  %244 = load i64, ptr %1, align 8, !tbaa !10
  %245 = mul i64 %244, 31
  %246 = load i16, ptr %3, align 2, !tbaa !17
  %247 = zext i16 %246 to i32
  %248 = srem i32 %247, 255
  %249 = sext i32 %248 to i64
  %250 = add i64 %245, %249
  store i64 %250, ptr %1, align 8, !tbaa !10
  %251 = load i64, ptr %1, align 8, !tbaa !10
  %252 = mul i64 %251, 31
  %253 = load i16, ptr %3, align 2, !tbaa !17
  %254 = zext i16 %253 to i32
  %255 = sdiv i32 %254, 256
  %256 = sext i32 %255 to i64
  %257 = add i64 %252, %256
  store i64 %257, ptr %1, align 8, !tbaa !10
  %258 = load i64, ptr %1, align 8, !tbaa !10
  %259 = mul i64 %258, 31
  %260 = load i16, ptr %3, align 2, !tbaa !17
  %261 = zext i16 %260 to i32
  %262 = srem i32 %261, 256
  %263 = sext i32 %262 to i64
  %264 = add i64 %259, %263
  store i64 %264, ptr %1, align 8, !tbaa !10
  %265 = load i64, ptr %1, align 8, !tbaa !10
  %266 = mul i64 %265, 31
  %267 = load i16, ptr %3, align 2, !tbaa !17
  %268 = zext i16 %267 to i32
  %269 = sdiv i32 %268, 641
  %270 = sext i32 %269 to i64
  %271 = add i64 %266, %270
  store i64 %271, ptr %1, align 8, !tbaa !10
  %272 = load i64, ptr %1, align 8, !tbaa !10
  %273 = mul i64 %272, 31
  %274 = load i16, ptr %3, align 2, !tbaa !17
  %275 = zext i16 %274 to i32
  %276 = srem i32 %275, 641
  %277 = sext i32 %276 to i64
  %278 = add i64 %273, %277
  store i64 %278, ptr %1, align 8, !tbaa !10
  %279 = load i64, ptr %1, align 8, !tbaa !10
  %280 = mul i64 %279, 31
  %281 = load i16, ptr %3, align 2, !tbaa !17
  %282 = zext i16 %281 to i32
  %283 = sdiv i32 %282, 1000
  %284 = sext i32 %283 to i64
  %285 = add i64 %280, %284
  store i64 %285, ptr %1, align 8, !tbaa !10
  %286 = load i64, ptr %1, align 8, !tbaa !10
  %287 = mul i64 %286, 31
  %288 = load i16, ptr %3, align 2, !tbaa !17
  %289 = zext i16 %288 to i32
  %290 = srem i32 %289, 1000
  %291 = sext i32 %290 to i64
  %292 = add i64 %287, %291
  store i64 %292, ptr %1, align 8, !tbaa !10
  %293 = load i64, ptr %1, align 8, !tbaa !10
  %294 = mul i64 %293, 31
  %295 = load i16, ptr %3, align 2, !tbaa !17
  %296 = zext i16 %295 to i32
  %297 = sdiv i32 %296, 3600
  %298 = sext i32 %297 to i64
  %299 = add i64 %294, %298
  store i64 %299, ptr %1, align 8, !tbaa !10
  %300 = load i64, ptr %1, align 8, !tbaa !10
  %301 = mul i64 %300, 31
  %302 = load i16, ptr %3, align 2, !tbaa !17
  %303 = zext i16 %302 to i32
  %304 = srem i32 %303, 3600
  %305 = sext i32 %304 to i64
  %306 = add i64 %301, %305
  store i64 %306, ptr %1, align 8, !tbaa !10
  %307 = load i64, ptr %1, align 8, !tbaa !10
  %308 = mul i64 %307, 31
  %309 = load i16, ptr %3, align 2, !tbaa !17
  %310 = zext i16 %309 to i32
  %311 = sdiv i32 %310, 65535
  %312 = sext i32 %311 to i64
  %313 = add i64 %308, %312
  store i64 %313, ptr %1, align 8, !tbaa !10
  %314 = load i64, ptr %1, align 8, !tbaa !10
  %315 = mul i64 %314, 31
  %316 = load i16, ptr %3, align 2, !tbaa !17
  %317 = zext i16 %316 to i32
  %318 = srem i32 %317, 65535
  %319 = sext i32 %318 to i64
  %320 = add i64 %315, %319
  store i64 %320, ptr %1, align 8, !tbaa !10
  br label %321

321:                                              ; preds = %8
  %322 = load i32, ptr %2, align 4, !tbaa !12
  %323 = add i32 %322, 1
  store i32 %323, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !20

324:                                              ; preds = %4
  %325 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %325
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @i32_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %325, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 14
  br i1 %7, label %8, label %328

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [14 x i32], ptr @i32_values, i64 0, i64 %10
  %12 = load i32, ptr %11, align 4, !tbaa !12
  store i32 %12, ptr %3, align 4, !tbaa !12
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i32, ptr %3, align 4, !tbaa !12
  %16 = sdiv i32 %15, 2
  %17 = sext i32 %16 to i64
  %18 = add i64 %14, %17
  store i64 %18, ptr %1, align 8, !tbaa !10
  %19 = load i64, ptr %1, align 8, !tbaa !10
  %20 = mul i64 %19, 31
  %21 = load i32, ptr %3, align 4, !tbaa !12
  %22 = srem i32 %21, 2
  %23 = sext i32 %22 to i64
  %24 = add i64 %20, %23
  store i64 %24, ptr %1, align 8, !tbaa !10
  %25 = load i64, ptr %1, align 8, !tbaa !10
  %26 = mul i64 %25, 31
  %27 = load i32, ptr %3, align 4, !tbaa !12
  %28 = sdiv i32 %27, 3
  %29 = sext i32 %28 to i64
  %30 = add i64 %26, %29
  store i64 %30, ptr %1, align 8, !tbaa !10
  %31 = load i64, ptr %1, align 8, !tbaa !10
  %32 = mul i64 %31, 31
  %33 = load i32, ptr %3, align 4, !tbaa !12
  %34 = srem i32 %33, 3
  %35 = sext i32 %34 to i64
  %36 = add i64 %32, %35
  store i64 %36, ptr %1, align 8, !tbaa !10
  %37 = load i64, ptr %1, align 8, !tbaa !10
  %38 = mul i64 %37, 31
  %39 = load i32, ptr %3, align 4, !tbaa !12
  %40 = sdiv i32 %39, -3
  %41 = sext i32 %40 to i64
  %42 = add i64 %38, %41
  store i64 %42, ptr %1, align 8, !tbaa !10
  %43 = load i64, ptr %1, align 8, !tbaa !10
  %44 = mul i64 %43, 31
  %45 = load i32, ptr %3, align 4, !tbaa !12
  %46 = srem i32 %45, -3
  %47 = sext i32 %46 to i64
  %48 = add i64 %44, %47
  store i64 %48, ptr %1, align 8, !tbaa !10
  %49 = load i64, ptr %1, align 8, !tbaa !10
  %50 = mul i64 %49, 31
  %51 = load i32, ptr %3, align 4, !tbaa !12
  %52 = sdiv i32 %51, 5
  %53 = sext i32 %52 to i64
  %54 = add i64 %50, %53
  store i64 %54, ptr %1, align 8, !tbaa !10
  %55 = load i64, ptr %1, align 8, !tbaa !10
  %56 = mul i64 %55, 31
  %57 = load i32, ptr %3, align 4, !tbaa !12
  %58 = srem i32 %57, 5
  %59 = sext i32 %58 to i64
  %60 = add i64 %56, %59
  store i64 %60, ptr %1, align 8, !tbaa !10
  %61 = load i64, ptr %1, align 8, !tbaa !10
  %62 = mul i64 %61, 31
  %63 = load i32, ptr %3, align 4, !tbaa !12
  %64 = sdiv i32 %63, 6
  %65 = sext i32 %64 to i64
  %66 = add i64 %62, %65
  store i64 %66, ptr %1, align 8, !tbaa !10
  %67 = load i64, ptr %1, align 8, !tbaa !10
  %68 = mul i64 %67, 31
  %69 = load i32, ptr %3, align 4, !tbaa !12
  %70 = srem i32 %69, 6
  %71 = sext i32 %70 to i64
  %72 = add i64 %68, %71
  store i64 %72, ptr %1, align 8, !tbaa !10
  %73 = load i64, ptr %1, align 8, !tbaa !10
  %74 = mul i64 %73, 31
  %75 = load i32, ptr %3, align 4, !tbaa !12
  %76 = sdiv i32 %75, 7
  %77 = sext i32 %76 to i64
  %78 = add i64 %74, %77
  store i64 %78, ptr %1, align 8, !tbaa !10
  %79 = load i64, ptr %1, align 8, !tbaa !10
  %80 = mul i64 %79, 31
  %81 = load i32, ptr %3, align 4, !tbaa !12
  %82 = srem i32 %81, 7
  %83 = sext i32 %82 to i64
  %84 = add i64 %80, %83
  store i64 %84, ptr %1, align 8, !tbaa !10
  %85 = load i64, ptr %1, align 8, !tbaa !10
  %86 = mul i64 %85, 31
  %87 = load i32, ptr %3, align 4, !tbaa !12
  %88 = sdiv i32 %87, -7
  %89 = sext i32 %88 to i64
  %90 = add i64 %86, %89
  store i64 %90, ptr %1, align 8, !tbaa !10
  %91 = load i64, ptr %1, align 8, !tbaa !10
  %92 = mul i64 %91, 31
  %93 = load i32, ptr %3, align 4, !tbaa !12
  %94 = srem i32 %93, -7
  %95 = sext i32 %94 to i64
  %96 = add i64 %92, %95
  store i64 %96, ptr %1, align 8, !tbaa !10
  %97 = load i64, ptr %1, align 8, !tbaa !10
  %98 = mul i64 %97, 31
  %99 = load i32, ptr %3, align 4, !tbaa !12
  %100 = sdiv i32 %99, 9
  %101 = sext i32 %100 to i64
  %102 = add i64 %98, %101
  store i64 %102, ptr %1, align 8, !tbaa !10
  %103 = load i64, ptr %1, align 8, !tbaa !10
  %104 = mul i64 %103, 31
  %105 = load i32, ptr %3, align 4, !tbaa !12
  %106 = srem i32 %105, 9
  %107 = sext i32 %106 to i64
  %108 = add i64 %104, %107
  store i64 %108, ptr %1, align 8, !tbaa !10
  %109 = load i64, ptr %1, align 8, !tbaa !10
  %110 = mul i64 %109, 31
  %111 = load i32, ptr %3, align 4, !tbaa !12
  %112 = sdiv i32 %111, 10
  %113 = sext i32 %112 to i64
  %114 = add i64 %110, %113
  store i64 %114, ptr %1, align 8, !tbaa !10
  %115 = load i64, ptr %1, align 8, !tbaa !10
  %116 = mul i64 %115, 31
  %117 = load i32, ptr %3, align 4, !tbaa !12
  %118 = srem i32 %117, 10
  %119 = sext i32 %118 to i64
  %120 = add i64 %116, %119
  store i64 %120, ptr %1, align 8, !tbaa !10
  %121 = load i64, ptr %1, align 8, !tbaa !10
  %122 = mul i64 %121, 31
  %123 = load i32, ptr %3, align 4, !tbaa !12
  %124 = sdiv i32 %123, -10
  %125 = sext i32 %124 to i64
  %126 = add i64 %122, %125
  store i64 %126, ptr %1, align 8, !tbaa !10
  %127 = load i64, ptr %1, align 8, !tbaa !10
  %128 = mul i64 %127, 31
  %129 = load i32, ptr %3, align 4, !tbaa !12
  %130 = srem i32 %129, -10
  %131 = sext i32 %130 to i64
  %132 = add i64 %128, %131
  store i64 %132, ptr %1, align 8, !tbaa !10
  %133 = load i64, ptr %1, align 8, !tbaa !10
  %134 = mul i64 %133, 31
  %135 = load i32, ptr %3, align 4, !tbaa !12
  %136 = sdiv i32 %135, 11
  %137 = sext i32 %136 to i64
  %138 = add i64 %134, %137
  store i64 %138, ptr %1, align 8, !tbaa !10
  %139 = load i64, ptr %1, align 8, !tbaa !10
  %140 = mul i64 %139, 31
  %141 = load i32, ptr %3, align 4, !tbaa !12
  %142 = srem i32 %141, 11
  %143 = sext i32 %142 to i64
  %144 = add i64 %140, %143
  store i64 %144, ptr %1, align 8, !tbaa !10
  %145 = load i64, ptr %1, align 8, !tbaa !10
  %146 = mul i64 %145, 31
  %147 = load i32, ptr %3, align 4, !tbaa !12
  %148 = sdiv i32 %147, 12
  %149 = sext i32 %148 to i64
  %150 = add i64 %146, %149
  store i64 %150, ptr %1, align 8, !tbaa !10
  %151 = load i64, ptr %1, align 8, !tbaa !10
  %152 = mul i64 %151, 31
  %153 = load i32, ptr %3, align 4, !tbaa !12
  %154 = srem i32 %153, 12
  %155 = sext i32 %154 to i64
  %156 = add i64 %152, %155
  store i64 %156, ptr %1, align 8, !tbaa !10
  %157 = load i64, ptr %1, align 8, !tbaa !10
  %158 = mul i64 %157, 31
  %159 = load i32, ptr %3, align 4, !tbaa !12
  %160 = sdiv i32 %159, 13
  %161 = sext i32 %160 to i64
  %162 = add i64 %158, %161
  store i64 %162, ptr %1, align 8, !tbaa !10
  %163 = load i64, ptr %1, align 8, !tbaa !10
  %164 = mul i64 %163, 31
  %165 = load i32, ptr %3, align 4, !tbaa !12
  %166 = srem i32 %165, 13
  %167 = sext i32 %166 to i64
  %168 = add i64 %164, %167
  store i64 %168, ptr %1, align 8, !tbaa !10
  %169 = load i64, ptr %1, align 8, !tbaa !10
  %170 = mul i64 %169, 31
  %171 = load i32, ptr %3, align 4, !tbaa !12
  %172 = sdiv i32 %171, 16
  %173 = sext i32 %172 to i64
  %174 = add i64 %170, %173
  store i64 %174, ptr %1, align 8, !tbaa !10
  %175 = load i64, ptr %1, align 8, !tbaa !10
  %176 = mul i64 %175, 31
  %177 = load i32, ptr %3, align 4, !tbaa !12
  %178 = srem i32 %177, 16
  %179 = sext i32 %178 to i64
  %180 = add i64 %176, %179
  store i64 %180, ptr %1, align 8, !tbaa !10
  %181 = load i64, ptr %1, align 8, !tbaa !10
  %182 = mul i64 %181, 31
  %183 = load i32, ptr %3, align 4, !tbaa !12
  %184 = sdiv i32 %183, -16
  %185 = sext i32 %184 to i64
  %186 = add i64 %182, %185
  store i64 %186, ptr %1, align 8, !tbaa !10
  %187 = load i64, ptr %1, align 8, !tbaa !10
  %188 = mul i64 %187, 31
  %189 = load i32, ptr %3, align 4, !tbaa !12
  %190 = srem i32 %189, -16
  %191 = sext i32 %190 to i64
  %192 = add i64 %188, %191
  store i64 %192, ptr %1, align 8, !tbaa !10
  %193 = load i64, ptr %1, align 8, !tbaa !10
  %194 = mul i64 %193, 31
  %195 = load i32, ptr %3, align 4, !tbaa !12
  %196 = sdiv i32 %195, 25
  %197 = sext i32 %196 to i64
  %198 = add i64 %194, %197
  store i64 %198, ptr %1, align 8, !tbaa !10
  %199 = load i64, ptr %1, align 8, !tbaa !10
  %200 = mul i64 %199, 31
  %201 = load i32, ptr %3, align 4, !tbaa !12
  %202 = srem i32 %201, 25
  %203 = sext i32 %202 to i64
  %204 = add i64 %200, %203
  store i64 %204, ptr %1, align 8, !tbaa !10
  %205 = load i64, ptr %1, align 8, !tbaa !10
  %206 = mul i64 %205, 31
  %207 = load i32, ptr %3, align 4, !tbaa !12
  %208 = sdiv i32 %207, 100
  %209 = sext i32 %208 to i64
  %210 = add i64 %206, %209
  store i64 %210, ptr %1, align 8, !tbaa !10
  %211 = load i64, ptr %1, align 8, !tbaa !10
  %212 = mul i64 %211, 31
  %213 = load i32, ptr %3, align 4, !tbaa !12
  %214 = srem i32 %213, 100
  %215 = sext i32 %214 to i64
  %216 = add i64 %212, %215
  store i64 %216, ptr %1, align 8, !tbaa !10
  %217 = load i64, ptr %1, align 8, !tbaa !10
  %218 = mul i64 %217, 31
  %219 = load i32, ptr %3, align 4, !tbaa !12
  %220 = sdiv i32 %219, -100
  %221 = sext i32 %220 to i64
  %222 = add i64 %218, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i32, ptr %3, align 4, !tbaa !12
  %226 = srem i32 %225, -100
  %227 = sext i32 %226 to i64
  %228 = add i64 %224, %227
  store i64 %228, ptr %1, align 8, !tbaa !10
  %229 = load i64, ptr %1, align 8, !tbaa !10
  %230 = mul i64 %229, 31
  %231 = load i32, ptr %3, align 4, !tbaa !12
  %232 = sdiv i32 %231, 125
  %233 = sext i32 %232 to i64
  %234 = add i64 %230, %233
  store i64 %234, ptr %1, align 8, !tbaa !10
  %235 = load i64, ptr %1, align 8, !tbaa !10
  %236 = mul i64 %235, 31
  %237 = load i32, ptr %3, align 4, !tbaa !12
  %238 = srem i32 %237, 125
  %239 = sext i32 %238 to i64
  %240 = add i64 %236, %239
  store i64 %240, ptr %1, align 8, !tbaa !10
  %241 = load i64, ptr %1, align 8, !tbaa !10
  %242 = mul i64 %241, 31
  %243 = load i32, ptr %3, align 4, !tbaa !12
  %244 = sdiv i32 %243, 255
  %245 = sext i32 %244 to i64
  %246 = add i64 %242, %245
  store i64 %246, ptr %1, align 8, !tbaa !10
  %247 = load i64, ptr %1, align 8, !tbaa !10
  %248 = mul i64 %247, 31
  %249 = load i32, ptr %3, align 4, !tbaa !12
  %250 = srem i32 %249, 255
  %251 = sext i32 %250 to i64
  %252 = add i64 %248, %251
  store i64 %252, ptr %1, align 8, !tbaa !10
  %253 = load i64, ptr %1, align 8, !tbaa !10
  %254 = mul i64 %253, 31
  %255 = load i32, ptr %3, align 4, !tbaa !12
  %256 = sdiv i32 %255, -256
  %257 = sext i32 %256 to i64
  %258 = add i64 %254, %257
  store i64 %258, ptr %1, align 8, !tbaa !10
  %259 = load i64, ptr %1, align 8, !tbaa !10
  %260 = mul i64 %259, 31
  %261 = load i32, ptr %3, align 4, !tbaa !12
  %262 = srem i32 %261, -256
  %263 = sext i32 %262 to i64
  %264 = add i64 %260, %263
  store i64 %264, ptr %1, align 8, !tbaa !10
  %265 = load i64, ptr %1, align 8, !tbaa !10
  %266 = mul i64 %265, 31
  %267 = load i32, ptr %3, align 4, !tbaa !12
  %268 = sdiv i32 %267, 641
  %269 = sext i32 %268 to i64
  %270 = add i64 %266, %269
  store i64 %270, ptr %1, align 8, !tbaa !10
  %271 = load i64, ptr %1, align 8, !tbaa !10
  %272 = mul i64 %271, 31
  %273 = load i32, ptr %3, align 4, !tbaa !12
  %274 = srem i32 %273, 641
  %275 = sext i32 %274 to i64
  %276 = add i64 %272, %275
  store i64 %276, ptr %1, align 8, !tbaa !10
  %277 = load i64, ptr %1, align 8, !tbaa !10
  %278 = mul i64 %277, 31
  %279 = load i32, ptr %3, align 4, !tbaa !12
  %280 = sdiv i32 %279, 1000
  %281 = sext i32 %280 to i64
  %282 = add i64 %278, %281
  store i64 %282, ptr %1, align 8, !tbaa !10
  %283 = load i64, ptr %1, align 8, !tbaa !10
  %284 = mul i64 %283, 31
  %285 = load i32, ptr %3, align 4, !tbaa !12
  %286 = srem i32 %285, 1000
  %287 = sext i32 %286 to i64
  %288 = add i64 %284, %287
  store i64 %288, ptr %1, align 8, !tbaa !10
  %289 = load i64, ptr %1, align 8, !tbaa !10
  %290 = mul i64 %289, 31
  %291 = load i32, ptr %3, align 4, !tbaa !12
  %292 = sdiv i32 %291, -1000
  %293 = sext i32 %292 to i64
  %294 = add i64 %290, %293
  store i64 %294, ptr %1, align 8, !tbaa !10
  %295 = load i64, ptr %1, align 8, !tbaa !10
  %296 = mul i64 %295, 31
  %297 = load i32, ptr %3, align 4, !tbaa !12
  %298 = srem i32 %297, -1000
  %299 = sext i32 %298 to i64
  %300 = add i64 %296, %299
  store i64 %300, ptr %1, align 8, !tbaa !10
  %301 = load i64, ptr %1, align 8, !tbaa !10
  %302 = mul i64 %301, 31
  %303 = load i32, ptr %3, align 4, !tbaa !12
  %304 = sdiv i32 %303, 3600
  %305 = sext i32 %304 to i64
  %306 = add i64 %302, %305
  store i64 %306, ptr %1, align 8, !tbaa !10
  %307 = load i64, ptr %1, align 8, !tbaa !10
  %308 = mul i64 %307, 31
  %309 = load i32, ptr %3, align 4, !tbaa !12
  %310 = srem i32 %309, 3600
  %311 = sext i32 %310 to i64
  %312 = add i64 %308, %311
  store i64 %312, ptr %1, align 8, !tbaa !10
  %313 = load i64, ptr %1, align 8, !tbaa !10
  %314 = mul i64 %313, 31
  %315 = load i32, ptr %3, align 4, !tbaa !12
  %316 = sdiv i32 %315, 65537
  %317 = sext i32 %316 to i64
  %318 = add i64 %314, %317
  store i64 %318, ptr %1, align 8, !tbaa !10
  %319 = load i64, ptr %1, align 8, !tbaa !10
  %320 = mul i64 %319, 31
  %321 = load i32, ptr %3, align 4, !tbaa !12
  %322 = srem i32 %321, 65537
  %323 = sext i32 %322 to i64
  %324 = add i64 %320, %323
  store i64 %324, ptr %1, align 8, !tbaa !10
  br label %325

325:                                              ; preds = %8
  %326 = load i32, ptr %2, align 4, !tbaa !12
  %327 = add i32 %326, 1
  store i32 %327, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !21

328:                                              ; preds = %4
  %329 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %329
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @u32_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %301, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 11
  br i1 %7, label %8, label %304

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [11 x i32], ptr @u32_values, i64 0, i64 %10
  %12 = load i32, ptr %11, align 4, !tbaa !12
  store i32 %12, ptr %3, align 4, !tbaa !12
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i32, ptr %3, align 4, !tbaa !12
  %16 = udiv i32 %15, 2
  %17 = zext i32 %16 to i64
  %18 = add i64 %14, %17
  store i64 %18, ptr %1, align 8, !tbaa !10
  %19 = load i64, ptr %1, align 8, !tbaa !10
  %20 = mul i64 %19, 31
  %21 = load i32, ptr %3, align 4, !tbaa !12
  %22 = urem i32 %21, 2
  %23 = zext i32 %22 to i64
  %24 = add i64 %20, %23
  store i64 %24, ptr %1, align 8, !tbaa !10
  %25 = load i64, ptr %1, align 8, !tbaa !10
  %26 = mul i64 %25, 31
  %27 = load i32, ptr %3, align 4, !tbaa !12
  %28 = udiv i32 %27, 3
  %29 = zext i32 %28 to i64
  %30 = add i64 %26, %29
  store i64 %30, ptr %1, align 8, !tbaa !10
  %31 = load i64, ptr %1, align 8, !tbaa !10
  %32 = mul i64 %31, 31
  %33 = load i32, ptr %3, align 4, !tbaa !12
  %34 = urem i32 %33, 3
  %35 = zext i32 %34 to i64
  %36 = add i64 %32, %35
  store i64 %36, ptr %1, align 8, !tbaa !10
  %37 = load i64, ptr %1, align 8, !tbaa !10
  %38 = mul i64 %37, 31
  %39 = load i32, ptr %3, align 4, !tbaa !12
  %40 = udiv i32 %39, 5
  %41 = zext i32 %40 to i64
  %42 = add i64 %38, %41
  store i64 %42, ptr %1, align 8, !tbaa !10
  %43 = load i64, ptr %1, align 8, !tbaa !10
  %44 = mul i64 %43, 31
  %45 = load i32, ptr %3, align 4, !tbaa !12
  %46 = urem i32 %45, 5
  %47 = zext i32 %46 to i64
  %48 = add i64 %44, %47
  store i64 %48, ptr %1, align 8, !tbaa !10
  %49 = load i64, ptr %1, align 8, !tbaa !10
  %50 = mul i64 %49, 31
  %51 = load i32, ptr %3, align 4, !tbaa !12
  %52 = udiv i32 %51, 6
  %53 = zext i32 %52 to i64
  %54 = add i64 %50, %53
  store i64 %54, ptr %1, align 8, !tbaa !10
  %55 = load i64, ptr %1, align 8, !tbaa !10
  %56 = mul i64 %55, 31
  %57 = load i32, ptr %3, align 4, !tbaa !12
  %58 = urem i32 %57, 6
  %59 = zext i32 %58 to i64
  %60 = add i64 %56, %59
  store i64 %60, ptr %1, align 8, !tbaa !10
  %61 = load i64, ptr %1, align 8, !tbaa !10
  %62 = mul i64 %61, 31
  %63 = load i32, ptr %3, align 4, !tbaa !12
  %64 = udiv i32 %63, 7
  %65 = zext i32 %64 to i64
  %66 = add i64 %62, %65
  store i64 %66, ptr %1, align 8, !tbaa !10
  %67 = load i64, ptr %1, align 8, !tbaa !10
  %68 = mul i64 %67, 31
  %69 = load i32, ptr %3, align 4, !tbaa !12
  %70 = urem i32 %69, 7
  %71 = zext i32 %70 to i64
  %72 = add i64 %68, %71
  store i64 %72, ptr %1, align 8, !tbaa !10
  %73 = load i64, ptr %1, align 8, !tbaa !10
  %74 = mul i64 %73, 31
  %75 = load i32, ptr %3, align 4, !tbaa !12
  %76 = udiv i32 %75, 9
  %77 = zext i32 %76 to i64
  %78 = add i64 %74, %77
  store i64 %78, ptr %1, align 8, !tbaa !10
  %79 = load i64, ptr %1, align 8, !tbaa !10
  %80 = mul i64 %79, 31
  %81 = load i32, ptr %3, align 4, !tbaa !12
  %82 = urem i32 %81, 9
  %83 = zext i32 %82 to i64
  %84 = add i64 %80, %83
  store i64 %84, ptr %1, align 8, !tbaa !10
  %85 = load i64, ptr %1, align 8, !tbaa !10
  %86 = mul i64 %85, 31
  %87 = load i32, ptr %3, align 4, !tbaa !12
  %88 = udiv i32 %87, 10
  %89 = zext i32 %88 to i64
  %90 = add i64 %86, %89
  store i64 %90, ptr %1, align 8, !tbaa !10
  %91 = load i64, ptr %1, align 8, !tbaa !10
  %92 = mul i64 %91, 31
  %93 = load i32, ptr %3, align 4, !tbaa !12
  %94 = urem i32 %93, 10
  %95 = zext i32 %94 to i64
  %96 = add i64 %92, %95
  store i64 %96, ptr %1, align 8, !tbaa !10
  %97 = load i64, ptr %1, align 8, !tbaa !10
  %98 = mul i64 %97, 31
  %99 = load i32, ptr %3, align 4, !tbaa !12
  %100 = udiv i32 %99, 11
  %101 = zext i32 %100 to i64
  %102 = add i64 %98, %101
  store i64 %102, ptr %1, align 8, !tbaa !10
  %103 = load i64, ptr %1, align 8, !tbaa !10
  %104 = mul i64 %103, 31
  %105 = load i32, ptr %3, align 4, !tbaa !12
  %106 = urem i32 %105, 11
  %107 = zext i32 %106 to i64
  %108 = add i64 %104, %107
  store i64 %108, ptr %1, align 8, !tbaa !10
  %109 = load i64, ptr %1, align 8, !tbaa !10
  %110 = mul i64 %109, 31
  %111 = load i32, ptr %3, align 4, !tbaa !12
  %112 = udiv i32 %111, 12
  %113 = zext i32 %112 to i64
  %114 = add i64 %110, %113
  store i64 %114, ptr %1, align 8, !tbaa !10
  %115 = load i64, ptr %1, align 8, !tbaa !10
  %116 = mul i64 %115, 31
  %117 = load i32, ptr %3, align 4, !tbaa !12
  %118 = urem i32 %117, 12
  %119 = zext i32 %118 to i64
  %120 = add i64 %116, %119
  store i64 %120, ptr %1, align 8, !tbaa !10
  %121 = load i64, ptr %1, align 8, !tbaa !10
  %122 = mul i64 %121, 31
  %123 = load i32, ptr %3, align 4, !tbaa !12
  %124 = udiv i32 %123, 13
  %125 = zext i32 %124 to i64
  %126 = add i64 %122, %125
  store i64 %126, ptr %1, align 8, !tbaa !10
  %127 = load i64, ptr %1, align 8, !tbaa !10
  %128 = mul i64 %127, 31
  %129 = load i32, ptr %3, align 4, !tbaa !12
  %130 = urem i32 %129, 13
  %131 = zext i32 %130 to i64
  %132 = add i64 %128, %131
  store i64 %132, ptr %1, align 8, !tbaa !10
  %133 = load i64, ptr %1, align 8, !tbaa !10
  %134 = mul i64 %133, 31
  %135 = load i32, ptr %3, align 4, !tbaa !12
  %136 = udiv i32 %135, 15
  %137 = zext i32 %136 to i64
  %138 = add i64 %134, %137
  store i64 %138, ptr %1, align 8, !tbaa !10
  %139 = load i64, ptr %1, align 8, !tbaa !10
  %140 = mul i64 %139, 31
  %141 = load i32, ptr %3, align 4, !tbaa !12
  %142 = urem i32 %141, 15
  %143 = zext i32 %142 to i64
  %144 = add i64 %140, %143
  store i64 %144, ptr %1, align 8, !tbaa !10
  %145 = load i64, ptr %1, align 8, !tbaa !10
  %146 = mul i64 %145, 31
  %147 = load i32, ptr %3, align 4, !tbaa !12
  %148 = udiv i32 %147, 16
  %149 = zext i32 %148 to i64
  %150 = add i64 %146, %149
  store i64 %150, ptr %1, align 8, !tbaa !10
  %151 = load i64, ptr %1, align 8, !tbaa !10
  %152 = mul i64 %151, 31
  %153 = load i32, ptr %3, align 4, !tbaa !12
  %154 = urem i32 %153, 16
  %155 = zext i32 %154 to i64
  %156 = add i64 %152, %155
  store i64 %156, ptr %1, align 8, !tbaa !10
  %157 = load i64, ptr %1, align 8, !tbaa !10
  %158 = mul i64 %157, 31
  %159 = load i32, ptr %3, align 4, !tbaa !12
  %160 = udiv i32 %159, 25
  %161 = zext i32 %160 to i64
  %162 = add i64 %158, %161
  store i64 %162, ptr %1, align 8, !tbaa !10
  %163 = load i64, ptr %1, align 8, !tbaa !10
  %164 = mul i64 %163, 31
  %165 = load i32, ptr %3, align 4, !tbaa !12
  %166 = urem i32 %165, 25
  %167 = zext i32 %166 to i64
  %168 = add i64 %164, %167
  store i64 %168, ptr %1, align 8, !tbaa !10
  %169 = load i64, ptr %1, align 8, !tbaa !10
  %170 = mul i64 %169, 31
  %171 = load i32, ptr %3, align 4, !tbaa !12
  %172 = udiv i32 %171, 60
  %173 = zext i32 %172 to i64
  %174 = add i64 %170, %173
  store i64 %174, ptr %1, align 8, !tbaa !10
  %175 = load i64, ptr %1, align 8, !tbaa !10
  %176 = mul i64 %175, 31
  %177 = load i32, ptr %3, align 4, !tbaa !12
  %178 = urem i32 %177, 60
  %179 = zext i32 %178 to i64
  %180 = add i64 %176, %179
  store i64 %180, ptr %1, align 8, !tbaa !10
  %181 = load i64, ptr %1, align 8, !tbaa !10
  %182 = mul i64 %181, 31
  %183 = load i32, ptr %3, align 4, !tbaa !12
  %184 = udiv i32 %183, 100
  %185 = zext i32 %184 to i64
  %186 = add i64 %182, %185
  store i64 %186, ptr %1, align 8, !tbaa !10
  %187 = load i64, ptr %1, align 8, !tbaa !10
  %188 = mul i64 %187, 31
  %189 = load i32, ptr %3, align 4, !tbaa !12
  %190 = urem i32 %189, 100
  %191 = zext i32 %190 to i64
  %192 = add i64 %188, %191
  store i64 %192, ptr %1, align 8, !tbaa !10
  %193 = load i64, ptr %1, align 8, !tbaa !10
  %194 = mul i64 %193, 31
  %195 = load i32, ptr %3, align 4, !tbaa !12
  %196 = udiv i32 %195, 125
  %197 = zext i32 %196 to i64
  %198 = add i64 %194, %197
  store i64 %198, ptr %1, align 8, !tbaa !10
  %199 = load i64, ptr %1, align 8, !tbaa !10
  %200 = mul i64 %199, 31
  %201 = load i32, ptr %3, align 4, !tbaa !12
  %202 = urem i32 %201, 125
  %203 = zext i32 %202 to i64
  %204 = add i64 %200, %203
  store i64 %204, ptr %1, align 8, !tbaa !10
  %205 = load i64, ptr %1, align 8, !tbaa !10
  %206 = mul i64 %205, 31
  %207 = load i32, ptr %3, align 4, !tbaa !12
  %208 = udiv i32 %207, 255
  %209 = zext i32 %208 to i64
  %210 = add i64 %206, %209
  store i64 %210, ptr %1, align 8, !tbaa !10
  %211 = load i64, ptr %1, align 8, !tbaa !10
  %212 = mul i64 %211, 31
  %213 = load i32, ptr %3, align 4, !tbaa !12
  %214 = urem i32 %213, 255
  %215 = zext i32 %214 to i64
  %216 = add i64 %212, %215
  store i64 %216, ptr %1, align 8, !tbaa !10
  %217 = load i64, ptr %1, align 8, !tbaa !10
  %218 = mul i64 %217, 31
  %219 = load i32, ptr %3, align 4, !tbaa !12
  %220 = udiv i32 %219, 256
  %221 = zext i32 %220 to i64
  %222 = add i64 %218, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i32, ptr %3, align 4, !tbaa !12
  %226 = urem i32 %225, 256
  %227 = zext i32 %226 to i64
  %228 = add i64 %224, %227
  store i64 %228, ptr %1, align 8, !tbaa !10
  %229 = load i64, ptr %1, align 8, !tbaa !10
  %230 = mul i64 %229, 31
  %231 = load i32, ptr %3, align 4, !tbaa !12
  %232 = udiv i32 %231, 641
  %233 = zext i32 %232 to i64
  %234 = add i64 %230, %233
  store i64 %234, ptr %1, align 8, !tbaa !10
  %235 = load i64, ptr %1, align 8, !tbaa !10
  %236 = mul i64 %235, 31
  %237 = load i32, ptr %3, align 4, !tbaa !12
  %238 = urem i32 %237, 641
  %239 = zext i32 %238 to i64
  %240 = add i64 %236, %239
  store i64 %240, ptr %1, align 8, !tbaa !10
  %241 = load i64, ptr %1, align 8, !tbaa !10
  %242 = mul i64 %241, 31
  %243 = load i32, ptr %3, align 4, !tbaa !12
  %244 = udiv i32 %243, 1000
  %245 = zext i32 %244 to i64
  %246 = add i64 %242, %245
  store i64 %246, ptr %1, align 8, !tbaa !10
  %247 = load i64, ptr %1, align 8, !tbaa !10
  %248 = mul i64 %247, 31
  %249 = load i32, ptr %3, align 4, !tbaa !12
  %250 = urem i32 %249, 1000
  %251 = zext i32 %250 to i64
  %252 = add i64 %248, %251
  store i64 %252, ptr %1, align 8, !tbaa !10
  %253 = load i64, ptr %1, align 8, !tbaa !10
  %254 = mul i64 %253, 31
  %255 = load i32, ptr %3, align 4, !tbaa !12
  %256 = udiv i32 %255, 3600
  %257 = zext i32 %256 to i64
  %258 = add i64 %254, %257
  store i64 %258, ptr %1, align 8, !tbaa !10
  %259 = load i64, ptr %1, align 8, !tbaa !10
  %260 = mul i64 %259, 31
  %261 = load i32, ptr %3, align 4, !tbaa !12
  %262 = urem i32 %261, 3600
  %263 = zext i32 %262 to i64
  %264 = add i64 %260, %263
  store i64 %264, ptr %1, align 8, !tbaa !10
  %265 = load i64, ptr %1, align 8, !tbaa !10
  %266 = mul i64 %265, 31
  %267 = load i32, ptr %3, align 4, !tbaa !12
  %268 = udiv i32 %267, 65535
  %269 = zext i32 %268 to i64
  %270 = add i64 %266, %269
  store i64 %270, ptr %1, align 8, !tbaa !10
  %271 = load i64, ptr %1, align 8, !tbaa !10
  %272 = mul i64 %271, 31
  %273 = load i32, ptr %3, align 4, !tbaa !12
  %274 = urem i32 %273, 65535
  %275 = zext i32 %274 to i64
  %276 = add i64 %272, %275
  store i64 %276, ptr %1, align 8, !tbaa !10
  %277 = load i64, ptr %1, align 8, !tbaa !10
  %278 = mul i64 %277, 31
  %279 = load i32, ptr %3, align 4, !tbaa !12
  %280 = udiv i32 %279, 65537
  %281 = zext i32 %280 to i64
  %282 = add i64 %278, %281
  store i64 %282, ptr %1, align 8, !tbaa !10
  %283 = load i64, ptr %1, align 8, !tbaa !10
  %284 = mul i64 %283, 31
  %285 = load i32, ptr %3, align 4, !tbaa !12
  %286 = urem i32 %285, 65537
  %287 = zext i32 %286 to i64
  %288 = add i64 %284, %287
  store i64 %288, ptr %1, align 8, !tbaa !10
  %289 = load i64, ptr %1, align 8, !tbaa !10
  %290 = mul i64 %289, 31
  %291 = load i32, ptr %3, align 4, !tbaa !12
  %292 = udiv i32 %291, 1000003
  %293 = zext i32 %292 to i64
  %294 = add i64 %290, %293
  store i64 %294, ptr %1, align 8, !tbaa !10
  %295 = load i64, ptr %1, align 8, !tbaa !10
  %296 = mul i64 %295, 31
  %297 = load i32, ptr %3, align 4, !tbaa !12
  %298 = urem i32 %297, 1000003
  %299 = zext i32 %298 to i64
  %300 = add i64 %296, %299
  store i64 %300, ptr %1, align 8, !tbaa !10
  br label %301

301:                                              ; preds = %8
  %302 = load i32, ptr %2, align 4, !tbaa !12
  %303 = add i32 %302, 1
  store i32 %303, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !22

304:                                              ; preds = %4
  %305 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %305
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @i64_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i64, align 8
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %273, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 14
  br i1 %7, label %8, label %276

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [14 x i64], ptr @i64_values, i64 0, i64 %10
  %12 = load i64, ptr %11, align 8, !tbaa !10
  store i64 %12, ptr %3, align 8, !tbaa !10
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i64, ptr %3, align 8, !tbaa !10
  %16 = sdiv i64 %15, 2
  %17 = add i64 %14, %16
  store i64 %17, ptr %1, align 8, !tbaa !10
  %18 = load i64, ptr %1, align 8, !tbaa !10
  %19 = mul i64 %18, 31
  %20 = load i64, ptr %3, align 8, !tbaa !10
  %21 = srem i64 %20, 2
  %22 = add i64 %19, %21
  store i64 %22, ptr %1, align 8, !tbaa !10
  %23 = load i64, ptr %1, align 8, !tbaa !10
  %24 = mul i64 %23, 31
  %25 = load i64, ptr %3, align 8, !tbaa !10
  %26 = sdiv i64 %25, 3
  %27 = add i64 %24, %26
  store i64 %27, ptr %1, align 8, !tbaa !10
  %28 = load i64, ptr %1, align 8, !tbaa !10
  %29 = mul i64 %28, 31
  %30 = load i64, ptr %3, align 8, !tbaa !10
  %31 = srem i64 %30, 3
  %32 = add i64 %29, %31
  store i64 %32, ptr %1, align 8, !tbaa !10
  %33 = load i64, ptr %1, align 8, !tbaa !10
  %34 = mul i64 %33, 31
  %35 = load i64, ptr %3, align 8, !tbaa !10
  %36 = sdiv i64 %35, -3
  %37 = add i64 %34, %36
  store i64 %37, ptr %1, align 8, !tbaa !10
  %38 = load i64, ptr %1, align 8, !tbaa !10
  %39 = mul i64 %38, 31
  %40 = load i64, ptr %3, align 8, !tbaa !10
  %41 = srem i64 %40, -3
  %42 = add i64 %39, %41
  store i64 %42, ptr %1, align 8, !tbaa !10
  %43 = load i64, ptr %1, align 8, !tbaa !10
  %44 = mul i64 %43, 31
  %45 = load i64, ptr %3, align 8, !tbaa !10
  %46 = sdiv i64 %45, 5
  %47 = add i64 %44, %46
  store i64 %47, ptr %1, align 8, !tbaa !10
  %48 = load i64, ptr %1, align 8, !tbaa !10
  %49 = mul i64 %48, 31
  %50 = load i64, ptr %3, align 8, !tbaa !10
  %51 = srem i64 %50, 5
  %52 = add i64 %49, %51
  store i64 %52, ptr %1, align 8, !tbaa !10
  %53 = load i64, ptr %1, align 8, !tbaa !10
  %54 = mul i64 %53, 31
  %55 = load i64, ptr %3, align 8, !tbaa !10
  %56 = sdiv i64 %55, 6
  %57 = add i64 %54, %56
  store i64 %57, ptr %1, align 8, !tbaa !10
  %58 = load i64, ptr %1, align 8, !tbaa !10
  %59 = mul i64 %58, 31
  %60 = load i64, ptr %3, align 8, !tbaa !10
  %61 = srem i64 %60, 6
  %62 = add i64 %59, %61
  store i64 %62, ptr %1, align 8, !tbaa !10
  %63 = load i64, ptr %1, align 8, !tbaa !10
  %64 = mul i64 %63, 31
  %65 = load i64, ptr %3, align 8, !tbaa !10
  %66 = sdiv i64 %65, 7
  %67 = add i64 %64, %66
  store i64 %67, ptr %1, align 8, !tbaa !10
  %68 = load i64, ptr %1, align 8, !tbaa !10
  %69 = mul i64 %68, 31
  %70 = load i64, ptr %3, align 8, !tbaa !10
  %71 = srem i64 %70, 7
  %72 = add i64 %69, %71
  store i64 %72, ptr %1, align 8, !tbaa !10
  %73 = load i64, ptr %1, align 8, !tbaa !10
  %74 = mul i64 %73, 31
  %75 = load i64, ptr %3, align 8, !tbaa !10
  %76 = sdiv i64 %75, -7
  %77 = add i64 %74, %76
  store i64 %77, ptr %1, align 8, !tbaa !10
  %78 = load i64, ptr %1, align 8, !tbaa !10
  %79 = mul i64 %78, 31
  %80 = load i64, ptr %3, align 8, !tbaa !10
  %81 = srem i64 %80, -7
  %82 = add i64 %79, %81
  store i64 %82, ptr %1, align 8, !tbaa !10
  %83 = load i64, ptr %1, align 8, !tbaa !10
  %84 = mul i64 %83, 31
  %85 = load i64, ptr %3, align 8, !tbaa !10
  %86 = sdiv i64 %85, 9
  %87 = add i64 %84, %86
  store i64 %87, ptr %1, align 8, !tbaa !10
  %88 = load i64, ptr %1, align 8, !tbaa !10
  %89 = mul i64 %88, 31
  %90 = load i64, ptr %3, align 8, !tbaa !10
  %91 = srem i64 %90, 9
  %92 = add i64 %89, %91
  store i64 %92, ptr %1, align 8, !tbaa !10
  %93 = load i64, ptr %1, align 8, !tbaa !10
  %94 = mul i64 %93, 31
  %95 = load i64, ptr %3, align 8, !tbaa !10
  %96 = sdiv i64 %95, 10
  %97 = add i64 %94, %96
  store i64 %97, ptr %1, align 8, !tbaa !10
  %98 = load i64, ptr %1, align 8, !tbaa !10
  %99 = mul i64 %98, 31
  %100 = load i64, ptr %3, align 8, !tbaa !10
  %101 = srem i64 %100, 10
  %102 = add i64 %99, %101
  store i64 %102, ptr %1, align 8, !tbaa !10
  %103 = load i64, ptr %1, align 8, !tbaa !10
  %104 = mul i64 %103, 31
  %105 = load i64, ptr %3, align 8, !tbaa !10
  %106 = sdiv i64 %105, -10
  %107 = add i64 %104, %106
  store i64 %107, ptr %1, align 8, !tbaa !10
  %108 = load i64, ptr %1, align 8, !tbaa !10
  %109 = mul i64 %108, 31
  %110 = load i64, ptr %3, align 8, !tbaa !10
  %111 = srem i64 %110, -10
  %112 = add i64 %109, %111
  store i64 %112, ptr %1, align 8, !tbaa !10
  %113 = load i64, ptr %1, align 8, !tbaa !10
  %114 = mul i64 %113, 31
  %115 = load i64, ptr %3, align 8, !tbaa !10
  %116 = sdiv i64 %115, 11
  %117 = add i64 %114, %116
  store i64 %117, ptr %1, align 8, !tbaa !10
  %118 = load i64, ptr %1, align 8, !tbaa !10
  %119 = mul i64 %118, 31
  %120 = load i64, ptr %3, align 8, !tbaa !10
  %121 = srem i64 %120, 11
  %122 = add i64 %119, %121
  store i64 %122, ptr %1, align 8, !tbaa !10
  %123 = load i64, ptr %1, align 8, !tbaa !10
  %124 = mul i64 %123, 31
  %125 = load i64, ptr %3, align 8, !tbaa !10
  %126 = sdiv i64 %125, 12
  %127 = add i64 %124, %126
  store i64 %127, ptr %1, align 8, !tbaa !10
  %128 = load i64, ptr %1, align 8, !tbaa !10
  %129 = mul i64 %128, 31
  %130 = load i64, ptr %3, align 8, !tbaa !10
  %131 = srem i64 %130, 12
  %132 = add i64 %129, %131
  store i64 %132, ptr %1, align 8, !tbaa !10
  %133 = load i64, ptr %1, align 8, !tbaa !10
  %134 = mul i64 %133, 31
  %135 = load i64, ptr %3, align 8, !tbaa !10
  %136 = sdiv i64 %135, 13
  %137 = add i64 %134, %136
  store i64 %137, ptr %1, align 8, !tbaa !10
  %138 = load i64, ptr %1, align 8, !tbaa !10
  %139 = mul i64 %138, 31
  %140 = load i64, ptr %3, align 8, !tbaa !10
  %141 = srem i64 %140, 13
  %142 = add i64 %139, %141
  store i64 %142, ptr %1, align 8, !tbaa !10
  %143 = load i64, ptr %1, align 8, !tbaa !10
  %144 = mul i64 %143, 31
  %145 = load i64, ptr %3, align 8, !tbaa !10
  %146 = sdiv i64 %145, 16
  %147 = add i64 %144, %146
  store i64 %147, ptr %1, align 8, !tbaa !10
  %148 = load i64, ptr %1, align 8, !tbaa !10
  %149 = mul i64 %148, 31
  %150 = load i64, ptr %3, align 8, !tbaa !10
  %151 = srem i64 %150, 16
  %152 = add i64 %149, %151
  store i64 %152, ptr %1, align 8, !tbaa !10
  %153 = load i64, ptr %1, align 8, !tbaa !10
  %154 = mul i64 %153, 31
  %155 = load i64, ptr %3, align 8, !tbaa !10
  %156 = sdiv i64 %155, -16
  %157 = add i64 %154, %156
  store i64 %157, ptr %1, align 8, !tbaa !10
  %158 = load i64, ptr %1, align 8, !tbaa !10
  %159 = mul i64 %158, 31
  %160 = load i64, ptr %3, align 8, !tbaa !10
  %161 = srem i64 %160, -16
  %162 = add i64 %159, %161
  store i64 %162, ptr %1, align 8, !tbaa !10
  %163 = load i64, ptr %1, align 8, !tbaa !10
  %164 = mul i64 %163, 31
  %165 = load i64, ptr %3, align 8, !tbaa !10
  %166 = sdiv i64 %165, 25
  %167 = add i64 %164, %166
  store i64 %167, ptr %1, align 8, !tbaa !10
  %168 = load i64, ptr %1, align 8, !tbaa !10
  %169 = mul i64 %168, 31
  %170 = load i64, ptr %3, align 8, !tbaa !10
  %171 = srem i64 %170, 25
  %172 = add i64 %169, %171
  store i64 %172, ptr %1, align 8, !tbaa !10
  %173 = load i64, ptr %1, align 8, !tbaa !10
  %174 = mul i64 %173, 31
  %175 = load i64, ptr %3, align 8, !tbaa !10
  %176 = sdiv i64 %175, 100
  %177 = add i64 %174, %176
  store i64 %177, ptr %1, align 8, !tbaa !10
  %178 = load i64, ptr %1, align 8, !tbaa !10
  %179 = mul i64 %178, 31
  %180 = load i64, ptr %3, align 8, !tbaa !10
  %181 = srem i64 %180, 100
  %182 = add i64 %179, %181
  store i64 %182, ptr %1, align 8, !tbaa !10
  %183 = load i64, ptr %1, align 8, !tbaa !10
  %184 = mul i64 %183, 31
  %185 = load i64, ptr %3, align 8, !tbaa !10
  %186 = sdiv i64 %185, -100
  %187 = add i64 %184, %186
  store i64 %187, ptr %1, align 8, !tbaa !10
  %188 = load i64, ptr %1, align 8, !tbaa !10
  %189 = mul i64 %188, 31
  %190 = load i64, ptr %3, align 8, !tbaa !10
  %191 = srem i64 %190, -100
  %192 = add i64 %189, %191
  store i64 %192, ptr %1, align 8, !tbaa !10
  %193 = load i64, ptr %1, align 8, !tbaa !10
  %194 = mul i64 %193, 31
  %195 = load i64, ptr %3, align 8, !tbaa !10
  %196 = sdiv i64 %195, 125
  %197 = add i64 %194, %196
  store i64 %197, ptr %1, align 8, !tbaa !10
  %198 = load i64, ptr %1, align 8, !tbaa !10
  %199 = mul i64 %198, 31
  %200 = load i64, ptr %3, align 8, !tbaa !10
  %201 = srem i64 %200, 125
  %202 = add i64 %199, %201
  store i64 %202, ptr %1, align 8, !tbaa !10
  %203 = load i64, ptr %1, align 8, !tbaa !10
  %204 = mul i64 %203, 31
  %205 = load i64, ptr %3, align 8, !tbaa !10
  %206 = sdiv i64 %205, 255
  %207 = add i64 %204, %206
  store i64 %207, ptr %1, align 8, !tbaa !10
  %208 = load i64, ptr %1, align 8, !tbaa !10
  %209 = mul i64 %208, 31
  %210 = load i64, ptr %3, align 8, !tbaa !10
  %211 = srem i64 %210, 255
  %212 = add i64 %209, %211
  store i64 %212, ptr %1, align 8, !tbaa !10
  %213 = load i64, ptr %1, align 8, !tbaa !10
  %214 = mul i64 %213, 31
  %215 = load i64, ptr %3, align 8, !tbaa !10
  %216 = sdiv i64 %215, -256
  %217 = add i64 %214, %216
  store i64 %217, ptr %1, align 8, !tbaa !10
  %218 = load i64, ptr %1, align 8, !tbaa !10
  %219 = mul i64 %218, 31
  %220 = load i64, ptr %3, align 8, !tbaa !10
  %221 = srem i64 %220, -256
  %222 = add i64 %219, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i64, ptr %3, align 8, !tbaa !10
  %226 = sdiv i64 %225, 641
  %227 = add i64 %224, %226
  store i64 %227, ptr %1, align 8, !tbaa !10
  %228 = load i64, ptr %1, align 8, !tbaa !10
  %229 = mul i64 %228, 31
  %230 = load i64, ptr %3, align 8, !tbaa !10
  %231 = srem i64 %230, 641
  %232 = add i64 %229, %231
  store i64 %232, ptr %1, align 8, !tbaa !10
  %233 = load i64, ptr %1, align 8, !tbaa !10
  %234 = mul i64 %233, 31
  %235 = load i64, ptr %3, align 8, !tbaa !10
  %236 = sdiv i64 %235, 1000
  %237 = add i64 %234, %236
  store i64 %237, ptr %1, align 8, !tbaa !10
  %238 = load i64, ptr %1, align 8, !tbaa !10
  %239 = mul i64 %238, 31
  %240 = load i64, ptr %3, align 8, !tbaa !10
  %241 = srem i64 %240, 1000
  %242 = add i64 %239, %241
  store i64 %242, ptr %1, align 8, !tbaa !10
  %243 = load i64, ptr %1, align 8, !tbaa !10
  %244 = mul i64 %243, 31
  %245 = load i64, ptr %3, align 8, !tbaa !10
  %246 = sdiv i64 %245, -1000
  %247 = add i64 %244, %246
  store i64 %247, ptr %1, align 8, !tbaa !10
  %248 = load i64, ptr %1, align 8, !tbaa !10
  %249 = mul i64 %248, 31
  %250 = load i64, ptr %3, align 8, !tbaa !10
  %251 = srem i64 %250, -1000
  %252 = add i64 %249, %251
  store i64 %252, ptr %1, align 8, !tbaa !10
  %253 = load i64, ptr %1, align 8, !tbaa !10
  %254 = mul i64 %253, 31
  %255 = load i64, ptr %3, align 8, !tbaa !10
  %256 = sdiv i64 %255, 3600
  %257 = add i64 %254, %256
  store i64 %257, ptr %1, align 8, !tbaa !10
  %258 = load i64, ptr %1, align 8, !tbaa !10
  %259 = mul i64 %258, 31
  %260 = load i64, ptr %3, align 8, !tbaa !10
  %261 = srem i64 %260, 3600
  %262 = add i64 %259, %261
  store i64 %262, ptr %1, align 8, !tbaa !10
  %263 = load i64, ptr %1, align 8, !tbaa !10
  %264 = mul i64 %263, 31
  %265 = load i64, ptr %3, align 8, !tbaa !10
  %266 = sdiv i64 %265, 65537
  %267 = add i64 %264, %266
  store i64 %267, ptr %1, align 8, !tbaa !10
  %268 = load i64, ptr %1, align 8, !tbaa !10
  %269 = mul i64 %268, 31
  %270 = load i64, ptr %3, align 8, !tbaa !10
  %271 = srem i64 %270, 65537
  %272 = add i64 %269, %271
  store i64 %272, ptr %1, align 8, !tbaa !10
  br label %273

273:                                              ; preds = %8
  %274 = load i32, ptr %2, align 4, !tbaa !12
  %275 = add i32 %274, 1
  store i32 %275, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !23

276:                                              ; preds = %4
  %277 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %277
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @u64_run() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  %3 = alloca i64, align 8
  store i64 7, ptr %1, align 8, !tbaa !10
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %253, %0
  %5 = load i32, ptr %2, align 4, !tbaa !12
  %6 = zext i32 %5 to i64
  %7 = icmp ult i64 %6, 10
  br i1 %7, label %8, label %256

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = zext i32 %9 to i64
  %11 = getelementptr inbounds nuw [10 x i64], ptr @u64_values, i64 0, i64 %10
  %12 = load i64, ptr %11, align 8, !tbaa !10
  store i64 %12, ptr %3, align 8, !tbaa !10
  %13 = load i64, ptr %1, align 8, !tbaa !10
  %14 = mul i64 %13, 31
  %15 = load i64, ptr %3, align 8, !tbaa !10
  %16 = udiv i64 %15, 2
  %17 = add i64 %14, %16
  store i64 %17, ptr %1, align 8, !tbaa !10
  %18 = load i64, ptr %1, align 8, !tbaa !10
  %19 = mul i64 %18, 31
  %20 = load i64, ptr %3, align 8, !tbaa !10
  %21 = urem i64 %20, 2
  %22 = add i64 %19, %21
  store i64 %22, ptr %1, align 8, !tbaa !10
  %23 = load i64, ptr %1, align 8, !tbaa !10
  %24 = mul i64 %23, 31
  %25 = load i64, ptr %3, align 8, !tbaa !10
  %26 = udiv i64 %25, 3
  %27 = add i64 %24, %26
  store i64 %27, ptr %1, align 8, !tbaa !10
  %28 = load i64, ptr %1, align 8, !tbaa !10
  %29 = mul i64 %28, 31
  %30 = load i64, ptr %3, align 8, !tbaa !10
  %31 = urem i64 %30, 3
  %32 = add i64 %29, %31
  store i64 %32, ptr %1, align 8, !tbaa !10
  %33 = load i64, ptr %1, align 8, !tbaa !10
  %34 = mul i64 %33, 31
  %35 = load i64, ptr %3, align 8, !tbaa !10
  %36 = udiv i64 %35, 5
  %37 = add i64 %34, %36
  store i64 %37, ptr %1, align 8, !tbaa !10
  %38 = load i64, ptr %1, align 8, !tbaa !10
  %39 = mul i64 %38, 31
  %40 = load i64, ptr %3, align 8, !tbaa !10
  %41 = urem i64 %40, 5
  %42 = add i64 %39, %41
  store i64 %42, ptr %1, align 8, !tbaa !10
  %43 = load i64, ptr %1, align 8, !tbaa !10
  %44 = mul i64 %43, 31
  %45 = load i64, ptr %3, align 8, !tbaa !10
  %46 = udiv i64 %45, 6
  %47 = add i64 %44, %46
  store i64 %47, ptr %1, align 8, !tbaa !10
  %48 = load i64, ptr %1, align 8, !tbaa !10
  %49 = mul i64 %48, 31
  %50 = load i64, ptr %3, align 8, !tbaa !10
  %51 = urem i64 %50, 6
  %52 = add i64 %49, %51
  store i64 %52, ptr %1, align 8, !tbaa !10
  %53 = load i64, ptr %1, align 8, !tbaa !10
  %54 = mul i64 %53, 31
  %55 = load i64, ptr %3, align 8, !tbaa !10
  %56 = udiv i64 %55, 7
  %57 = add i64 %54, %56
  store i64 %57, ptr %1, align 8, !tbaa !10
  %58 = load i64, ptr %1, align 8, !tbaa !10
  %59 = mul i64 %58, 31
  %60 = load i64, ptr %3, align 8, !tbaa !10
  %61 = urem i64 %60, 7
  %62 = add i64 %59, %61
  store i64 %62, ptr %1, align 8, !tbaa !10
  %63 = load i64, ptr %1, align 8, !tbaa !10
  %64 = mul i64 %63, 31
  %65 = load i64, ptr %3, align 8, !tbaa !10
  %66 = udiv i64 %65, 9
  %67 = add i64 %64, %66
  store i64 %67, ptr %1, align 8, !tbaa !10
  %68 = load i64, ptr %1, align 8, !tbaa !10
  %69 = mul i64 %68, 31
  %70 = load i64, ptr %3, align 8, !tbaa !10
  %71 = urem i64 %70, 9
  %72 = add i64 %69, %71
  store i64 %72, ptr %1, align 8, !tbaa !10
  %73 = load i64, ptr %1, align 8, !tbaa !10
  %74 = mul i64 %73, 31
  %75 = load i64, ptr %3, align 8, !tbaa !10
  %76 = udiv i64 %75, 10
  %77 = add i64 %74, %76
  store i64 %77, ptr %1, align 8, !tbaa !10
  %78 = load i64, ptr %1, align 8, !tbaa !10
  %79 = mul i64 %78, 31
  %80 = load i64, ptr %3, align 8, !tbaa !10
  %81 = urem i64 %80, 10
  %82 = add i64 %79, %81
  store i64 %82, ptr %1, align 8, !tbaa !10
  %83 = load i64, ptr %1, align 8, !tbaa !10
  %84 = mul i64 %83, 31
  %85 = load i64, ptr %3, align 8, !tbaa !10
  %86 = udiv i64 %85, 11
  %87 = add i64 %84, %86
  store i64 %87, ptr %1, align 8, !tbaa !10
  %88 = load i64, ptr %1, align 8, !tbaa !10
  %89 = mul i64 %88, 31
  %90 = load i64, ptr %3, align 8, !tbaa !10
  %91 = urem i64 %90, 11
  %92 = add i64 %89, %91
  store i64 %92, ptr %1, align 8, !tbaa !10
  %93 = load i64, ptr %1, align 8, !tbaa !10
  %94 = mul i64 %93, 31
  %95 = load i64, ptr %3, align 8, !tbaa !10
  %96 = udiv i64 %95, 12
  %97 = add i64 %94, %96
  store i64 %97, ptr %1, align 8, !tbaa !10
  %98 = load i64, ptr %1, align 8, !tbaa !10
  %99 = mul i64 %98, 31
  %100 = load i64, ptr %3, align 8, !tbaa !10
  %101 = urem i64 %100, 12
  %102 = add i64 %99, %101
  store i64 %102, ptr %1, align 8, !tbaa !10
  %103 = load i64, ptr %1, align 8, !tbaa !10
  %104 = mul i64 %103, 31
  %105 = load i64, ptr %3, align 8, !tbaa !10
  %106 = udiv i64 %105, 13
  %107 = add i64 %104, %106
  store i64 %107, ptr %1, align 8, !tbaa !10
  %108 = load i64, ptr %1, align 8, !tbaa !10
  %109 = mul i64 %108, 31
  %110 = load i64, ptr %3, align 8, !tbaa !10
  %111 = urem i64 %110, 13
  %112 = add i64 %109, %111
  store i64 %112, ptr %1, align 8, !tbaa !10
  %113 = load i64, ptr %1, align 8, !tbaa !10
  %114 = mul i64 %113, 31
  %115 = load i64, ptr %3, align 8, !tbaa !10
  %116 = udiv i64 %115, 15
  %117 = add i64 %114, %116
  store i64 %117, ptr %1, align 8, !tbaa !10
  %118 = load i64, ptr %1, align 8, !tbaa !10
  %119 = mul i64 %118, 31
  %120 = load i64, ptr %3, align 8, !tbaa !10
  %121 = urem i64 %120, 15
  %122 = add i64 %119, %121
  store i64 %122, ptr %1, align 8, !tbaa !10
  %123 = load i64, ptr %1, align 8, !tbaa !10
  %124 = mul i64 %123, 31
  %125 = load i64, ptr %3, align 8, !tbaa !10
  %126 = udiv i64 %125, 16
  %127 = add i64 %124, %126
  store i64 %127, ptr %1, align 8, !tbaa !10
  %128 = load i64, ptr %1, align 8, !tbaa !10
  %129 = mul i64 %128, 31
  %130 = load i64, ptr %3, align 8, !tbaa !10
  %131 = urem i64 %130, 16
  %132 = add i64 %129, %131
  store i64 %132, ptr %1, align 8, !tbaa !10
  %133 = load i64, ptr %1, align 8, !tbaa !10
  %134 = mul i64 %133, 31
  %135 = load i64, ptr %3, align 8, !tbaa !10
  %136 = udiv i64 %135, 25
  %137 = add i64 %134, %136
  store i64 %137, ptr %1, align 8, !tbaa !10
  %138 = load i64, ptr %1, align 8, !tbaa !10
  %139 = mul i64 %138, 31
  %140 = load i64, ptr %3, align 8, !tbaa !10
  %141 = urem i64 %140, 25
  %142 = add i64 %139, %141
  store i64 %142, ptr %1, align 8, !tbaa !10
  %143 = load i64, ptr %1, align 8, !tbaa !10
  %144 = mul i64 %143, 31
  %145 = load i64, ptr %3, align 8, !tbaa !10
  %146 = udiv i64 %145, 60
  %147 = add i64 %144, %146
  store i64 %147, ptr %1, align 8, !tbaa !10
  %148 = load i64, ptr %1, align 8, !tbaa !10
  %149 = mul i64 %148, 31
  %150 = load i64, ptr %3, align 8, !tbaa !10
  %151 = urem i64 %150, 60
  %152 = add i64 %149, %151
  store i64 %152, ptr %1, align 8, !tbaa !10
  %153 = load i64, ptr %1, align 8, !tbaa !10
  %154 = mul i64 %153, 31
  %155 = load i64, ptr %3, align 8, !tbaa !10
  %156 = udiv i64 %155, 100
  %157 = add i64 %154, %156
  store i64 %157, ptr %1, align 8, !tbaa !10
  %158 = load i64, ptr %1, align 8, !tbaa !10
  %159 = mul i64 %158, 31
  %160 = load i64, ptr %3, align 8, !tbaa !10
  %161 = urem i64 %160, 100
  %162 = add i64 %159, %161
  store i64 %162, ptr %1, align 8, !tbaa !10
  %163 = load i64, ptr %1, align 8, !tbaa !10
  %164 = mul i64 %163, 31
  %165 = load i64, ptr %3, align 8, !tbaa !10
  %166 = udiv i64 %165, 125
  %167 = add i64 %164, %166
  store i64 %167, ptr %1, align 8, !tbaa !10
  %168 = load i64, ptr %1, align 8, !tbaa !10
  %169 = mul i64 %168, 31
  %170 = load i64, ptr %3, align 8, !tbaa !10
  %171 = urem i64 %170, 125
  %172 = add i64 %169, %171
  store i64 %172, ptr %1, align 8, !tbaa !10
  %173 = load i64, ptr %1, align 8, !tbaa !10
  %174 = mul i64 %173, 31
  %175 = load i64, ptr %3, align 8, !tbaa !10
  %176 = udiv i64 %175, 255
  %177 = add i64 %174, %176
  store i64 %177, ptr %1, align 8, !tbaa !10
  %178 = load i64, ptr %1, align 8, !tbaa !10
  %179 = mul i64 %178, 31
  %180 = load i64, ptr %3, align 8, !tbaa !10
  %181 = urem i64 %180, 255
  %182 = add i64 %179, %181
  store i64 %182, ptr %1, align 8, !tbaa !10
  %183 = load i64, ptr %1, align 8, !tbaa !10
  %184 = mul i64 %183, 31
  %185 = load i64, ptr %3, align 8, !tbaa !10
  %186 = udiv i64 %185, 256
  %187 = add i64 %184, %186
  store i64 %187, ptr %1, align 8, !tbaa !10
  %188 = load i64, ptr %1, align 8, !tbaa !10
  %189 = mul i64 %188, 31
  %190 = load i64, ptr %3, align 8, !tbaa !10
  %191 = urem i64 %190, 256
  %192 = add i64 %189, %191
  store i64 %192, ptr %1, align 8, !tbaa !10
  %193 = load i64, ptr %1, align 8, !tbaa !10
  %194 = mul i64 %193, 31
  %195 = load i64, ptr %3, align 8, !tbaa !10
  %196 = udiv i64 %195, 641
  %197 = add i64 %194, %196
  store i64 %197, ptr %1, align 8, !tbaa !10
  %198 = load i64, ptr %1, align 8, !tbaa !10
  %199 = mul i64 %198, 31
  %200 = load i64, ptr %3, align 8, !tbaa !10
  %201 = urem i64 %200, 641
  %202 = add i64 %199, %201
  store i64 %202, ptr %1, align 8, !tbaa !10
  %203 = load i64, ptr %1, align 8, !tbaa !10
  %204 = mul i64 %203, 31
  %205 = load i64, ptr %3, align 8, !tbaa !10
  %206 = udiv i64 %205, 1000
  %207 = add i64 %204, %206
  store i64 %207, ptr %1, align 8, !tbaa !10
  %208 = load i64, ptr %1, align 8, !tbaa !10
  %209 = mul i64 %208, 31
  %210 = load i64, ptr %3, align 8, !tbaa !10
  %211 = urem i64 %210, 1000
  %212 = add i64 %209, %211
  store i64 %212, ptr %1, align 8, !tbaa !10
  %213 = load i64, ptr %1, align 8, !tbaa !10
  %214 = mul i64 %213, 31
  %215 = load i64, ptr %3, align 8, !tbaa !10
  %216 = udiv i64 %215, 3600
  %217 = add i64 %214, %216
  store i64 %217, ptr %1, align 8, !tbaa !10
  %218 = load i64, ptr %1, align 8, !tbaa !10
  %219 = mul i64 %218, 31
  %220 = load i64, ptr %3, align 8, !tbaa !10
  %221 = urem i64 %220, 3600
  %222 = add i64 %219, %221
  store i64 %222, ptr %1, align 8, !tbaa !10
  %223 = load i64, ptr %1, align 8, !tbaa !10
  %224 = mul i64 %223, 31
  %225 = load i64, ptr %3, align 8, !tbaa !10
  %226 = udiv i64 %225, 65535
  %227 = add i64 %224, %226
  store i64 %227, ptr %1, align 8, !tbaa !10
  %228 = load i64, ptr %1, align 8, !tbaa !10
  %229 = mul i64 %228, 31
  %230 = load i64, ptr %3, align 8, !tbaa !10
  %231 = urem i64 %230, 65535
  %232 = add i64 %229, %231
  store i64 %232, ptr %1, align 8, !tbaa !10
  %233 = load i64, ptr %1, align 8, !tbaa !10
  %234 = mul i64 %233, 31
  %235 = load i64, ptr %3, align 8, !tbaa !10
  %236 = udiv i64 %235, 65537
  %237 = add i64 %234, %236
  store i64 %237, ptr %1, align 8, !tbaa !10
  %238 = load i64, ptr %1, align 8, !tbaa !10
  %239 = mul i64 %238, 31
  %240 = load i64, ptr %3, align 8, !tbaa !10
  %241 = urem i64 %240, 65537
  %242 = add i64 %239, %241
  store i64 %242, ptr %1, align 8, !tbaa !10
  %243 = load i64, ptr %1, align 8, !tbaa !10
  %244 = mul i64 %243, 31
  %245 = load i64, ptr %3, align 8, !tbaa !10
  %246 = udiv i64 %245, 1000003
  %247 = add i64 %244, %246
  store i64 %247, ptr %1, align 8, !tbaa !10
  %248 = load i64, ptr %1, align 8, !tbaa !10
  %249 = mul i64 %248, 31
  %250 = load i64, ptr %3, align 8, !tbaa !10
  %251 = urem i64 %250, 1000003
  %252 = add i64 %249, %251
  store i64 %252, ptr %1, align 8, !tbaa !10
  br label %253

253:                                              ; preds = %8
  %254 = load i32, ptr %2, align 4, !tbaa !12
  %255 = add i32 %254, 1
  store i32 %255, ptr %2, align 4, !tbaa !12
  br label %4, !llvm.loop !24

256:                                              ; preds = %4
  %257 = load i64, ptr %1, align 8, !tbaa !10
  ret i64 %257
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

!llvm.module.flags = !{!0, !1, !2, !3}
!llvm.ident = !{!4}
!llvm.errno.tbaa = !{!5}

!0 = !{i32 2, !"SDK Version", [2 x i32] [i32 27, i32 0]}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"uwtable", i32 1}
!3 = !{i32 7, !"frame-pointer", i32 4}
!4 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!5 = !{!6, !7, i64 0}
!6 = !{!"__libc_errno", !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !8, i64 0}
!12 = !{!7, !7, i64 0}
!13 = !{!8, !8, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
!17 = !{!18, !18, i64 0}
!18 = !{!"short", !8, i64 0}
!19 = distinct !{!19, !15}
!20 = distinct !{!20, !15}
!21 = distinct !{!21, !15}
!22 = distinct !{!22, !15}
!23 = distinct !{!23, !15}
!24 = distinct !{!24, !15}
